#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    BlockEntity, BlockStateId, Dim, Game, lan_mount_menu::MountClickWindow, machines::HopperState,
    maps::MapData,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::After;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::AttachmentKey;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::BlockReplay;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::BlockStep;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::ClosingPhase;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::ComposterRandoms;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::DeferredEffect;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::JournaledPlayer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::Kept;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptAttachments;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptBlock;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptClick;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptEquipment;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptGeneralEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptItemEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptMenus;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptMobEffects;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptMobHealth;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptPlayer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptPlayerHealth;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::KeptSlot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::Level;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::MenuDataJournal;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::OpenTransactions;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::PlayerBodyKey;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::PlayerStorageHome;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::Pos;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::QuickMoveNotice;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::QuickMoveNotices;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::RemovedRow;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::SaveChunkScope;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::StorageScope;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::StorageSettlement;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::StrictBlockStep;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::StrictBlockSteps;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::entities::{
    Announcement, ArrivalRollback,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::merge_block_update_flags;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::take_replay_row;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::transactions::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    __private::TransactionFrame, LevelRef, TransactionContext, TransactionLifecycle,
    TransactionScopeError,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::{BTreeMap, HashMap};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    block_entities::BlockEntityIdentity,
    effects::EffectInstance,
    fast_hash::FxHashSet,
    items::ItemStack,
    living_attributes::ModifierChanges,
    menu::QuickCraftDrag,
    mobs::{Mob, StoredEffect, finalize::EffectModifierUndo, gear::Gear},
    world::{BlockEntityExtras, ModifiedChunks, World},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3, position::BlockPos, random::LegacyRandom,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_items::equipment::Equipment;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_mod_runtime::attachments::{
    AttachmentQueryScopes, Attachments, Holder,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::level::flags;

impl<'g> Game<'g> {
    pub(in crate::game) fn run_after(&mut self, after: After) {
        match after {
            After::PlayerRespawn(conn) => self.commit_player_respawn(conn),
            After::Trade(effect) => self.commit_merchant_effect(*effect),
            After::Payload(payload) => payload.publish(),
            After::Packet(packet, body) => self.publish_mod_held(packet, body),
            After::Composter(effects) => self.commit_composter_effects(effects),
            After::FluidWrite(written) => self.commit_fluid_write(*written),
            After::FluidEffects(effects) => self.commit_fluid_effects(*effects),
            After::DispenserDrop {
                pos,
                facing,
                spawn,
                play_events,
            } => {
                self.commit_dispensed(pos, facing, *spawn, play_events);
            }
            After::DispenserFeedback { pos, facing, event } => {
                self.commit_dispense_feedback(pos, facing, event);
            }
            After::LevelWrite(write) => self.commit_level_write(*write),
            After::BlockGameEvent(id, pos, who, state) => {
                self.post_block_game_event(id, pos, Some(who), Some(state));
            }
            After::Changed(pos) => self.container_changed(pos),
            After::ChangedWithoutHook(pos) => self.container_changed_without_hook(pos),
            After::ChangedHolder { holder, silent } => {
                if !silent && let Some(queue) = &mut self.transactions.quick_move_notices {
                    queue.push(QuickMoveNotice {
                        dim: self.dim,
                        holder,
                    });
                } else {
                    self.publish_holder_changed(holder, silent);
                }
            }
            After::GuestMenu(conn, effects) => self.publish_guest_menu_effects(conn, *effects),
            After::PersonalSidebar(conn, contents) => {
                if contents.is_some() {
                    self.load_scoreboard();
                }
                let board = &self.score.board;
                if let Some(host) = &mut self.lan {
                    host.hub.update_guest(conn, |guest| {
                        guest.set_personal_sidebar(board, contents.map(|contents| *contents));
                    });
                }
            }
            After::MountEquipment {
                dim,
                id,
                at,
                sound,
                equips,
            } => {
                let _ = self.with_effect_level(dim, |game| {
                    if let Some((sound, center)) = sound {
                        game.audio.0.play(&sound, Some(center), 1.0, 1.0);
                    }
                    game.mob_equip_event(id, at, equips);
                });
            }
            After::MenuClick(reply) => self.publish_menu_click(*reply),
            After::MenuPlacesNotice(conn, uuid, window) => {
                self.queue_menu_places_notice(conn, uuid, window);
            }
            After::FurnaceMenu(effects) => self.publish_furnace_menu_effects(*effects),
            After::StoreCartItems(pos) => self.store_cart_items(pos),
            After::BackedMenuClick(conn, uuid, menu, window) => {
                self.publish_backed_menu_click(conn, uuid, menu, window);
            }
            After::Capabilities(pos) => self.world.capability_generations.invalidate(pos),

            After::BreakProgress(breaker, pos, progress) => {
                self.server_destroy_block_progress(breaker, pos, progress);
            }
            After::Biome(key) => {
                self.commit_biome_columns(&std::collections::HashSet::from([key]), &[key]);
            }
            After::Effect(effect) => match effect {
                DeferredEffect::CustomStat {
                    player,
                    key,
                    amount,
                } => {
                    self.native_as_player(player, |g| g.award_custom_stat(key, amount));
                }
                DeferredEffect::ItemStat {
                    player,
                    stat,
                    item,
                    amount,
                } => {
                    self.native_as_player(player, |g| g.award_stat(stat, &item, amount));
                }
                DeferredEffect::Sound {
                    sound,
                    at,
                    volume,
                    pitch,
                } => self.audio.0.play(sound, Some(at), volume, pitch),
                DeferredEffect::RestoreStatistics(player, values) => {
                    self.restore_player_statistics(player, *values);
                }
            },
            // Only reached through a root commit, which replays blocks and connection groups in order.
            After::Block(_) | After::BulkBlocks(..) | After::ConnectionGroup(..) => {}
            After::Drop(pos, stack) => {
                self.spawn_item(stack, pos.map(|p| f64::from(p) + 0.5), [0.0; 3], 10);
            }
            After::GivePickup {
                uuid,
                dim,
                prototype,
            } => {
                self.native_as_player(uuid, |g| {
                    let back = g.enter_command_level(dim);
                    if g.dim == dim {
                        g.give_pickup(prototype);
                    }
                    if let Some(back) = back {
                        g.enter_command_level(back);
                    }
                });
            }
            After::InventoryTrigger(uuid) => {
                self.native_as_player(uuid, |g| g.check_inventory_trigger());
            }
            After::GrindstoneEvent(pos) => self.level_event_at(1042, pos, 0),
            After::StationContent(conn, owner, window) => {
                if self
                    .lan
                    .as_ref()
                    .and_then(|host| host.menus.get(&conn))
                    .is_some_and(|menus| {
                        menus.station.id() == owner
                            && menus.station.as_ref().is_some_and(|s| s.window == window)
                    })
                {
                    self.lan_send_station(conn);
                }
            }
            After::GiveRest {
                uuid,
                rest,
                owner,
                dim,
                eye,
            } => {
                let mut rest = Some(rest);
                self.native_as_player(uuid, |g| {
                    let back = g.enter_command_level(dim);
                    if let Some(rest) = rest.take() {
                        g.give_remainder(rest, owner);
                    }
                    if let Some(back) = back {
                        g.enter_command_level(back);
                    }
                });
                // The stack was accepted (the command succeeded, `give_stack` returned no
                // rest): a player who died or left since, so it was not thrown, still has it
                // land where they were.
                if let Some(rest) = rest {
                    let back = self.enter_command_level(dim);
                    self.give_remainder_unheld(rest, owner, eye);
                    if let Some(back) = back {
                        self.enter_command_level(back);
                    }
                }
            }
            After::PlayerEquip(uuid, slot, stack) => {
                self.native_as_player(uuid, |g| {
                    g.player_equipped(&slot, stack.as_ref());
                    g.apply_equipment();
                });
            }
            After::PlayerDrop(uuid, stack) => {
                // A player who left meanwhile drops nothing; their inventory had changed for good.
                self.native_as_player(uuid, |g| {
                    g.drop_plain(stack);
                });
            }
            After::Spawn(at, stack) => self.spawn_item(stack, at, [0.0; 3], 10),
            After::Emptied(id) => {
                let emptied = self
                    .dropped
                    .iter()
                    .position(|e| e.id == id && e.stack.count == 0);
                if let Some(i) = emptied {
                    self.remove_dropped(i);
                }
            }
            After::EquipmentSync(id) => {
                if let Some(mob) = self.mobs.iter_mut().find(|m| m.id == id && !m.removed) {
                    mob.needs_sync = true;
                }
            }
            After::Equip {
                at,
                sound,
                equips,
                who,
            } => {
                // `equip_item_effects` rebuilt from the entity it names: the
                // sculk context reads the mob as it is at the root commit.
                let ctx = self.who_context(Some(sunrise_client::mobs::Who::Mob(who)), None);
                self.equip_item_effects(at, sound, equips, ctx);
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::api_values::TryIntoApi;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::{
        Audio, BlockEntity, Game, Options, Progress, Settings, load_resources,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::transactions::tests::chest_items;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::transactions::tests::extract;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::transactions::tests::extract_fluid;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::transactions::tests::insert;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::transactions::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::HostTransaction;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::ServerHost;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::{
        Authority, BlockSide, BlockUpdateFlags, FLUIDS, FluidStack, ITEMS, ItemResource,
        TransactionContext, TransactionError,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::{sync::Arc, time::Instant};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::{chunk::Chunk, items::ItemStack};

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
            game.inventory[0] = Some(ItemStack::new("minecraft:water_bucket", 1));
            let before = game.stats.get("custom", "minecraft:fill_cauldron");
            game.audio.0.capture = Some(Vec::new());
            game.open_storage_transaction();
            assert_eq!(
                game.use_on_cauldron(pos).unwrap(),
                sunrise_client::cauldron::Outcome::Success
            );
            assert_eq!(game.stats.get("custom", "minecraft:fill_cauldron"), before);
            assert!(game.audio.0.capture.as_ref().unwrap().is_empty());
            game.close_storage_transaction(false);
            assert_eq!(game.world.block(pos[0], pos[1], pos[2]), empty);
            assert_eq!(
                &*game.inventory[0].as_ref().unwrap().item,
                "minecraft:water_bucket"
            );
            assert_eq!(game.stats.get("custom", "minecraft:fill_cauldron"), before);
            assert!(game.audio.0.capture.as_ref().unwrap().is_empty());
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
            assert!(game.audio.0.capture.as_ref().unwrap().is_empty());
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
                game.audio
                    .0
                    .capture
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|s| s.event.as_str())
                    .collect::<Vec<_>>(),
                ["minecraft:item.bucket.empty"]
            );
        });
    }
}
