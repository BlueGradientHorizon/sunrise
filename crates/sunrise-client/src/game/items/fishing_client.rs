use super::{Game, billboard_axes};
use crate::game::server_side::fishing::Hook;
use crate::game::server_side::fishing::State;
use crate::game::server_side::fishing::bite_impulse;
use crate::game::server_side::fishing::near_plane_hand;
use crate::game::server_side::fishing::push_remote_hook;
use crate::game::server_side::fishing::remove_remote_hook_from;
use crate::game::server_side::fishing::synchronized_random;
use crate::game::server_side::fishing::*;
use sunrise_client::{gizmos::Line, mesher::Vertex, pose::Pose};
use sunrise_core::geometry::{Vec3, trig};
use sunrise_protocol::entity_data::Value;

impl Game<'_, crate::game::client_side::ClientState> {
    /// FishingHookRenderer: camera-facing half-block quads centred on each bobber.
    pub(in crate::game) fn fishing_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
    ) {
        let hooks: Vec<glam::DVec3> = self
            .hook_position(alpha)
            .into_iter()
            .chain(self.remote_hook_ends(alpha).into_iter().map(|(h, _)| h))
            .collect();
        if hooks.is_empty() {
            return;
        }
        let Some(r) = self
            .side
            .gui
            .atlas
            .sprites
            .get("entity/fishing/fishing_hook")
            .copied()
        else {
            return;
        };
        // FishingHookRenderer.submit: turned by the camera's orientation.
        let [right, up] = billboard_axes(self.camera_orientation());
        let s = 0.25;
        let uv = [[r.u0, r.v1], [r.u1, r.v1], [r.u1, r.v0], [r.u0, r.v0]];
        for p in hooks {
            let light =
                self.world
                    .light(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
            let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
            let corners = [
                p - right * s - up * s,
                p + right * s - up * s,
                p + right * s + up * s,
                p - right * s + up * s,
            ];
            for (v, uv) in corners.iter().zip(uv) {
                out.push(Vertex {
                    normal: [0; 4],
                    pos: (*v - camera).as_vec3().to_array(),
                    uv,
                    layer: 0,
                    color: 0xFFFF_FFFF,
                    light: packed,

                    glint_uv: [0.0, 0.0],
                });
            }
        }
    }
    pub(in crate::game::client_side) fn clear_remote_hooks(&mut self) {
        if self.remote.is_some() {
            self.hook.discard();
        }
        self.side.remote_hooks.clear();
    }
    /// The hooks drawn here, with their owners' ids (0 for this player's own).
    pub(in crate::game::client_side) fn hooks_status(&self) -> Vec<serde_json::Value> {
        let at = |p: Vec3| [p.x, p.y, p.z];
        let own = self.hook.iter().map(|h| (h.id, 0, h.position));
        let others = self
            .side
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
        } else if let Some(h) = self.side.remote_hooks.iter_mut().find(|h| h.id == id) {
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
        self.side
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
    pub(in crate::game::client_side) fn remote_hook_motion(&mut self, id: u32, v: Vec3) -> bool {
        if self.remote.is_none() {
            return false;
        }
        if let Some(h) = self.hook.as_mut().filter(|h| h.id == id) {
            h.velocity = v;
        } else if let Some(h) = self.side.remote_hooks.iter_mut().find(|h| h.id == id) {
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
        remove_remote_hook_from(&mut self.side.remote_hooks, id);
    }
    /// `FishingHookRenderer.getPlayerHandPos`: on the near plane in first
    /// person, beside the body otherwise.
    pub(in crate::game) fn rod_hand_position(&self, alpha: f64) -> Option<glam::DVec3> {
        let p = self.player.as_ref()?;
        let right_arm = self.main_hand_rod() != self.side.settings.main_hand_left;
        let invert: f32 = if right_arm { 1.0 } else { -1.0 };
        let swing = trig::sin(f64::from(
            self.swing.animation(alpha as f32).sqrt() * std::f32::consts::PI,
        ));
        let feet = self.previous_position.lerp(p.position, alpha);
        let eye = glam::DVec3::new(feet.x, feet.y + f64::from(p.dimensions.eye_height), feet.z);
        if self.side.camera_type.is_first_person() {
            let fov = self.side.settings.fov as i32 as f32;
            Some(eye + near_plane_hand(self.near_plane(), fov, invert, swing))
        } else {
            let model = self.side.player_model.as_ref()?;
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
            self.hook.discard();
            self.hook.replace(hook);
        } else {
            push_remote_hook(
                &mut self.side.remote_hooks,
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
        let mut hooks = std::mem::take(&mut self.side.remote_hooks);
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
        self.side.remote_hooks = hooks;
    }
    /// FishingHookRenderer: each line, 16 black `lines` segments sagging
    /// from a quarter block above the hook to the rod in its owner's hand,
    /// `width` pixels wide (`Window.getAppropriateLineWidth`).
    pub(in crate::game) fn fishing_line(&self, alpha: f64, size: [u32; 2]) -> Vec<Line> {
        let own = self.hook_position(alpha).zip(self.rod_hand_position(alpha));
        let width = sunrise_client::gfx::outline_width(size[0]);
        let mut lines = Vec::new();
        for (hook, hand) in own.into_iter().chain(self.remote_hook_ends(alpha)) {
            let offset = (hand - (hook + glam::DVec3::new(0.0, 0.25, 0.0))).as_vec3();
            let point = |a: f32| {
                let y = offset.y * (a * a + a) * 0.5 + 0.25;
                hook + glam::Vec3::new(offset.x * a, y, offset.z * a).as_dvec3()
            };
            lines.extend((0..16).map(|i| Line {
                start: point(i as f32 / 16.0).to_array(),
                end: point((i + 1) as f32 / 16.0).to_array(),
                color: 0xFF00_0000,
                width,
            }));
        }
        lines
    }
    /// Client-side bobber collision response without server tables or sound/event writes.
    pub(in crate::game) fn client_move_bobber_response(
        &mut self,
        id: u32,
        position: Vec3,
        ground: &mut sunrise_world::ground_support::GroundState,
        wanted: Vec3,
        moved: Vec3,
        velocity: &mut Vec3,
    ) {
        use sunrise_items::equipment::{BODY, SADDLE};
        let level = self.native_client_level();
        let side = crate::game::native_surface::GameSide {
            dim: self.dim,
            level,
            facts: self.res.facts(),
            biomes: self.res.biomes(),
            behaviours: self.res.behaviours(),
            features: match &self.remote {
                Some(remote) => &remote.features,
                None => crate::game::tables::default_client_features(),
            },
            items: self.res.items(),
            damage_types: &self.health_resources.damage,
            player_equipment: [
                self.inventory[self.selected].as_ref(),
                self.offhand.as_ref(),
                self.armor[3].as_ref(),
                self.armor[2].as_ref(),
                self.armor[1].as_ref(),
                self.armor[0].as_ref(),
                self.equipment.get(BODY),
                self.equipment.get(SADDLE),
            ],
            hit_borrows: crate::game::native_surface::HitBorrows {
                living_types: self.res.living_type_keys(),
                attributes: &self.motion.attributes,
                player_effects: &self.potions.effects,
                player_uuid: None,
                player_attachments: &self.player_state.attachments,
                entity_attachments: &self.entity_attachments,
            },
            tag_items: self.remote.as_ref().and_then(|remote| remote.tags.as_ref()),
        };
        let motion = sunrise_client::motion::BodyMotion {
            world: &self.world,
            collision: self.collision,
            facts: self.res.facts(),
            blocks: Some(&self.res.motion().blocks),
            context: &self.loop_context,
            side: Some(&side),
        };
        motion.record_move(
            sunrise_client::motion::BodyCollision {
                footprint: (position, bounds(position)),
                movement: (wanted, moved),
                authoritative: false,
                context: sunrise_client::motion::BodyMotion::entity_context(id, position),
                subject: None,
            },
            ground,
        );
        let effect = motion.legacy_contact(position, ground);
        let entity = (id, "minecraft:fishing_bobber");
        let response = motion.restitute(
            entity,
            sunrise_client::motion::BodyMove {
                wanted,
                moved,
                effect,
            },
            f64::from(0.03_f32),
            0.98_f32,
            *velocity,
            0.0,
        );
        *velocity = response.velocity;
        motion.apply_block_speed(position, velocity);
        motion.step_on(entity, (position, bounds(position)), ground, velocity);
    }
}

#[cfg(test)]
#[path = "fishing_tests/fishing_targets_tests_client.rs"]
pub(in crate::game) mod fishing_targets_tests;
