#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, Hit, dragon::FightWho, item_draw::owner_at, view_vector,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::FIREWORK_EXPLODE_EVENT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::Rocket;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::blast_damage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::fireworks_of;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::rocket_draws;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::use_on_passes;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::fireworks::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    block_entity_render::vertex_light,
    collisions::SceneCollisions,
    entity_nbt::RocketNbt,
    items::ItemStack,
    mesher::Vertex,
    mobs::{Damage, Species, Who},
    particles::{FireworkExplosion, ParticleOptions, ParticleType},
    projectiles::{Hit as ProjectileHit, ProjectileWorld},
    random_source,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::RandomSource,
};

impl Game<'_> {
    /// FireworkRocketEntity.handleEntityEvent: use the synchronized item data.
    pub(in crate::game::client_side) fn remote_firework_explosion(&mut self, id: i32) {
        let camera = self.eye_position();
        if let Some(r) = self.rockets.iter().find(|r| r.id == id as u32) {
            let at = [r.position.x, r.position.y, r.position.z];
            let v = [r.velocity.x, r.velocity.y, r.velocity.z];
            self.particles.0.fireworks(
                at,
                v,
                r.explosions.as_ref().clone(),
                camera.unwrap_or(at),
                !r.silent,
            );
        }
    }
    /// The trail (`FireworkRocketEntity.tick` on a client): a spark each tick
    /// at the rocket, drifting against its motion.
    pub(in crate::game::client_side) fn rocket_spark(&mut self, r: &Rocket) {
        let g = |g: &mut sunrise_core::random::LegacyRandom| g.next_gaussian() * 0.05;
        let v = [
            g(&mut self.world.random),
            -r.velocity.y * 0.5,
            g(&mut self.world.random),
        ];
        let at = [r.position.x, r.position.y, r.position.z];
        self.particles
            .0
            .add(&ParticleOptions::simple(ParticleType::Firework), at, v);
    }
}

#[cfg(test)]
#[path = "tests/fireworks_events_tests_client.rs"]
pub(in crate::game) mod events_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::fireworks::tests::*;

    /// A flying rocket's vertices carry its own id for frame generation's and a temporal
    /// upscaler's motion vectors (`Frame::entity_motion`), so the motion matched between
    /// two frames is its flight, not only the camera's.
    #[test]
    #[ignore = "daily"]
    fn a_flying_rocket_opens_a_motion_run_of_its_own() {
        use crate::game::{Audio, Options, Progress, Settings, load_resources};
        use sunrise_core::geometry::Vec3;

        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        let res = loaded.resources;
        let mut game = super::Game::new(
            &res,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
        )
        .unwrap();
        let camera = glam::DVec3::new(0.5, 64.0, 0.5);
        let start = Vec3::new(0.5, 64.0, 4.5);
        game.rockets
            .push(super::Rocket::remote(77, start, Vec3::ZERO));
        let id = crate::game::MOTION_OBJECT | 77;
        // One frame's motion runs: the rocket's vertices open a run under its own id, and
        // a camera-only run (id 0) closes it so nothing drawn after takes its motion.
        game.client.entity_motion.borrow_mut().clear();
        let mut out = Vec::new();
        game.rocket_geometry(camera, 1.0, &mut out, 0..game.rockets.len());
        let marks = game.client.entity_motion.borrow().clone();
        let at = marks
            .iter()
            .position(|m| m.1 == id)
            .expect("the rocket's run");
        assert_eq!(
            marks[at].0, 0,
            "the run starts at the rocket's first vertex"
        );
        assert_eq!(marks.last(), Some(&(out.len() as u32, 0)));
    }
}
