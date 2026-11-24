#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Arg, Context, Game, R, Target, err, wrap_degrees};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::command_operation;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::holder;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::java_double_text;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::look_at;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::num;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::operation_named;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::playsound_placement;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::take_new_reinforcements;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::text_error;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::living_attributes::AttributeWrite;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::CartHit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    mobs::{Damage, DamageKind, MobEvent, NamedDamage, Who, finalize},
    scoreboard_cmd::{Holder, Tracker},
    settings::SOUND_SOURCES,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::RandomSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::{self, Clientbound as C};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::attribute_map::ValueKind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::damage_source::Catalog as DamageCatalog;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn cmd_particle_client(
        &mut self,
        options: &sunrise_client::particles::ParticleOptions,
        pos: [f64; 3],
        delta: [f64; 3],
        speed: f32,
        count: i32,
    ) {
        if count <= 0 {
            // handleParticleEvent, count 0: one particle, the speed times
            // the delta as velocity (both sent as floats, multiplied as floats).
            self.particles.0.create_with_world(
                Some(&self.world),
                options,
                pos,
                delta.map(|d| f64::from(speed * d as f32)),
            );
        } else {
            for _ in 0..count {
                let r = self.particles.0.random();
                let p = [
                    pos[0] + r.next_gaussian() * delta[0],
                    pos[1] + r.next_gaussian() * delta[1],
                    pos[2] + r.next_gaussian() * delta[2],
                ];
                let v = [
                    r.next_gaussian() * f64::from(speed),
                    r.next_gaussian() * f64::from(speed),
                    r.next_gaussian() * f64::from(speed),
                ];
                if !self
                    .particles
                    .0
                    .create_with_world(Some(&self.world), options, p, v)
                {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "particle_command_tests_client.rs"]
pub(in crate::game) mod particle_command_tests;
