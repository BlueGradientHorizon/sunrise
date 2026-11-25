#[allow(unused_imports)]
pub(in crate::game::client_side) use super::api_values::{TryIntoApi, block_state};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, GameLevel};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_block_sounds::sound_type;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_block_sounds::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{BlockSoundCause, BlockSoundType};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{identifier::Identifier, position::BlockPos};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    level::data::Sounds, native_load_context::LoopContext, registry::BlockStateId,
};

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
