//! Common entry points for `player_destroy_progress` and `player_has_correct_tool_for_drops`.
use super::Game;
use mod_api::{LevelRef, PlayerRef};
use sunrise_world::registry::BlockStateId;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// `BlockBehaviour.getDestroyProgress(state, player, level, pos)` for a supplied state.
    pub(in crate::game) fn player_destroy_progress(
        &mut self,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<f32> {
        self.pack.assets.registry.state(state)?;
        if crate::game::hand::BreakSpeedDispatch::running()
            || crate::game::hand::HarvestDispatch::running()
        {
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
        if crate::game::hand::HarvestDispatch::running() {
            return None;
        }
        self.side_player_has_correct_tool_for_drops(player, level, pos, state)
    }
}
