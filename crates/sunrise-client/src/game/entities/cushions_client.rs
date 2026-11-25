#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, Hit, InteractTarget, Keys, entity_culling::default_range,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cushions::PlacementWorld;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cushions::aabb;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cushions::snap_yaw;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cushions::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::cushion::blocks_in;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::entities::{Cushion, dye_index};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::ItemStack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::anim::Poses;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::{Aabb, Vec3};

impl Game<'_> {
    /// `Cushion.showBreakingParticles`: ten of the matching wool block.
    pub(in crate::game::client_side) fn cushion_break_particles(&mut self, c: &Cushion) {
        let name = format!("minecraft:{}_wool", c.color_name());
        let Some(state) = self.pack.assets.registry.default_state(&name) else {
            return;
        };
        let p = c.position;
        // Entity.getY(2/3) is relative to the cushion's quarter-block height.
        let particle_at = [p.x, p.y + 0.25 * (2.0 / 3.0), p.z];
        // Guest actions temporarily take self.lan; flush once the hub is back.
        // Queue before resolving a sprite: a dedicated server has no renderer.
        if self.audio.0.capture.is_some() {
            self.cushion_break_particles_server(state, particle_at);
        }
        let at = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
        let Some((sprite, tint)) = self.particle_sprite(state, at) else {
            return;
        };
        let options = sunrise_client::particles::ParticleOptions::block(
            sunrise_client::particles::ParticleType::Block,
            sprite,
            tint,
        );
        self.particles
            .0
            .send_particles(&options, particle_at, [0.25, 0.0625, 0.25], 0.05, 10);
    }
    /// `CushionRenderer`: the coloured model, rotated to the nearest
    /// direction, with its bottom on the cushion's position.
    pub(in crate::game::client_side) fn cushion_geometry(
        &self,
        camera: glam::DVec3,
        out: &mut Vec<sunrise_client_render::render_geometry::Vertex>,
    ) {
        let Some(model) = self.client.cushion_model.as_ref() else {
            return;
        };
        let sprites = &self.gui.atlas.sprites;
        for c in &self.cushions {
            if self.hides_as_camera(c.id) {
                continue;
            }
            let a = aabb(c.bounds());
            let (lo, hi) = (a.min_position(), a.max_position());
            let (lo, hi) = (
                glam::DVec3::new(lo.x, lo.y, lo.z),
                glam::DVec3::new(hi.x, hi.y, hi.z),
            );
            let range = default_range(hi - lo, self.view_scale());
            if !self.entity_in_view(camera, (lo + hi) / 2.0, range, lo, hi) {
                continue;
            }
            let Some(uv) = sprites
                .get(&format!("entity/cushion/{}_cushion", c.color_name()))
                .map(|r| [r.u0, r.v0, r.u1, r.v1])
            else {
                continue;
            };
            let culling = self.entity_cull_decision(c, 1.0);
            if culling.hides_entity() {
                continue;
            }
            let entity_start = out.len();
            let p = glam::DVec3::new(c.position.x, c.position.y, c.position.z) - camera;
            let base = glam::Mat4::from_translation(p.as_vec3())
                * glam::Mat4::from_rotation_y((180.0 - snap_yaw(c.yaw)).to_radians())
                * glam::Mat4::from_rotation_x(std::f32::consts::PI)
                * glam::Mat4::from_translation(glam::Vec3::new(0.0, -0.25, 0.0));
            // `EntityRenderer.getPackedLightCoords` probes at the eye height.
            let l = self.world.light(
                c.position.x.floor() as i32,
                (c.position.y + 0.2125).floor() as i32,
                c.position.z.floor() as i32,
            );
            let packed = (u32::from(l >> 4) * 16) | ((u32::from(l & 15) * 16) << 8);
            sunrise_client::mobs::render::draw(
                model,
                base,
                &Poses::new(model),
                uv,
                0xFFFF_FFFF,
                packed,
                0,
                out,
            );
            self.finish_entity_occlusion(culling, c, out, entity_start);
        }
    }
    /// `LivingEntity.stopRiding` off a cushion: `dismountVehicle` beside it,
    /// and `Cushion.removePassenger`'s get-up sound for a rider that left
    /// rather than one whose cushion was killed.
    pub(in crate::game) fn get_up_from_cushion(&mut self, c: &Cushion) {
        if self.riding == Some(c.id) && !self.native_may_leave(c.id) {
            return;
        }
        let at = c.dismount();
        self.riding = None;
        if let Some(p) = self.player.as_mut() {
            self.previous_position = p.position;
            p.position = at;
            p.velocity = Vec3::ZERO;
            p.fall_distance = 0.0;
        }
        self.audio.0.play(
            "minecraft:entity.cushion.get_up",
            Some([c.position.x, c.position.y, c.position.z]),
            1.0,
            1.0,
        );
    }
    /// `Cushion.interact`: sit down unless sneaking or already occupied. The
    /// cushion under the crosshair, or the one a guest's interact packet named.
    pub(in crate::game) fn interact_cushion(&mut self, target: Option<InteractTarget>) -> bool {
        let Some((i, d, _)) = (match target {
            Some(t) => {
                Self::named_entity(&self.cushions, t.id, |c| c.id).map(|i| (i, 0.0, [0.0; 3]))
            }
            None => self.cushion_hit(),
        }) else {
            return false;
        };
        // Only a client's own pick is stopped by a block: the range check passed.
        if target.is_none() && !self.before_block(d) {
            return false;
        }
        // isSecondaryUseActive.
        if self.held_mapping(Action::Sneak) {
            return false;
        }
        let id = self.cushions[i].id;
        let occupied = self.riding == Some(id)
            || self
                .lan
                .as_ref()
                .is_some_and(|h| h.hub.guests.iter().any(|g| g.vehicle == Some(id)));
        // `Cushion.interact`: an occupied cushion (`isVehicle`) passes.
        if occupied {
            return false;
        }
        // `Entity.startRiding`: a passenger first gets off what it rides
        // (`stopRiding`), so a seated player moves to this cushion.
        if !self.native_may_board(id) {
            return true;
        }
        // After each hook the cushion is found again by its id: a listener
        // may have removed it, and the click is then spent.
        if Self::named_entity(&self.cushions, id as i32, |c| c.id).is_none() {
            return true;
        }
        self.stop_riding();
        // startRiding stops when stopRiding left the passenger seated (a mod
        // canceled the dismount).
        if self.riding.is_some() {
            return true;
        }
        let Some(i) = Self::named_entity(&self.cushions, id as i32, |c| c.id) else {
            return true;
        };
        self.start_riding(id);
        let p = self.cushions[i].position;
        self.audio.0.play(
            "minecraft:entity.cushion.sit",
            Some([p.x, p.y, p.z]),
            1.0,
            1.0,
        );
        self.swing_arm();
        true
    }
    /// The removal both paths share; `killed` posts `BlockAttachedEntity.kill`'s
    /// `entity_die` event before `dropItem`'s sound and drop.
    pub(in crate::game) fn remove_cushion(
        &mut self,
        index: usize,
        source: Option<super::commands::Target>,
        killed: bool,
    ) -> bool {
        let id = self.cushions[index].id;
        if self.target_removed(&super::commands::Target::Cushion(id)) {
            return false;
        }
        let Some(c) = self.take_removed_cushion(id) else {
            return false;
        };
        let p = c.position;
        if killed {
            // BlockAttachedEntity.kill, before dropItem.
            self.entity_killed_event_by([p.x, p.y, p.z], source);
        }
        self.audio.0.play(
            "minecraft:entity.cushion.break",
            Some([p.x, p.y, p.z]),
            1.0,
            1.0,
        );
        self.cushion_break_particles(&c);
        // `Cushion.dropItem`: the item unless drops are off or the cause is a
        // player with infinite materials (the TNT's owner for a blast).
        if !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::ENTITY_DROPS)
            || self.infinite_materials(source.as_ref())
        {
            return true;
        }
        let stack = c.item_stack_with_data();
        self.spawn_item(stack, [p.x, p.y, p.z], [0.0; 3], 10);
        true
    }
}
