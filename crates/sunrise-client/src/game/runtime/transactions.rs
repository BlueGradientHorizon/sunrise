//! `host.transaction` (#41 §7): NeoForge 26.3's `TransactionContext` and `SnapshotJournal`, kept by
//! the game at the storage level, so a mod's handlers hold no state and write no journal.
//!
//! A capability write applies in place and keeps its storage's first before-image per depth.
//! Game-held vanilla containers defer implicit changed/comparator effects to root commit;
//! nested commits hand their snapshots and waiting work to the parent. An abort restores
//! storage quietly and discards those commit effects. Raw mod-owned storage writes and
//! restores synthesize no notice. Explicit notifications a mod issues run immediately and
//! survive abort: the journal restores storage, not observer calls or scheduled comparator
//! ticks that already happened.
//!
//! Covered here: block entities (item and data slots, saved compound, queued client update fields,
//! custom name, lock, loot roll, attachments), a hopper's cooldown and cauldron and composter
//! block states. The attachment map of every other holder (an entity, a player, a chunk,
//! the level) is kept on the first access to it through `attachment_map`.
//! A block's tentative state is stored only (`World::set_stored`): the
//! one real write, with its remesh, relight and logs, is at the root commit. A jukebox is
//! told at the root commit from its record before and after, so a disc swapped in one
//! transaction changes the song. A player's inventory (slots, armour, off hand, cursor) is kept
//! whole on its first write, from wherever the player's state lives then: the acting fields
//! or a parked guest's record. On abort, the journal follows that storage through guest turns.
//! An item a write drops from a player waits for
//! the root commit too. Item entities keep their stack, pickup delay, target and velocity on
//! first write; one emptied stays, empty, until the root commit removes it. General entity views
//! keep velocity, rotation and fall distance on first write. A guest's menu storage (ender chest,
//! crafting grid, a mod's chest menu) is kept whole on its first write through a menu slot; a
//! crafting grid changed by the transaction has its result made again at the root commit.
//! `LivingRef::set_health` keeps the player's synced float storage and unpublished update
//! events, or the mob's health and sync dirtiness. This does not journal other health,
//! damage, death or attribute operations. Mob effect writes keep their effect lists, dirty bit,
//! health and absorption, and the attribute modifier rows the effects own.
//! An entity added inside one goes at an abort, one removed is only marked until the root
//! commit, and mods hear of joins and leaves at the root commit (`transactions/entities.rs`).
use super::{
    BlockEntity, BlockStateId, Dim, Game, lan_mount_menu::MountClickWindow, machines::HopperState,
    maps::MapData,
};
use mod_api::{
    __private::TransactionFrame, LevelRef, TransactionContext, TransactionLifecycle,
    TransactionScopeError,
};
use std::collections::{BTreeMap, HashMap};
use sunrise_client::{
    block_entities::BlockEntityIdentity,
    effects::EffectInstance,
    items::ItemStack,
    living_attributes::ModifierChanges,
    menu::QuickCraftDrag,
    mobs::{StoredEffect, finalize::EffectModifierUndo, gear::Gear},
    world::{BlockEntityExtras, ModifiedChunks, World},
};
use sunrise_core::{geometry::Vec3, random::LegacyRandom};
use sunrise_items::equipment::Equipment;
use sunrise_mod_runtime::attachments::{AttachmentQueryScopes, Attachments, Holder};
use sunrise_world::level::flags;

#[path = "transactions/decision.rs"]
pub(in crate::game) mod decision;
#[path = "transactions/entities.rs"]
pub(in crate::game) mod entities;
#[allow(unused_imports)]
pub(in crate::game) use entities::{Announcement, ArrivalRollback};
#[path = "transactions/scratch.rs"]
pub(in crate::game) mod scratch;

/// A notice names the container that received the write, not a replacement at its position.
pub(in crate::game) struct QuickMoveNotice {
    pub(in crate::game) dim: Dim,
    pub(in crate::game) holder: BlockEntityIdentity,
}

pub(in crate::game) struct QuickMoveNotices<'a, 'g, S: crate::game::Side> {
    pub(in crate::game) game: &'a mut Game<'g, S>,
    pub(in crate::game) outer: Option<Vec<QuickMoveNotice>>,
}

impl<S: crate::game::Side> Drop for QuickMoveNotices<'_, '_, S> {
    fn drop(&mut self) {
        self.game.transactions.quick_move_notices = self.outer.take();
    }
}

pub(in crate::game) type Pos = (i32, i32, i32);

/// A level-local save follows that level's block writes. A transaction save follows the
/// dimension-keyed owner journal (e.g. an entity's equipment), even through a different level.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::game) enum SaveChunkScope {
    Level,
    Transaction,
}

/// The last pending value at this depth for a connection, menu generation and data index.
pub(in crate::game) type MenuDataJournal = BTreeMap<(u32, u64, usize), i32>;
pub(in crate::game) type ComposterRandoms =
    HashMap<(mod_api::DimensionKey, (i32, i32)), HashMap<Pos, sunrise_core::random::LegacyRandom>>;

/// The open transactions, innermost last, and which close is running, if any: what
/// `transaction_lifecycle` reads.
#[derive(Default)]
pub(in crate::game) struct OpenTransactions {
    pub(in crate::game) levels: Vec<Level>,
    pub(in crate::game) attachment_query_scopes: AttachmentQueryScopes,
    // A live, non-ZST address distinguishes journal instances.
    pub(in crate::game) tag: Option<Box<u8>>,
    pub(in crate::game) next_frame: u64,
    /// Live scratch rows; the frame opening a loan owns its ID.
    pub(in crate::game) scratch: HashMap<u64, scratch::Scratch>,
    pub(in crate::game) closing: Option<TransactionLifecycle>,
    /// Implicit container notices waiting for this quick move's source settlement.
    pub(in crate::game) quick_move_notices: Option<Vec<QuickMoveNotice>>,
    /// How many writes the journals have been asked to keep, ever: a whole-state epoch that no
    /// restore moves back. A decision that read a player's inventory or a container's slots
    /// compares it before and after its listeners ran; an equal rewrite counts as a write.
    pub(in crate::game) writes: u64,
    /// The same count for a player's own state only (inventory, cursor, menu storage): what a
    /// decision that must stay open to other writes (a mod's move into another block) reads.
    pub(in crate::game) player_writes: u64,
    /// The root commits telling their joins and leaves, outermost first: a listener's command in
    /// another level can commit a transaction there while this level's commit is told.
    pub(in crate::game) committing: Vec<entities::Committing>,
    pub(in crate::game) composter_epoch: Option<mod_api::WorldEpoch>,
    /// Each composter port owns its transactional random stream, independently of Level.random.
    pub(in crate::game) composter_randoms: ComposterRandoms,
    /// Active retained original-player attachment maps during `player_clone` dispatch,
    /// kept in sync with open transaction frames so mutations roll back on abort.
    pub(in crate::game) retained_original_attachments:
        Vec<std::sync::Arc<std::sync::Mutex<super::original_player::OriginalAttachmentSlot>>>,
    #[cfg(test)]
    pub(in crate::game) trace: Option<std::sync::Arc<std::sync::Mutex<Vec<String>>>>,
}

impl OpenTransactions {
    pub(in crate::game) fn next_frame(&mut self) -> TransactionFrame {
        let tag = self.tag.get_or_insert_with(|| Box::new(0));
        let journal = std::ptr::from_ref(&**tag) as usize;
        self.next_frame = self
            .next_frame
            .checked_add(1)
            .expect("transaction frame sequence exhausted");
        TransactionFrame::new(journal, self.next_frame, self.levels.len())
    }

    pub(in crate::game) fn push_retained_original(
        &mut self,
        slot: std::sync::Arc<std::sync::Mutex<super::original_player::OriginalAttachmentSlot>>,
    ) {
        self.retained_original_attachments.push(slot);
    }

    pub(in crate::game) fn pop_retained_original(&mut self) {
        self.retained_original_attachments.pop();
    }

    pub(in crate::game) fn close_retained_originals(
        &self,
        frame: Option<TransactionFrame>,
        commit: bool,
    ) {
        if let Some(frame) = frame {
            for slot in &self.retained_original_attachments {
                slot.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .close_frame(frame, commit);
            }
        }
    }

    pub(in crate::game) fn level_frame(&mut self, dim: super::Dim) -> Option<&mut Level> {
        entities::open_in(self, dim)
    }

    pub(in crate::game) fn insert_scratch(
        &mut self,
        entry: scratch::Scratch,
    ) -> mod_api::ScratchSlot {
        let slot = mod_api::ScratchSlot::fresh();
        self.scratch.insert(slot.id(), entry);
        slot
    }

    pub(in crate::game) fn player_write(&mut self) -> Option<&mut Level> {
        self.player_writes = self.player_writes.wrapping_add(1);
        self.writes = self.writes.wrapping_add(1);
        self.last_mut()
    }

    pub(in crate::game) fn journal_modifier_changes(
        &mut self,
        dim: Dim,
        id: u32,
        changes: ModifierChanges,
    ) {
        if let Some(level) = self.last_mut() {
            level
                .modifier_changes
                .entry((dim, id))
                .or_default()
                .merge(changes);
        }
    }

    pub(in crate::game) fn keep_explicit_modifier_write(
        &mut self,
        dim: Dim,
        mob: u32,
        attribute: &str,
        id: &str,
    ) {
        for level in self.iter_mut() {
            if let Some(changes) = level.modifier_changes.get_mut(&(dim, mob)) {
                changes.keep_explicit_write(attribute, id);
            }
            for (_, kept) in level
                .effect_modifiers
                .iter_mut()
                .filter(|(owner, _)| *owner == (dim, mob))
            {
                kept.keep_explicit_write(attribute, id);
            }
        }
    }

    /// Keep a resident row just before its first write, without relinquishing the loan.
    pub(in crate::game) fn journal_block_entity_loan(
        &mut self,
        dim: super::Dim,
        pos: Pos,
        loan: &sunrise_client::block_entities::BlockEntityLoan<'_>,
        chunks: &sunrise_client::chunk_map::ChunkMap,
        name: Option<&sunrise_core::nbt::Tag>,
        hopper: Option<HopperState>,
    ) {
        self.writes = self.writes.wrapping_add(1);
        if let Some(level) = entities::open_in(self, dim) {
            level.block_entities.entry(pos).or_insert_with(|| Kept {
                entity: Some(BlockEntity::Mod(Box::new(loan.entity.clone()))),
                client_update_fields: loan.entity.snapshot_client_update_fields(),
                generation: Some(loan.identity.generation),
                attachments: loan.attachments.cloned(),
                hopper,
                name: name.cloned(),
                extras: BlockEntityExtras::capture(chunks, pos),
                contents_watches: loan.contents_watches.snapshot(loan.identity),
            });
        }
    }

    pub(in crate::game) fn mark_scoped_menu_write(
        &mut self,
        dim: Dim,
        chunk: (i32, i32),
        modified: &mut ModifiedChunks,
    ) {
        if let Some(level) = entities::open_in(self, dim) {
            level
                .save_chunks
                .insert((dim, chunk, SaveChunkScope::Level));
        } else {
            modified.insert(chunk);
        }
    }

    /// Keep the first write to each field of a live general-entity subject.
    pub(in crate::game) fn journal_general_entity(
        &mut self,
        join: crate::join_seq::JoinSeq,
    ) -> Option<&mut KeptGeneralEntity> {
        Some(self.last_mut()?.general_entities.entry(join).or_default())
    }

    /// The journaled-write epoch.
    pub(in crate::game) fn writes(&self) -> u64 {
        self.writes
    }
    /// The journaled-write epoch of players' own state.
    pub(in crate::game) fn player_writes(&self) -> u64 {
        self.player_writes
    }
}

impl std::ops::Deref for OpenTransactions {
    type Target = Vec<Level>;
    fn deref(&self) -> &Vec<Level> {
        &self.levels
    }
}

impl std::ops::DerefMut for OpenTransactions {
    fn deref_mut(&mut self) -> &mut Vec<Level> {
        &mut self.levels
    }
}

/// Puts the closing phase back when a close ends, however it ends.
pub(in crate::game) struct ClosingPhase<'a, 'g, S: crate::game::Side> {
    pub(in crate::game) game: &'a mut Game<'g, S>,
    pub(in crate::game) outer: Option<TransactionLifecycle>,
}

impl<S: crate::game::Side> Drop for ClosingPhase<'_, '_, S> {
    fn drop(&mut self) {
        self.game.transactions.closing = self.outer;
    }
}

pub(in crate::game) struct KeptBlock {
    pub(in crate::game) state: BlockStateId,
    pub(in crate::game) replay: Option<BlockReplay>,
    pub(in crate::game) strict: bool,
}

pub(in crate::game) type RemovedRow = (Box<BlockEntity>, u64);

pub(in crate::game) fn take_replay_row(world: &mut World, pos: Pos) -> Option<RemovedRow> {
    let generation = world.block_entities.generation(&pos);
    world
        .block_entities
        .remove(&pos)
        .zip(generation)
        .map(|(row, generation)| (Box::new(row), generation))
}

pub(in crate::game) struct BlockStep {
    pub(in crate::game) pos: Pos,
    pub(in crate::game) to: BlockStateId,
    pub(in crate::game) replay: BlockReplay,
    // Some means staging took the resident row out (even if it was already absent).
    pub(in crate::game) removed: Option<Option<RemovedRow>>,
}

pub(in crate::game) struct StrictBlockStep {
    pub(in crate::game) to: BlockStateId,
    // Before the first replacement on this cell; swapped for the new row at commit.
    pub(in crate::game) row: Option<Option<RemovedRow>>,
    pub(in crate::game) clear_connection_group: bool,
}

pub(in crate::game) type StrictBlockSteps = indexmap::IndexMap<Pos, StrictBlockStep>;

#[derive(Clone, Copy)]
pub(in crate::game) struct BlockReplay {
    pub(in crate::game) state: BlockStateId,
    pub(in crate::game) flags: i32,
    /// Replay LevelWriteHost writes through the direct host's block-entity creation path.
    pub(in crate::game) level_host: bool,
    /// A level-host write only hid the entity from other reads. At commit
    /// the real Level.setBlock still has to run its removal side effects.
    pub(in crate::game) replay_removal: bool,
    /// Any tentative write at this position replaced the block with one that does not keep
    /// connection groups (`!Behaviours::keeps_block_entity`), even if a later write returned
    /// it to a compatible block.
    pub(in crate::game) clear_connection_group: bool,
}

/// Preserve requested updates, and suppress an effect only if every write
/// suppressed it. Other bits describe the final write (e.g. moved by piston).
pub(in crate::game) fn merge_block_update_flags(earlier: i32, later: i32) -> i32 {
    const REQUESTED: i32 =
        flags::UPDATE_NEIGHBORS | flags::UPDATE_CLIENTS | flags::UPDATE_IMMEDIATE;
    const SUPPRESSED: i32 = flags::UPDATE_INVISIBLE
        | flags::UPDATE_KNOWN_SHAPE
        | flags::UPDATE_SUPPRESS_DROPS
        | flags::UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE
        | flags::UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS
        | flags::UPDATE_SKIP_ON_PLACE;
    ((earlier | later) & REQUESTED)
        | ((earlier & later) & SUPPRESSED)
        | (later & !(REQUESTED | SUPPRESSED))
}

/// Before-images of the fields a general-entity scope wrote, at one depth.
#[derive(Default)]
pub(in crate::game) struct KeptGeneralEntity {
    pub(in crate::game) velocity: Option<Vec3>,
    pub(in crate::game) rotation: Option<[f32; 2]>,
    pub(in crate::game) fall_distance: Option<f64>,
}

/// One journal, either an open frame or a level-local batch split from one at close.
#[derive(Default)]
pub(in crate::game) struct Level {
    pub(in crate::game) frame: Option<TransactionFrame>,
    pub(in crate::game) parent_attachment_query_scope: u64,
    pub(in crate::game) dimension: Option<super::Dim>,
    /// Each block entity as it was before this transaction first wrote it.
    pub(in crate::game) block_entities: HashMap<Pos, Kept>,
    /// Rows moved out of storage when their block was tentatively replaced. Commit replays
    /// removal with the row as it was then; abort uses the first write in `block_entities`.
    /// A removal without a resident row still marks the position for abort.
    pub(in crate::game) removed_block_entities: HashMap<Pos, Option<RemovedRow>>,
    /// A decorated pot lives in its chunk's saved block-entity tags, not the slot table.
    pub(in crate::game) saved_pots: HashMap<Pos, Option<sunrise_core::nbt::Tag>>,
    /// Each item slot (block entity, slot) a slot-only write changed, as it was before this
    /// transaction first wrote it, for a block entity not kept whole in `block_entities`: a
    /// move through a container keeps the slot it touches, not the entity's whole saved data.
    pub(in crate::game) slots: HashMap<(Pos, usize), KeptSlot>,
    /// Each block's original state and the combined update flags of its writes.
    pub(in crate::game) blocks: HashMap<Pos, KeptBlock>,
    /// Each cell's original connection group before this transaction first changed it, used for
    /// silent rollback on abort and pre-replay restoration on root commit.
    pub(in crate::game) connection_groups: HashMap<Pos, Option<u64>>,
    /// Level.random in this frame's own dimension before its first draw.
    pub(in crate::game) level_random: Option<LegacyRandom>,
    /// Other levels drawn from at this depth. Only those draws allocate a map entry.
    pub(in crate::game) parked_level_randoms: HashMap<LevelRef, LegacyRandom>,
    /// Named loot streams before tentative block loot.
    pub(in crate::game) loot_sequences: Option<sunrise_items::random_sequences::RandomSequences>,
    /// Composter-local random streams before this depth first evaluated their layers.
    pub(in crate::game) composter_randoms: ComposterRandoms,
    /// Each quart cell before this transaction first wrote it, keyed by level.
    pub(in crate::game) biomes: HashMap<(Dim, Pos), u16>,
    /// Each player's inventory and saved data before this transaction first wrote them.
    pub(in crate::game) players: HashMap<JournaledPlayer, KeptPlayer>,
    /// Ender storage has one journal shared by player and menu writes.
    pub(in crate::game) ender_items: HashMap<JournaledPlayer, Vec<Option<ItemStack>>>,
    /// Saved player extra NBT (`level.player_extra` / `guest.saved`), shared by player and
    /// persistent-data writes, including dead players.
    pub(in crate::game) player_extra: HashMap<JournaledPlayer, Option<sunrise_core::nbt::Tag>>,
    /// Synced player health and its unpublished update events before set_health, by player body.
    pub(in crate::game) player_health: HashMap<PlayerBodyKey, KeptPlayerHealth>,
    /// Mob health and synchronization dirtiness before set_health, keyed by level.
    pub(in crate::game) mob_health: HashMap<(Dim, u32), KeptMobHealth>,
    /// Active and hidden effects before the first write to the mob at this depth, keyed by level.
    pub(in crate::game) mob_effects: HashMap<(Dim, u32), KeptMobEffects>,
    /// Effect-owned modifier changes in write order, so rows can return to their positions.
    pub(in crate::game) effect_modifiers: Vec<((Dim, u32), EffectModifierUndo)>,
    /// Each entity's inventory (`entity_slots`) before this transaction first wrote it, by level.
    pub(in crate::game) entities: HashMap<(Dim, u32), Vec<Option<ItemStack>>>,
    /// Each mob's or armor stand's equipment before this transaction first
    /// wrote it (#932), told apart for the way back and keyed by level.
    pub(in crate::game) equipment: HashMap<(Dim, u32), KeptEquipment>,
    /// Live modifier diffs, also kept for rebuilds that write no slot, keyed by level.
    pub(in crate::game) modifier_changes: HashMap<(Dim, u32), ModifierChanges>,
    /// Each chested mob's item list's length before `entity_slots` first grew it to its slots
    /// in this transaction, keyed by level.
    pub(in crate::game) mob_lengths: HashMap<(Dim, u32), usize>,
    /// Each item entity's changed fields before this transaction first wrote them.
    pub(in crate::game) item_entities: HashMap<crate::join_seq::JoinSeq, KeptItemEntity>,
    /// Each general entity subject's changed fields before the first write.
    pub(in crate::game) general_entities: HashMap<crate::join_seq::JoinSeq, KeptGeneralEntity>,
    /// The merge fields of resident experience orbs before the first award touched them, by level.
    pub(in crate::game) orbs: HashMap<(Dim, crate::join_seq::JoinSeq), (i32, u32, i32)>,
    /// Each detached station before the first click wrote it, by connection and owner.
    pub(in crate::game) stations:
        HashMap<(u32, decision::MenuId), super::lan_stations::GuestStation>,
    /// Each guest's inventory crafting grid before the first write, by connection.
    pub(in crate::game) menus: HashMap<u32, KeptMenus>,
    /// Drag, machine and preview state before a scoped default click.
    pub(in crate::game) menu_clicks: HashMap<u64, KeptClick>,
    /// The first drag state written in a guest's mount window.
    pub(in crate::game) mount_drags: HashMap<MountClickWindow, QuickCraftDrag>,
    /// Mount window identities to reconcile once the outermost transaction has settled.
    pub(in crate::game) mount_replies: indexmap::IndexSet<MountClickWindow>,
    pub(in crate::game) menu_click_active: Option<u32>,
    /// Each native window's slots before the first write, by menu generation.
    pub(in crate::game) native_menu_slots: HashMap<u64, Vec<Option<ItemStack>>>,
    pub(in crate::game) menu_data: MenuDataJournal,
    /// The scoreboard before this transaction first wrote it (`journal_scoreboard`), whole.
    pub(in crate::game) board: Option<sunrise_client::scoreboard::Scoreboard>,
    /// Each entity's, player's, chunk's and the level's attachment map before this transaction
    /// first reached it (a block entity's is kept with it). Per-level holders are keyed by level;
    /// player-owned attachment maps are keyed by player body across dimensions.
    pub(in crate::game) attachments: HashMap<AttachmentKey, KeptAttachments>,
    /// Held once this transaction reaches an attachment map, until it closes: the save
    /// admission gate (`passes_settled`) refuses a save while a map may hold tentative
    /// values, which only the root commit makes saveable.
    pub(in crate::game) save_gate: Option<sunrise_client::join_seq::visibility::PassLease>,
    /// Each chunk's unsaved mark and save queue entry before this transaction first reached an
    /// attachment map in it: an abort puts them back, so a chunk only a tentative write marked
    /// is clean again.
    pub(in crate::game) chunk_marks: HashMap<(i32, i32), sunrise_client::world::ChunkMark>,
    /// Chunks whose committed storage writes must be saved, in first-write order and in the
    /// levels written.
    pub(in crate::game) save_chunks: indexmap::IndexSet<(Dim, (i32, i32), SaveChunkScope)>,
    /// New map data and its reserved IDs. Neither enters the saved map table until root commit.
    pub(in crate::game) new_maps: BTreeMap<i32, MapData>,
    /// What the root commit does, in order.
    pub(in crate::game) after: Vec<After>,
    /// Internal decide/fire/apply state for menu actions.
    pub(in crate::game) actions: decision::Journal,
    /// Merchant takes keep their session, mob, statistics and criterion progress together.
    pub(in crate::game) trades: super::lan_trade_decisions::Journal,
    pub(in crate::game) packets: Option<sunrise_client::lan::conn::transaction_packets::Scope>,
    pub(in crate::game) captured_packets:
        Vec<sunrise_client::lan::conn::transaction_packets::Packets>,
    pub(in crate::game) action: Option<decision::ActiveAction>,
    /// A scratch slot owned by this frame, and before-images of ancestor loans it wrote.
    pub(in crate::game) owned_scratch: Option<u64>,
    pub(in crate::game) scratch_kept: HashMap<u64, scratch::Scratch>,
    /// The entities it added and removed, and the announcements it holds.
    pub(in crate::game) lifetimes: entities::EntityJournal,
    /// Each attachment map that went with an entity to another level after this transaction
    /// first reached it, by the id it arrived under, as it was then.
    pub(in crate::game) travelled: HashMap<u32, KeptAttachments>,
    /// The index rows of the entities that left this level, kept until the root closes.
    pub(in crate::game) departed: Vec<entities::Departure>,
    /// The entities taken for a transfer whose arrival is not adopted yet, by source id, each
    /// with the slot its arrival reads an abort's way back from.
    pub(in crate::game) in_flight: HashMap<u32, entities::ArrivalRollback>,
}

/// A slot's stack before a transaction first wrote it, and the generation of the block entity
/// it belongs to: an abort puts it back only into that same block entity, never into one that
/// replaced it.
pub(in crate::game) struct KeptSlot {
    pub(in crate::game) stack: Option<ItemStack>,
    pub(in crate::game) generation: Option<u64>,
}

/// Identity of one player body for transaction rollback: either the local host player
/// (`lan_acting.is_none()`) or one guest record in `lan.hub.guests` (a real online guest shared
/// across dimensions, or a per-level connectionless fake player), keyed by its unique `conn` id.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::game) enum PlayerBodyKey {
    Host,
    Guest(u32),
}

/// Key for `Level::attachments`: per-level holders (`Entity`, `Chunk`, `Level`) are isolated by
/// dimension, whereas player-owned attachment maps are keyed by the specific player body
/// regardless of which level is swapped in. Retained originals have their own journal.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::game) enum AttachmentKey {
    Level(Dim, Holder),
    Player(PlayerBodyKey),
}

pub(in crate::game) struct KeptAttachments {
    pub(in crate::game) contents: Attachments,
    pub(in crate::game) indexed: bool,
}

pub(in crate::game) struct Kept {
    pub(in crate::game) entity: Option<BlockEntity>,
    /// The pre-write batch, stored apart from the entity clone's fresh identity.
    pub(in crate::game) client_update_fields: Option<sunrise_core::nbt::Compound>,
    pub(in crate::game) generation: Option<u64>,
    /// None where it had no map: an abort removes one a write made.
    pub(in crate::game) attachments: Option<Attachments>,
    pub(in crate::game) hopper: Option<HopperState>,
    /// The custom name Sunrise keeps beside the model (`World::block_names`).
    pub(in crate::game) name: Option<sunrise_core::nbt::Tag>,
    /// The container lock and loot table roll kept beside the model.
    pub(in crate::game) extras: BlockEntityExtras,
    pub(in crate::game) contents_watches: Vec<(Pos, u64)>,
}

impl Kept {
    pub(in crate::game) fn capture(
        world: &sunrise_client::world::World,
        hoppers: &HashMap<Pos, HopperState>,
        pos: Pos,
    ) -> Self {
        let entity = world.block_entities.get(&pos);
        Self {
            entity: entity.cloned(),
            client_update_fields: match entity {
                Some(BlockEntity::Mod(entity)) => entity.snapshot_client_update_fields(),
                _ => None,
            },
            generation: world.block_entities.generation(&pos),
            attachments: world.block_entities.attachments(pos).cloned(),
            hopper: hoppers.get(&pos).copied(),
            name: world.block_names.get(&pos).cloned(),
            extras: world.block_entity_extras(pos),
            contents_watches: world
                .block_entities
                .identity(pos)
                .map_or_else(Vec::new, |observer| {
                    world.block_entities.contents_watches.snapshot(observer)
                }),
        }
    }

    pub(in crate::game) fn capture_client(world: &sunrise_client::world::World, pos: Pos) -> Self {
        let entity = world.block_entities.get(&pos);
        Self {
            entity: entity.cloned(),
            client_update_fields: match entity {
                Some(BlockEntity::Mod(entity)) => entity.snapshot_client_update_fields(),
                _ => None,
            },
            generation: world.block_entities.generation(&pos),
            attachments: world.block_entities.attachments(pos).cloned(),
            hopper: None,
            name: world.block_names.get(&pos).cloned(),
            extras: world.block_entity_extras(pos),
            contents_watches: world
                .block_entities
                .identity(pos)
                .map_or_else(Vec::new, |observer| {
                    world.block_entities.contents_watches.snapshot(observer)
                }),
        }
    }

    pub(in crate::game) fn restore_client(
        self,
        world: &mut sunrise_client::world::World,
        removed: bool,
        pos: Pos,
    ) {
        let current = world.block_entities.generation(&pos);
        let replaced = current != self.generation;
        if replaced && current.is_some() {
            world.block_entities.remove(&pos);
        }
        if let (Some(BlockEntity::Mod(entity)), Some(fields)) =
            (&self.entity, self.client_update_fields)
        {
            entity.set_client_update_fields(fields);
        }
        match (self.entity, world.block_entities.get_mut(&pos)) {
            (Some(entity), Some(now)) => {
                *now = entity;
                world
                    .block_entities
                    .restore_generation(pos, self.generation.unwrap());
            }
            (Some(entity), None) if replaced || removed => {
                let generation = self.generation.unwrap();
                world.block_entities.insert(pos, entity);
                world.block_entities.restore_generation(pos, generation);
                world.block_entities.reattach_retired(pos, generation);
            }
            _ => {}
        }
        world
            .block_entities
            .put_back_attachments(pos, self.generation, self.attachments);
        match self.name {
            Some(name) => world.block_names.insert(pos, name),
            None => world.block_names.remove(&pos),
        };
        let chunk = (pos.0 >> 4, pos.2 >> 4);
        let mark = world.modified.mark(chunk);
        world.set_block_entity_extras(pos, self.extras);
        world.modified.restore(chunk, mark);
        if let Some(observer) = world.block_entities.identity(pos)
            && Some(observer.generation) == self.generation
        {
            world
                .block_entities
                .contents_watches
                .restore(observer, self.contents_watches);
        }
    }
}

pub(in crate::game) struct KeptPlayerHealth {
    pub(in crate::game) data: sunrise_world::synced_floats::SyncedFloats,
    pub(in crate::game) events_len: usize,
    pub(in crate::game) dead: bool,
    pub(in crate::game) guest: Option<(u32, bool, u32)>,
}

pub(in crate::game) struct KeptMobHealth {
    pub(in crate::game) absorption: f32,
    pub(in crate::game) health: f32,
    pub(in crate::game) needs_sync: bool,
    pub(in crate::game) death_time: i32,
    pub(in crate::game) death_canceled: bool,
    pub(in crate::game) processed: bool,
}

pub(in crate::game) struct KeptMobEffects {
    pub(in crate::game) active: Vec<StoredEffect>,
    pub(in crate::game) chains: Vec<EffectInstance>,
    pub(in crate::game) ambient: Vec<&'static str>,
    pub(in crate::game) hidden: Vec<&'static str>,
    pub(in crate::game) dirty: bool,
}

/// Storage identity, not a public player handle. Fake players in different levels
/// can share a UUID; their hub connections still distinguish their records.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::game) enum JournaledPlayer {
    Local(u128),
    Guest(u32),
}

pub(in crate::game) enum PlayerStorageHome {
    Game,
    Guest(usize),
}

#[derive(Clone)]
pub(in crate::game) struct KeptPlayer {
    pub(in crate::game) inventory: Vec<Option<ItemStack>>,
    pub(in crate::game) armor: [Option<ItemStack>; 4],
    pub(in crate::game) offhand: Option<ItemStack>,
    pub(in crate::game) equipment: Equipment,
    pub(in crate::game) carried: Option<ItemStack>,
}

/// The raw slots before the first write, with a mob's separate gear overlay.
/// Projecting the slots here would bake the overlay into the resident stacks.
pub(in crate::game) enum KeptEquipment {
    Mob {
        slots: Box<Equipment>,
        gear: Option<Box<Gear>>,
    },
    Stand(Box<Equipment>),
}

pub(in crate::game) struct KeptClick {
    pub(in crate::game) conn: u32,
    pub(in crate::game) drag: Option<(u8, Vec<usize>)>,
    pub(in crate::game) craft: Option<sunrise_client::lan::menu::ModCraft>,
    pub(in crate::game) slot_changed_seen: Option<Vec<Option<ItemStack>>>,
    pub(in crate::game) owned: Option<(u64, Box<sunrise_client::menu::Menu>)>,
    pub(in crate::game) machine: Option<(i32, Box<sunrise_client::menu::Menu>)>,
    pub(in crate::game) places: Option<super::native_menu_places::MenuSlotPlaces>,
}

#[derive(Clone)]
pub(in crate::game) struct KeptMenus {
    pub(in crate::game) craft_grid: [Option<ItemStack>; 4],
}

/// What a write tells the world, at the root commit.
pub(in crate::game) enum After {
    /// Reserved raw bytes publish only when their connection incarnation is still live.
    Payload(super::payload_transport::Deferred),
    /// A mod's validated vanilla packet, bound to its physical protocol incarnation.
    Packet(
        sunrise_client::net::PreparedPacket,
        mod_api::ServerboundPlayPacket,
    ),
    Composter(super::composter::Effects),
    /// A mod's world pickup or placement of fluid, its block already set: what is left of it
    /// (`Game::commit_fluid_write`).
    FluidWrite(Box<super::world_fluids::WrittenFluid>),
    /// A bucket's sound and fluid game event (`level_play_fluid_effects`).
    FluidEffects(Box<super::world_fluids::FluidEffects>),
    /// A dispenser drop in the source level, with optional overflow click and smoke.
    DispenserDrop {
        pos: Pos,
        facing: sunrise_core::geometry::Direction,
        spawn: Box<super::spill::Spawn>,
        play_events: bool,
    },
    /// Sound and smoke for a mod's accepted dispense reply, after its behaviour's effects.
    DispenserFeedback {
        pos: Pos,
        facing: sunrise_core::geometry::Direction,
        event: i32,
    },
    /// `BlockEntity.setChanged`.
    Changed(Pos),
    /// `BlockEntity.setChanged` dirty/comparator updates without reentering `contents_changed`.
    ChangedWithoutHook(Pos),
    /// A notice for a specific resident row, not a replacement at the same position.
    ChangedHolder {
        holder: BlockEntityIdentity,
        silent: bool,
    },
    GuestMenu(u32, Box<super::lan_crafting::GuestMenuEffects>),
    /// Publish both the connection-local state and its packets together, never on abort.
    PersonalSidebar(
        u32,
        Option<Box<sunrise_client::lan::scoreboard::SidebarContents>>,
    ),
    /// Equipment transitions from a mount menu, with distinct sound and game event positions.
    MountEquipment {
        dim: Dim,
        id: u32,
        at: [f64; 3],
        sound: Option<(String, [f64; 3])>,
        equips: bool,
    },
    MenuClick(Box<super::lan_menus::MenuClickReply>),
    MenuPlacesNotice(u32, u128, i32),
    FurnaceMenu(Box<super::lan_crafting::FurnaceMenuEffects>),
    StoreCartItems(Pos),
    BackedMenuClick(u32, u128, mod_api::MenuRef, i32),
    /// A provider's answer changed here (`level_invalidate_capabilities`): the capability
    /// generation moves on.
    Capabilities(Pos),

    /// `ServerLevel.destroyBlockProgress`: breaker id, position, stage. Sent once, at the root commit.
    BreakProgress(i32, [i32; 3], i32),
    /// Replay a block write (coalesced by position unless a strict edit shares the cell).
    Block(BlockStep),
    /// Publish the storage and watched writes of a strict bulk edit, not ordinary updates.
    BulkBlocks(
        sunrise_client::block_edit::BulkPublication,
        Vec<super::block_edits::Tracked>,
        Vec<[i32; 3]>,
        StrictBlockSteps,
    ),
    /// An explicit `LevelRef::set_connection_group` write, replayed in order with block writes at
    /// the root commit so cross-position cascades (e.g. popping a supported block) clear it.
    ConnectionGroup(Pos, Option<u64>),
    /// A validated level write host effect; never published on abort.
    LevelWrite(Box<super::journal_level_write::PendingWrite>),
    BlockGameEvent(
        &'static str,
        [i32; 3],
        sunrise_client::mobs::Who,
        BlockStateId,
    ),
    /// Save, remesh and resend a changed biome column.
    Biome((i32, i32)),
    /// Deferred interaction effects, replayed in the same root-commit order.
    Effect(DeferredEffect),
    /// Effects of a tentative merchant take, published only at the root.
    Trade(Box<super::lan_trade_decisions::Effect>),
    /// `Block.popResource`.
    Drop([i32; 3], ItemStack),
    /// `Player.drop(stack, false)` by the player with this UUID: what no longer fits.
    PlayerDrop(u128, ItemStack),
    /// A respawn replaces the holder behind the player's UUID. Do not install a new
    /// attachment generation or copy tentative data until the old generation is committed.
    PlayerRespawn(u32),
    /// `/give`'s fake item and pickup sound in the receiving player's level.
    GivePickup {
        uuid: u128,
        dim: super::Dim,
        prototype: ItemStack,
    },
    InventoryTrigger(u128),
    GrindstoneEvent([i32; 3]),
    StationContent(u32, decision::MenuId, i32),
    /// What did not fit of a `/give` to the player `uuid`, thrown in the level `dim` that was in
    /// place when it was accepted; at `eye`, the player's eyes then, if they are dead or gone
    /// by the commit.
    GiveRest {
        uuid: u128,
        rest: ItemStack,
        owner: Option<[i32; 4]>,
        dim: super::Dim,
        eye: [f64; 3],
    },
    PlayerEquip(u128, String, Option<ItemStack>),
    /// An item split off an item entity, dropped where it is (`Block.popResource`'s delay).
    Spawn([f64; 3], ItemStack),
    /// The item entity with this id was emptied: removed, unless it holds items again.
    Emptied(u32),
    /// A component-only mob equipment write must reach the normal tracker even if the mob is idle.
    EquipmentSync(u32),
    /// `LivingEntity.onEquipItem`'s sound and equip/unequip game event, at the entity `who`
    /// is: what an equipment write through a transaction tells the world only at the root
    /// commit, so an abort stays quiet (#932).
    Equip {
        at: [f64; 3],
        sound: Option<String>,
        equips: bool,
        who: u32,
    },
}

impl After {
    pub(in crate::game) fn level_owned(&self) -> bool {
        matches!(
            self,
            Self::Block(_)
                | Self::BulkBlocks(..)
                | Self::ConnectionGroup(..)
                | Self::Changed(_)
                | Self::ChangedWithoutHook(_)
                | Self::ChangedHolder { .. }
                | Self::Capabilities(_)
                | Self::LevelWrite(_)
                | Self::FluidWrite(_)
                | Self::FluidEffects(_)
                | Self::DispenserDrop { .. }
                | Self::DispenserFeedback { .. }
                | Self::BlockGameEvent(..)
                | Self::Biome(_)
                | Self::Drop(..)
                | Self::Spawn(..)
                | Self::Emptied(_)
        )
    }
}

/// Effects shared by transactional interaction contexts.
pub(in crate::game) enum DeferredEffect {
    CustomStat {
        player: u128,
        key: &'static str,
        amount: i32,
    },
    /// `Player.awardStat` of an item statistic (`used`, `crafted`...) for `item`.
    ItemStat {
        player: u128,
        stat: &'static str,
        item: std::sync::Arc<str>,
        amount: i32,
    },
    Sound {
        sound: &'static str,
        at: [f64; 3],
        volume: f32,
        pitch: f32,
    },
    /// `PlayerRef::set_statistics` inside a transaction: the counters and their criterion
    /// scores are replaced together at the root commit.
    RestoreStatistics(mod_api::PlayerRef, Box<sunrise_gameplay::stats::StatValues>),
}

impl Level {
    /// Keep the first snapshot for this stream. `None` names the frame's own level.
    pub(in crate::game) fn keep_level_random(
        &mut self,
        owner: Option<LevelRef>,
        before: impl FnOnce() -> LegacyRandom,
    ) {
        match owner {
            Some(level) if self.dimension.map(super::Dim::key) != Some(level.dimension()) => {
                self.parked_level_randoms
                    .entry(level)
                    .or_insert_with(before);
            }
            _ => {
                self.level_random.get_or_insert_with(before);
            }
        }
    }

    pub(in crate::game) fn keep_bulk_block(&mut self, pos: Pos, state: u32) {
        self.blocks
            .entry(pos)
            .and_modify(|kept| kept.strict = true)
            .or_insert(KeptBlock {
                state: BlockStateId(state),
                replay: None,
                strict: true,
            });
    }

    pub(in crate::game) fn keep_connection_group(&mut self, pos: Pos, group: Option<u64>) {
        self.connection_groups.entry(pos).or_insert(group);
    }

    pub(in crate::game) fn merge_level_writes(&mut self, child: &mut Self) {
        for (pos, biome) in std::mem::take(&mut child.biomes) {
            self.biomes.entry(pos).or_insert(biome);
        }
        for (pos, group) in std::mem::take(&mut child.connection_groups) {
            self.keep_connection_group(pos, group);
        }
        // Slots before whole entities: a slot the outer transaction already kept the
        // whole entity of is older there, but one kept whole only by this child is not,
        // so the child's slot entries go up beside its capture.
        for ((pos, slot), kept) in std::mem::take(&mut child.slots) {
            if !self.block_entities.contains_key(&pos) {
                self.slots.entry((pos, slot)).or_insert(kept);
            }
        }
        for (pos, kept) in std::mem::take(&mut child.block_entities) {
            self.block_entities.entry(pos).or_insert(kept);
        }
        for (pos, tag) in std::mem::take(&mut child.saved_pots) {
            self.saved_pots.entry(pos).or_insert(tag);
        }
        for (pos, removed) in std::mem::take(&mut child.removed_block_entities) {
            let first = self.removed_block_entities.entry(pos).or_default();
            if first.is_none() {
                *first = removed;
            }
        }
        for (pos, block) in std::mem::take(&mut child.blocks) {
            self.blocks
                .entry(pos)
                .and_modify(|kept| {
                    kept.strict |= block.strict;
                    if let Some(replay) = block.replay {
                        if let Some(earlier) = &mut kept.replay {
                            earlier.flags = merge_block_update_flags(earlier.flags, replay.flags);
                            earlier.level_host = replay.level_host;
                            earlier.replay_removal |= replay.replay_removal;
                            earlier.clear_connection_group |= replay.clear_connection_group;
                        } else {
                            kept.replay = Some(replay);
                        }
                    }
                })
                .or_insert(block);
        }
        for (id, kept) in std::mem::take(&mut child.item_entities) {
            self.item_entities.entry(id).or_default().merge(kept);
        }
        for (join, kept) in std::mem::take(&mut child.general_entities) {
            let first = self.general_entities.entry(join).or_default();
            first.velocity = first.velocity.or(kept.velocity);
            first.rotation = first.rotation.or(kept.rotation);
            first.fall_distance = first.fall_distance.or(kept.fall_distance);
        }
        for (chunk, mark) in std::mem::take(&mut child.chunk_marks) {
            self.chunk_marks.entry(chunk).or_insert(mark);
        }
    }

    pub(in crate::game) fn keep_player(
        &mut self,
        owner: JournaledPlayer,
        ender_items: &[Option<ItemStack>],
        player_extra: Option<&sunrise_core::nbt::Tag>,
        capture: impl FnOnce() -> KeptPlayer,
    ) {
        self.ender_items
            .entry(owner)
            .or_insert_with(|| ender_items.to_vec());
        self.player_extra
            .entry(owner)
            .or_insert_with(|| player_extra.cloned());
        self.players.entry(owner).or_insert_with(capture);
    }

    /// Keeps chested mob `id`'s item list's length, `len`, before `entity_slots` grows it, if
    /// this transaction hasn't yet.
    pub(in crate::game) fn keep_mob_length(&mut self, dim: Dim, id: u32, len: usize) {
        self.mob_lengths.entry((dim, id)).or_insert(len);
    }

    /// Keep a resolved equipment row at its first write in this transaction.
    /// `mob_gear` is absent for a stand; a mob's overlay may itself be absent.
    pub(in crate::game) fn keep_equipment(
        &mut self,
        dim: Dim,
        id: u32,
        equipment: &Equipment,
        mob_gear: Option<&Option<Box<Gear>>>,
    ) {
        self.equipment.entry((dim, id)).or_insert_with(|| {
            let slots = Box::new(equipment.clone());
            match mob_gear {
                Some(gear) => KeptEquipment::Mob {
                    slots,
                    gear: gear.clone(),
                },
                None => KeptEquipment::Stand(slots),
            }
        });
    }
}

pub(in crate::game) enum StorageSettlement {
    Committed(Option<scratch::Scratch>),
    Aborted,
    Refused,
}

/// RAII guard for one open transaction frame. Dropping it without `close` aborts this frame
/// and any nested frames still open inside it.
pub(in crate::game) struct StorageScope<'a, 'g, S: crate::game::Side> {
    pub(in crate::game) game: &'a mut Game<'g, S>,
    pub(in crate::game) frame: Option<TransactionFrame>,
}

impl<S: crate::game::Side> StorageScope<'_, '_, S> {
    pub(in crate::game) fn close(mut self, commit: bool) -> Option<StorageSettlement> {
        self.frame
            .take()
            .and_then(|frame| self.game.close_storage_frame(frame, commit))
    }
}

impl<S: crate::game::Side> Drop for StorageScope<'_, '_, S> {
    fn drop(&mut self) {
        if let Some(frame) = self.frame.take() {
            let _ = self.game.close_storage_frame(frame, false);
        }
    }
}

impl<'g, __Side: crate::game::Side> Game<'g, __Side> {
    /// Actual transaction/restore events for storage-attempt regression traces.
    #[cfg(test)]
    pub(in crate::game) fn trace_storage_attempt(&self, entry: String) {
        if let Some(trace) = &self.transactions.trace {
            trace.lock().unwrap().push(entry);
        }
    }
    /// Pending payloads in the innermost transaction (before publication).
    #[cfg(test)]
    pub(in crate::game) fn pending_transaction_payloads(&self) -> usize {
        self.transactions
            .last()
            .unwrap()
            .after
            .iter()
            .filter(|after| matches!(after, After::Payload(_)))
            .count()
    }
    /// Pending item-stat effects in the innermost transaction (before publication).
    #[cfg(test)]
    pub(in crate::game) fn pending_transaction_item_stats(&self) -> usize {
        self.transactions
            .last()
            .unwrap()
            .after
            .iter()
            .filter(|after| matches!(after, After::Effect(DeferredEffect::ItemStat { .. })))
            .count()
    }

    /// Opens a transaction and gives back its frame identity.
    pub(in crate::game) fn open_storage_transaction(&mut self) -> TransactionFrame {
        let frame = self.transactions.next_frame();
        if !self.world.block_entities.defers_retirement() {
            self.world.block_entities.defer_retirement();
        }
        let lifetimes = self.entity_journal_open();
        let parent_attachment_query_scope = self.transactions.attachment_query_scopes.enter();
        self.transactions.push(Level {
            frame: Some(frame),
            parent_attachment_query_scope,
            dimension: Some(self.dim),
            packets: sunrise_client::lan::conn::transaction_packets::Scope::nested(),
            lifetimes,
            ..Level::default()
        });
        for slot in &self.transactions.retained_original_attachments {
            slot.lock()
                .unwrap_or_else(|e| e.into_inner())
                .open_frame(frame);
        }
        #[cfg(test)]
        self.trace_storage_attempt("open".into());
        frame
    }
    /// Closes `frame`, first aborting every transaction opened inside it and left open, so a
    /// scope closing its own is never stranded behind an abandoned child.
    pub(in crate::game) fn close_storage_frame(
        &mut self,
        frame: TransactionFrame,
        commit: bool,
    ) -> Option<StorageSettlement> {
        let at = self
            .transactions
            .iter()
            .rposition(|level| level.frame == Some(frame))?;
        if self.transactions.len() > at + 1 {
            log::warn!("Sunrise: aborted a transaction left open inside the one being closed");
        }
        while self.transactions.len() > at + 1 {
            let _ = self.close_storage_transaction(false);
        }
        self.close_storage_transaction(commit)
    }
    /// An engine batch opens a frame only when it has a callback to journal.
    pub(in crate::game) fn storage_scope_guard(
        &mut self,
        open: bool,
    ) -> StorageScope<'_, 'g, __Side> {
        let frame = open.then(|| self.open_storage_transaction());
        StorageScope { game: self, frame }
    }

    /// The trident tick opens a frame at contact, then closes it after applying the
    /// decision. The same guard aborts an open frame if dispatch or apply unwinds.
    pub(in crate::game) fn with_late_storage_scope<R>(
        &mut self,
        f: impl FnOnce(&mut Self, &mut Option<TransactionFrame>) -> R,
    ) -> R {
        let mut scope = self.storage_scope_guard(false);
        f(&mut *scope.game, &mut scope.frame)
    }

    /// Runs `f` inside a new (or nested) storage transaction, committing iff `f` returns
    /// `(out, true)` and aborting on `false` or unwind.
    pub(in crate::game) fn storage_scope<R>(
        &mut self,
        f: impl FnOnce(&mut Self, TransactionFrame) -> (R, bool),
    ) -> R {
        self.run_storage_scope(f).0
    }

    /// Also returns the frame's own detached scratch row when it commits.
    pub(in crate::game) fn run_storage_scope<R>(
        &mut self,
        f: impl FnOnce(&mut Self, TransactionFrame) -> (R, bool),
    ) -> (R, StorageSettlement) {
        let frame = self.open_storage_transaction();
        let guard = StorageScope {
            game: self,
            frame: Some(frame),
        };
        let (out, commit) = f(&mut *guard.game, frame);
        let settled = guard.close(commit).unwrap_or(if commit {
            StorageSettlement::Refused
        } else {
            StorageSettlement::Aborted
        });
        (out, settled)
    }
    /// Runs `f` inside a new (or nested) storage transaction, lending a scoped
    /// [`TransactionContext`]; commits on `Ok` and aborts on `Err` or unwind.
    pub(in crate::game) fn storage_transaction<T, E>(
        &mut self,
        f: impl for<'loan> FnOnce(&'loan mut Self, &'loan TransactionContext) -> Result<T, E>,
    ) -> Result<T, E> {
        self.storage_scope(|game, frame| {
            let outcome = frame.lend(|tx| f(game, tx));
            let commit = outcome.is_ok();
            (outcome, commit)
        })
    }
    /// Runs `f` inside a new (or nested) storage transaction and reports whether a requested
    /// commit was refused and rolled back.
    pub(in crate::game) fn storage_transaction_settled<T, E>(
        &mut self,
        f: impl for<'loan> FnOnce(&'loan mut Self, &'loan TransactionContext) -> Result<T, E>,
    ) -> Result<T, mod_api::TransactionError<E>> {
        let (outcome, settled) = self.run_storage_scope(|game, frame| {
            let outcome = frame.lend(|tx| f(game, tx));
            let commit = outcome.is_ok();
            (outcome, commit)
        });
        match (outcome, settled) {
            (Ok(value), StorageSettlement::Committed(_)) => Ok(value),
            (Ok(_), StorageSettlement::Refused | StorageSettlement::Aborted) => {
                Err(mod_api::TransactionError::CommitRefused)
            }
            (Err(error), _) => Err(mod_api::TransactionError::Aborted(error)),
        }
    }
    /// Runs an API scope and reports actual settlement, not just the callback's intent.
    pub(in crate::game) fn scoped_storage<T>(
        &mut self,
        f: impl for<'loan> FnOnce(&'loan mut Self, &'loan TransactionContext) -> Option<T>,
    ) -> Result<Option<T>, TransactionScopeError> {
        let (outcome, settled) = self.run_storage_scope(|game, frame| {
            let outcome = frame.lend(|tx| f(game, tx));
            let commit = outcome.is_some();
            (outcome, commit)
        });
        match settled {
            StorageSettlement::Committed(_) => Ok(outcome),
            StorageSettlement::Aborted => Ok(None),
            StorageSettlement::Refused => Err(TransactionScopeError::CommitRefused),
        }
    }
    /// Whether at least one storage transaction is currently open.
    pub(in crate::game) fn has_open_transaction(&self) -> bool {
        !self.transactions.is_empty()
    }
    /// The innermost open storage transaction's frame identity, if any.
    pub(in crate::game) fn current_storage_frame(&self) -> Option<TransactionFrame> {
        self.transactions.last().and_then(|level| level.frame)
    }
    /// Whether `tx` is the live loan for this game's innermost open transaction.
    pub(in crate::game) fn transaction_context_is_current(&self, tx: &TransactionContext) -> bool {
        self.current_storage_frame()
            .is_some_and(|frame| frame.matches(tx))
    }
    /// `transaction_lifecycle`: the close running, else whether one is open.
    pub(in crate::game) fn transaction_lifecycle(&self) -> TransactionLifecycle {
        match self.transactions.closing {
            Some(closing) => closing,
            None if self.transactions.is_empty() => TransactionLifecycle::None,
            None => TransactionLifecycle::Open,
        }
    }
    pub(in crate::game) fn current_transaction_is_here(&self) -> bool {
        self.transactions
            .last()
            .is_some_and(|frame| frame.dimension == Some(self.dim))
    }
    pub(in crate::game) fn has_level_transaction(&self) -> bool {
        self.transactions
            .iter()
            .any(|level| level.dimension == Some(self.dim))
    }
    /// Closes the innermost transaction, delegating to the active role's handler.
    pub(in crate::game) fn close_storage_transaction(
        &mut self,
        commit: bool,
    ) -> Option<StorageSettlement> {
        self.side_tx_close_storage_transaction(commit)
    }
    /// Aborts abandoned frames before the next operation can join them. Call only when
    /// there is no caller-owned frame; the tick start also uses this as a backstop.
    pub(in crate::game) fn abort_open_transactions(&mut self) {
        while !self.transactions.is_empty() {
            let _ = self.close_storage_transaction(false);
        }
        // If called during a root close, the enclosing close still owns retirement.
        if self.transactions.closing.is_none() {
            self.world.block_entities.publish_retirement();
        }
    }

    /// `ComposterWrapper.transactionalRandom`: snapshot only the position's private owner,
    /// including its initial state on a first access that later aborts.
    pub(in crate::game) fn take_composter_random(
        &mut self,
        pos: [i32; 3],
    ) -> sunrise_core::random::LegacyRandom {
        let epoch = self.native_lifecycle.epoch();
        if self.transactions.composter_epoch != epoch {
            self.clear_composter_randoms();
            self.transactions.composter_epoch = epoch;
        }
        let chunk = (self.dim.key(), (pos[0] >> 4, pos[2] >> 4));
        let key = (pos[0], pos[1], pos[2]);
        let random = self
            .transactions
            .composter_randoms
            .entry(chunk)
            .or_default()
            .remove(&key)
            .unwrap_or_else(|| (*self.world.random).clone());
        if let Some(level) = self.transactions.last_mut() {
            level
                .composter_randoms
                .entry(chunk)
                .or_default()
                .entry(key)
                .or_insert_with(|| random.clone());
        }
        random
    }

    pub(in crate::game) fn put_composter_random(
        &mut self,
        pos: [i32; 3],
        random: sunrise_core::random::LegacyRandom,
    ) {
        self.transactions
            .composter_randoms
            .entry((self.dim.key(), (pos[0] >> 4, pos[2] >> 4)))
            .or_default()
            .insert((pos[0], pos[1], pos[2]), random);
    }

    /// Retire only an unloading chunk's cached owners and their pending savepoints.
    pub(in crate::game) fn forget_composter_chunk(
        &mut self,
        dimension: mod_api::DimensionKey,
        chunk: [i32; 2],
    ) {
        let key = (dimension, (chunk[0], chunk[1]));
        self.transactions.composter_randoms.remove(&key);
        for level in &mut self.transactions.levels {
            level.composter_randoms.remove(&key);
        }
    }

    pub(in crate::game) fn clear_composter_randoms(&mut self) {
        self.transactions.composter_epoch = None;
        self.transactions.composter_randoms.clear();
        for level in &mut self.transactions.levels {
            level.composter_randoms.clear();
        }
    }
    /// Keeps a pot's saved item and loot metadata until its level commits.
    pub(in crate::game) fn journal_saved_pot(&mut self, pos: Pos) {
        self.transactions.writes = self.transactions.writes.wrapping_add(1);
        if let Some(level) = entities::open_in(&mut self.transactions, self.dim) {
            level
                .saved_pots
                .entry(pos)
                .or_insert_with(|| super::spill::saved_entity(&self.world, [pos.0, pos.1, pos.2]));
            level
                .save_gate
                .get_or_insert_with(sunrise_client::join_seq::visibility::PassLease::enter);
        }
    }

    /// Keeps the block entity at `pos` as it is if a transaction is open.
    pub(in crate::game) fn journal_block_entity(&mut self, pos: Pos) {
        self.side_tx_journal_block_entity(pos);
    }
    /// Puts back a removed block entity and its generation where none stands there now.
    pub(in crate::game) fn restore_removed_block_entity(
        &mut self,
        pos: Pos,
        entity: BlockEntity,
        generation: u64,
    ) {
        let entities = &mut self.world.block_entities;
        if entities.get(&pos).is_none() {
            entities.insert(pos, entity);
            entities.restore_generation(pos, generation);
            entities.reattach_retired(pos, generation);
        }
    }
    /// Hides the block entity from tentative reads. The first-write journal can undo all edits;
    /// the removed row itself still contains the edits that matter to removal at commit.
    pub(in crate::game) fn journal_remove_block_entity(&mut self, pos: Pos) {
        self.journal_block_entity(pos);
        let Some(level) = entities::open_in(&mut self.transactions, self.dim) else {
            self.world.block_entities.remove(&pos);
            return;
        };
        let removed = take_replay_row(&mut self.world, pos);
        if let Some(After::Block(step)) = level.after.last_mut()
            && step.pos == pos
        {
            step.removed = Some(removed);
        } else {
            let first = level.removed_block_entities.entry(pos).or_default();
            if first.is_none() {
                *first = removed;
            }
        }
    }
    /// How many block entities the open transactions keep whole, and how many single slots:
    /// what a test reads to tell a slot journal from a whole capture without timing it.
    #[cfg(test)]
    pub(in crate::game) fn journal_kept(&self) -> (usize, usize) {
        self.transactions
            .iter()
            .fold((0, 0), |(whole, slots), level| {
                (
                    whole + level.block_entities.len(),
                    slots + level.slots.len(),
                )
            })
    }
    /// Keeps item slot `slot` of the block entity at `pos` as it is, if a transaction is open
    /// and keeps neither it nor the whole entity yet. Only for a write that changes that slot
    /// and nothing else of the entity (no cooldown, cooking or saved data beside it), so the
    /// journal holds one stack however large the entity's saved data is. A slot past the end of
    /// the entity's slot list (a mod's list shorter than its container) is a write that grows
    /// the list: the entity is kept whole instead, length and all.
    pub(in crate::game) fn journal_block_entity_slot(&mut self, pos: Pos, slot: usize) {
        self.transactions.writes = self.transactions.writes.wrapping_add(1);
        let entity = self.world.block_entities.get(&pos);
        // Claiming an opaque row changes the saved compound as well as the slot.
        if entity.is_none_or(|entity| slot >= entity.slots().len())
            || matches!(entity, Some(BlockEntity::Mod(m)) if m.has_opaque_item_slot(slot))
        {
            return self.journal_block_entity(pos);
        }
        let Some(level) = entities::open_in(&mut self.transactions, self.dim) else {
            return;
        };
        if level.block_entities.contains_key(&pos) {
            return;
        }
        level.slots.entry((pos, slot)).or_insert_with(|| KeptSlot {
            stack: entity.and_then(|entity| entity.slots()[slot].clone()),
            generation: self.world.block_entities.generation(&pos),
        });
    }
    /// Keeps the inventory of `uuid` on its first write in the open frame.
    pub(in crate::game) fn journal_player(&mut self, uuid: u128) {
        self.side_tx_journal_player(uuid);
    }
    /// Keeps `player`'s menu storage as it is if a transaction is open.
    pub(in crate::game) fn journal_menus(&mut self, player: mod_api::PlayerRef) {
        self.side_tx_journal_menus(player);
    }

    pub(in crate::game) fn journal_mount_drag(
        &mut self,
        window: MountClickWindow,
        before: QuickCraftDrag,
    ) {
        if let Some(level) = self.transactions.last_mut() {
            level.mount_drags.entry(window).or_insert(before);
        }
    }

    /// Keeps entity `id`'s inventory as it is, if a transaction is open and hasn't yet.
    pub(in crate::game) fn journal_entity(&mut self, id: u32) {
        self.side_journal_entity(id);
    }
    /// Keeps the scoreboard as it is, if a transaction is open and hasn't yet: a copy of the
    /// whole board, taken on the transaction's first scoreboard write and never otherwise, so a
    /// transaction that writes none pays nothing. An abort puts it back.
    pub(in crate::game) fn journal_scoreboard(&mut self) {
        let Some(level) = self.transactions.last_mut() else {
            return;
        };
        if level.board.is_none() {
            level.board = Some(self.score.board.clone());
        }
    }
    /// Keeps the slots at their first write. Equipment collection records any
    /// actual modifier edits separately; a quiet edit does not run it.
    pub(in crate::game) fn journal_equipment(&mut self, id: u32) {
        let dim = self.dim;
        let Some(level) = self.transactions.last_mut() else {
            return;
        };
        if level.equipment.contains_key(&(dim, id)) {
            return;
        }
        if let Some(mob) = self.mobs.iter().find(|m| m.id == id && !m.removed) {
            level.keep_equipment(dim, id, &mob.extra.equipment, Some(&mob.extra.gear));
        } else if let Some(stand) = self.armor_stands.iter().find(|a| a.id == id) {
            level.keep_equipment(dim, id, &stand.equipment, None);
        }
    }
    /// `drop_plain` by the player whose state is in `Game` now, `uuid`: at once, or at the root
    /// commit inside a transaction.
    pub(in crate::game) fn drop_after_commit(&mut self, uuid: u128, stack: ItemStack) {
        match self.transactions.last_mut() {
            Some(level) => level.after.push(After::PlayerDrop(uuid, stack)),
            None => {
                self.drop_plain(stack);
            }
        }
    }
    /// Set or clear a connection group at `pos`. Inside an open Host transaction, writes
    /// storage quietly, records the prior group so abort restores it silently, and defers
    /// remesh + client sync to root commit. Returns `false` if unloaded, out of height, or
    /// unchanged.
    pub(in crate::game) fn journal_set_connection_group(
        &mut self,
        pos: [i32; 3],
        group: Option<u64>,
    ) -> bool {
        let dim = self.dim;
        let Some(level) = entities::open_in(&mut self.transactions, dim) else {
            return self
                .world
                .set_connection_group(pos[0], pos[1], pos[2], group, true);
        };
        let key = (pos[0], pos[1], pos[2]);
        let old = self.world.connection_group(pos[0], pos[1], pos[2]);
        let changed = self
            .world
            .set_connection_group_stored(pos[0], pos[1], pos[2], group);
        if changed {
            level.keep_connection_group(key, old);
            level.after.push(After::ConnectionGroup(key, group));
        }
        changed
    }
    /// Sets the block at `pos` to `state`. In a transaction, stored only, keeping the old
    /// state, and set with its updates at the root commit; otherwise at once
    /// (`Level.setBlock`).
    pub(in crate::game) fn journal_set_block(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> bool {
        self.journal_set_block_flags(pos, state, sunrise_world::level::flags::UPDATE_ALL, false)
    }
    /// The root commit replays one write per position with the combined update flags.
    pub(in crate::game) fn journal_set_block_flags(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        flags: i32,
        level_host: bool,
    ) -> bool {
        let Some(level) = entities::open_in(&mut self.transactions, self.dim) else {
            return self.level_set_with_flags(pos, state, flags).is_ok();
        };
        let key = (pos[0], pos[1], pos[2]);
        let old = self.world.block(pos[0], pos[1], pos[2]);
        let clears_group = !self.res.behaviours().keeps_block_entity(old, state);
        if !self.world.set_stored(pos[0], pos[1], pos[2], state) {
            return false;
        }
        // Mirror live `chunk_set_block_state`: replacing the block with one that does not keep
        // connection groups clears any tentative group at `pos`, while compatible state changes
        // preserve it.
        let old_group = clears_group
            .then(|| self.world.connection_group(pos[0], pos[1], pos[2]))
            .flatten();
        if old_group.is_some() {
            let _ = self
                .world
                .set_connection_group_stored(pos[0], pos[1], pos[2], None);
        }
        let block = level.blocks.entry(key).or_insert(KeptBlock {
            state: old,
            replay: None,
            strict: false,
        });
        let step = BlockReplay {
            state: old,
            flags,
            level_host,
            replay_removal: false,
            clear_connection_group: clears_group,
        };
        let replay = block.replay.get_or_insert(step);
        replay.flags = merge_block_update_flags(replay.flags, flags);
        replay.level_host = level_host;
        replay.clear_connection_group |= clears_group;
        if old_group.is_some() {
            level.keep_connection_group(key, old_group);
        }
        level.after.push(After::Block(BlockStep {
            pos: key,
            to: state,
            replay: step,
            removed: None,
        }));
        true
    }
    /// Hide a replaced block entity from staged reads, without dropping its
    /// contents until the real write. Fluid pickup/pour instead rolls its own
    /// drops against the old entity before journalling the removal.
    pub(in crate::game) fn journal_block_replacement(&mut self, pos: Pos) {
        self.journal_remove_block_entity(pos);
        let Some(level) = entities::open_in(&mut self.transactions, self.dim) else {
            return;
        };
        if let Some(After::Block(step)) = level.after.last_mut() {
            step.replay.replay_removal = true;
        }
        if let Some(replay) = level
            .blocks
            .get_mut(&pos)
            .and_then(|block| block.replay.as_mut())
        {
            replay.replay_removal = true;
        }
    }
    pub(in crate::game) fn journal_level_random(&mut self) {
        self.journal_active_level_random(LegacyRandom::clone);
    }

    /// Keep an engine or restricted-writer draw from the stream currently in place.
    pub(in crate::game) fn journal_active_level_random(
        &mut self,
        before: impl FnOnce(&LegacyRandom) -> LegacyRandom,
    ) {
        let owner = match self.transactions.last() {
            None => return,
            Some(frame) if frame.dimension == Some(self.dim) => None,
            Some(_) => {
                let Some(epoch) = self.native_lifecycle.epoch() else {
                    return;
                };
                let Some(level_ref) = self.side_active_level_ref(epoch) else {
                    return;
                };
                Some(level_ref)
            }
        };
        if let Some(frame) = self.transactions.last_mut() {
            frame.keep_level_random(owner, || before(&self.world.random));
        }
    }
    pub(in crate::game) fn journal_orb_merge(
        &mut self,
        join: crate::join_seq::JoinSeq,
        fields: (i32, u32, i32),
    ) {
        let dim = self.dim;
        if let Some(level) = self.transactions.last_mut() {
            level.orbs.entry((dim, join)).or_insert(fields);
        }
    }
    pub(in crate::game) fn journal_station_click(
        &mut self,
        conn: u32,
        owner: decision::MenuId,
        station: &super::lan_stations::GuestStation,
    ) {
        if let Some(level) = self.transactions.last_mut() {
            level
                .stations
                .entry((conn, owner))
                .or_insert_with(|| station.clone());
        }
    }
    /// Keep the original cell in the transaction for the level in place.
    pub(in crate::game) fn journal_biome(&mut self, pos: [i32; 3], old: u16) {
        let dim = self.dim;
        if let Some(level) = entities::open_in(&mut self.transactions, dim) {
            level
                .biomes
                .entry((dim, (pos[0] >> 2, pos[1] >> 2, pos[2] >> 2)))
                .or_insert(old);
        }
    }
    /// Mark a written chunk unsaved at commit, without issuing any notice.
    pub(in crate::game) fn journal_chunk_save(&mut self, chunk: (i32, i32), scope: SaveChunkScope) {
        let dim = self.dim;
        let level = match scope {
            SaveChunkScope::Level => entities::open_in(&mut self.transactions, dim),
            SaveChunkScope::Transaction => self.transactions.last_mut(),
        };
        if let Some(level) = level {
            level.save_chunks.insert((dim, chunk, scope));
        }
    }
    /// Queue a reusable interaction effect for the outermost successful commit.
    pub(in crate::game) fn defer_effect(&mut self, effect: DeferredEffect) {
        self.after_commit(After::Effect(effect));
    }
    /// Collect committed implicit `setChanged` notices while the callback runs without opening
    /// a transaction. Direct calls to `BlockEntityRef::set_changed` remain immediate.
    pub(in crate::game) fn collect_quick_move_notices<R>(
        &mut self,
        dispatch: impl FnOnce(&mut Self) -> R,
    ) -> (R, Vec<QuickMoveNotice>) {
        let outer = self.transactions.quick_move_notices.replace(Vec::new());
        let scope = QuickMoveNotices { game: self, outer };
        let result = dispatch(&mut *scope.game);
        let notices = scope
            .game
            .transactions
            .quick_move_notices
            .take()
            .expect("active quick-move notice scope");
        (result, notices)
    }

    /// Journal a container notice already bound to the row that received the write.
    pub(in crate::game) fn after_holder_changed(&mut self, holder: BlockEntityIdentity) {
        let [x, y, z] = holder.position;
        self.after_commit(self.side_tx_container_change_effect((x, y, z), Some(holder)));
    }

    /// Runs `after` at once, or at the root commit inside a transaction.
    pub(in crate::game) fn after_commit(&mut self, after: After) {
        let after = match after {
            After::Changed(pos) => self.side_tx_container_change_effect(pos, None),
            after => after,
        };
        let frame = if after.level_owned() {
            entities::open_in(&mut self.transactions, self.dim)
        } else {
            self.transactions.last_mut()
        };
        match frame {
            Some(level) => level.after.push(after),
            None => self.side_tx_run_after(after),
        }
    }
    pub(in crate::game) fn pending_menu_data(&self, key: (u32, u64, usize)) -> Option<i32> {
        self.transactions
            .iter()
            .rev()
            .find_map(|level| level.menu_data.get(&key).copied())
    }

    pub(in crate::game) fn journal_menu_data(
        &mut self,
        key: (u32, u64, usize),
        value: i32,
    ) -> bool {
        let Some(level) = self.transactions.last_mut() else {
            return false;
        };
        level.menu_data.insert(key, value);
        true
    }
}

include!("transactions/item_entity.rs");

#[cfg(test)]
#[path = "transactions/player_tests.rs"]
pub(in crate::game) mod player_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::super::api_values::TryIntoApi;
    use super::super::{BlockEntity, Options, Progress, Settings, load_resources};
    use mod_api::HostTransaction;
    use mod_api::ServerHost;
    use mod_api::{
        Authority, BlockSide, BlockUpdateFlags, FLUIDS, FluidStack, ITEMS, ItemResource,
        TransactionContext, TransactionError,
    };
    use std::{sync::Arc, time::Instant};
    use sunrise_client::{chunk::Chunk, items::ItemStack};

    #[test]
    #[ignore = "daily"]
    fn packet_refusal_rolls_back_the_child_and_reaches_the_transaction_caller() {
        use crate::game::native_entity_refs::tests::{run_fresh_cases, with_game};
        use mod_api::prelude::BlockStatesHost;
        use std::sync::{atomic::AtomicUsize, mpsc};
        use sunrise_client::lan::{PlayerSender, conn::transaction_packets::Scope};

        if run_fresh_cases(
            "game::transactions::tests::packet_refusal_rolls_back_the_child_and_reaches_the_transaction_caller",
            "SUNRISE_TRANSACTION_PACKET_REFUSAL_CHILD",
            &["run"],
        ) {
            return;
        }
        with_game(|game| {
            const AT: [i32; 3] = [1, 64, 1];
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let level = game.level("minecraft:overworld").unwrap();
            let before = level.block_state(game, AT);
            let stone = BlockStatesHost::block_state_id(game, "minecraft:stone").unwrap();
            let (send, receive) = mpsc::channel();
            // A real bounded LAN sender with no room left to reserve another packet.
            let sender = PlayerSender::new(send, Arc::new(AtomicUsize::new(usize::MAX)));
            let _packets = Scope::begin();
            let result = game.transaction(|host, _| {
                let child = host.transaction(|host, _| {
                    assert_eq!(
                        level.set_block(host, AT, stone, BlockUpdateFlags::NONE),
                        Ok(true)
                    );
                    assert_eq!(level.block_state(host, AT), Some(stone));
                    sender.send(vec![1]);
                    Ok::<_, ()>(())
                });
                assert_eq!(child, Err(TransactionError::CommitRefused));
                assert_eq!(level.block_state(host, AT), before);
                Ok::<_, ()>(())
            });
            assert_eq!(result, Err(TransactionError::CommitRefused));
            assert_eq!(level.block_state(game, AT), before);
            assert!(receive.try_recv().is_err());
        });
    }

    #[test]
    #[ignore = "daily"]
    fn cauldron_stat_listener_observes_committed_state_and_can_cancel() {
        use super::super::native_entity_refs::tests::run_fresh_cases;
        use crate::game::api_values::TryIntoApi;
        use mod_api::{
            Flow, Hooks, ItemSpawnEvent, ItemTossEvent, ModEntry, Mods, PlayerStatAwardEvent,
        };
        use std::sync::Mutex;
        if run_fresh_cases(
            "game::transactions::tests::cauldron_stat_listener_observes_committed_state_and_can_cancel",
            "SUNRISE_CAULDRON_STAT_CHILD",
            &["run"],
        ) {
            return;
        }
        struct Listener {
            seen: Arc<Mutex<Vec<(Option<mod_api::BlockState>, Option<String>)>>>,
            drops: Arc<Mutex<Vec<(&'static str, Option<mod_api::BlockState>)>>>,
        }
        #[mod_api::__subscriptions]
        impl Hooks for Listener {
            fn player_stat_award(
                &self,
                host: &mut dyn ServerHost,
                event: &mut PlayerStatAwardEvent<'_>,
            ) -> Flow {
                if event.stat.as_str() != "minecraft:fill_cauldron" {
                    return Flow::Continue;
                }
                let level = host.level("minecraft:overworld").unwrap();
                let hand = mod_api::ItemPlace::PlayerHand {
                    player: event.player,
                    hand: mod_api::client::first_person::Hand::Main,
                };
                self.seen.lock().unwrap().push((
                    level.block_state(host, [2, 64, 2]),
                    hand.with_stack(host, |s| {
                        mod_api::__private::item_key_id(s.item())
                            .unwrap()
                            .to_string()
                    }),
                ));
                Flow::Cancel
            }
            fn item_toss(&self, host: &mut dyn ServerHost, event: &mut ItemTossEvent) -> Flow {
                let potion = event.item.with_stack(host, |stack| {
                    mod_api::__private::item_key_id(stack.item()) == Some("minecraft:potion")
                });
                if potion == Some(true) {
                    let level = host.level("minecraft:overworld").unwrap();
                    self.drops
                        .lock()
                        .unwrap()
                        .push(("toss", level.block_state(host, [2, 64, 2])));
                }
                Flow::Continue
            }
            fn item_spawn(&self, host: &mut dyn ServerHost, event: &mut ItemSpawnEvent<'_>) {
                if event.item == "minecraft:potion" {
                    let level = host.level("minecraft:overworld").unwrap();
                    self.drops
                        .lock()
                        .unwrap()
                        .push(("spawn", level.block_state(host, [2, 64, 2])));
                }
            }
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        let drops = Arc::new(Mutex::new(Vec::new()));
        let mods: Mods = vec![ModEntry::new(
            "observer",
            Box::new(Listener {
                seen: seen.clone(),
                drops: drops.clone(),
            }),
        )]
        .into();
        mod_platform::__private::install(mods);
        super::super::native_guest_turn_tests::with_guests(|game| {
            let pos = [2, 64, 2];
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let empty = game
                .pack
                .assets
                .registry
                .default_state("minecraft:cauldron")
                .unwrap();
            let full = game
                .pack
                .assets
                .registry
                .resolve(&sunrise_world::chunk::BlockState {
                    name: "minecraft:water_cauldron".into(),
                    properties: [("level".into(), "3".into())].into_iter().collect(),
                })
                .unwrap();
            game.world.set(pos[0], pos[1], pos[2], empty).unwrap();
            game.as_guest(12, |game| {
                game.selected = 0;
                game.creative = false;
                game.inventory[0] = Some(ItemStack::new("minecraft:water_bucket", 1));
                game.open_storage_transaction();
                assert_eq!(
                    game.use_on_cauldron(pos).unwrap(),
                    sunrise_client::cauldron::Outcome::Success
                );
                assert!(seen.lock().unwrap().is_empty());
                game.close_storage_transaction(false);
                assert!(seen.lock().unwrap().is_empty());
                assert_eq!(
                    game.use_on_cauldron(pos).unwrap(),
                    sunrise_client::cauldron::Outcome::Success
                );
                assert_eq!(
                    seen.lock().unwrap().as_slice(),
                    &[(
                        Some(empty.try_into_api().unwrap()),
                        Some("minecraft:bucket".into())
                    )]
                );
                game.world.set(pos[0], pos[1], pos[2], empty).unwrap();
                game.inventory[0] = Some(ItemStack::new("minecraft:water_bucket", 1));
                game.open_storage_transaction();
                assert_eq!(
                    game.use_on_cauldron(pos).unwrap(),
                    sunrise_client::cauldron::Outcome::Success
                );
                assert_eq!(seen.lock().unwrap().len(), 1);
                game.close_storage_transaction(true);
            })
            .unwrap();
            assert_eq!(
                *seen.lock().unwrap(),
                vec![
                    (
                        Some(empty.try_into_api().unwrap()),
                        Some("minecraft:bucket".into())
                    ),
                    (
                        Some(full.try_into_api().unwrap()),
                        Some("minecraft:bucket".into())
                    )
                ]
            );
            let level2 = game
                .pack
                .assets
                .registry
                .resolve(&sunrise_world::chunk::BlockState {
                    name: "minecraft:water_cauldron".into(),
                    properties: [("level".into(), "2".into())].into_iter().collect(),
                })
                .unwrap();
            game.as_guest(12, |game| {
                let setup = |game: &mut crate::game::ServerGame<'_>| {
                    game.world.set(pos[0], pos[1], pos[2], full).unwrap();
                    game.inventory
                        .iter_mut()
                        .for_each(|s| *s = Some(ItemStack::new("minecraft:stone", 64)));
                    game.offhand = Some(ItemStack::new("minecraft:stone", 64));
                    game.inventory[0] = Some(ItemStack::new("minecraft:glass_bottle", 2));
                };
                setup(game);
                assert_eq!(
                    game.use_on_cauldron(pos).unwrap(),
                    sunrise_client::cauldron::Outcome::Success
                );
                assert_eq!(
                    drops.lock().unwrap().as_slice(),
                    &[
                        ("toss", Some(full.try_into_api().unwrap())),
                        ("spawn", Some(full.try_into_api().unwrap()))
                    ]
                );
                drops.lock().unwrap().clear();
                setup(game);
                game.open_storage_transaction();
                assert_eq!(
                    game.use_on_cauldron(pos).unwrap(),
                    sunrise_client::cauldron::Outcome::Success
                );
                assert!(drops.lock().unwrap().is_empty());
                game.close_storage_transaction(false);
                assert!(drops.lock().unwrap().is_empty());
                game.open_storage_transaction();
                assert_eq!(
                    game.use_on_cauldron(pos).unwrap(),
                    sunrise_client::cauldron::Outcome::Success
                );
                game.close_storage_transaction(true);
                assert_eq!(
                    drops.lock().unwrap().as_slice(),
                    &[
                        ("toss", Some(level2.try_into_api().unwrap())),
                        ("spawn", Some(level2.try_into_api().unwrap()))
                    ]
                );
            })
            .unwrap();
            assert_eq!(
                game.lan.as_ref().unwrap().play[&12]
                    .stats
                    .get("custom", "minecraft:fill_cauldron"),
                0
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_menu_write_outside_a_transaction_keeps_no_journal() {
        super::super::native_guest_turn_tests::with_guests(|game| {
            let player = game.player(12).unwrap();
            let menu = game.player_container_menu(player).unwrap();
            let stone = ItemStack::new("minecraft:stone", 4);
            game.journal_menus(player);
            *game.native_menu_storage(menu, 1).unwrap() = Some(stone.clone());
            assert!(game.transactions.is_empty());
            assert_eq!(
                menu.slot(game, 1),
                Some(Some(stone.try_into_api().unwrap()))
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn menu_journals_keep_only_the_first_write_inside_a_transaction() {
        super::super::native_guest_turn_tests::with_guests(|game| {
            let player = game.player(12).unwrap();
            let menu = game.player_container_menu(player).unwrap();
            let stone = ItemStack::new("minecraft:stone", 4);
            *game.native_menu_storage(menu, 1).unwrap() = Some(stone.clone());
            let place = mod_api::ItemPlace::MenuSlot { menu, slot: 1 };
            let resource = ItemResource::of(&stone.try_into_api().unwrap());
            let aborted: Result<(), ()> = game.storage_transaction(|game, tx| {
                assert_eq!(place.insert(game, tx, &resource, 2), Ok(2));
                assert_eq!(place.insert(game, tx, &resource, 3), Ok(3));
                let journal = &game.transactions.last().unwrap().menus;
                assert_eq!(journal.len(), 1);
                assert_eq!(journal[&12].craft_grid[0], Some(stone.clone()));
                assert_eq!(menu.slot(game, 1).unwrap().unwrap().count(), 9);
                Err(())
            });
            assert_eq!(aborted, Err(()));
            assert!(game.transactions.is_empty());
            assert_eq!(
                menu.slot(game, 1),
                Some(Some(stone.try_into_api().unwrap()))
            );
        });
    }

    pub(in crate::game) fn insert(
        host: &mut dyn ServerHost,
        at: BlockSide,
        slot: usize,
        stack: ItemStack,
        tx: &TransactionContext,
    ) -> Option<ItemStack> {
        let handler = at
            .level
            .capability(host, &ITEMS, at.pos, at.side)
            .expect("item capability");
        let inserted = handler
            .insert_at(
                host,
                tx,
                slot,
                &ItemResource::of(&stack.try_into_api().unwrap()),
                stack.count,
            )
            .unwrap();
        (inserted < stack.count).then(|| ItemStack {
            count: stack.count - inserted,
            ..stack
        })
    }

    pub(in crate::game) fn extract(
        host: &mut dyn ServerHost,
        at: BlockSide,
        slot: usize,
        amount: u32,
        tx: &TransactionContext,
    ) -> Option<mod_api::ItemStack> {
        let handler = at
            .level
            .capability(host, &ITEMS, at.pos, at.side)
            .expect("item capability");
        let resource = handler.resource(host, slot)?;
        let taken = handler
            .extract_at(host, tx, slot, &resource, amount)
            .unwrap();
        resource.to_stack(taken)
    }

    pub(in crate::game) fn extract_fluid(
        host: &mut dyn ServerHost,
        at: BlockSide,
        tank: usize,
        amount: u32,
        tx: &TransactionContext,
    ) -> Option<FluidStack> {
        let handler = at
            .level
            .capability(host, &FLUIDS, at.pos, at.side)
            .expect("fluid capability");
        let resource = handler.resource(host, tank)?;
        let taken = handler
            .extract_at(host, tx, tank, &resource, amount)
            .unwrap();
        (taken > 0).then(|| resource.to_stack(taken))
    }

    pub(in crate::game) fn chest_items(game: &crate::game::ServerGame<'_>) -> Vec<(usize, u32)> {
        let Some(BlockEntity::Chest(items)) = game.world.block_entities.get(&(0, 64, 0)) else {
            panic!("chest block entity missing")
        };
        let held = items.iter().enumerate();
        held.filter_map(|(i, s)| Some((i, s.as_ref()?.count)))
            .collect()
    }

    #[test]
    #[ignore = "daily"]
    fn container_limits_preserve_capacity_and_transaction_rollback() {
        super::super::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let level = game.level("minecraft:overworld").unwrap();
            for (pos, id) in [
                ([0, 64, 0], "minecraft:chest"),
                ([4, 64, 0], "minecraft:jukebox"),
            ] {
                let state = game.pack.assets.registry.default_state(id).unwrap();
                game.level_set_block(level, pos, state, BlockUpdateFlags::NEIGHBORS)
                    .unwrap();
            }
            let mut large = ItemStack::new("minecraft:stick", 1);
            large.data_mut().set_other(
                "minecraft:max_stack_size",
                Some(sunrise_core::nbt::Tag::Int(99)),
            );
            let large = ItemResource::of(&large.try_into_api().unwrap());
            let handler = level.capability(game, &ITEMS, [0, 64, 0], None).unwrap();
            assert_eq!(handler.capacity(game, 0, Some(&large)), 99);
            assert_eq!(game.world.container_stack_limit((0, 64, 0)), None);
            let stick =
                ItemResource::of(&ItemStack::new("minecraft:stick", 1).try_into_api().unwrap());
            let host: &mut dyn ServerHost = game;
            let handler = level
                .capability(host, &mod_api::ITEMS, [0, 64, 0], None)
                .unwrap();
            let aborted = host.transaction(|host, tx| {
                assert_eq!(handler.set_max_stack_size(host, tx, 10), Ok(true));
                assert_eq!(handler.insert_at(host, tx, 0, &stick, 64), Ok(10));
                host.transaction(|host, tx| {
                    assert_eq!(handler.set_max_stack_size(host, tx, 4), Ok(true));
                    Ok::<_, ()>(())
                })
                .unwrap();
                assert_eq!(handler.capacity(host, 0, Some(&stick)), 4);
                Err::<(), ()>(())
            });
            assert_eq!(aborted, Err(TransactionError::Aborted(())));
            assert_eq!(handler.capacity(host, 0, Some(&stick)), 64);
            assert_eq!(handler.capacity(host, 0, Some(&large)), 99);
            assert_eq!(handler.amount(host, 0), 0);
            host.transaction(|host, tx| {
                assert_eq!(handler.set_max_stack_size(host, tx, 10), Ok(true));
                let _ = host.transaction(|host, tx| {
                    assert_eq!(handler.set_max_stack_size(host, tx, 4), Ok(true));
                    Err::<(), ()>(())
                });
                assert_eq!(handler.capacity(host, 0, Some(&stick)), 10);
                for invalid in [0, 100, u32::MAX] {
                    assert_eq!(handler.set_max_stack_size(host, tx, invalid), Ok(false));
                    assert_eq!(handler.capacity(host, 0, Some(&stick)), 10);
                }
                assert_eq!(handler.set_max_stack_size(host, tx, 64), Ok(true));
                assert_eq!(handler.insert_at(host, tx, 0, &stick, 20), Ok(20));
                assert_eq!(handler.set_max_stack_size(host, tx, 10), Ok(true));
                assert_eq!(handler.amount(host, 0), 20);
                assert_eq!(handler.insert_at(host, tx, 0, &stick, 1), Ok(0));
                assert_eq!(handler.extract_at(host, tx, 0, &stick, 15), Ok(15));
                assert_eq!(handler.amount(host, 0), 5);
                assert_eq!(handler.insert_at(host, tx, 0, &stick, 64), Ok(5));
                Err::<(), ()>(())
            })
            .unwrap_err();

            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: Result<(), TransactionError<()>> = host.transaction(|host, tx| {
                    assert_eq!(handler.set_max_stack_size(host, tx, 10), Ok(true));
                    assert_eq!(handler.insert_at(host, tx, 0, &stick, 64), Ok(10));
                    panic!("abort container writes");
                });
            }));
            assert!(panic.is_err());
            assert_eq!(handler.capacity(host, 0, Some(&stick)), 64);
            assert_eq!(handler.capacity(host, 0, Some(&large)), 99);
            assert_eq!(handler.amount(host, 0), 0);
            host.transaction(|host, tx| {
                let pearl = ItemResource::of(
                    &ItemStack::new("minecraft:ender_pearl", 1)
                        .try_into_api()
                        .unwrap(),
                );
                assert_eq!(handler.set_max_stack_size(host, tx, 32), Ok(true));
                assert_eq!(handler.insert_at(host, tx, 0, &pearl, 64), Ok(16));
                let disc = level.capability(host, &ITEMS, [4, 64, 0], None).unwrap();
                assert_eq!(disc.set_max_stack_size(host, tx, 10), Ok(true));
                let record = ItemResource::of(
                    &ItemStack::new("minecraft:music_disc_13", 1)
                        .try_into_api()
                        .unwrap(),
                );
                assert_eq!(disc.insert_at(host, tx, 0, &record, 64), Ok(1));
                Err::<(), ()>(())
            })
            .unwrap_err();
            // Player clicks and shift-clicks use the menu's separate slot rules.
            let env = game.menu_env();
            let mut menu = sunrise_client::menu::Menu::new(sunrise_client::menu::Kind::Chest(1));
            menu.set_container_limits(vec![Some(10); 9]);
            menu.carried = Some(ItemStack::new("minecraft:stick", 20));
            menu.click(&env, 0, 0, sunrise_client::menu::Input::Pickup)
                .unwrap();
            assert_eq!(menu.slot(0).unwrap().count, 10);
            assert_eq!(menu.carried.as_ref().unwrap().count, 10);
            menu.carried = None;
            menu.set_slot(&env, 9, Some(ItemStack::new("minecraft:stick", 20)));
            menu.click(&env, 9, 0, sunrise_client::menu::Input::QuickMove)
                .unwrap();
            assert_eq!(menu.slot(1).unwrap().count, 10);
            assert_eq!(menu.slot(2).unwrap().count, 10);
            let hopper = sunrise_world::chunk::BlockState {
                name: "minecraft:hopper".into(),
                properties: [("facing".into(), "east".into())].into_iter().collect(),
            };
            let hopper = game.pack.assets.registry.resolve(&hopper).unwrap();
            let chest = game
                .pack
                .assets
                .registry
                .default_state("minecraft:chest")
                .unwrap();
            let flags = BlockUpdateFlags::NEIGHBORS;
            game.level_set_block(level, [0, 64, 4], hopper, flags)
                .unwrap();
            if let Some(BlockEntity::Chest(items)) = game.world.block_entities.get_mut(&(0, 64, 4))
            {
                items[0] = Some(ItemStack::new("minecraft:stick", 2));
            }
            let sticks = |count| ItemStack::new("minecraft:stick", count);
            let slots =
                |game: &crate::game::ServerGame<'_>, pos| match game.world.block_entities.get(&pos)
                {
                    Some(BlockEntity::Chest(items)) => {
                        items.iter().flatten().map(|s| s.count).sum::<u32>()
                    }
                    _ => panic!("missing chest"),
                };
            assert_eq!(
                game.level_set_block(level, [1, 64, 4], chest, flags),
                Ok(true)
            );
            if let Some(BlockEntity::Chest(items)) = game.world.block_entities.get_mut(&(1, 64, 4))
            {
                items.fill(Some(sticks(10)));
                items[0] = Some(sticks(9));
            }
            let target = level.capability(game, &ITEMS, [1, 64, 4], None).unwrap();
            game.storage_transaction(|host, tx| {
                assert_eq!(target.set_max_stack_size(host, tx, 10), Ok(true));
                Ok::<_, ()>(())
            })
            .unwrap();
            game.test_clear_hopper((0, 64, 4));
            game.hopper_try_move_items((0, 64, 4));
            assert_eq!(slots(game, (1, 64, 4)), 270);
            let _ = game.storage_transaction(|host, tx| {
                assert_eq!(target.set_max_stack_size(host, tx, 64), Ok(true));
                Err::<(), ()>(())
            });
            game.test_clear_hopper((0, 64, 4));
            game.hopper_try_move_items((0, 64, 4));
            assert_eq!(slots(game, (1, 64, 4)), 270);
            assert_eq!(slots(game, (0, 64, 4)), 1);
            // Dropper uses add_item, dispenser ejects into the world instead.
            let dropper = sunrise_world::chunk::BlockState {
                name: "minecraft:dropper".into(),
                properties: [("facing".into(), "east".into())].into_iter().collect(),
            };
            let dropper = game.pack.assets.registry.resolve(&dropper).unwrap();
            game.level_set_block(level, [0, 64, 4], dropper, flags)
                .unwrap();
            if let Some(BlockEntity::Chest(items)) = game.world.block_entities.get_mut(&(0, 64, 4))
            {
                items[0] = Some(sticks(2));
            }
            game.dispense_from(sunrise_core::position::BlockPos::new(0, 64, 4), dropper)
                .unwrap();
            assert_eq!(slots(game, (1, 64, 4)), 270);
            assert_eq!(slots(game, (0, 64, 4)), 2);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn an_aborted_transaction_puts_back_what_it_and_its_committed_children_changed() {
        let Some(root) = super::super::native_entity_refs::tests::declared_part_root(
            "game::transactions::tests::an_aborted_transaction_puts_back_what_it_and_its_committed_children_changed",
        ) else {
            return;
        };
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            Instant::now(),
            false,
            super::super::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = super::super::execution_role::ExecutionRole::IntegratedServer;
        options.dedicated = Some(Vec::new());
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.native_lifecycle
            .begin_open(Authority::IntegratedServer);
        super::super::native_entity_refs::tests::resolve_parts(&game).unwrap();
        // Pins the opaque backend: a real game always hands one out, so a None
        // from Host::backend is a bug, never a fallback.
        assert!(mod_api::Host::backend(&game).is_some());
        assert!(ServerHost::backend_mut(&mut game).is_some());
        let level = game.level("minecraft:overworld").unwrap();
        let registry = &game.pack.assets.registry;
        let chest = registry.default_state("minecraft:chest").unwrap();
        let full = sunrise_world::chunk::BlockState {
            name: "minecraft:water_cauldron".to_owned(),
            properties: [("level".to_owned(), "3".to_owned())].into_iter().collect(),
        };
        let full = registry.resolve(&full).unwrap();
        let empty = registry.default_state("minecraft:cauldron").unwrap();
        let jukebox = registry.default_state("minecraft:jukebox").unwrap();
        let flags = BlockUpdateFlags::NEIGHBORS;
        assert_eq!(
            game.level_set_block(level, [0, 64, 0], chest, flags),
            Ok(true)
        );
        assert_eq!(
            game.level_set_block(level, [2, 64, 0], full, flags),
            Ok(true)
        );
        assert_eq!(
            game.level_set_block(level, [4, 64, 0], jukebox, flags),
            Ok(true)
        );
        let sticks = |count| ItemStack::new("minecraft:stick", count);
        let stick = ItemResource::of(&sticks(1).try_into_api().unwrap());
        let rod = ItemResource::of(
            &ItemStack::new("minecraft:blaze_rod", 1)
                .try_into_api()
                .unwrap(),
        );
        let on = |pos, side| BlockSide { level, pos, side };
        let host: &mut dyn ServerHost = &mut game;

        // Five in, then a committed child takes two and an aborted one adds three: the
        // outer abort puts all of it back, the cauldron's water too.
        let refused: Result<(), TransactionError<()>> = host.transaction(|host, tx| {
            assert!(insert(host, on([0, 64, 0], None), 0, sticks(5), tx).is_none());
            let taken = host
                .transaction(|host, tx| Ok::<_, ()>(extract(host, on([0, 64, 0], None), 0, 2, tx)));
            assert_eq!(taken, Ok(Some(sticks(2).try_into_api().unwrap())));
            let _ = host.transaction(|host, tx| {
                insert(host, on([0, 64, 0], None), 1, sticks(3), tx);
                Err::<(), ()>(())
            });
            let water = extract_fluid(host, on([2, 64, 0], None), 0, 1000, tx);
            assert_eq!(water.map(|w| w.amount), Some(1000));
            Err(())
        });
        assert_eq!(refused, Err(TransactionError::Aborted(())));
        assert_eq!(chest_items(&game), []);
        assert_eq!(game.world.block(2, 64, 0), full);
        assert!(game.transactions.is_empty());

        // The same, committed: what the children left stands, the cauldron is set for good.
        let host: &mut dyn ServerHost = &mut game;
        let done: Result<(), TransactionError<()>> = host.transaction(|host, tx| {
            insert(host, on([0, 64, 0], None), 0, sticks(5), tx);
            let _ = host.transaction(|host, tx| {
                extract(host, on([0, 64, 0], None), 0, 2, tx);
                Ok::<_, ()>(())
            });
            let _ = host.transaction(|host, tx| {
                insert(host, on([0, 64, 0], None), 1, sticks(3), tx);
                Err::<(), ()>(())
            });
            extract_fluid(host, on([2, 64, 0], None), 0, 1000, tx);
            Ok(())
        });
        assert_eq!(done, Ok(()));
        assert_eq!(chest_items(&game), [(0, 3)]);
        assert_eq!(game.world.block(2, 64, 0), empty);

        // A transaction a mod opened through the helpers and never closed is aborted before
        // the next tick.
        game.open_storage_transaction();
        let host: &mut dyn ServerHost = &mut game;
        let _ = host.transaction(|host, tx| {
            Ok::<_, ()>(insert(host, on([0, 64, 0], None), 0, sticks(1), tx))
        });
        game.abort_open_transactions();
        assert_eq!(chest_items(&game), [(0, 3)]);

        // A panic caught outside the root: what it changed is gone before the catch returns,
        // and the transaction is closed.
        let host: &mut dyn ServerHost = &mut game;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            host.transaction(|host, tx| -> Result<(), ()> {
                insert(host, on([0, 64, 0], None), 0, sticks(1), tx);
                panic!("a broken handler")
            })
        }));
        assert!(caught.is_err());
        assert_eq!(chest_items(&game), [(0, 3)]);
        assert!(game.transactions.is_empty());

        // `transaction_matches` checks the exact innermost open frame only, and passing an
        // outer/ancestor `TransactionContext` to a write inside a nested transaction is refused
        // cleanly without modifying state.
        let host: &mut dyn ServerHost = &mut game;
        assert!(host.transaction_matches(None));
        let ancestor_check: Result<(), TransactionError<()>> = host.transaction(|host, outer| {
            assert!(!host.transaction_matches(None));
            assert!(host.transaction_matches(Some(outer)));
            let inner_ok: Result<(), TransactionError<()>> = host.transaction(|host, inner| {
                assert!(!host.transaction_matches(None));
                assert!(!host.transaction_matches(Some(outer)));
                assert!(host.transaction_matches(Some(inner)));
                let handler = level
                    .capability(host, &ITEMS, [0, 64, 0], None)
                    .expect("item capability");
                assert_eq!(
                    handler.insert_at(
                        host,
                        outer,
                        0,
                        &ItemResource::of(&sticks(1).try_into_api().unwrap()),
                        1,
                    ),
                    Err(TransactionError::InvalidContext)
                );
                Ok(())
            });
            assert_eq!(inner_ok, Ok(()));
            assert!(host.transaction_matches(Some(outer)));
            Ok(())
        });
        assert_eq!(ancestor_check, Ok(()));
        assert!(game.transaction_matches(None));
        assert_eq!(chest_items(&game), [(0, 3)]);
        assert!(game.transactions.is_empty());

        // A panic inside a nested `storage_transaction` rolls back only the inner frame;
        // the outer frame stays open and keeps its own writes until it closes.
        let outer_aborted: Result<(), ()> = game.storage_transaction(|game, outer| {
            assert!(insert(game, on([0, 64, 0], None), 0, sticks(1), outer).is_none());
            assert_eq!(chest_items(game), [(0, 4)]);
            let inner_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: Result<(), ()> = game.storage_transaction(|game, inner| {
                    assert!(insert(game, on([0, 64, 0], None), 0, sticks(2), inner).is_none());
                    assert_eq!(chest_items(game), [(0, 6)]);
                    panic!("nested storage panic");
                });
            }));
            assert!(inner_panic.is_err());
            assert!(game.transaction_matches(Some(outer)));
            assert_eq!(chest_items(game), [(0, 4)]);
            Err(())
        });
        assert_eq!(outer_aborted, Err(()));
        assert_eq!(chest_items(&game), [(0, 3)]);
        assert!(game.transactions.is_empty());

        // An attachment map a write made inside an aborted transaction is gone with it.
        let chest_holder = mod_api::Holder::BlockEntity([0, 64, 0]);
        // So is the unsaved mark it put on a clean chunk, through a committed child too.
        assert!(game.world.block_entities.attachments((0, 64, 0)).is_none());
        game.world.modified.clear();
        game.open_storage_transaction();
        game.open_storage_transaction();
        assert!(
            game.attachment_map_quiet(chest_holder, mod_api::AttachmentAccess::Existing)
                .is_ok()
        );
        assert!(game.world.modified.contains(&(0, 0)));
        game.close_storage_transaction(true);
        assert!(game.world.block_entities.attachments((0, 64, 0)).is_some());
        game.close_storage_transaction(false);
        assert!(game.world.block_entities.attachments((0, 64, 0)).is_none());
        assert!(!game.world.modified.contains(&(0, 0)), "clean again");
        // A committed one keeps it.
        game.open_storage_transaction();
        assert!(
            game.attachment_map_quiet(chest_holder, mod_api::AttachmentAccess::Existing)
                .is_ok()
        );
        game.close_storage_transaction(true);
        assert!(game.world.modified.contains(&(0, 0)));
        game.world.block_entities.set_attachments((0, 64, 0), None);

        // An aborted cauldron change leaves no write behind in the world's queues.
        assert_eq!(
            game.level_set_block(level, [2, 64, 0], full, flags),
            Ok(true)
        );
        game.world.block_log = Some(Vec::new());
        game.world.path_writes = Some(Vec::new());
        let queues = |game: &crate::game::ServerGame<'_>| {
            let world = &game.world;
            (
                world.block_log.as_ref().map(Vec::len),
                world.path_writes.as_ref().map(Vec::len),
                world.light_edits.len(),
                world.dirty_light.len(),
                world.watched_writes.len(),
            )
        };
        let before = queues(&game);
        let host: &mut dyn ServerHost = &mut game;
        let _ = host.transaction(|host, tx| {
            extract_fluid(host, on([2, 64, 0], None), 0, 1000, tx);
            Err::<(), ()>(())
        });
        assert_eq!(game.world.block(2, 64, 0), full);
        assert_eq!(queues(&game), before);

        // A disc swapped for another in one transaction changes the song: the old one's
        // stops and the new one's starts.
        let disc = |item: &str| ItemStack::new(item, 1);
        let host: &mut dyn ServerHost = &mut game;
        let _ = host.transaction(|host, tx| {
            let cat = disc("minecraft:music_disc_cat");
            Ok::<_, ()>(insert(host, on([4, 64, 0], None), 0, cat, tx))
        });
        let Some(BlockEntity::Jukebox(j)) = game.world.block_entities.get_mut(&(4, 64, 0)) else {
            panic!("jukebox block entity missing")
        };
        assert!(j.playing, "a disc put in through a transfer plays");
        j.ticks = 500;
        let host: &mut dyn ServerHost = &mut game;
        let swapped = host.transaction(|host, tx| {
            extract(host, on([4, 64, 0], None), 0, 1, tx);
            Ok::<_, ()>(insert(
                host,
                on([4, 64, 0], None),
                0,
                disc("minecraft:music_disc_13"),
                tx,
            ))
        });
        assert_eq!(swapped, Ok(None));
        let Some(BlockEntity::Jukebox(j)) = game.world.block_entities.get(&(4, 64, 0)) else {
            panic!("jukebox block entity missing")
        };
        assert_eq!(
            j.record().map(|r| &*r.item),
            Some("minecraft:music_disc_13")
        );
        assert!(j.playing);
        assert_eq!(j.ticks, 0, "the new disc's song starts from its beginning");

        // A player's place: one of three buckets in the hand is exchanged for a filled one; the
        // hand keeps two and the filled one goes to the inventory, and what doesn't fit would be
        // thrown. Aborted, the hand and the inventory are as they were, and nothing was thrown.
        game.player = Some(
            super::super::PlayerMotion::new(
                game.motion,
                sunrise_core::geometry::Vec3::new(0.5, 65.0, 3.5),
            )
            .unwrap(),
        );
        let bucket = |item: &str, count| ItemStack::new(item, count);
        game.inventory
            .iter_mut()
            .for_each(|slot| *slot = Some(bucket("minecraft:stone", 64)));
        game.selected = 0;
        game.inventory[0] = Some(bucket("minecraft:bucket", 3));
        game.carried = None;
        let before = (
            game.inventory.clone(),
            game.offhand.clone(),
            game.carried.clone(),
        );
        let dropped = game.dropped.len();
        let me = game.native_acting_id();
        let hand = mod_api::ItemPlace::PlayerHand {
            player: me,
            hand: mod_api::client::first_person::Hand::Main,
        };
        let water = ItemResource::of(&bucket("minecraft:water_bucket", 1).try_into_api().unwrap());
        let host: &mut dyn ServerHost = &mut game;
        let _ = host.transaction(|host, tx| {
            assert_eq!(hand.exchange(host, tx, &water, 1), Ok(1));
            Err::<(), ()>(())
        });
        assert_eq!(
            (
                game.inventory.clone(),
                game.offhand.clone(),
                game.carried.clone()
            ),
            before
        );
        assert_eq!(
            game.dropped.len(),
            dropped,
            "a thrown bucket waits for the root commit"
        );

        // Committed, the split stands and the full inventory throws the filled bucket.
        let host: &mut dyn ServerHost = &mut game;
        let _ = host.transaction(|host, tx| hand.exchange(host, tx, &water, 1));
        assert_eq!(game.inventory[0], Some(bucket("minecraft:bucket", 2)));
        assert_eq!(game.dropped.len(), dropped + 1);
        let thrown = &game.dropped[game.dropped.len() - 1].stack;
        assert_eq!(&*thrown.item, "minecraft:water_bucket");

        // A chest minecart's, a chest boat's and a chested donkey's slots: an abort puts back
        // what an insert changed; a commit keeps an exchange, whose new item goes into the next
        // free slot.
        let mut chested = sunrise_core::nbt::Compound::new();
        chested.insert("ChestedHorse".into(), sunrise_core::nbt::Tag::Byte(1));
        let vehicles = [
            (
                "minecraft:chest_minecart",
                sunrise_core::nbt::Compound::new(),
            ),
            (
                "minecraft:oak_chest_boat",
                sunrise_core::nbt::Compound::new(),
            ),
            ("minecraft:donkey", chested),
        ];
        // An aborted write leaves a chested donkey's item list as long as it was.
        let id = game
            .summon(
                "minecraft:overworld",
                "minecraft:donkey",
                [8.5, 70.0, 8.5],
                {
                    let mut chested = sunrise_core::nbt::Compound::new();
                    chested.insert("ChestedHorse".into(), sunrise_core::nbt::Tag::Byte(1));
                    chested
                },
            )
            .unwrap();
        let len = |game: &crate::game::ServerGame<'_>| {
            game.mobs
                .iter()
                .find(|m| m.id == id)
                .unwrap()
                .extra
                .items
                .len()
        };
        // Its list is short (none kept yet): reading it as slots grows it, and the abort puts
        // that back too.
        let mob = game.mobs.iter_mut().find(|m| m.id == id).unwrap();
        mob.extra.items.clear();
        let host: &mut dyn ServerHost = &mut game;
        let _ = host.transaction(|host, tx| {
            let place = mod_api::ItemPlace::EntitySlot {
                entity: id,
                slot: 0,
            };
            assert!(place.with_stack(host, |_| ()).is_none());
            assert_eq!(place.insert(host, tx, &stick, 1), Ok(1));
            Err::<(), ()>(())
        });
        assert_eq!(len(&game), 0);
        // Grown three transactions deep, the middle one committing: the root's abort still
        // truncates it.
        let place = mod_api::ItemPlace::EntitySlot {
            entity: id,
            slot: 0,
        };
        let host: &mut dyn ServerHost = &mut game;
        let result = host.transaction(|host, _| {
            host.transaction(|host, _| {
                host.transaction(|host, tx| {
                    assert!(place.with_stack(host, |_| ()).is_none());
                    assert_eq!(place.insert(host, tx, &stick, 1), Ok(1));
                    Ok::<_, ()>(())
                })
                .unwrap();
                Ok::<_, ()>(())
            })
            .unwrap();
            Err::<(), ()>(())
        });
        assert!(result.is_err());
        assert_eq!(len(&game), 0);

        for (kind, nbt) in vehicles {
            let id = game
                .summon("minecraft:overworld", kind, [8.5, 70.0, 8.5], nbt)
                .unwrap_or_else(|| panic!("{kind} not summoned"));
            let place = |slot| mod_api::ItemPlace::EntitySlot { entity: id, slot };
            let host: &mut dyn ServerHost = &mut game;
            let went = host.transaction(|host, tx| place(0).insert(host, tx, &stick, 5));
            assert_eq!(went, Ok(5), "{kind}");
            let _ = host.transaction(|host, tx| {
                assert_eq!(place(0).insert(host, tx, &stick, 4), Ok(4), "{kind}");
                assert_eq!(place(1).insert(host, tx, &stick, 2), Ok(2), "{kind}");
                Err::<(), ()>(())
            });
            let host: &mut dyn ServerHost = &mut game;
            assert_eq!(
                place(0).with_stack(host, |stack| stack.project()).flatten(),
                Some(sticks(5).try_into_api().unwrap()),
                "{kind}"
            );
            assert_eq!(place(1).with_stack(host, |_| ()), None, "{kind}");
            let _ = host.transaction(|host, tx| place(0).exchange(host, tx, &rod, 1));
            let host: &mut dyn ServerHost = &mut game;
            assert_eq!(
                place(0).with_stack(host, |stack| stack.project()).flatten(),
                Some(sticks(4).try_into_api().unwrap()),
                "{kind}"
            );
            let rod =
                place(1).with_stack(host, |s| mod_api::__private::item_key_id(s.item()).unwrap());
            assert_eq!(rod, Some("minecraft:blaze_rod"), "{kind}");
        }

        // An item entity: emptied in an aborted transaction, it's back whole; one of four
        // exchanged in a committed one is dropped beside it at the commit; emptied in a
        // committed one, it's gone.
        game.spawn_item(sticks(4), [0.5, 70.0, 6.5], [0.0; 3], 10);
        let id = game.dropped.last().unwrap().id;
        let entity = mod_api::ItemPlace::ItemEntity { id };
        let count = |game: &crate::game::ServerGame<'_>| game.dropped.len();
        let items = count(&game);
        let host: &mut dyn ServerHost = &mut game;
        let _ = host.transaction(|host, tx| {
            assert_eq!(entity.extract(host, tx, &stick, 4), Ok(4));
            assert!(entity.with_stack(host, |_| ()).is_none());
            Err::<(), ()>(())
        });
        let host: &mut dyn ServerHost = &mut game;
        assert_eq!(
            entity.with_stack(host, |stack| stack.project()).flatten(),
            Some(sticks(4).try_into_api().unwrap())
        );
        let _ = host.transaction(|host, tx| entity.exchange(host, tx, &rod, 1));
        let host: &mut dyn ServerHost = &mut game;
        assert_eq!(
            entity.with_stack(host, |stack| stack.project()).flatten(),
            Some(sticks(3).try_into_api().unwrap())
        );
        assert_eq!(count(&game), items + 1);
        let host: &mut dyn ServerHost = &mut game;
        let _ = host.transaction(|host, tx| entity.extract(host, tx, &stick, 3));
        assert!(game.dropped.iter().all(|e| e.id != id));
        assert_eq!(count(&game), items);

        // A hopper moving into a mod's part: a truthful part (a pipe into the chest east of
        // it) takes the item; a part answering more than it was offered has its move undone,
        // so the item stays in the hopper and the chest it wrote is as it was.
        let hopper = sunrise_world::chunk::BlockState {
            name: "minecraft:hopper".to_owned(),
            properties: [("facing".to_owned(), "east".to_owned())]
                .into_iter()
                .collect(),
        };
        let hopper = game.pack.assets.registry.resolve(&hopper).unwrap();
        let chest = game
            .pack
            .assets
            .registry
            .default_state("minecraft:chest")
            .unwrap();
        assert_eq!(
            game.level_set_block(level, [0, 64, 4], hopper, flags),
            Ok(true)
        );
        assert_eq!(
            game.level_set_block(level, [2, 64, 4], chest, flags),
            Ok(true)
        );
        let slots =
            |game: &crate::game::ServerGame<'_>, pos| match game.world.block_entities.get(&pos) {
                Some(BlockEntity::Chest(items)) => {
                    items.iter().flatten().map(|s| s.count).sum::<u32>()
                }
                _ => panic!("no container at {pos:?}"),
            };
        for (id, moved) in [("test:pipe", 1), ("test:greedy", 0)] {
            let state = game.pack.assets.registry.default_state(id).unwrap();
            assert_eq!(
                game.level_set_block(level, [1, 64, 4], state, mod_api::BlockUpdateFlags::NONE),
                Ok(true)
            );
            if let Some(BlockEntity::Chest(items)) = game.world.block_entities.get_mut(&(0, 64, 4))
            {
                items[0] = Some(sticks(2));
            }
            if let Some(BlockEntity::Chest(items)) = game.world.block_entities.get_mut(&(2, 64, 4))
            {
                items.fill(None);
            }
            game.test_clear_hopper((0, 64, 4));
            game.hopper_try_move_items((0, 64, 4));
            assert_eq!(slots(&game, (0, 64, 4)), 2 - moved, "{id}");
            assert_eq!(slots(&game, (2, 64, 4)), moved, "{id}");
            assert!(game.transactions.is_empty());
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod mod_capability_tests {
    use super::super::native_entity_refs::tests::with_game;
    use mod_api::ServerHost;
    use mod_api::{BlockSide, HostTransaction, ResourceHandler, resource_util::move_resources};
    use mod_platform::__private::part_tests::{CHEMICALS, ChemTank, Chemical};
    use std::sync::Arc;
    use sunrise_client::chunk::Chunk;
    use sunrise_client::containers::{BlockEntity, ModBlockEntity, ModBlockEntityFields};
    use sunrise_core::nbt::{Compound, Tag};

    /// Two mods sharing a capability one of them defines, as Mekanism's chemicals are shared
    /// through its API: the first's tank and the second's vat each answer `CHEMICALS`, and a
    /// move between them in a committed transaction moves the chemical, kept in their saved
    /// data; in an aborted one, both are put back.
    #[test]
    #[ignore = "daily"]
    fn a_chemical_moves_between_two_mods_parts_and_an_abort_puts_it_back() {
        use super::super::native_entity_refs::tests::{declared_part_root, with_game_at};
        let Some(root) = declared_part_root(
            "game::transactions::mod_capability_tests::a_chemical_moves_between_two_mods_parts_and_an_abort_puts_it_back",
        ) else {
            return;
        };
        with_game_at(root, |game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let level = game.level("minecraft:overworld").unwrap();
            let hydrogen = Chemical("test:hydrogen".into());
            let mut nbt = Compound::new();
            nbt.insert("chemical".into(), Tag::String("test:hydrogen".into()));
            nbt.insert("amount".into(), Tag::Int(600));
            for (x, id, nbt) in [
                (0, "test:chem_tank", nbt),
                (1, "test:chem_vat", Compound::new()),
            ] {
                let state = game.pack.assets.registry.default_state(id).unwrap();
                game.world.set(x, 64, 0, state).unwrap();
                let fields = ModBlockEntityFields {
                    nbt,
                    ..Default::default()
                };
                let entity = BlockEntity::Mod(Box::new(ModBlockEntity::new(id.into(), fields)));
                game.world.block_entities.insert((x, 64, 0), entity);
            }
            let host: &mut dyn ServerHost = game;
            let tank = level
                .capability(host, &CHEMICALS, [0, 64, 0], None)
                .unwrap();
            let vat = level
                .capability(host, &CHEMICALS, [1, 64, 0], None)
                .unwrap();
            // Each answered by its own mod.
            assert_ne!(tank.owner(), vat.owner());
            assert!(![tank.owner(), vat.owner()].contains(&"the game".to_owned()));
            let amounts = |host: &mut dyn ServerHost| {
                let side = |x| BlockSide {
                    level,
                    pos: [x, 64, 0],
                    side: None,
                };
                (
                    ChemTank(side(0)).amount(host, 0),
                    ChemTank(side(1)).amount(host, 0),
                )
            };

            let moved = host.transaction(|host, tx| {
                move_resources(host, &tank, &vat, |c| *c == hydrogen, 250, Some(tx)).map_err(drop)
            });
            assert_eq!(moved, Ok(250));
            assert_eq!(amounts(host), (350, 250));
            assert_eq!(vat.resource(host, 0), Some(hydrogen.clone()));

            let aborted = host.transaction(|host, tx| {
                let moved = move_resources(host, &tank, &vat, |_| true, 100, Some(tx));
                assert_eq!(moved.ok(), Some(100));
                assert_eq!(amounts(host), (250, 350));
                Err::<(), ()>(())
            });
            assert!(aborted.is_err());
            assert_eq!(amounts(host), (350, 250));
            assert!(game.transactions.is_empty());
        });
    }

    /// A mod's attachments on an entity and on a player are kept by a transaction as a block
    /// entity's are: a commit keeps what it wrote, an abort puts each map back as it was before
    /// the transaction first reached it (none for an entity that had none), and a guest's is
    /// put back wherever their state lives at the abort, swapped in for their turn or not.
    #[test]
    #[ignore = "daily"]
    fn entity_and_player_attachments_stay_on_commit_and_go_back_on_abort() {
        use mod_api::{AttachmentType, Holder};
        use sunrise_client::entities::ArmorStand;
        use sunrise_core::geometry::Vec3;
        const ENERGY: AttachmentType<i32> = AttachmentType {
            namespace: "robot",
            name: "energy",
            default: || 0,
            codec: None,
            copy_on_death: false,
            sync: false,
        };
        with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            game.armor_stands.push(ArmorStand::new(13, Vec3::ZERO, 0.0));
            game.armor_stands.push(ArmorStand::new(14, Vec3::ZERO, 0.0));
            let guest = 0x5eed_u128;
            let config = sunrise_client::lan::hub::HubConfig {
                default_game_mode: 0,
                guest_commands: false,
                force_game_mode: false,
                hardcore: false,
                view_distance: 10,
                simulation_distance: 10,
                dimension: "minecraft:overworld".into(),
                allow_flight: false,
                seed: 0,
                max_players: 20,
                integrated: false,
                levels: Default::default(),
                level_seeds: Default::default(),
            };
            game.lan_host(
                super::super::lan::LanSettings {
                    port: 0,
                    guest_commands: false,
                    force_game_mode: false,
                },
                sunrise_client::lan::NetConfig {
                    announce: false,
                    ..sunrise_client::lan::NetConfig::lan(0, "test".into(), 2)
                },
                config,
            )
            .unwrap();
            let level = game.level(game.dim.id()).unwrap();
            let player_ref = game.fake_player(level, guest, "test").unwrap();
            let conn = game
                .lan
                .as_ref()
                .unwrap()
                .hub
                .guests
                .iter()
                .find(|g| g.entity_id as u32 == player_ref.entity().id())
                .unwrap()
                .conn;
            let (stand, fresh, player) = (
                Holder::Entity(13),
                Holder::Entity(14),
                Holder::Player(guest),
            );
            let energy =
                |host: &mut dyn ServerHost, holder| ENERGY.get_existing(host, holder).unwrap();

            let host: &mut dyn ServerHost = game;
            let done = host.transaction(|host, _| {
                ENERGY.set(host, stand, 100)?;
                ENERGY.set(host, player, 40)
            });
            assert_eq!(done, Ok(()));
            assert_eq!(
                (energy(host, stand), energy(host, player)),
                (Some(100), Some(40))
            );

            // Aborted, with a committed child inside: both maps as they were, and the entity
            // that had no value has none.
            let aborted: Result<(), _> = host.transaction(|host, _| {
                ENERGY.set(host, stand, 250)?;
                ENERGY.set(host, player, 90)?;
                ENERGY.set(host, fresh, 7)?;
                host.transaction(|host, _| ENERGY.set(host, stand, 300))
                    .unwrap();
                assert_eq!(energy(host, stand), Some(300));
                Err(mod_api::AttachmentError::MissingHolder)
            });
            assert!(aborted.is_err());
            assert_eq!(
                (energy(host, stand), energy(host, player)),
                (Some(100), Some(40))
            );
            assert_eq!(energy(host, fresh), None);

            // Written while the guest's state is swapped in for their turn, aborted after it
            // is swapped back out: the guest's own map is put back.
            game.open_storage_transaction();
            game.side.lan_acting_uuid = Some(guest);
            std::mem::swap(
                &mut game.player_state.attachments,
                game.lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .attachments_mut(conn)
                    .unwrap(),
            );
            let host: &mut dyn ServerHost = game;
            ENERGY.set(host, player, 70).unwrap();
            std::mem::swap(
                &mut game.player_state.attachments,
                game.lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .attachments_mut(conn)
                    .unwrap(),
            );
            game.side.lan_acting_uuid = None;
            game.close_storage_transaction(false);
            let host: &mut dyn ServerHost = game;
            assert_eq!(energy(host, player), Some(40));
            assert!(game.transactions.is_empty());
        });
    }

    #[test]
    #[ignore = "daily"]
    fn cauldron_effects_wait_for_root_and_abort_with_inventory() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let pos = [2, 64, 2];
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let empty = game
                .pack
                .assets
                .registry
                .default_state("minecraft:cauldron")
                .unwrap();
            game.world.set(pos[0], pos[1], pos[2], empty).unwrap();
            game.player = Some(
                super::super::PlayerMotion::new(
                    game.motion,
                    sunrise_core::geometry::Vec3::new(0.5, 65.0, 3.5),
                )
                .unwrap(),
            );
            game.selected = 0;
            game.creative = false;
            game.inventory[0] = Some(sunrise_client::items::ItemStack::new(
                "minecraft:water_bucket",
                1,
            ));
            let before = game.stats.get("custom", "minecraft:fill_cauldron");
            game.start_audio_capture();
            game.open_storage_transaction();
            assert_eq!(
                game.use_on_cauldron(pos).unwrap(),
                sunrise_client::cauldron::Outcome::Success
            );
            assert_eq!(game.stats.get("custom", "minecraft:fill_cauldron"), before);
            assert!(game.side.pending_sounds.is_empty());
            game.close_storage_transaction(false);
            assert_eq!(game.world.block(pos[0], pos[1], pos[2]), empty);
            assert_eq!(
                &*game.inventory[0].as_ref().unwrap().item,
                "minecraft:water_bucket"
            );
            assert_eq!(game.stats.get("custom", "minecraft:fill_cauldron"), before);
            assert!(game.side.pending_sounds.is_empty());
            game.open_storage_transaction();
            game.open_storage_transaction();
            assert_eq!(
                game.use_on_cauldron(pos).unwrap(),
                sunrise_client::cauldron::Outcome::Success
            );
            game.close_storage_transaction(false);
            assert_eq!(game.world.block(pos[0], pos[1], pos[2]), empty);
            assert_eq!(
                &*game.inventory[0].as_ref().unwrap().item,
                "minecraft:water_bucket"
            );
            game.close_storage_transaction(true);
            assert_eq!(game.stats.get("custom", "minecraft:fill_cauldron"), before);
            assert!(game.side.pending_sounds.is_empty());
            assert_eq!(
                game.use_on_cauldron(pos).unwrap(),
                sunrise_client::cauldron::Outcome::Success
            );
            assert_ne!(game.world.block(pos[0], pos[1], pos[2]), empty);
            assert_eq!(
                &*game.inventory[0].as_ref().unwrap().item,
                "minecraft:bucket"
            );
            assert_eq!(
                game.stats.get("custom", "minecraft:fill_cauldron"),
                before + 1
            );
            assert_eq!(
                game.side
                    .pending_sounds
                    .iter()
                    .map(|s| s.event.as_str())
                    .collect::<Vec<_>>(),
                ["minecraft:item.bucket.empty"]
            );
        });
    }
}
