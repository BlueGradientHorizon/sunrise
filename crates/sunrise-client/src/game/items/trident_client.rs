#[allow(unused_imports)]
pub(in crate::game::client_side) use super::level_sounds::LevelSound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, commands::Target};
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
    pub(in crate::game) fn pickup_trident_geometry(
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
    /// Apply the environment before impact callbacks can change the world.
    pub(in crate::game) fn trident_block_effects(
        &mut self,
        t: &mut ThrownTrident,
        from: Vec3,
        to: Vec3,
    ) {
        let extinguished = {
            let rain = self.arrow_rain();
            // Block-contact hooks are already delivered by the game's inside pass.
            let (ticks, extinguished) =
                self.arrow_scene(&rain)
                    .apply_block_effects(from, to, t.fire_ticks, |_, _, _| {});
            t.fire_ticks = ticks;
            extinguished
        };
        if extinguished {
            let pitch = 1.6 + (t.random.next_float() - t.random.next_float()) * 0.4;
            self.audio.0.play(
                "minecraft:entity.generic.extinguish_fire",
                Some([to.x, to.y, to.z]),
                0.7,
                pitch,
            );
        }
    }
    /// `ThrownTrident.tryPickup` for a trident that has landed or is
    /// returning: the player takes the item (a creative thrower's trident is
    /// only removed) and the rest that does not fit drops at their feet.
    /// False when it could not be given.
    pub(in crate::game) fn trident_taken(&mut self, t: &ThrownTrident, by: &Catcher) -> bool {
        let creative_pickup = t.pickup == Pickup::CreativeOnly && by.creative;
        let returning_to_owner = t.no_physics && t.owner_uuid.is_some() && t.owner_uuid == by.uuid;
        if !creative_pickup {
            if t.pickup != Pickup::Allowed && !returning_to_owner {
                return false;
            }
            let rest = match by.guest {
                None => self.add_stack_to_inventory(t.stack.clone()),
                Some(conn) => {
                    let rest = self.as_guest(conn, |g| g.add_stack_to_inventory(t.stack.clone()));
                    let Some(rest) = rest else {
                        return false;
                    };
                    rest
                }
            };
            if let Some(rest) = rest {
                self.spawn_item(rest, [by.feet.x, by.feet.y, by.feet.z], [0.0; 3], 0);
            }
        }
        // Player.take, for a creative thrower's trident too: the clients near
        // play the pickup sound and fly the trident to its collector.
        if by.guest.is_some()
            && let Some(host) = self.lan.as_ref()
        {
            let at = [t.position.x, t.position.y, t.position.z];
            host.hub.take_item(t.id as i32, by.id, 1, at);
        }
        if by.guest.is_none() {
            let pitch = ((self.world.random.next_float() - self.world.random.next_float()) * 0.7
                + 1.0)
                * 2.0;
            self.audio.0.play(
                "minecraft:entity.item.pickup",
                Some([by.feet.x, by.feet.y, by.feet.z]),
                0.2,
                pitch,
            );
        }
        true
    }
}

#[cfg(test)]
#[path = "trident/impact_tests_client.rs"]
pub(in crate::game) mod impact_tests;
