//! The `sound_type` Part method (`IBlockExtension.getSoundType`). The client library asks through
//! `native_client_hooks`; only a state whose block has a subscribing part is converted, and only
//! then is its own sound type kept to fall back on.
use super::api_values::{TryIntoApi, block_state};
use super::GameLevel;
use mod_api::{BlockSoundCause, BlockSoundType};
use std::sync::Arc;
use sunrise_core::{identifier::Identifier, position::BlockPos};
use sunrise_world::{
    level::data::Sounds, native_load_context::LoopContext, registry::BlockStateId,
};

/// The game's side of `sunrise_client::block_sounds::native_sound_type`.
pub(crate) fn sound_type(
    context: &LoopContext,
    pos: BlockPos,
    state: BlockStateId,
    entity: Option<(u32, &str)>,
    cause: BlockSoundCause,
    sound: &mut BlockSoundType,
) {
    if !block_state::block_listens(mod_api::HookId::SoundType, state) {
        return;
    }
    let Ok(api_state) = state.try_into_api() else {
        return;
    };
    let own = sound.clone();
    mod_api::dispatch_sound_type(
        &sunrise_level::api_context::LoopReader(context).context(),
        pos,
        api_state,
        entity,
        cause,
        sound,
    );
    if !playable(sound) {
        *sound = own;
    }
}

/// Whether a part's answer can be played: a finite volume and pitch of at least 0, and every
/// event a `namespace:path` id (a mod's own included; one the resources lack plays nothing).
pub fn playable(sound: &BlockSoundType) -> bool {
    let level = |value: f32| value.is_finite() && value >= 0.0;
    let events = [
        &sound.break_sound,
        &sound.step,
        &sound.place,
        &sound.hit,
        &sound.fall,
    ];
    level(sound.volume)
        && level(sound.pitch)
        && events
            .iter()
            .all(|event| Identifier::parse_namespaced(event).is_some())
}

impl crate::game::CommonGame<'_> {
    /// The sound type of `state` at `pos` for `cause` (`IBlockExtension.getSoundType`), as the
    /// block's parts answer on this side; `entity` is the id and type of the entity stepping,
    /// landing, placing or digging, and None for a break or a placement by no entity.
    pub fn block_sound_type(
        &self,
        [x, y, z]: [i32; 3],
        state: BlockStateId,
        entity: Option<(u32, &str)>,
        cause: BlockSoundCause,
    ) -> BlockSoundType {
        sunrise_sim::block_sounds::sound_type(
            &self.loop_context,
            self.res.facts(),
            BlockPos::new(x, y, z),
            state,
            entity,
            cause,
        )
    }
}

impl GameLevel<'_> {
    /// `LevelAccess::place_sound_type`: the state's own `sounds` unless a part on its block
    /// answers another, asked with the acting player, or none when no player places it (a
    /// dispenser).
    pub(in crate::game) fn part_place_sound_type(
        &self,
        pos: BlockPos,
        state: BlockStateId,
        sounds: Arc<Sounds>,
    ) -> Arc<Sounds> {
        if !block_state::block_listens(mod_api::HookId::SoundType, state) {
            return sounds;
        }
        let entity = self
            .native_player_entity
            .map(|id| (id as u32, "minecraft:player"));
        let own = sunrise_sim::block_sounds::own(&self.facts.get(state).sounds);
        let mut answer = own.clone();
        sunrise_sim::block_sounds::native_sound_type(
            self.loop_context,
            pos,
            state,
            entity,
            BlockSoundCause::Place,
            &mut answer,
        );
        if answer == own {
            return sounds;
        }
        Arc::new(Sounds {
            volume: answer.volume,
            pitch: answer.pitch,
            place: answer.place,
            break_: answer.break_sound,
            fall: answer.fall,
        })
    }
}
