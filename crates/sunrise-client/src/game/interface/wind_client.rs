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

impl Game<'_, crate::game::client_side::ClientState> {
    /// `WindChargeRenderer`: the spinning charge and its wind.
    pub(in crate::game::client_side) fn wind_charge_geometry(
        &self,
        p: &Projectile,
        at: glam::Vec3,
        light: u32,
        partial: f32,
        out: &mut Vec<sunrise_client_render::render_geometry::Vertex>,
    ) {
        let Some(model) = self.side.entity_models.wind_charge.as_ref() else {
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
