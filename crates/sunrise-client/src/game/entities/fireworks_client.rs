use super::{Game, item_draw::owner_at};
#[cfg(test)]
use crate::game::fireworks::FIREWORK_EXPLODE_EVENT;
use crate::game::fireworks::Rocket;
use sunrise_assets::item_model::DisplayContext;
use sunrise_client::{block_entity_render::vertex_light, mesher::Vertex};
use sunrise_core::random::RandomSource;
use sunrise_level::particle_options::{ParticleOptions, ParticleType};

impl Game<'_, crate::game::client_side::ClientState> {
    /// `FireworkEntityRenderer.submit`: ground item facing camera, lit at the rocket's probe.
    pub(in crate::game) fn rocket_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
        run: std::ops::Range<usize>,
    ) {
        let orientation = glam::Mat4::from_quat(self.camera_orientation());
        // shouldRender: not while attached to an entity (`isAttachedToEntity`).
        for r in self.rockets[run]
            .iter()
            .filter(|r| !r.attached && r.attached_to.is_none() && !self.hides_as_camera(r.id))
        {
            // FireworkRocketEntity.shouldRenderAtSqrDistance: 0.25 box within 64 blocks.
            if !self.standing_entity_in_view(
                camera,
                super::entity_culling::at(r.previous),
                super::entity_culling::at(r.position),
                alpha,
                0.25,
                0.25,
                64.0,
                0.0,
            ) {
                continue;
            }
            let p = glam::DVec3::new(
                r.previous.x + (r.position.x - r.previous.x) * alpha,
                r.previous.y + (r.position.y - r.previous.y) * alpha,
                r.previous.z + (r.position.z - r.previous.z) * alpha,
            );
            // `updateForNonLiving`: GROUND, owned by the rocket.
            let owner = owner_at(p.to_array(), 0.0, "minecraft:firework_rocket");
            let state = self.resolve_item(&r.item, DisplayContext::Ground, Some(&owner), 0);
            if state.is_empty() {
                continue;
            }
            let mut pose = glam::Mat4::from_translation((p - camera).as_vec3()) * orientation;
            if r.angled {
                pose *= glam::Mat4::from_rotation_z(std::f32::consts::PI)
                    * glam::Mat4::from_rotation_y(std::f32::consts::PI)
                    * glam::Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2);
            }
            // getLightProbePosition: the eye, 0.85 of its 0.25 height.
            let light = self.world.light(
                p.x.floor() as i32,
                (p.y + 0.2125).floor() as i32,
                p.z.floor() as i32,
            );
            // Frame generation and temporal upscaling follow its own flight.
            self.mark_motion(out.len(), crate::game::MOTION_OBJECT | u64::from(r.id));
            self.submit_item_world(&state, pose, vertex_light(light), out);
        }
        self.mark_motion(out.len(), 0);
    }
    /// FireworkRocketEntity.handleEntityEvent: use the synchronized item data.
    pub(in crate::game::client_side) fn remote_firework_explosion(&mut self, id: i32) {
        let camera = self.eye_position();
        if let Some(r) = self.rockets.iter().find(|r| r.id == id as u32) {
            let at = [r.position.x, r.position.y, r.position.z];
            let v = [r.velocity.x, r.velocity.y, r.velocity.z];
            self.side.particles.0.fireworks(
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
        self.side
            .particles
            .0
            .add(&ParticleOptions::simple(ParticleType::Firework), at, v);
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {

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
            crate::game::execution_role::ExecutionRole::Client,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        let res = loaded.resources;
        let mut game = crate::game::ClientGame::new(
            &res,
            loaded.client_res.unwrap(),
            loaded.pack,
            loaded.client_pack.unwrap(),
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.clone(),
            )),
            None,
            crate::game::server_side::integrated::serve,
            crate::game::server_side::integrated::run_component_harness_on_server,
            crate::game::session_verifier(),
            crate::game::profile_resolver,
        )
        .unwrap();
        let camera = glam::DVec3::new(0.5, 64.0, 0.5);
        let start = Vec3::new(0.5, 64.0, 4.5);
        game.rockets
            .push(super::Rocket::remote(77, start, Vec3::ZERO));
        let id = crate::game::MOTION_OBJECT | 77;
        // One frame's motion runs: the rocket's vertices open a run under its own id, and
        // a camera-only run (id 0) closes it so nothing drawn after takes its motion.
        game.side.entity_motion.borrow_mut().clear();
        let mut out = Vec::new();
        game.rocket_geometry(camera, 1.0, &mut out, 0..game.rockets.len());
        let marks = game.side.entity_motion.borrow().clone();
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

    pub(in crate::game) fn assert_client_silent_rocket_burst_without_blast_or_twinkle(
        explosion: sunrise_client::particles::FireworkExplosion,
    ) {
        use sunrise_client::particles::ParticleLevel;
        use sunrise_core::geometry::Vec3;
        use sunrise_protocol::packets::play::{Clientbound as C, EntityEvent};
        crate::game::native_entity_refs::tests::with_client_game(|game| {
            game.player = Some(
                sunrise_sim::motion::PlayerMotion::new(game.motion, Vec3::new(2.5, 200.0, 2.5))
                    .unwrap(),
            );
            let (quiet_at, loud_at) = ([6.5, 201.5, 10.5], [12.5, 201.5, 10.5]);
            let mut quiet = super::Rocket::remote(
                101,
                Vec3::new(quiet_at[0], quiet_at[1], quiet_at[2]),
                Vec3::ZERO,
            );
            quiet.silent = true;
            quiet.explosions = vec![explosion.clone()].into();
            let mut loud = super::Rocket::remote(
                102,
                Vec3::new(loud_at[0], loud_at[1], loud_at[2]),
                Vec3::ZERO,
            );
            loud.explosions = vec![explosion].into();
            game.rockets.push(quiet);
            game.rockets.push(loud);
            for id in [101, 102] {
                game.apply_packet(C::EntityEvent(EntityEvent {
                    id,
                    event: super::FIREWORK_EXPLODE_EVENT,
                }))
                .unwrap();
            }
            game.side.particles.0.sounds.clear();
            for _ in 0..20 {
                let level = ParticleLevel {
                    world: &game.world,
                    facts: game.res.facts(),
                    collision: game.collision,
                    player: None,
                    cached_chunk: std::cell::Cell::new(None),
                };
                game.side.particles.0.tick(&level);
            }
            let bursts: Vec<(&str, [f64; 3])> = game
                .side
                .particles
                .0
                .sounds
                .iter()
                .filter(|s| s.0.starts_with("minecraft:entity.firework_rocket."))
                .map(|s| (s.0, s.1))
                .collect();
            assert_eq!(
                bursts,
                vec![
                    ("minecraft:entity.firework_rocket.blast", loud_at),
                    ("minecraft:entity.firework_rocket.twinkle", loud_at),
                ],
                "only the loud rocket bangs and twinkles, both at its own position"
            );
            assert!(
                !game.side.particles.0.is_empty(),
                "the silent rocket still drew its sparks and flash"
            );
        });
    }
}
