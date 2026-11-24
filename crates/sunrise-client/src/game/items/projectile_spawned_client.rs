#[allow(unused_imports)]
pub(in crate::game::client_side) use super::projectile_spawned_validation::{
    requirements_supported, valid_ranges,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, enchant_effects::look_impulse};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::projectile_spawned::COMPONENT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::projectile_spawned::SpawnedEffect;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::projectile_spawned::applied;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::projectile_spawned::float_provider;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::projectile_spawned::level_sound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::projectile_spawned::spawned_effect;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::projectile_spawned::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use serde_json::Value;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    enchant::{EffectContext, level_value},
    items::ItemStack,
    projectiles::Projectile,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::worldgen::value_provider::FloatProvider;

impl Game<'_> {
    /// `EnchantmentEntityEffect.apply` on the new projectile `id`, at its
    /// position.
    pub(in crate::game) fn apply_spawned_effect(&mut self, id: u32, effect: &Value, level: u32) {
        let Some(kind) = spawned_effect(effect) else {
            return;
        };
        if let SpawnedEffect::AllOf = kind {
            for e in effect["effects"].as_array().into_iter().flatten() {
                self.apply_spawned_effect(id, e, level);
            }
            return;
        }
        let Some(index) = self.creatures.projectiles.iter().position(|p| p.id == id) else {
            return;
        };
        let sound = self.creatures.projectiles.update(index, |p| match kind {
            SpawnedEffect::Ignite => {
                p.ignite_for_seconds(level_value(&effect["duration"], level));
                None
            }
            // Entity.addDeltaMovement skips a non-finite impulse; the motion
            // is sent either way (syncVelocity).
            SpawnedEffect::Impulse => {
                let impulse = look_impulse(effect, level, p.yaw, p.pitch);
                if impulse.is_finite() {
                    p.velocity = p.velocity.add_vector(impulse);
                }
                p.needs_sync = true;
                None
            }
            // A new projectile is never silent: the volume, then the pitch,
            // from its random, at its position.
            SpawnedEffect::Sound => {
                let sound = level_sound(effect, level)?;
                let volume = float_provider(&effect["volume"])?;
                let pitch = float_provider(&effect["pitch"])?;
                let volume = volume.sample(&mut p.random.0);
                let pitch = pitch.sample(&mut p.random.0);
                Some((
                    sound,
                    [p.position.x, p.position.y, p.position.z],
                    volume,
                    pitch,
                ))
            }
            SpawnedEffect::AllOf | SpawnedEffect::Inert => None,
        });
        if let Some((sound, at, volume, pitch)) = sound {
            self.audio.0.play_on(
                mod_api::SoundSource::Neutral as usize,
                sound,
                Some(at),
                volume,
                pitch,
            );
        }
    }
}
