//! A player's break for native mods (`PlayerRef::destroy_block`): what a vein miner needs to
//! break the blocks around the one its player broke as that player breaks them, taking their
//! drops and experience in `block_drops`.
use crate::game::ServerGame;
use mod_api::PlayerRef;
use std::cell::Cell;

/// How many player breaks may be inside one another on the server thread. A `break_block` or
/// `block_drops` listener may break again as the player, which runs the hooks again, and the
/// block it started from is still there: a listener with no guard of its own would recurse until
/// the stack overflows. At the limit the call is refused (false). A guarded miner nests one
/// level deep, so this is far beyond any use, and each level costs only a few frames of the
/// thread's stack. A Sunrise safeguard: NeoForge has none, and there the same listener crashes
/// the server.
const MAX_NESTED_BREAKS: u32 = 8;

thread_local! {
    /// The player breaks now inside one another on this thread.
    static NESTED_BREAKS: Cell<u32> = const { Cell::new(0) };
}

/// One level of [`NESTED_BREAKS`], given back when dropped, a panic unwinding included.
struct NestedBreak;

impl NestedBreak {
    /// A level, or none at [`MAX_NESTED_BREAKS`].
    fn enter() -> Option<Self> {
        NESTED_BREAKS.with(|depth| {
            (depth.get() < MAX_NESTED_BREAKS).then(|| {
                depth.set(depth.get() + 1);
                Self
            })
        })
    }
}

impl Drop for NestedBreak {
    fn drop(&mut self) {
        NESTED_BREAKS.with(|depth| depth.set(depth.get() - 1));
    }
}

impl ServerGame<'_> {
    /// `ServerPlayerGameMode.destroyBlock` at `pos` as `player` breaks it, as FTB Ultimine does
    /// (`PlayerInteractionBackendMut::player_destroy_block` says what holds and what is refused). With
    /// `into_inventory`, the drops `block_drops` leaves go to the player's inventory, and what
    /// doesn't fit drops from them as `Inventory.placeItemBackInInventory` drops it. Refused
    /// (false) once [`MAX_NESTED_BREAKS`] breaks are inside one another.
    pub(in crate::game) fn player_destroy_block(
        &mut self,
        player: PlayerRef,
        pos: [i32; 3],
        into_inventory: bool,
    ) -> bool {
        if !self.native_authoritative() || !self.native_player_current(player) {
            return false;
        }
        let Some(_nested) = NestedBreak::enter() else {
            return false;
        };
        let mut broke = false;
        self.native_as_player_handle(player, |game| {
            // The break is in the player's level: for a player in another level than the one
            // in place, nothing is broken.
            if game.acting_in_level_in_place() {
                broke = game.break_as_acting_player(pos, into_inventory);
            }
        });
        broke
    }

    fn break_as_acting_player(&mut self, pos: [i32; 3], into_inventory: bool) -> bool {
        if !self.common.world.loaded(pos[0], pos[2]) {
            return false;
        }
        let state = self.common.world.block(pos[0], pos[1], pos[2]);
        // A survival player never finishes mining a block with no hardness:
        // their client never sends the break.
        if self.common.res.behaviours().is_air(state)
            || (!self.common.creative && self.common.res.facts().get(state).hardness < 0.0)
        {
            return false;
        }
        // ServerPlayerGameMode.handleBlockBreakAction's `mayInteract`.
        if self
            .server
            .lan_acting_spawn_protection
            .is_some_and(|radius| self.common.spawn_protects(radius, self.common.dim.id(), pos))
        {
            return false;
        }
        // A break inside another's hook keeps where the outer one's drops go.
        let outer = std::mem::replace(&mut self.server.break_collect, into_inventory.then(Vec::new));
        let broke = self.player_break_block(pos).unwrap_or_else(|e| {
            log::warn!("a mod's break at {pos:?} failed: {e}");
            false
        });
        let drops = std::mem::replace(&mut self.server.break_collect, outer).unwrap_or_default();
        for stack in drops {
            self.give_or_drop(stack);
        }
        broke
    }
}

impl ServerGame<'_> {
    /// `ServerPlayerGameMode.destroyBlock`'s `Level.removeBlock`, then `Block.destroy`: the mods'
    /// `player_destroyed_block` hears the removal once it succeeded, before a creative player's
    /// return and the drops. It is told the block the removal took out, read here, after
    /// `playerWillDestroy` and the mods' `break_block` had their say, together with that block's
    /// block entity data. True when the block was removed.
    pub(in crate::game::server_side) fn remove_block_as_player(
        &mut self,
        pos: [i32; 3],
        creative: bool,
        correct_tool: bool,
    ) -> bool {
        let source = self
            .server
            .lan_acting_uuid
            .map(sunrise_world::level::LevelEventPlayer);
        let state = self.common.world.block(pos[0], pos[1], pos[2]);
        let block_entity = self.removed_block_entity_data(pos, state);
        let removed = self.common.with_level(|b, level, ticks| {
            b.player_destroy_block(
                level,
                ticks,
                sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]),
                creative,
                correct_tool,
                source,
            )
        });
        if removed {
            self.native_player_destroyed_block(pos, state, block_entity);
        }
        removed
    }
}
