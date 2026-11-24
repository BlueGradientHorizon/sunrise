#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, creatures::seen, persist::EntityUuids};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::creaking::CreakingState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::creaking::Host;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::creaking::Pos;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::creaking::heart_signal;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::creaking::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    chunk_map::{ChunkMap, ChunkMembershipDelta},
    containers::{BlockEntity, CreakingHeart},
    mobs::{
        Mob, MobType,
        creaking_heart::{self, HeartEvent, HeartHost},
    },
    particles::{ParticleOptions, ParticleType},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3,
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::registry::BlockStateId;

impl Game<'_> {
    /// `Creaking.tearDown`: the crumble particles.
    pub(in crate::game) fn creaking_tear_down(&mut self, center: [f64; 3], spread: [f64; 3]) {
        let registry = self.pack.assets.registry;
        let crumble = |this: &mut Self, name: &str, props: &[(&str, &str)], count: i32| {
            let Some(mut state) = registry
                .default_state(name)
                .and_then(|s| registry.state(s).cloned())
            else {
                return;
            };
            for (k, v) in props {
                state.properties.insert((*k).to_owned(), (*v).to_owned());
            }
            let Ok(id) = registry.resolve(&state) else {
                return;
            };
            let Some((sprite, tint)) = this.particle_sprite(id, center.map(|c| c.floor() as i32))
            else {
                return;
            };
            for _ in 0..count {
                let r = &mut this.world.random;
                let p = [
                    center[0] + r.next_gaussian() * spread[0],
                    center[1] + r.next_gaussian() * spread[1],
                    center[2] + r.next_gaussian() * spread[2],
                ];
                this.particles.0.add(
                    &ParticleOptions::block(ParticleType::BlockCrumble, sprite.clone(), tint),
                    p,
                    [0.0; 3],
                );
            }
        };
        crumble(self, "minecraft:pale_oak_wood", &[], 100);
        crumble(
            self,
            "minecraft:creaking_heart",
            &[("creaking_heart_state", "awake")],
            10,
        );
    }
}
