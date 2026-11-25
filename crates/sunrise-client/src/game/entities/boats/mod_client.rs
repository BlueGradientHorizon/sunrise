#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, InteractTarget, entity::Entity, entity_culling::at, view_vector,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::BUBBLE_DOWN_EVENT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::BUBBLE_TIME;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::BUBBLE_UP_EVENT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::Boat;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::BoatModels;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::CHEST_SLOTS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::DrivenLerp;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::HEIGHT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::LERP_STEPS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::Status;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::WIDTH;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::any_boat_wood;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::put_ticks_frozen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::saved_ticks_frozen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::boats::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{collections::HashMap, f32::consts::PI, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::entity_rules::EntityRules;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    blocks::Fluid,
    collisions::{EntityCollisions, SceneCollisions, no_collision_with_context},
    items::ItemStack,
    mesher::Vertex,
    mobs::{
        Part,
        anim::Poses,
        leash::{Holder, Pending},
        riders,
    },
    motion::{BubbleColumn, RiddenBoat, bubble_column_at},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, ShapeOp, Vec3, VoxelShape},
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::collision_context::CollisionContext;

impl Game<'_> {
    /// `AbstractBoat.tick` advances the oars even when this client does not move the hull.
    pub(in crate::game::client_side) fn tick_boat_paddles(b: &mut Boat, audio: &mut super::Audio) {
        // Paddle sounds at the top of each stroke.
        for i in 0..2 {
            if b.paddle_state(i) {
                let p = b.paddles[i] % (2.0 * PI);
                if p <= PI / 4.0 && (b.paddles[i] + PI / 8.0) % (2.0 * PI) >= PI / 4.0 {
                    let sound = match b.status {
                        Status::InWater | Status::UnderWater | Status::UnderFlowingWater => {
                            Some("minecraft:entity.boat.paddle_water")
                        }
                        Status::OnLand => Some("minecraft:entity.boat.paddle_land"),
                        Status::InAir => None,
                    };
                    if let Some(sound) = sound {
                        let v = view_vector(b.yaw, 0.0);
                        let (dx, dz) = if i == 1 { (-v[2], v[0]) } else { (v[2], -v[0]) };
                        let pitch = 0.8 + 0.4 * b.random.next_float();
                        audio.play(
                            sound,
                            Some([b.position.x + dx, b.position.y, b.position.z + dz]),
                            1.0,
                            pitch,
                        );
                    }
                }
                b.paddles[i] += PI / 8.0;
            } else {
                b.paddles[i] = 0.0;
            }
        }
    }
    /// `AbstractBoat.onAboveBubbleColumn`: the client's half after the roll,
    /// the splash sound and particle at the boat.
    pub(in crate::game::client_side) fn boat_bubble_splash(&mut self, i: usize) {
        use sunrise_client::particles::{ParticleOptions, ParticleType};
        let p = self.boats[i].position;
        let pitch = 0.8 + 0.4 * self.world.random.next_float();
        self.audio.0.play(
            "minecraft:entity.generic.splash",
            Some([p.x, p.y, p.z]),
            1.0,
            pitch,
        );
        let x = p.x + f64::from(self.world.random.next_float());
        let z = p.z + f64::from(self.world.random.next_float());
        self.particles.0.add(
            &ParticleOptions::simple(ParticleType::Splash),
            [x, p.y + 0.7, z],
            [0.0; 3],
        );
    }
    /// AbstractBoatRenderer.
    pub(in crate::game) fn boat_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
        run: std::ops::Range<usize>,
    ) {
        let a = alpha as f32;
        let range = super::entity_culling::default_range(
            glam::DVec3::new(WIDTH, HEIGHT, WIDTH),
            self.view_scale(),
        );
        for b in &self.boats[run] {
            // LevelExtractor: the camera entity is not drawn from its own eyes.
            if self.hides_as_camera(b.id) {
                continue;
            }
            // EntityRenderer.shouldRender.
            let (previous, position) = (at(b.previous), at(b.position));
            if !self.standing_entity_in_view(
                camera, previous, position, alpha, WIDTH, HEIGHT, range, 0.0,
            ) {
                continue;
            }
            let culling = self.entity_cull_decision(b, alpha);
            if culling.hides_entity() {
                continue;
            }
            let entity_start = out.len();
            self.mark_motion(entity_start, crate::game::MOTION_OBJECT | u64::from(b.id));
            let models = if b.chest {
                &self.client.boat_models.chest_models
            } else {
                &self.client.boat_models.models
            };
            let Some(model) = models.get(&*b.wood) else {
                continue;
            };
            let folder = if b.chest { "chest_boat" } else { "boat" };
            let Some(uv) = self
                .gui
                .atlas
                .sprites
                .get(&format!("entity/{folder}/{}", b.wood))
                .map(|r| [r.u0, r.v0, r.u1, r.v1])
            else {
                continue;
            };
            let p = glam::DVec3::new(
                b.previous.x + (b.position.x - b.previous.x) * alpha,
                b.previous.y + (b.position.y - b.previous.y) * alpha,
                b.previous.z + (b.position.z - b.previous.z) * alpha,
            ) - camera;
            let yaw = b.previous_yaw + (b.yaw - b.previous_yaw) * a;
            let hurt = b.hurt_time as f32 - a;
            let damage = (b.damage - a).max(0.0);
            let mut base =
                glam::Mat4::from_translation(p.as_vec3() + glam::Vec3::new(0.0, 0.375, 0.0))
                    * glam::Mat4::from_rotation_y((180.0 - yaw).to_radians());
            if hurt > 0.0 {
                base *= glam::Mat4::from_rotation_x(
                    (hurt.sin() * hurt * damage / 10.0 * b.hurt_dir as f32).to_radians(),
                );
            }
            // AbstractBoatRenderer.extractRenderState: bubbleAngle.
            let wobble = b.bubble_angle(a);
            // AbstractBoatRenderer.submit: a boat above water wobbles in a
            // bubble column, about the axis (1, 0, 1).
            if !b.under_water() && wobble.abs() >= 1.0e-5 {
                base *= glam::Mat4::from_axis_angle(
                    glam::Vec3::new(1.0, 0.0, 1.0).normalize(),
                    wobble * (std::f64::consts::PI / 180.0) as f32,
                );
            }
            base *= glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0))
                * glam::Mat4::from_rotation_y(90f32.to_radians());
            let mut poses = Poses::new(model);
            // AbstractBoatModel.animatePaddle
            for (side, name) in [(0usize, "left_paddle"), (1, "right_paddle")] {
                if !poses.has(name) {
                    continue;
                }
                let t = b.rowing_time(side, a);
                let lerp = |f: f32, lo: f32, hi: f32| lo + f.clamp(0.0, 1.0) * (hi - lo);
                let pose = poses.get(name);
                pose[3] = lerp(((-t).sin() + 1.0) / 2.0, -PI / 3.0, -PI / 12.0);
                pose[4] = lerp(((-t + 1.0).sin() + 1.0) / 2.0, -PI / 4.0, PI / 4.0);
                if side == 1 {
                    pose[4] = PI - pose[4];
                }
            }
            let l = self.world.light(
                b.position.x.floor() as i32,
                (b.position.y + 0.3).floor() as i32,
                b.position.z.floor() as i32,
            );
            let packed = (u32::from(l >> 4) * 16) | ((u32::from(l & 15) * 16) << 8);
            sunrise_client::mobs::render::draw(
                model,
                base,
                &poses,
                uv,
                0xFFFF_FFFF,
                packed,
                0,
                out,
            );
            // BoatRenderer.submitTypeAdditions: the water patch hides the water
            // inside the hull unless the boat is under water (RaftRenderer has none).
            if let Some(patch) = self.client.boat_models.water_patch.as_ref()
                && &*b.wood != "bamboo"
                && !matches!(b.status, Status::UnderWater | Status::UnderFlowingWater)
            {
                // Depth only, so its texture is never seen; one texel of the white sprite, the
                // same at every corner (no derivatives, so the base level at any distance), keeps
                // a shader pack's alpha test from throwing the patch away.
                let white = (self.gui.atlas.sprites.get("sunrise/white")).map_or(uv, |r| {
                    let (u, v) = ((r.u0 + r.u1) / 2.0, (r.v0 + r.v1) / 2.0);
                    [u, v, u, v]
                });
                sunrise_client::mobs::render::draw(
                    patch,
                    base,
                    &Poses::new(patch),
                    white,
                    0xFFFF_FFFF,
                    packed,
                    sunrise_client::gfx::WATER_MASK_OVERLAY,
                    out,
                );
            }
            self.finish_entity_occlusion(culling, b, out, entity_start);
        }
    }
    /// The horizontal water plane around the camera for the water mirror: the median
    /// standing-water surface of a grid of columns 48 blocks around, each scanned down from
    /// the camera to its ground. A lake or the sea has one plane; the mirror shows only the
    /// water on it. With no water in reach the last plane found stays, so the mirror never
    /// switches off as the camera moves.
    pub(in crate::game::client_side) fn water_plane(&self, camera: glam::DVec3) -> Option<f32> {
        const RADIUS: i32 = 48;
        const STEP: usize = 4;
        const DEPTH: i32 = 96;
        let (cx, cz) = (camera.x.floor() as i32, camera.z.floor() as i32);
        let top = camera.y.floor() as i32 + 2;
        // Scanned again once the camera moves 8 blocks, or every 5 seconds: a scan reads up
        // to 60 000 blocks on the game thread, and the plane rarely changes.
        let scan = (
            [cx >> 3, top >> 3, cz >> 3],
            self.world.game_time.div_euclid(100),
        );
        if self.client.water_plane_scan.replace(Some(scan)) == Some(scan) {
            return self.client.water_plane_memo.get();
        }
        let mut heights = Vec::with_capacity(625);
        for x in (cx - RADIUS..=cx + RADIUS).step_by(STEP) {
            for z in (cz - RADIUS..=cz + RADIUS).step_by(STEP) {
                // The top of the column's water: the highest standing water under the
                // camera (a full block's water continues above it).
                for y in (top - DEPTH..=top).rev() {
                    if let Some((h, source)) = self.fluid_height(x, y, z) {
                        if source && h < 1.0 {
                            heights.push(y as f32 + h);
                        }
                        break;
                    }
                    if self.res.facts().get(self.world.block(x, y, z)).solid_render {
                        break;
                    }
                }
            }
        }
        if heights.is_empty() {
            return self.client.water_plane_memo.get();
        }
        heights.sort_by(f32::total_cmp);
        let found = heights[heights.len() / 2];
        // The plane in use stays while a quarter of the columns are still on it, so two
        // lakes at different heights do not make the mirror flicker between them.
        let plane = match self.client.water_plane_memo.get() {
            Some(last)
                if 4 * heights.iter().filter(|h| (*h - last).abs() < 0.05).count()
                    >= heights.len() =>
            {
                last
            }
            _ => found,
        };
        self.client.water_plane_memo.set(Some(plane));
        Some(plane)
    }
}
