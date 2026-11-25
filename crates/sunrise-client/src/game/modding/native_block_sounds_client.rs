use super::Game;
use mod_api::{BlockSoundCause, BlockSoundType};
use sunrise_world::registry::BlockStateId;

impl Game<'_, crate::game::client_side::ClientState> {
    /// [`Game::block_sound_type`] for this window's own player, as its client asks for the
    /// sounds it plays itself: its steps and the blocks it places.
    pub(in crate::game::client_side) fn own_sound_type(
        &self,
        pos: [i32; 3],
        state: BlockStateId,
        cause: BlockSoundCause,
    ) -> BlockSoundType {
        let player = (self.own_entity_id() as u32, "minecraft:player");
        self.block_sound_type(pos, state, Some(player), cause)
    }
    /// `LevelEventHandler`'s break sound (level event 2001, which a client's predicted break
    /// also plays): the block's break sound at its centre, at `(volume + 1) / 2` and pitch × 0.8.
    /// None for air.
    pub(in crate::game) fn play_break_sound(&mut self, pos: [i32; 3], state: BlockStateId) {
        if self.res.facts().is_air(state) {
            return;
        }
        let sound = self.block_sound_type(pos, state, None, BlockSoundCause::Break);
        self.side.audio.0.play(
            &sound.break_sound,
            Some(pos.map(|v| f64::from(v) + 0.5)),
            (sound.volume + 1.0) / 2.0,
            sound.pitch * 0.8,
        );
    }
}
