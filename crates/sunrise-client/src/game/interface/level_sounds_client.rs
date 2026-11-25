#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::native_hooks::level_sound_playable;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::level_sounds::BoundSound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::level_sounds::HeardSound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::level_sounds::LevelSound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::level_sounds::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{HookId, Identifier, SoundSource};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::borrow::Cow;

impl Game<'_> {
    /// `ClientLevel.playSeededSound`: ask this client's mods before playback.
    /// A packet reaches this independently of the server's level event.
    pub(in crate::game) fn play_client_level_sound(
        &mut self,
        mut sound: LevelSound<'_>,
        seed: i64,
    ) {
        assert!(
            !self.is_logical_server(),
            "play_client_level_sound called on Server role"
        );
        let entity = match sound.emitter {
            Some(id) => {
                let Some(state) = self.remote_sound_entity_state(id as i32) else {
                    return;
                };
                sound.at = state.position;
                let entity = sunrise_client::sound_engine::SoundEntity::new(
                    id,
                    state.position,
                    state.silent,
                );
                entity.update(state);
                Some(entity)
            }
            None => None,
        };
        let at = sound.at;
        let Some(heard) = self.ask_level_sound(self.dim.id(), sound) else {
            return;
        };
        match entity {
            Some(entity) => self.audio.0.play_entity_seeded(
                &heard.name,
                heard.source.index(),
                heard.volume,
                heard.pitch,
                seed,
                entity,
            ),
            None => self.audio.0.play_on_seeded(
                heard.source.index(),
                &heard.name,
                Some(at),
                heard.volume,
                heard.pitch,
                seed,
            ),
        }
    }
    /// A client-local level request without a packet seed. LocalPlayer.playSound
    /// asks AtEntity but still creates a positional, not entity-bound, instance.
    pub(in crate::game) fn play_client_local_sound(&mut self, sound: LevelSound<'_>) {
        assert!(
            !self.is_logical_server(),
            "play_client_local_sound called on Server role"
        );
        let seed = self.audio.0.next_sound_seed();
        let at = sound.at;
        if let Some(heard) = self.ask_level_sound(self.dim.id(), sound) {
            self.audio.0.play_on_seeded(
                heard.source.index(),
                &heard.name,
                Some(at),
                heard.volume,
                heard.pitch,
                seed,
            );
        }
    }
}
