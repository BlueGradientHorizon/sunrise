//! Client-side sculk visuals and darkness effect fade blending.
use super::Game;
use sunrise_client::{
    effects::{EffectInstance, PlayerEffects},
    mobs::Who,
    particles::{ParticleOptions, ParticleType},
    sculk::Context,
};
use sunrise_core::{geometry::Direction, position::BlockPos, random::RandomSource};
use sunrise_world::registry::BlockStateId;

#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game) use crate::game::side_ops::events::{
    TEST_POSTED_EVENT_ORDER, TEST_POSTED_EVENTS,
};

fn center(p: (i32, i32, i32)) -> [f64; 3] {
    [
        f64::from(p.0) + 0.5,
        f64::from(p.1) + 0.5,
        f64::from(p.2) + 0.5,
    ]
}

impl Game<'_> {
    pub(in crate::game) fn client_post_block_game_event(
        &mut self,
        _event: &'static str,
        _pos: [i32; 3],
        _actor: Option<Who>,
        _state: Option<BlockStateId>,
    ) {
    }

    #[allow(dead_code)]
    pub(in crate::game) fn client_post_game_event(
        &mut self,
        _event: &'static str,
        _pos: [f64; 3],
        _ctx: Context,
    ) {
    }

    pub(in crate::game) fn client_sculk_actor(&self) -> Option<Who> {
        None
    }

    // ---- darkness ----

    /// The darkness effect's blend, where this game ticks the player in its
    /// own level (a server's client ticks it in `tick_client_effects`).
    #[allow(dead_code)]
    pub(in crate::game::client_side) fn tick_darkness_blend(&mut self) {
        self.client.darkness_blend =
            darkness_blend_tick(&self.potions.effects, self.client.darkness_blend);
    }
    /// The darkness effect's blend factor at the partial tick.
    pub(in crate::game) fn darkness_blend(&self, partial: f32) -> f32 {
        darkness_blend_at(self.client.darkness_blend, partial)
    }
    /// The fog's view of the darkness effect, while it is on.
    pub(in crate::game::client_side) fn darkness_fog(&self) -> Option<f32> {
        self.potions
            .effects
            .get("darkness")
            .map(|_| self.darkness_blend(self.clock.fraction() as f32))
    }
    /// `LightmapRenderStateExtractor`: (brightness taken off the gamma
    /// option, the pulsing light scale).
    pub(in crate::game::client_side) fn darkness_lighting(&self, partial: f32) -> (f32, f32) {
        let scale = self.darkness_effect_scale();
        let modifier = self.darkness_blend(partial) * scale;
        let pulse = ((self.visual_ticks() as f32 - partial) * std::f32::consts::PI * 0.025).cos()
            * 0.45
            * modifier;
        (modifier, pulse.max(0.0) * scale)
    }

    /// The Darkness Effect option (`darknessEffectScale`), 0..1.
    pub(in crate::game::client_side) fn darkness_effect_scale(&self) -> f32 {
        self.settings.file.double("darknessEffectScale") as f32
    }

    /// `LightmapRenderStateExtractor`: the night vision effect's intensity
    /// (`GameRenderer.nightVisionScale`), or conduit power's water vision.
    pub(in crate::game::client_side) fn night_vision_intensity(&self, partial: f32) -> f32 {
        use mod_api::client::fog::FogType;
        if let Some(e) = self.potions.effects.get("night_vision") {
            // MobEffectInstance.endsWithin(200); -1 is infinite.
            return if e.duration < 0 || e.duration > 200 {
                1.0
            } else {
                0.7 + ((e.duration as f32 - partial) * std::f32::consts::PI * 0.2).sin() * 0.3
            };
        }
        if self.potions.effects.get("conduit_power").is_some()
            && let Some(eye) = self.eye_position()
        {
            let in_water = self.camera_fog_type(eye) == FogType::Water;
            return self.client.fog.water_vision(in_water);
        }
        0.0
    }

    // ---- particles ----

    /// `LevelEvent` 3006: charge cursors moving over sculk, or popping.
    pub(in crate::game) fn sculk_charge_event(&mut self, pos: BlockPos, data: i32) {
        let c = center((pos.x, pos.y, pos.z));
        let count = data >> 6;
        if count > 0 {
            // Client-side (LevelEventHandler / particles): the client random.
            let r = self.particles.0.random();
            if r.next_float() < 0.3 + count as f32 * 0.1 {
                let volume = 0.15 + 0.02 * (count * count) as f32 * r.next_float();
                let pitch = 0.4 + 0.3 * count as f32 * r.next_float();
                self.audio
                    .0
                    .play("minecraft:block.sculk.charge", Some(c), volume, pitch);
            }
            let face_data = data & 63;
            let faces: Vec<(Direction, f32, f64)> = if face_data == 0 {
                Direction::ALL
                    .into_iter()
                    .map(|d| {
                        let roll = if d == Direction::Down {
                            std::f32::consts::PI
                        } else {
                            0.0
                        };
                        let factor = if matches!(d, Direction::Up | Direction::Down) {
                            0.65
                        } else {
                            0.57
                        };
                        (d, roll, factor)
                    })
                    .collect()
            } else {
                sunrise_world::level::sculk::unpack_faces(face_data)
                    .into_iter()
                    .map(|d| {
                        let roll = if d == Direction::Up {
                            std::f32::consts::PI
                        } else {
                            0.0
                        };
                        (d, roll, 0.35)
                    })
                    .collect()
            };
            for (direction, roll, factor) in faces {
                let repetitions = self
                    .particles
                    .0
                    .random()
                    .next_int_bound(count + 1)
                    .unwrap_or(0);
                for _ in 0..repetitions {
                    self.spawn_particle_on_face(pos, direction, roll, factor);
                }
            }
        } else {
            self.audio
                .0
                .play("minecraft:block.sculk.charge", Some(c), 1.0, 1.0);
            let solid = self
                .res
                .facts()
                .get(self.world.block(pos.x, pos.y, pos.z))
                .full_collision;
            let (particles, spread) = if solid { (40, 0.45f32) } else { (20, 0.25f32) };
            let options = ParticleOptions::simple(ParticleType::SculkChargePop);
            for _ in 0..particles {
                // Client-side (LevelEventHandler / particles): the client random.
                let r = self.particles.0.random();
                let v = [
                    2.0 * r.next_float() - 1.0,
                    2.0 * r.next_float() - 1.0,
                    2.0 * r.next_float() - 1.0,
                ];
                let at = [
                    c[0] + f64::from(v[0] * spread),
                    c[1] + f64::from(v[1] * spread),
                    c[2] + f64::from(v[2] * spread),
                ];
                let speed = [
                    f64::from(v[0] * 0.07),
                    f64::from(v[1] * 0.07),
                    f64::from(v[2] * 0.07),
                ];
                self.particles.0.add(&options, at, speed);
            }
        }
    }
    /// `ParticleUtils.spawnParticleOnFace` for a sculk charge.
    pub(in crate::game::client_side) fn spawn_particle_on_face(
        &mut self,
        pos: BlockPos,
        face: Direction,
        roll: f32,
        factor: f64,
    ) {
        let c = center((pos.x, pos.y, pos.z));
        let step = face.normal();
        // Client-side (LevelEventHandler / particles): the client random.
        let r = self.particles.0.random();
        let mut jitter = || r.next_double() - 0.5;
        let mut at = [0.0; 3];
        for i in 0..3 {
            at[i] = c[i]
                + if step[i] == 0 {
                    jitter()
                } else {
                    f64::from(step[i]) * factor
                };
        }
        let mut speed = [0.0; 3];
        for i in 0..3 {
            let s = (r.next_double() * 2.0 - 1.0) * f64::from(0.005f32);
            speed[i] = if step[i] == 0 { s } else { 0.0 };
        }
        self.particles
            .0
            .add(&ParticleOptions::sculk_charge(roll), at, speed);
    }
    /// `LevelEventHandler.levelEvent` 3007: ten shrieks and a local sound.
    pub(in crate::game) fn shriek_event(&mut self, pos: BlockPos) {
        let top = f64::from(pos.y) + 0.5;
        for i in 0..10 {
            self.particles.0.add(
                &ParticleOptions::shriek(i * 5),
                [f64::from(pos.x) + 0.5, top, f64::from(pos.z) + 0.5],
                [0.0; 3],
            );
        }
        let state = self.world.block(pos.x, pos.y, pos.z);
        let waterlogged = self
            .pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get("waterlogged"))
            .map(String::as_str)
            == Some("true");
        if !waterlogged {
            let pitch = 0.6 + self.particles.0.random().next_float() * 0.4;
            self.audio.0.play_unshared(
                "minecraft:block.sculk_shrieker.shriek",
                Some([f64::from(pos.x) + 0.5, top, f64::from(pos.z) + 0.5]),
                2.0,
                pitch,
            );
        }
    }

    /// `Warden.clientDiggingParticles`: thirty crumbs of the block underfoot.
    pub(in crate::game) fn warden_dig_particles(&mut self, at: [f64; 3]) {
        let below = (
            at[0].floor() as i32,
            (at[1] - 0.2).floor() as i32,
            at[2].floor() as i32,
        );
        let state = self.world.block(below.0, below.1, below.2);
        let Some((sprite, tint)) = self.particle_sprite(state, [below.0, below.1, below.2]) else {
            return;
        };
        for _ in 0..30 {
            // Client-side (LevelEventHandler / particles): the client random.
            let r = self.particles.0.random();
            let off = |v: f32| f64::from(v * 1.4 - 0.7);
            let p = [
                at[0] + off(r.next_float()),
                at[1],
                at[2] + off(r.next_float()),
            ];
            let particle =
                sunrise_client::particles::Particle::terrain(r, p, [0.0; 3], sprite.clone(), tint);
            self.particles.0.push(particle);
        }
    }

    /// `RedStoneOreBlock.spawnParticles`: a red dust puff for each face of the
    /// ore that is not against a solid block. Non-axis coordinates draw from
    /// the client level random, x then y then z. It is `level.addParticle`, so
    /// only the local player's own click or step draws it, not a guest's turn.
    pub(in crate::game) fn redstone_ore_particles(&mut self, pos: [i32; 3]) {
        if !matches!(
            &*self.block_name(pos),
            "minecraft:redstone_ore" | "minecraft:deepslate_redstone_ore"
        ) {
            return;
        }
        // Direction.values(): down, up, north, south, west, east.
        const FACES: [[i32; 3]; 6] = [
            [0, -1, 0],
            [0, 1, 0],
            [0, 0, -1],
            [0, 0, 1],
            [-1, 0, 0],
            [1, 0, 0],
        ];
        for step in FACES {
            let next = [pos[0] + step[0], pos[1] + step[1], pos[2] + step[2]];
            let state = self.world.block(next[0], next[1], next[2]);
            if self.res.facts().get(state).solid_render {
                continue;
            }
            let mut at = [0.0f64; 3];
            for axis in 0..3 {
                at[axis] = if step[axis] != 0 {
                    0.5 + 0.5625 * f64::from(step[axis])
                } else {
                    f64::from(self.particles.0.level_random().next_float())
                };
            }
            self.particles.0.add(
                &sunrise_client::particles::ParticleOptions::dust([1.0, 0.0, 0.0], 1.0),
                [
                    f64::from(pos[0]) + at[0],
                    f64::from(pos[1]) + at[1],
                    f64::from(pos[2]) + at[2],
                ],
                [0.0; 3],
            );
        }
    }
}

/// `MobEffects.DARKNESS`'s `setBlendDuration(22)`: its fade in, its fade out,
/// and how long before the end the fade out starts.
pub(in crate::game) const DARKNESS_BLEND: i32 = 22;

/// `MobEffectInstance.BlendState.hasEffect` for the darkness effect.
pub(in crate::game::client_side) fn darkness_target(e: &EffectInstance) -> f32 {
    if e.ends_within(DARKNESS_BLEND) {
        0.0
    } else {
        1.0
    }
}

/// `MobEffectInstance.BlendState.tick` for the darkness effect: from the
/// (previous, current) factors, the next ones. It fades in and out over 22
/// ticks; with no darkness it is gone.
pub(in crate::game::client_side) fn darkness_blend_tick(
    effects: &PlayerEffects,
    (_, current): (f32, f32),
) -> (f32, f32) {
    let next = match effects.get("darkness") {
        Some(e) => {
            let step = 1.0 / DARKNESS_BLEND as f32;
            current + (darkness_target(e) - current).clamp(-step, step)
        }
        None => 0.0,
    };
    (current, next)
}

/// The darkness effect's blend factor at the partial tick, from its (previous, current)
/// factors (`Mth.lerp(partial, previous, current)`).
pub(in crate::game::client_side) fn darkness_blend_at(
    (previous, current): (f32, f32),
    partial: f32,
) -> f32 {
    previous + (current - previous) * partial
}

/// `GameRenderer.nightVisionScale` of another entity's night vision instance with `duration`
/// ticks left: 1, or over its last 200 ticks (`MobEffectInstance.endsWithin(200)`: only -1 is
/// infinite) a flicker on `Mth.sin`'s table, in vanilla's float order.
pub(in crate::game::client_side) fn night_vision_scale(duration: i32, partial: f32) -> f32 {
    if duration == -1 || duration > 200 {
        return 1.0;
    }
    let angle = (duration as f32 - partial) * std::f32::consts::PI * 0.2;
    0.7 + sunrise_core::geometry::trig::sin(f64::from(angle)) * 0.3
}

/// A new darkness instance's `BlendState`: from nothing, or, when the server
/// asked for no blending, at once (`MobEffectInstance.skipBlending`'s
/// `BlendState.setImmediate`).
pub(in crate::game) fn darkness_blend_added(e: &EffectInstance, blend: bool) -> (f32, f32) {
    let factor = if blend { 0.0 } else { darkness_target(e) };
    (factor, factor)
}

#[cfg(test)]
pub(in crate::game) mod shriek_capture_tests {
    use super::*;
    use crate::game::{Audio, Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_client::chunk::Chunk;
    use sunrise_core::{position::ChunkPos, random::LegacyRandom};
    use sunrise_world::level::LevelEvent;

    #[test]
    #[ignore = "daily"]
    fn level_event_shriek_keeps_local_play_and_random_but_not_shared_capture() {
        let root = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources required"),
        );
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
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = Game::new(
            &loaded.resources,
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
        game.build_registries();
        game.set_allow_commands(true);
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        assert!(game.run_command("setblock 4 200 4 sculk_shrieker[waterlogged=false] strict"));
        assert!(game.run_command("setblock 7 200 4 sculk_shrieker[waterlogged=true] strict"));
        assert_eq!(
            game.state_property(game.world.block(4, 200, 4), "waterlogged"),
            Some("false")
        );
        assert_eq!(
            game.state_property(game.world.block(7, 200, 4), "waterlogged"),
            Some("true")
        );
        let clock = game.world.game_time;
        let mut world_random = game.world.random.clone();
        let effect = "minecraft:block.sculk_shrieker.shriek";
        let before = "minecraft:block.note_block.pling";
        let after = "minecraft:block.amethyst_block.chime";
        let dry_state = game.world.block(4, 200, 4);
        let wet_state = game.world.block(7, 200, 4);
        // Server role: does not play 3007 locally or record it into pending_sounds.
        game.start_audio_capture();
        game.server_play_sound_on(4, before, [1.5, 200.0, 4.5], 1.0, 1.0);
        game.level_events.push(LevelEvent::LevelEvent {
            source: None,
            id: 3007,
            pos: BlockPos::new(4, 200, 4),
            data: 0,
        });
        game.level_events.push(LevelEvent::LevelEvent {
            source: None,
            id: 3007,
            pos: BlockPos::new(7, 200, 4),
            data: 0,
        });
        game.server_process_level_events().unwrap();
        game.server_play_sound_on(4, after, [2.5, 200.0, 4.5], 0.5, 1.25);
        assert!(
            game.audio.0.heard.is_empty(),
            "server must not play shriek locally"
        );
        assert_eq!(game.world.game_time, clock);
        assert_eq!(
            game.world.random.next_long(),
            world_random.next_long(),
            "level random continuation"
        );
        let shared = &game.server.pending_sounds;
        assert_eq!(
            shared.iter().map(|s| s.event.as_str()).collect::<Vec<_>>(),
            vec![before, after],
            "shriek level event must not enter pending_sounds"
        );
        for (sound, (position, volume, pitch)) in shared.iter().zip([
            ([1.5, 200.0, 4.5], 1.0, 1.0),
            ([2.5, 200.0, 4.5], 0.5, 1.25),
        ]) {
            assert_eq!(
                (sound.position, sound.volume, sound.pitch, sound.source),
                (position, volume, pitch, 4)
            );
            assert_eq!((sound.entity, sound.except_entity), (None, None));
        }
        // Client role: plays dry shriek locally with client particle RNG and unshared audio, silent when wet.
        let client_settings = Settings::default();
        let client_loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &client_settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::Client,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut client_options = Options::parse(std::iter::empty()).unwrap().unwrap();
        client_options.resources = root.clone();
        client_options.execution_role = crate::game::execution_role::ExecutionRole::Client;
        let mut client = Game::new(
            &client_loaded.resources,
            client_loaded.pack,
            client_loaded.gui.unwrap(),
            client_options,
            client_settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root,
            )),
            None,
        )
        .unwrap();
        client
            .world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        client.world.set(4, 200, 4, dry_state).unwrap();
        client.world.set(7, 200, 4, wet_state).unwrap();
        client.particles.0.requested = Some(Vec::new());
        client.particles.0.seed_randoms(6500, 6501, 6502);
        client.audio.0.capture = Some(Vec::new());
        let mut random = LegacyRandom::new(6500);
        let mut client_world_random = client.world.random.clone();
        client.level_events.push(LevelEvent::LevelEvent {
            source: None,
            id: 3007,
            pos: BlockPos::new(4, 200, 4),
            data: 0,
        });
        client.client_process_level_events().unwrap();
        assert_eq!(
            client.audio.0.heard.get(effect),
            Some(&1),
            "dry effect must play locally on client"
        );
        random.next_float();
        assert_eq!(
            client.particles.0.random().next_long(),
            random.next_long(),
            "dry random continuation"
        );
        client.level_events.push(LevelEvent::LevelEvent {
            source: None,
            id: 3007,
            pos: BlockPos::new(7, 200, 4),
            data: 0,
        });
        client.client_process_level_events().unwrap();
        assert_eq!(
            client.audio.0.heard.get(effect),
            Some(&1),
            "waterlogged effect stays silent"
        );
        assert_eq!(
            client.particles.0.random().next_long(),
            random.next_long(),
            "wet branch consumes no pitch draw"
        );
        let requests = client.particles.0.requested.as_ref().unwrap();
        assert_eq!(requests.len(), 20);
        for (i, (options, position, velocity)) in requests.iter().enumerate() {
            assert_eq!(options.ty, ParticleType::Shriek);
            assert_eq!(options.scale, ((i % 10) * 5) as f32);
            assert_eq!(*position, [if i < 10 { 4.5 } else { 7.5 }, 200.5, 4.5]);
            assert_eq!(*velocity, [0.0; 3]);
        }
        assert_eq!(
            client.world.random.next_long(),
            client_world_random.next_long(),
            "client level random untouched"
        );
        assert!(
            client.audio.0.capture.as_ref().unwrap().is_empty(),
            "client local shriek must not enter capture"
        );
    }
}

// Milking and shearing farm animals; lives here for `TEST_POSTED_EVENTS`.
#[cfg(test)]
#[path = "tests/farm_animal_interaction_tests.rs"]
pub(in crate::game) mod farm_animal_interaction_tests;
