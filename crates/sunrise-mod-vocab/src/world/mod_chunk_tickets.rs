//! Moved here from the game so mods can name it.
#[allow(unused_imports)]
use crate::*;
// no wire: encoded by the existing manual Wire implementation on TicketOwner.
/// What a ticket belongs to: a block (a machine) or an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: a closed set the game matches without a wildcard
pub enum TicketOwner {
    /// The block at `[x, y, z]`.
    Block([i32; 3]),
    /// The entity with this UUID.
    Entity(u128),
}
/// One chunk a mod holds loaded for an owner.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: the game builds it with a literal; mods only read it
#[derive(sunrise_mod_api_macros::Wire)]
pub struct ModTicket {
    /// The controller that holds it: its slot in the installed controller table
    /// (`mod_api::TicketController::slot`).
    pub controller: u32,
    pub owner: TicketOwner,
    pub chunk: (i32, i32),
    /// Whether the chunk joins the spawning chunks (inhabited time, thunder,
    /// a spawn attempt) with no player within 128 blocks
    /// (`forceChunk`'s `forceNaturalSpawning`). One owner holds the two kinds
    /// apart.
    pub natural_spawning: bool,
}
/// The chunks one owner holds, the two kinds apart (NeoForge's `TicketSet`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustive: the game builds it with a literal; mods only read it
#[derive(sunrise_mod_api_macros::Wire)]
pub struct TicketSet {
    /// Chunks held without natural spawning.
    pub non_natural_spawning: Vec<[i32; 2]>,
    /// Chunks held with natural spawning forced.
    pub natural_spawning: Vec<[i32; 2]>,
}
impl TicketSet {
    fn push(&mut self, chunk: [i32; 2], natural_spawning: bool) {
        if natural_spawning {
            self.natural_spawning.push(chunk);
        } else {
            self.non_natural_spawning.push(chunk);
        }
    }
}
/// The saved tickets lent to one controller's [`LoadingValidationCallback`](crate::LoadingValidationCallback)
/// (NeoForge's `TicketHelper`). The view contains only that controller's
/// staged rows, with block and entity owners apart. Removals take effect
/// immediately on those rows; a removed ticket never forces its chunk.
pub trait TicketValidation {
    /// The controller's block tickets: each block position with the chunks it
    /// holds without and with natural spawning (`TicketHelper.getBlockTickets`'
    /// `TicketSet`: `nonNaturalSpawning`, `naturalSpawning`), in position order.
    fn block_tickets(&self) -> Vec<([i32; 3], TicketSet)>;
    /// The controller's entity tickets: each entity UUID with the chunks it
    /// holds without and with natural spawning (`TicketHelper.getEntityTickets`),
    /// in UUID order.
    fn entity_tickets(&self) -> Vec<(u128, TicketSet)>;
    /// Remove every ticket the block holds, of both kinds
    /// (`removeAllTickets(BlockPos)`): how many went.
    fn remove_block_tickets(&mut self, pos: [i32; 3]) -> usize;
    /// Remove every ticket the entity holds, of both kinds
    /// (`removeAllTickets(UUID)`).
    fn remove_entity_tickets(&mut self, uuid: u128) -> usize;
    /// Remove the block's ticket of one kind on one chunk
    /// (`removeTicket(BlockPos, ChunkPos, forceNaturalSpawning)`): whether it
    /// held one. The other kind on the chunk stays.
    fn remove_block_ticket(
        &mut self,
        pos: [i32; 3],
        chunk: [i32; 2],
        natural_spawning: bool,
    ) -> bool;
    /// Remove the entity's ticket of one kind on one chunk
    /// (`removeTicket(UUID, ChunkPos, forceNaturalSpawning)`).
    fn remove_entity_ticket(&mut self, uuid: u128, chunk: [i32; 2], natural_spawning: bool)
    -> bool;
}

// no wire: borrows one controller's staged rows during native validation.
/// The game's [`TicketValidation`]: one controller's staged tickets.
// hidden: a game-side helper; mods receive the TicketValidation trait.
#[doc(hidden)]
// exhaustive: its field is private, so mods cannot build or destructure it
pub struct ControllerTickets<'a> {
    loaded: &'a mut Vec<ModTicket>,
}
impl std::fmt::Debug for ControllerTickets<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControllerTickets")
            .field("loaded", &self.loaded.len())
            .finish()
    }
}
impl<'a> ControllerTickets<'a> {
    /// Borrow the staged tickets of one controller. For the game.
    // hidden: a game-side helper, not part of the mod surface
    #[doc(hidden)]
    pub fn new(loaded: &'a mut Vec<ModTicket>) -> Self {
        Self { loaded }
    }
}
impl TicketValidation for ControllerTickets<'_> {
    fn block_tickets(&self) -> Vec<([i32; 3], TicketSet)> {
        let mut grouped = std::collections::BTreeMap::<[i32; 3], TicketSet>::new();
        for row in &*self.loaded {
            if let TicketOwner::Block(pos) = row.owner {
                grouped
                    .entry(pos)
                    .or_default()
                    .push([row.chunk.0, row.chunk.1], row.natural_spawning);
            }
        }
        grouped.into_iter().collect()
    }
    fn entity_tickets(&self) -> Vec<(u128, TicketSet)> {
        let mut grouped = std::collections::BTreeMap::<u128, TicketSet>::new();
        for row in &*self.loaded {
            if let TicketOwner::Entity(uuid) = row.owner {
                grouped
                    .entry(uuid)
                    .or_default()
                    .push([row.chunk.0, row.chunk.1], row.natural_spawning);
            }
        }
        grouped.into_iter().collect()
    }
    fn remove_block_tickets(&mut self, pos: [i32; 3]) -> usize {
        self.remove(TicketOwner::Block(pos), None)
    }
    fn remove_entity_tickets(&mut self, uuid: u128) -> usize {
        self.remove(TicketOwner::Entity(uuid), None)
    }
    fn remove_block_ticket(
        &mut self,
        pos: [i32; 3],
        chunk: [i32; 2],
        natural_spawning: bool,
    ) -> bool {
        self.remove(TicketOwner::Block(pos), Some((chunk, natural_spawning))) > 0
    }
    fn remove_entity_ticket(
        &mut self,
        uuid: u128,
        chunk: [i32; 2],
        natural_spawning: bool,
    ) -> bool {
        self.remove(TicketOwner::Entity(uuid), Some((chunk, natural_spawning))) > 0
    }
}
impl ControllerTickets<'_> {
    fn remove(&mut self, owner: TicketOwner, kind: Option<([i32; 2], bool)>) -> usize {
        let before = self.loaded.len();
        self.loaded.retain(|row| {
            row.owner != owner
                || kind.is_some_and(|([x, z], spawning)| {
                    row.chunk != (x, z) || row.natural_spawning != spawning
                })
        });
        before - self.loaded.len()
    }
}
