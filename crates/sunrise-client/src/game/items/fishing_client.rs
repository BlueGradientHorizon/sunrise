#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, billboard_axes, creatures::ProjectileHitter, lan_mobs::guest_bounds,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::EntityCollision;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::FISHING_TABLE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::Hook;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::HookLerp;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::HookMedium;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::HookTarget;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::RemoteHook;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::SIZE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::State;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::WARNED_TABLES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::bite_impulse;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::near_plane_hand;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::push_remote_hook;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::remove_remote_hook_from;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::synchronized_random;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fishing::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::advancements::EntityView;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::lan::entity::States;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    blocks::Fluid,
    collisions::SceneCollisions,
    gizmos::Line,
    items::ItemStack,
    mesher::Vertex,
    particles::{ParticleOptions, ParticleType},
    pose::Pose,
    random_source,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, BlockHit, Vec3, traverse_blocks, trig},
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::entity_data::{Value, serializer};

impl Game<'_> {
    pub(in crate::game::client_side) fn clear_remote_hooks(&mut self) {
        if self.remote.is_some() {
            self.hook.discard();
        }
        self.client.remote_hooks.clear();
    }
    /// The hooks drawn here, with their owners' ids (0 for this player's own).
    pub(in crate::game::client_side) fn hooks_status(&self) -> Vec<serde_json::Value> {
        let at = |p: Vec3| [p.x, p.y, p.z];
        let own = self.hook.iter().map(|h| (h.id, 0, h.position));
        let others = self
            .client
            .remote_hooks
            .iter()
            .map(|h| (h.id, h.owner, h.position));
        own.chain(others)
            .map(|(id, owner, p)| {
                serde_json::json!({"id": id, "kind": "minecraft:fishing_bobber", "owner": owner, "position": at(p)})
            })
            .collect()
    }
    /// `FishingHook.onSyncedDataUpdated`: this player's hook is biting
    /// (DATA_BITING), which `status` shows as a nibble.
    pub(in crate::game::client_side) fn remote_hook_data(
        &mut self,
        id: u32,
        values: &[(u8, i32, Value)],
    ) {
        // DATA_HOOKED_ENTITY (the hooked id plus one, 0 for none): a hook on a
        // client follows what it hooked (`FishingHook.onSyncedDataUpdated`).
        let time = self.world.game_time;
        let hooked = |value: &Value| match value {
            Value::Int(v) if *v > 0 => State::Hooked(*v as u32 - 1),
            _ => State::Flying,
        };
        if let Some(h) = self
            .hook
            .as_mut()
            .filter(|h| h.id == id && self.remote.is_some())
        {
            for (index, _, value) in values {
                match (index, value) {
                    (9, Value::Bool(biting)) => {
                        h.nibble = i32::from(*biting);
                        if *biting {
                            h.velocity.y = bite_impulse(&mut synchronized_random(h.uuid, time));
                        }
                    }
                    (8, value) => h.state = hooked(value),
                    _ => {}
                }
            }
        } else if let Some(h) = self.client.remote_hooks.iter_mut().find(|h| h.id == id) {
            for (index, _, value) in values {
                match (index, value) {
                    (9, Value::Bool(biting)) => {
                        if *biting {
                            h.velocity.y = bite_impulse(&mut synchronized_random(h.uuid, time));
                        }
                    }
                    (8, value) => h.state = hooked(value),
                    _ => {}
                }
            }
        }
    }
    /// Other players' hooks: the bobber and its line to the owner's hand
    /// (`FishingHookRenderer.getPlayerHandPos` in third person), as
    /// (hook, hand) this frame.
    pub(in crate::game) fn remote_hook_ends(&self, alpha: f64) -> Vec<(glam::DVec3, glam::DVec3)> {
        let lerp = |a: Vec3, b: Vec3| {
            glam::DVec3::new(
                a.x + (b.x - a.x) * alpha,
                a.y + (b.y - a.y) * alpha,
                a.z + (b.z - a.z) * alpha,
            )
        };
        self.client
            .remote_hooks
            .iter()
            .filter_map(|h| {
                if self.hides_as_camera(h.id) {
                    return None;
                }
                let owner = self.mobs.iter().find(|m| m.id == h.owner)?;
                let feet = lerp(owner.previous, owner.position);
                let body = mod_api::client::camera::lerp(
                    alpha as f32,
                    owner.previous_body_yaw,
                    owner.body_yaw,
                );
                let angle = f64::from(body * (std::f64::consts::PI / 180.0) as f32);
                let (sin, cos) = (f64::from(trig::sin(angle)), f64::from(trig::cos(angle)));
                // The right arm; a crouching owner's eyes are lower.
                let eye = owner.eye_y() - owner.position.y;
                let hand = feet
                    + glam::DVec3::new(
                        -cos * 0.35 - sin * 0.8,
                        eye - 0.45,
                        -sin * 0.35 + cos * 0.8,
                    );
                Some((lerp(h.previous, h.position), hand))
            })
            .collect()
    }
    /// `Entity.lerpMotion` for a hook a server tracks: the motion its own tick
    /// moves it on with between the tracker's updates.
    pub(in crate::game) fn remote_hook_motion(&mut self, id: u32, v: Vec3) -> bool {
        if self.remote.is_none() {
            return false;
        }
        if let Some(h) = self.hook.as_mut().filter(|h| h.id == id) {
            h.velocity = v;
        } else if let Some(h) = self.client.remote_hooks.iter_mut().find(|h| h.id == id) {
            h.velocity = v;
        } else {
            return false;
        }
        true
    }
    pub(in crate::game::client_side) fn remove_remote_hook(&mut self, id: u32) {
        if self.remote.is_some() && self.hook.as_ref().is_some_and(|h| h.id == id) {
            self.hook.discard();
        }
        remove_remote_hook_from(&mut self.client.remote_hooks, id);
    }
    /// `FishingHookRenderer.getPlayerHandPos`: on the near plane in first
    /// person, beside the body otherwise.
    pub(in crate::game) fn rod_hand_position(&self, alpha: f64) -> Option<glam::DVec3> {
        let p = self.player.as_ref()?;
        let right_arm = self.main_hand_rod() != self.settings.main_hand_left;
        let invert: f32 = if right_arm { 1.0 } else { -1.0 };
        let swing = trig::sin(f64::from(
            self.swing.animation(alpha as f32).sqrt() * std::f32::consts::PI,
        ));
        let feet = self.previous_position.lerp(p.position, alpha);
        let eye = glam::DVec3::new(feet.x, feet.y + f64::from(p.dimensions.eye_height), feet.z);
        if self.client.camera_type.is_first_person() {
            let fov = self.settings.fov as i32 as f32;
            Some(eye + near_plane_hand(self.near_plane(), fov, invert, swing))
        } else {
            let model = self.client.player_model.as_ref()?;
            let body = mod_api::client::camera::lerp(
                alpha as f32,
                model.previous_body_yaw,
                model.body_yaw,
            );
            let angle = f64::from(body * (std::f64::consts::PI / 180.0) as f32);
            let (sin, cos) = (f64::from(trig::sin(angle)), f64::from(trig::cos(angle)));
            let right = f64::from(invert) * 0.35;
            let crouch = if p.pose == Pose::Crouching {
                -0.1875
            } else {
                0.0
            };
            Some(
                eye + glam::DVec3::new(
                    -cos * right - sin * 0.8,
                    crouch - 0.45,
                    -sin * right + cos * 0.8,
                ),
            )
        }
    }
    /// `ServerLevel.sendParticles` of a simple particle.
    pub(in crate::game) fn send_particles(
        &mut self,
        kind: ParticleType,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: u32,
    ) {
        let options = ParticleOptions::simple(kind);
        self.particles
            .0
            .send_particles(&options, at, spread, speed, count);
        // The players a server serves are sent them too.
        self.lan_send_particles(super::lan_particles::QueuedParticle {
            dimension: self.dim.id(),
            options,
            state: None,
            at,
            spread,
            speed: [speed; 3],
            count: count as i32,
            randomization: 0,
        });
    }
    /// `handleAddEntity` for a fishing bobber: this player's own becomes its
    /// hook (`Player.fishing`), another player's is drawn on its line to them.
    pub(in crate::game::client_side) fn spawn_remote_hook(
        &mut self,
        id: u32,
        uuid: [i32; 4],
        at: Vec3,
        velocity: Vec3,
        owner: i32,
    ) {
        let own = self.remote.as_ref().map(|r| r.entity_id);
        if own == Some(owner) {
            let mut hook = Hook::remote(id, at, uuid);
            hook.velocity = velocity;
            self.install_fishing_hook(hook);
        } else {
            push_remote_hook(
                &mut self.client.remote_hooks,
                id,
                owner as u32,
                at,
                velocity,
                uuid,
            );
        }
    }
    /// `FishingHook.tick` on a client: the interpolation step towards the
    /// server's last update, then its own movement between the tracker's
    /// updates (every five ticks), floating in water and riding what it
    /// hooked, as the server's does.
    pub(in crate::game::client_side) fn tick_remote_hooks(&mut self) {
        if self.remote.is_none() {
            return;
        }
        if let Some(mut h) = self.hook.extract() {
            h.previous = h.position;
            h.position = h.lerp.step(h.position);
            let before = h.position;
            let owner = self.player.as_ref().map(|p| p.position);
            (h.position, h.velocity, h.state, h.ground) = self.client_hook_step(
                h.id,
                owner,
                true,
                (h.position, h.velocity, h.state, h.ground),
                &mut h.random,
            );
            h.lerp.moved(h.position.subtract(before));
            self.hook.put_back(h);
        }
        let mut hooks = std::mem::take(&mut self.client.remote_hooks);
        for h in &mut hooks {
            h.previous = h.position;
            h.position = h.lerp.step(h.position);
            let before = h.position;
            let owner = self
                .mobs
                .iter()
                .find(|m| m.id == h.owner)
                .map(|m| m.position);
            (h.position, h.velocity, h.state, h.ground) = self.client_hook_step(
                h.id,
                owner,
                false,
                (h.position, h.velocity, h.state, h.ground),
                &mut h.random,
            );
            h.lerp.moved(h.position.subtract(before));
        }
        self.client.remote_hooks = hooks;
    }
}

#[cfg(test)]
#[path = "fishing_tests/fishing_targets_tests_client.rs"]
pub(in crate::game) mod fishing_targets_tests;
