//! Common entry points for `player_destroy_progress` and `player_has_correct_tool_for_drops`.
use crate::game::CommonGame;
use crate::game::held_items::{BreakSpeedDispatch, HarvestDispatch};
use mod_api::{LevelRef, PlayerRef};
use sunrise_world::registry::BlockStateId;

impl CommonGame<'_> {
    /// `BlockBehaviour.getDestroyProgress(state, player, level, pos)` for a supplied state.
    pub(in crate::game) fn player_destroy_progress(
        &mut self,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<f32> {
        self.pack.assets.registry.state(state)?;
        if BreakSpeedDispatch::running() || HarvestDispatch::running() {
            return None;
        }
        self.side_player_destroy_progress(player, level, pos, state)
    }

    /// `Player.hasCorrectToolForDrops(state, level, pos)` and `canHarvestBlock`.
    pub(in crate::game) fn player_has_correct_tool_for_drops(
        &mut self,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<bool> {
        self.pack.assets.registry.state(state)?;
        // From inside a harvest hook the query would run the hook again, without bound.
        if HarvestDispatch::running() {
            return None;
        }
        self.side_player_has_correct_tool_for_drops(player, level, pos, state)
    }
}

use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;
