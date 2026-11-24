#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::entity::{Blast, Entity as _, for_each_entity_mut};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::BREEZE_BURST;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::BREEZE_RADIUS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::Burst;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::BurstCause;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::PLAYER_KNOCKBACK;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::PLAYER_RADIUS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::Source;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::VehicleWinds;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::WIND_CHARGE_BURST;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::above_the_top;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::push_waits_for_tick;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::reverse;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::triggers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::wind::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::BTreeSet;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::advancements::EntityView;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::chunk::LevelShape;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::join_seq::Joined as _;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    items::ItemStack,
    mobs::{Damage, DamageKind, Species, Who},
    particles::{ParticleOptions, ParticleType},
    projectiles::{Hit, Projectile, ProjectileKind},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Direction, Vec3},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::level::{PlayerContext, classes::Class};

impl Game<'_> {
    /// `WindChargeItem.use`.
    pub(in crate::game) fn throw_wind_charge(&mut self, stack: &ItemStack) -> Result<bool> {
        let (Some(eye), Some(p)) = (self.eye_position(), self.player.as_ref()) else {
            return Ok(false);
        };
        let (on_ground, pos) = (p.ground.on_ground, p.position);
        let velocity = self.known_player_movement();
        let start = Vec3::new(pos.x, eye[1], pos.z);
        let mut proj = Projectile::new(ProjectileKind::WindCharge, start, Some(Who::Player));
        proj.shoot_from_rotation(self.pitch, self.yaw, 0.0, 1.5, 1.0, velocity, on_ground);
        self.spawn_projectile(proj, stack);
        let pitch = 0.4 / (self.world.random.next_float() * 0.4 + 0.8);
        self.audio
            .0
            .play("minecraft:entity.wind_charge.throw", Some(eye), 0.5, pitch);
        self.award_stat("used", "minecraft:wind_charge", 1);
        self.swing_arm();
        if !self.creative
            && let Some(s) = self.held_mut().as_mut()
        {
            s.count -= 1;
            if s.count == 0 {
                *self.held_mut() = None;
            }
        }
        Ok(true)
    }
    /// `WindChargeRenderer`: the spinning charge and its wind.
    pub(in crate::game) fn wind_charge_geometry(
        &self,
        p: &Projectile,
        at: glam::Vec3,
        light: u32,
        partial: f32,
        out: &mut Vec<sunrise_client_render::render_geometry::Vertex>,
    ) {
        let Some(model) = self.client.entity_models.wind_charge.as_ref() else {
            return;
        };
        let Some(r) = self
            .gui
            .atlas
            .sprites
            .get("entity/projectiles/wind_charge")
            .copied()
        else {
            return;
        };
        // `EntityRenderState.ageInTicks`: the tick count plus the partial tick.
        let age = p.age as f32 + partial;
        let mut poses = sunrise_client::mobs::anim::Poses::new(model);
        poses.yrot("wind_charge", -age * 16.0_f32.to_radians());
        poses.yrot("wind", age * 16.0_f32.to_radians());
        let base = glam::Mat4::from_translation(at)
            * glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0));
        let start = out.len();
        sunrise_client::mobs::render::draw_scrolled(
            model,
            base,
            &poses,
            [r.u0, r.v0, r.u1, r.v1],
            sunrise_client::mobs::render::scroll_offset(
                age,
                sunrise_client::mobs::render::WIND_CHARGE_SCROLL,
            ),
            0xFFFF_FFFF,
            light,
            0,
            out,
        );
        // `RenderTypes.breezeWind` has an `OitPipelineSet`.
        for vertex in &mut out[start..] {
            vertex.layer |= sunrise_client::gfx::OIT_ENTITY;
        }
    }
}

#[cfg(test)]
#[path = "tests/wind_charged_tests_client.rs"]
pub(in crate::game) mod wind_charged_tests;
