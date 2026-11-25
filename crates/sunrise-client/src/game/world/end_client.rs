#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Dim, Game, Resources, remote::Remote};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::Arriving;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::CrystalDrawFrame;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::END_SPAWN;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::EndFlash;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::EndState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::EyeOfEnder;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::GATEWAY_RADIUS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::GATEWAY_Y;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::GROUND_LIFT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::GatewayArrival;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::GatewayJob;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::PORTAL_TICKET;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::SummonedEye;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::end::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{collections::HashMap, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    containers::BlockEntity,
    environment::Environment,
    gfx::CelestialBody,
    items::ItemStack,
    mesher::Vertex,
    mobs::Who,
    particles::{ParticleOptions, ParticleType},
    projectiles::{Projectile, ProjectileKind},
    vanillagen::VanillaTerrain,
    world_spawn::PlayerSpawnFinder,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3,
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};

impl Game<'_> {
    pub(in crate::game::client_side) fn begin_crystal_geometry(
        &self,
    ) -> Option<CrystalDrawFrame<'_>> {
        if self.crystals.is_empty() {
            return None;
        }
        let model = self.client.crystal_model.as_ref()?;
        let r = self
            .gui
            .atlas
            .sprites
            .get("entity/end_crystal/end_crystal")?;
        Some(CrystalDrawFrame {
            model,
            uv: [r.u0, r.v0, r.u1, r.v1],
            partial: self.clock.fraction() as f32,
        })
    }
    /// LightmapRenderStateExtractor: End flashes add to the sky light factor
    /// unless Hide Lightning Flashes is on (the sky sprite is not gated), a
    /// third of it under a boss bar's world fog.
    pub(in crate::game::client_side) fn end_flash_sky(
        &self,
        sky: &mut sunrise_client::environment::Sky,
        partial: f64,
    ) {
        if self.dimension_type().skybox == sunrise_world::dimension_type::Skybox::End
            && !self.settings.file.bool("hideLightningFlashes")
        {
            let intensity = self.end.flash.intensity(partial as f32);
            sky.sky_light_factor += if self.client.boss_overlay.should_create_world_fog() {
                intensity / 3.0
            } else {
                intensity
            };
        }
    }
    /// `LevelEventHandler` 2003: a shattered eye's item crumbs and two rings
    /// of portal particles drawn in towards it.
    pub(in crate::game) fn eye_break_particles(&mut self, pos: [i32; 3]) {
        let (x, y, z) = (
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]),
            f64::from(pos[2]) + 0.5,
        );
        let crumbs = self.item_particle(&ItemStack::new("minecraft:ender_eye", 1));
        for _ in 0..8 {
            let r = self.particles.0.random();
            let v = [
                r.next_gaussian() * 0.15,
                r.next_double() * 0.2,
                r.next_gaussian() * 0.15,
            ];
            self.particles.0.add(&crumbs, [x, y, z], v);
        }
        let portal = ParticleOptions::simple(ParticleType::Portal);
        let mut angle = 0.0f64;
        while angle < std::f64::consts::PI * 2.0 {
            let (c, s) = (angle.cos(), angle.sin());
            let at = [x + c * 5.0, y - 0.4, z + s * 5.0];
            self.particles.0.add(&portal, at, [c * -5.0, 0.0, s * -5.0]);
            self.particles.0.add(&portal, at, [c * -7.0, 0.0, s * -7.0]);
            angle += std::f64::consts::PI / 20.0;
        }
    }
    /// `EyeOfEnder.tick` on a client: behind each eye (a quarter of its last
    /// move back), four bubbles in water, else a portal particle.
    pub(in crate::game::client_side) fn eye_particles(&mut self) {
        for i in 0..self.end.eyes.len() {
            let e = &self.end.eyes[i];
            let movement = (e.position - e.previous).to_array();
            let origin = (e.position - (e.position - e.previous) * 0.25).to_array();
            let block = origin.map(|v| v.floor() as i32);
            let water = self
                .res
                .facts()
                .get(self.world.block(block[0], block[1], block[2]))
                .fluid
                == sunrise_client::blocks::Fluid::Water;
            if water {
                let bubble = ParticleOptions::simple(ParticleType::Bubble);
                for _ in 0..4 {
                    self.particles.0.add(&bubble, origin, movement);
                }
            } else {
                let r = self.particles.0.random();
                let x = origin[0] + r.next_double() * 0.6 - 0.3;
                let z = origin[2] + r.next_double() * 0.6 - 0.3;
                let portal = ParticleOptions::simple(ParticleType::Portal);
                self.particles
                    .0
                    .add(&portal, [x, origin[1] - 0.5, z], movement);
            }
        }
    }
    /// EnderEyeItem.use: findNearestMapStructure over the stronghold rings,
    /// then an EyeOfEnder signalled towards it. Without a stronghold (other
    /// dimensions) nothing is thrown.
    pub(in crate::game) fn throw_ender_eye(&mut self) {
        let rings = self
            .end
            .locator
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|t| t.ring_positions("minecraft:strongholds"));
        let (Some(rings), Some(p)) = (rings, self.player.as_ref()) else {
            return;
        };
        let block = [
            p.position.x.floor(),
            p.position.y.floor(),
            p.position.z.floor(),
        ];
        // ChunkGenerator.getNearestGeneratedStructure: the ring chunk closest to
        // its (8, 32, 8) block, located at the chunk's corner.
        let Some(nearest) = rings.iter().min_by(|a, b| {
            let d = |c: &sunrise_core::position::ChunkPos| {
                (f64::from(c.x * 16 + 8) - block[0]).powi(2)
                    + (32.0 - block[1]).powi(2)
                    + (f64::from(c.z * 16 + 8) - block[2]).powi(2)
            };
            d(a).total_cmp(&d(b))
        }) else {
            return;
        };
        let target = glam::DVec3::new(f64::from(nearest.x * 16), 0.0, f64::from(nearest.z * 16));
        let start = glam::DVec3::new(
            p.position.x,
            p.position.y + f64::from(p.dimensions.height) * 0.5,
            p.position.z,
        );
        // EyeOfEnder.signalTo.
        let delta = target - start;
        let horizontal = sunrise_core::geometry::length(delta.x, delta.z);
        let target = if horizontal > 12.0 {
            start
                + glam::DVec3::new(
                    delta.x / horizontal * 12.0,
                    8.0,
                    delta.z / horizontal * 12.0,
                )
        } else {
            target
        };
        let survive = self.world.random.next_int_bound(5).unwrap_or(0) > 0;
        let at = [p.position.x, p.position.y, p.position.z];
        let ctx = self.player_context(None);
        self.post_game_event(
            "minecraft:projectile_shoot",
            [start.x, start.y, start.z],
            ctx,
        );
        let id = self.take_entity_id();
        self.end.eyes.push(EyeOfEnder {
            id,
            position: start,
            previous: start,
            velocity: glam::DVec3::ZERO,
            target,
            life: 0,
            survive,
        });
        let r = self.world.random.next_float();
        let pitch = 0.33 + r * (0.5 - 0.33);
        self.server_play_sound("minecraft:entity.ender_eye.launch", at, 1.0, pitch);
        // CriteriaTriggers.USED_ENDER_EYE, against the feature's block.
        let (dx, dz) = (
            at[0] - f64::from(nearest.x * 16),
            at[2] - f64::from(nearest.z * 16),
        );
        self.trigger_used_ender_eye(dx * dx + dz * dz);
        self.consume_held();
        self.award_stat("used", "minecraft:ender_eye", 1);
        self.swing_arm();
    }
    /// `ClientLevel.tick`'s End flash, on every client in the End:
    /// `EndFlashState.tick`, and when a flash starts (not under the credits)
    /// its `DirectionalSoundInstance`, played 30 ticks later from the camera
    /// toward the flash.
    pub(in crate::game::client_side) fn tick_end_flash(&mut self) {
        if self.dimension_type().skybox != sunrise_world::dimension_type::Skybox::End {
            return;
        }
        // getDefaultClockTime: the End's own clock (the client keeps the
        // dimension's default clock in day_time), not the game time.
        self.end.flash.tick(self.world.day_time);
        if self.end.flash.started_this_tick() && self.client.win.is_none() {
            let flash = &self.end.flash;
            let instance = sunrise_client::sound_engine::SoundInstance::directional(
                "minecraft:weather.end_flash",
                sunrise_client::sound_engine::WEATHER,
                sunrise_client::random_source::create(),
                self.audio.0.listener,
                flash.x_angle,
                flash.y_angle,
            );
            self.audio.0.play_instance_after(instance, 30);
        }
    }
    /// EyeOfEnder.tick: fly, then drop (4 in 5) or shatter after 80 ticks.
    pub(in crate::game) fn tick_eyes(&mut self) {
        let _save_lease = sunrise_client::join_seq::visibility::PassLease::enter();
        let mut eyes = std::mem::take(&mut self.end.eyes);
        let mut shattered = Vec::new();
        eyes.retain_mut(|eye| {
            eye.previous = eye.position;
            let next = eye.position + eye.velocity;
            eye.velocity = EyeOfEnder::update_delta(eye.velocity, next, eye.target);
            eye.position = next;
            // Its move is filed at once (told with the dragon's step), also for one about to go.
            self.entity_section_step(eye.id, eye.previous.to_array(), next.to_array());
            eye.life += 1;
            if eye.life <= 80 {
                return true;
            }
            let at = eye.position.to_array();
            self.server_play_sound("minecraft:entity.ender_eye.death", at, 1.0, 1.0);
            if eye.survive {
                // new ItemEntity(level, x, y, z, item): random spread, 0.2 up.
                let vx = self.world.random.next_double() * 0.2 - 0.1;
                let vz = self.world.random.next_double() * 0.2 - 0.1;
                let stack = ItemStack {
                    item: "minecraft:ender_eye".into(),
                    count: 1,
                    damage: 0,
                    extra: None,
                };
                self.spawn_item(stack, at, [vx, 0.2, vz], 0);
            } else {
                shattered.push(at.map(|v| v.floor() as i32));
            }
            false
        });
        self.end.eyes = eyes;
        for pos in shattered {
            self.level_event_at(2003, pos, 0);
        }
    }
}
