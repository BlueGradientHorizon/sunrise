//! Client-side entity-bound level sound playback and entity sound state lookup.
use super::{Game, level_sounds::LevelSound};
use sunrise_core::nbt::Tag;
#[cfg(feature = "client")]
use sunrise_game_data::sound_source::SoundEntityState;

pub(in crate::game) fn saved_silent(extra: Option<&Tag>) -> bool {
    sunrise_sim::save::number(extra.and_then(|tag| tag.get("Silent"))).unwrap_or(0) != 0
}

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// `Entity.isSilent` for the player this window plays.
    #[allow(dead_code)]
    pub(in crate::game::client_side) fn player_silent(&self) -> bool {
        saved_silent(self.level.player_extra.as_ref())
    }

    pub(in crate::game) fn client_play_player_bound_sound(
        &mut self,
        event: &str,
        source: usize,
        volume: f32,
        pitch: f32,
        _except_actor: bool,
    ) {
        let id = self.remote.as_ref().map_or(0, |r| r.entity_id as u32);
        let mut sound = LevelSound::new(event, [0.0; 3], volume, pitch)
            .in_source(mod_api::SoundSource::ALL[source]);
        sound.emitter = Some(id);
        let seed = self.side.audio.0.next_sound_seed();
        self.play_client_level_sound(sound, seed);
    }

    pub(in crate::game::client_side) fn sound_entity_state(
        &mut self,
        id: u32,
    ) -> Option<SoundEntityState> {
        if self.remote.is_some() {
            self.remote_sound_entity_state(id as i32)
        } else {
            None
        }
    }
}
