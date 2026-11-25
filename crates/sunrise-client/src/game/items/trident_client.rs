#[allow(unused_imports)]
pub(in crate::game::client_side) use super::level_sounds::LevelSound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, commands_common::Target};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trident::Catcher;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trident::remote_tick;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trident::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{__private::TransactionFrame, SoundSource};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    arrows::{Pickup, back_off, clip_block_hit},
    collisions::SceneCollisions,
    enchant::EffectContext,
    items::ItemStack,
    join_seq::JoinSeq,
    mesher::Vertex,
    mobs::{Damage, DamageKind, Species, Who, anim::Poses},
    projectiles::{Hit, ProjectileWorld},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3, traverse_blocks},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::trident::{
    HALF, ThrownTrident, breeze_deflect,
};

impl Game<'_> {
    /// ThrownTridentRenderer.
    pub(in crate::game) fn trident_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
        run: std::ops::Range<usize>,
    ) {
        // AbstractArrow.shouldRenderAtSqrDistance: ten times the default.
        let range = 10.0
            * super::entity_culling::default_range(
                glam::DVec3::splat(2.0 * HALF),
                self.view_scale(),
            );
        let live = self.tridents[run]
            .iter()
            .filter(|t| {
                !self.hides_as_camera(t.id)
                    && self.standing_entity_in_view(
                        camera,
                        super::entity_culling::at(t.previous),
                        super::entity_culling::at(t.position),
                        alpha,
                        2.0 * HALF,
                        2.0 * HALF,
                        range,
                        1.5,
                    )
            })
            .map(|t| {
                let at = [
                    t.previous.x + (t.position.x - t.previous.x) * alpha,
                    t.previous.y + (t.position.y - t.previous.y) * alpha,
                    t.previous.z + (t.position.z - t.previous.z) * alpha,
                ];
                (t, at)
            });
        self.draw_tridents(camera, live, out);
    }
    pub(in crate::game) fn draw_tridents<'a>(
        &self,
        camera: glam::DVec3,
        tridents: impl Iterator<Item = (&'a ThrownTrident, [f64; 3])>,
        out: &mut Vec<Vertex>,
    ) {
        let Some(model) = &self.client.trident_model else {
            return;
        };
        let Some(uv) = self
            .gui
            .atlas
            .sprites
            .get("entity/trident/trident")
            .map(|r| [r.u0, r.v0, r.u1, r.v1])
        else {
            return;
        };
        // The texture's OptiFine emissive overlay, drawn again full bright.
        let glow = self
            .res
            .mob_data()
            .emissive
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .overlay("entity/trident/trident")
            .and_then(|e| self.gui.atlas.sprites.get(e))
            .map(|r| [r.u0, r.v0, r.u1, r.v1]);
        for (t, at) in tridents {
            let p = glam::DVec3::from_array(at) - camera;
            let base = glam::Mat4::from_translation(p.as_vec3())
                * glam::Mat4::from_rotation_y((t.yaw - 90.0).to_radians())
                * glam::Mat4::from_rotation_z((t.pitch + 90.0).to_radians());
            let l = self.world.light(
                t.position.x.floor() as i32,
                t.position.y.floor() as i32,
                t.position.z.floor() as i32,
            );
            if t.fire_ticks > 0 {
                // The flames face the camera, turned round in the front view.
                let fire_yaw = if self.client.camera_type
                    == sunrise_client::camera::CameraType::ThirdPersonFront
                {
                    self.yaw + 180.0
                } else {
                    self.yaw
                };
                sunrise_client::fire_render::flames(
                    at,
                    0.5,
                    0.5,
                    camera,
                    fire_yaw,
                    &self.gui.atlas.sprites,
                    l,
                    out,
                );
            }
            let packed = (u32::from(l >> 4) * 16) | ((u32::from(l & 15) * 16) << 8);
            let poses = Poses::new(model);
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
            if let Some(glow) = glow {
                let full = sunrise_client::mobs::render::FULL_BRIGHT;
                sunrise_client::mobs::render::draw(
                    model,
                    base,
                    &poses,
                    glow,
                    0xFFFF_FFFF,
                    full,
                    0,
                    out,
                );
            }
        }
        self.mark_motion(out.len(), 0);
    }
    pub(in crate::game::client_side) fn pickup_trident_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
    ) {
        // ItemPickupParticle: a picked-up trident flying to its collector,
        // drawn with the render state extracted at partial tick 1.
        let taken = self.pickups.0.live.iter().filter_map(|p| match &p.taken {
            super::pickups::Taken::Trident(t) => Some((&**t, super::pickups::position(p, alpha))),
            _ => None,
        });
        self.draw_tridents(camera, taken, out);
    }
}

#[cfg(test)]
#[path = "trident/impact_tests_client.rs"]
pub(in crate::game) mod impact_tests;
