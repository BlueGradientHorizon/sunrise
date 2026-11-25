#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, Hit, InteractTarget, Keys, entity::Entity};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::CART_KEY_Y;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::CartIndex;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::CartKind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::CartModels;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::DEFAULT_EXPLOSION_POWER;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::DEFAULT_EXPLOSION_SPEED_FACTOR;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::HEIGHT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::Minecart;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::PreparedModelFace;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::Rail;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::SEAT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::Shape;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::TNT_PRIME_EVENT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::WIDTH;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::can_ride_cart;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::cart_at;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::cart_general_seat;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::cart_is_destroyed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::cart_push_from;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::cart_render_bounds;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::cart_seat;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::container_keep;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::default_display_offset;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::display_offset;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::emit_model_faces;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::forget_unreadable_display;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::furnace_fuel_after;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::furnace_speed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::hit::CartHit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::java_degrees;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::offered_pickup;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::pickup_instead_of_push;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::push_along;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::scale_tint_alpha;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::shown_unless_air;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::takes_push_from_cart;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::tnt_ignites_by_collision;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{collections::HashMap, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::entity_rules::EntityRules;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::join_seq::{JoinList, ListStamp};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{Mob, seen_grid::SeenGrid};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    blocks::Fluid,
    containers::BlockEntity,
    items::{ItemStack, Items},
    mesher::Vertex,
    mobs::{Part, Species, Who, anim::Poses, riders},
    particles::{ParticleOptions, ParticleType},
    random_source,
    scoreboard::pushable_by,
    sound_engine::{BLOCKS, CartSound, PlayResult, SoundInstance, Sounds},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3, trig},
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::minecart_cells::CartCells;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::minecart_seat::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::registry::BlockStateId;

impl Game<'_> {
    /// `AbstractMinecartRenderer` (old render): on the rail between the
    /// front and back points, pitched on slopes, with the display block.
    pub(in crate::game::client_side) fn cart_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
        run: std::ops::Range<usize>,
    ) {
        let a = alpha as f32;
        let Some(uv) = self
            .gui
            .atlas
            .sprites
            .get("entity/minecart/minecart")
            .map(|r| [r.u0, r.v0, r.u1, r.v1])
        else {
            return;
        };
        // Entity.shouldRenderAtSqrDistance: 64 times the bounding box's mean
        // edge, scaled by the view scale.
        let range = (WIDTH + HEIGHT + WIDTH) / 3.0 * 64.0 * self.view_scale();
        for c in &self.carts[run] {
            if self.hides_as_camera(c.id) {
                continue;
            }
            let Some(model) = self.client.cart_models.models.get(&c.kind) else {
                continue;
            };
            let lerp = self.remote_minecart_lerp(c.id, a);
            let new_render = self.minecart_improvements() || lerp.is_some();
            let (x, y, z) = if let Some((p, _, _)) = lerp {
                (p[0], p[1], p[2])
            } else {
                (
                    c.previous.x + (c.position.x - c.previous.x) * alpha,
                    c.previous.y + (c.position.y - c.previous.y) * alpha,
                    c.previous.z + (c.position.z - c.previous.z) * alpha,
                )
            };
            // LevelExtractor.isEntityVisible: the cart the player rides always draws.
            let mut shown = None;
            if self.riding != Some(c.id) {
                let d = glam::DVec3::new(c.position.x, c.position.y, c.position.z) - camera;
                if d.length_squared() >= range * range {
                    continue;
                }
                // Looked up once, past the distance cut: the cull and the draw both use it.
                shown = Some(self.display_state(c));
                // EntityRenderer.shouldRender: the interpolated box grown by 0.5;
                // AbstractMinecartRenderer.getBoundingBoxForCulling stretches it
                // up by the display block's offset.
                if let Some(view) = &self.client.view_frustum {
                    let top = HEIGHT
                        + if shown.flatten().is_none() {
                            0.0
                        } else {
                            display_offset(c) * 0.75 / 16.0
                        };
                    let p = glam::DVec3::new(x, y, z) - camera;
                    let lo = p - glam::DVec3::new(WIDTH / 2.0 + 0.5, 0.5, WIDTH / 2.0 + 0.5);
                    let hi = p + glam::DVec3::new(WIDTH / 2.0 + 0.5, top + 0.5, WIDTH / 2.0 + 0.5);
                    if !view.intersects(lo.as_vec3(), hi.as_vec3()) {
                        continue;
                    }
                }
            }
            let mut yaw =
                c.previous_yaw + ((c.yaw - c.previous_yaw + 180.0).rem_euclid(360.0) - 180.0) * a;
            let mut pitch = 0.0f32;
            let mut offset = glam::DVec3::ZERO;
            if new_render {
                (yaw, pitch) = lerp.map_or((c.yaw, c.pitch), |(_, yaw, pitch)| (yaw, pitch));
            } else if let Some(on) = self.rail_pos(Some(c.id), x, y, z) {
                // getPosOffs(..., 0.3F): the float widened.
                let half = f64::from(0.3_f32);
                let front = self.rail_pos_offs(Some(c.id), x, y, z, half).unwrap_or(on);
                let back = self.rail_pos_offs(Some(c.id), x, y, z, -half).unwrap_or(on);
                offset = glam::DVec3::new(on[0] - x, (front[1] + back[1]) / 2.0 - y, on[2] - z);
                let d =
                    glam::DVec3::new(back[0] - front[0], back[1] - front[1], back[2] - front[2]);
                if d.length() != 0.0 {
                    let d = d.normalize();
                    yaw = d.z.atan2(d.x).to_degrees() as f32;
                    pitch = (d.y.atan() * 73.0) as f32;
                }
            }
            let render_position = Vec3::new(x + offset.x, y + offset.y, z + offset.z);
            let has_display = shown.as_ref().is_some_and(Option::is_some)
                || (shown.is_none() && self.display_state(c).is_some());
            let culling = self.entity_cull_decision_in_bounds(
                c,
                cart_render_bounds(render_position, display_offset(c), has_display),
            );
            if culling.hides_entity() {
                continue;
            }
            let entity_start = out.len();
            self.mark_motion(entity_start, Self::MOTION_OBJECT | u64::from(c.id));
            // The per-entity jitter (offsetSeed) that keeps stacked carts from z-fighting.
            let seed = i64::from(c.id).wrapping_mul(493_286_711);
            let seed = seed
                .wrapping_mul(seed)
                .wrapping_mul(4_392_167_121)
                .wrapping_add(seed.wrapping_mul(98_761));
            let jitter = |shift: i64| (((seed >> shift & 7) as f32 + 0.5) / 8.0 - 0.5) * 0.004;
            let p = glam::DVec3::new(x, y, z) + offset - camera;
            let jittered = p.as_vec3() + glam::Vec3::new(jitter(16), jitter(20), jitter(24));
            let mut base = if new_render {
                // AbstractMinecartRenderer.newRender translates the model
                // after rotating, so the seat follows the pitched cart.
                glam::Mat4::from_translation(jittered)
                    * glam::Mat4::from_rotation_y(yaw.to_radians())
                    * glam::Mat4::from_rotation_z((-pitch).to_radians())
                    * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.375, 0.0))
            } else {
                glam::Mat4::from_translation(jittered + glam::Vec3::new(0.0, 0.375, 0.0))
                    * glam::Mat4::from_rotation_y((180.0 - yaw).to_radians())
                    * glam::Mat4::from_rotation_z((-pitch).to_radians())
            };
            let hurt = c.hurt_time as f32 - a;
            if hurt > 0.0 {
                let damage = (c.damage - a).max(0.0);
                base *= glam::Mat4::from_rotation_x(
                    (hurt.sin() * hurt * damage / 10.0 * c.hurt_dir as f32).to_radians(),
                );
            }
            let l = self.world.light(
                c.position.x.floor() as i32,
                (c.position.y + 0.35).floor() as i32,
                c.position.z.floor() as i32,
            );
            let packed = (u32::from(l >> 4) * 16) | ((u32::from(l & 15) * 16) << 8);
            if let Some(state) = shown.unwrap_or_else(|| self.display_state(c)) {
                let display = base
                    * glam::Mat4::from_scale(glam::Vec3::splat(0.75))
                    * glam::Mat4::from_translation(glam::Vec3::new(
                        -0.5,
                        (display_offset(c) as f32 - 8.0) / 16.0,
                        0.5,
                    ))
                    * glam::Mat4::from_rotation_y(90f32.to_radians());
                if c.kind == CartKind::Chest {
                    self.special_block_model(state, display, l, out);
                } else {
                    // TNT minecarts flash white as the fuse runs out (TntMinecartRenderer).
                    let swell = if c.kind == CartKind::Tnt && c.fuse > -1 {
                        let fuse = c.fuse as f32 - a + 1.0;
                        if fuse < 10.0 {
                            let g = (1.0 - fuse / 10.0).clamp(0.0, 1.0);
                            1.0 + g * g * g * g * 0.3
                        } else {
                            1.0
                        }
                    } else {
                        1.0
                    };
                    let m = display
                        * glam::Mat4::from_translation(glam::Vec3::splat(0.5))
                        * glam::Mat4::from_scale(glam::Vec3::splat(swell))
                        * glam::Mat4::from_translation(glam::Vec3::splat(-0.5));
                    let flash = c.kind == CartKind::Tnt && c.fuse > -1 && (c.fuse / 5) % 2 == 0;
                    let start = out.len();
                    self.block_model_vertices(state, 0, m, packed, false, out);
                    if flash {
                        // submitWhiteSolidBlock: OverlayTexture.u(1.0) on the cart's own light.
                        for v in &mut out[start..] {
                            v.layer = 5;
                        }
                    }
                }
            }
            let m = base * glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0));
            let poses = Poses::new(model);
            sunrise_client::mobs::render::draw(model, m, &poses, uv, 0xFFFF_FFFF, packed, 0, out);
            self.finish_entity_occlusion(culling, c, out, entity_start);
        }
    }
    /// `MinecartTNT.primeFuse`: fuse 80, then `Level.broadcastEntityEvent`
    /// (`EntityEvent.TNT_PRIME`, 70) and, unless the cart is silent,
    /// `SoundEvents.TNT_PRIMED` on `SoundSource.BLOCKS`. A client that
    /// receives 70 only sets the fuse (`handleEntityEvent`).
    pub(in crate::game) fn prime_cart(&mut self, c: &mut Minecart) {
        if !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::TNT_EXPLODES)
        {
            return;
        }
        c.fuse = 80;
        let at = [c.position.x, c.position.y, c.position.z];
        let id = c.id as i32;
        self.lan_entity_event(id, TNT_PRIME_EVENT, at);
        if !self.entity_silent(c.id) {
            self.audio
                .0
                .play_on(BLOCKS, "minecraft:entity.tnt.primed", Some(at), 1.0, 1.0);
        }
    }
    /// `ClientPacketListener.postAddEntitySoundInstance` for the carts that
    /// joined the level since the last tick, and `LocalPlayer.startRiding`'s
    /// two inside sounds when the player got in one; then what the sound
    /// instances read this tick (the ridden cart and every cart).
    pub(in crate::game::client_side) fn start_cart_sounds(
        &mut self,
    ) -> (Option<u32>, Vec<CartSound>) {
        let improved = self.minecart_improvements();
        let carts: Vec<CartSound> = self
            .carts
            .iter()
            .map(|c| CartSound {
                id: c.id,
                position: [c.position.x, c.position.y, c.position.z],
                // Vec3.horizontalDistance.
                speed: (c.velocity.x * c.velocity.x + c.velocity.z * c.velocity.z).sqrt(),
                new_behavior: improved,
                on_rails: c.on_rails,
            })
            .collect();
        self.client
            .cart_sounds
            .retain(|id| carts.iter().any(|c| c.id == *id));
        for cart in &carts {
            if !self.client.cart_sounds.contains(&cart.id) {
                let instance = SoundInstance::minecart(cart, random_source::create());
                let (_, res) = self.audio.0.play_instance(instance, false);
                if res != PlayResult::NotStarted {
                    self.client.cart_sounds.insert(cart.id);
                }
            }
        }
        let vehicle = self.riding.filter(|id| carts.iter().any(|c| c.id == *id));
        if vehicle != self.client.riding_cart_sounds
            && let Some(cart) = carts.iter().find(|c| Some(c.id) == vehicle)
        {
            let mut started = true;
            for underwater in [true, false] {
                let instance =
                    SoundInstance::riding_minecart(cart, underwater, random_source::create());
                let (_, res) = self.audio.0.play_instance(instance, false);
                if res == PlayResult::NotStarted {
                    started = false;
                }
            }
            if started {
                self.client.riding_cart_sounds = vehicle;
            }
        } else if vehicle.is_none() {
            self.client.riding_cart_sounds = None;
        }
        (vehicle, carts)
    }
    /// `AbstractMinecart.tick` for one cart; returns false once it is gone.
    /// `slot` is where the cart is stored (where to look first).
    pub(in crate::game) fn tick_cart(
        &mut self,
        c: &mut Minecart,
        intent: [f64; 2],
        slot: usize,
        index: &mut CartIndex,
    ) -> bool {
        c.tick_count += 1;
        c.previous = c.position;
        c.previous_yaw = c.yaw;
        c.previous_pitch = c.pitch;
        // AbstractMinecart.tick: each counts down only while positive.
        if c.hurt_time > 0 {
            c.hurt_time -= 1;
        }
        if c.damage > 0.0 {
            c.damage -= 1.0;
        }
        // NewMinecartBehavior.tick snaps a newly created cart before gravity.
        let improved = self.minecart_improvements();
        if improved && c.first_tick {
            let pos = self.rail_block(c.position, true);
            if let Some(rail) = self.rail(Some(c.id), pos[0], pos[1], pos[2]) {
                c.on_rails = true;
                Self::adjust_cart_to_rails(c, pos, rail.shape);
            } else {
                c.on_rails = false;
            }
        }
        // applyGravity
        c.velocity.y -= c.get_gravity();
        if improved {
            if !self.move_along_track_new(c, intent) {
                return false;
            }
        } else {
            // OldMinecartBehavior.tick writes `onRails` every tick, before the move.
            let pos = self.rail_block(c.position, false);
            let rail = self.rail(Some(c.id), pos[0], pos[1], pos[2]);
            c.on_rails = rail.is_some();
            if let Some(rail) = rail {
                if !self.move_along_track(c, pos, &rail, intent, slot) {
                    return false;
                }
                self.note_cart_pass(c, pos);
                if &*rail.name == "minecraft:activator_rail" {
                    self.activate_cart(c, rail.powered);
                }
            } else if !self.come_off_track(c) {
                return false;
            }
        }
        c.first_tick = false;
        // The old behavior derives yaw once per tick; the experiment has
        // already recorded each step's yaw and pitch.
        if !self.minecart_improvements() {
            // OldMinecartBehavior.tick: setXRot(0.0F) before the yaw update.
            c.pitch = 0.0;
            let (xd, zd) = (c.previous.x - c.position.x, c.previous.z - c.position.z);
            if xd * xd + zd * zd > 0.001 {
                c.yaw = sunrise_core::geometry::trig::atan2_degrees_double(zd, xd);
                if c.flipped {
                    c.yaw += 180.0;
                }
            }
            let diff = (c.yaw - c.previous_yaw + 180.0).rem_euclid(360.0) - 180.0;
            if !(-170.0..170.0).contains(&diff) {
                c.yaw += 180.0;
                c.flipped = !c.flipped;
            }
            c.yaw %= 360.0;
            c.pitch %= 360.0;
        }
        if !self.minecart_improvements() {
            self.cart_block_effects(c);
            // OldMinecartBehavior.tick: applyEffectsFromBlocks() once settled.
            let here = c.position;
            self.cart_inside_blocks(c, slot, here, here);
        }
        // OldMinecartBehavior.tick ends with pushAndPickupEntities, before
        // AbstractMinecart.tick's updateFluidInteraction. The experiment still
        // uses this fallback until its per-move pickup/push retry is ported.
        self.push_from_ticked_cart(c, slot, index);
        // AbstractMinecart.tick: updateFluidInteraction, a stream pushes the cart.
        c.in_water = c
            .push_by_fluid(&self.world, &self.res.motion().fluids, &self.loop_context)
            .0
            > 0.0;
        // updateFluidInteraction: being in water resets the fall distance.
        if c.in_water {
            c.fall_distance = 0.0;
        }
        // Entity.checkInsideBlocks runs once after the cart tick, sweeping every
        // movement through powder snow. PowderSnowBlock uses getKnownMovement,
        // which is getDeltaMovement for a minecart.
        self.entity_powder_snow_particles(c.previous, c.position, c.bounds(), c.velocity, false);
        // Variants.
        match c.kind {
            CartKind::Furnace => {
                if c.fuel > 0 {
                    c.fuel -= 1;
                }
                if c.fuel <= 0 {
                    c.push = Vec3::ZERO;
                }
                if c.fuel > 0 && c.random.next_int_bound(4).unwrap_or(1) == 0 {
                    let at = [c.position.x, c.position.y + 0.8, c.position.z];
                    self.particles.0.add(
                        &ParticleOptions::simple(ParticleType::LargeSmoke),
                        at,
                        [0.0; 3],
                    );
                }
            }
            CartKind::Tnt => {
                // MinecartTNT.tick: the fuse and the collision check are separate.
                if c.fuse > 0 {
                    c.fuse -= 1;
                    let at = [c.position.x, c.position.y + 0.5, c.position.z];
                    self.particles.0.add(
                        &ParticleOptions::simple(ParticleType::Smoke),
                        at,
                        [0.0; 3],
                    );
                } else if c.fuse == 0 {
                    // explode(horizontalDistanceSqr()): a rising cart's
                    // vertical speed adds nothing.
                    let speed = c.velocity.x * c.velocity.x + c.velocity.z * c.velocity.z;
                    if self.explode_cart(c, speed) {
                        return false;
                    }
                }
                // MinecartTNT.tick reads the delta left after AbstractMinecart.tick
                // (pushes and fluids included). The move zeroed the blocked axes
                // (Entity.restituteMovementAfterCollisions) and no track step puts
                // them back: a head-on stop leaves nothing, a diagonal one keeps
                // the free axis (#11380).
                let speed = c.velocity.horizontal_distance_squared();
                if tnt_ignites_by_collision(c.ground.horizontal_collision, speed)
                    && self.explode_cart(c, speed)
                {
                    return false;
                }
            }
            // MinecartSpawner.tick: `BaseSpawner.serverTick` at the cart's block.
            CartKind::Spawner => {
                if let Some(mut spawner) = c.spawner.take() {
                    let key = (
                        c.position.x.floor() as i32,
                        c.position.y.floor() as i32,
                        c.position.z.floor() as i32,
                    );
                    let _ = self.server_tick_spawner_of(key, &mut spawner, Some(c.id));
                    c.spawner = Some(spawner);
                }
            }
            _ => {}
        }
        true
    }
}

#[cfg(test)]
#[path = "minecarts/minecart_tnt_tests_client.rs"]
pub(in crate::game) mod minecart_tnt_tests;
