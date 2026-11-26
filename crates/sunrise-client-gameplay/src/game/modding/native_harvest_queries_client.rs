//! Client arms for `player_destroy_progress` and `player_has_correct_tool_for_drops`.
use crate::game::ClientGame;
use mod_api::{LevelRef, PlayerRef};
use sunrise_world::registry::BlockStateId;

impl ClientGame<'_> {
    pub(in crate::game) fn client_player_destroy_progress(
        &mut self,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<f32> {
        if self.client_level_view_world(level).is_none()
            || level.dimension() != self.common.dim.key()
            || self.common.local_player() != Some(player)
        {
            return None;
        }
        Some(self.common.destroy_progress_for(pos, state))
    }

    pub(in crate::game) fn client_player_has_correct_tool_for_drops(
        &mut self,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Option<bool> {
        if self.client_level_view_world(level).is_none()
            || level.dimension() != self.common.dim.key()
            || self.common.local_player() != Some(player)
        {
            return None;
        }
        Some(self.common.acting_can_harvest(pos, state))
    }
}
