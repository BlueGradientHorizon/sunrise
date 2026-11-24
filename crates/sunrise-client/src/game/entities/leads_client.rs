#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, commands::Target};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::leads::LeashInteraction;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::leads::Rope;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::leads::abgr;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::leads::boat_hold;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::leads::pack_light;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::leads::rope_hold_angles;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::leads::rot_y;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::leads::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::LootDrops;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    items::ItemStack,
    mesher::Vertex,
    mobs::leash::{self, Holder},
    pose::Pose,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::leash_knot::{
    Knot, held_by_acting_player, resolve_pending_leash, write_mob_leash_tag,
};

impl Game<'_> {
    /// `Entity.interact`: retain the holder-detach success's item context.
    pub(in crate::game) fn interact_leash_with_item_context(
        &mut self,
        index: usize,
        item: &str,
    ) -> Option<LeashInteraction> {
        let sneaking = self.held_mapping(Action::Sneak);
        let id = self.mobs[index].id;
        let at = self.mobs[index].center();
        if !self.mobs[index].alive() {
            return None;
        }
        // Sneaking with mobs in hand: tie them to this one.
        if sneaking && leash::can_be_leashed(&self.mobs[index]) && !self.mobs[index].is_baby() {
            let b = self.mobs[index].bounds();
            let (lo, hi) = (b.min_position(), b.max_position());
            let center = glam::DVec3::new(
                (lo.x + hi.x) / 2.0,
                (lo.y + hi.y) / 2.0,
                (lo.z + hi.z) / 2.0,
            );
            if self.tie_player_leads(center, Holder::Mob(id), Some(index)) {
                self.lead_game_event("minecraft:entity_action", at);
                self.lead_sound("tied", at);
                return Some(LeashInteraction::WithoutItem);
            }
        }
        if item == "minecraft:shears" {
            let mut dropped = false;
            if self.mobs[index].extra.leash.is_some() {
                self.break_leash_used(id, true);
                dropped = true;
            }
            let others: Vec<u32> = self
                .mobs
                .iter()
                .filter(|m| m.extra.leash == Some(Holder::Mob(id)))
                .map(|m| m.id)
                .collect();
            for o in others {
                self.break_leash_used(o, true);
                dropped = true;
            }
            let boats = self.boats_on(Holder::Mob(id));
            dropped |= !boats.is_empty();
            self.drop_boat_leashes(&boats);
            if dropped {
                self.lead_game_event("minecraft:shear", at);
                self.audio
                    .0
                    .play("minecraft:item.shears.snip", Some(at), 1.0, 1.0);
                self.damage_held_tool();
                return Some(LeashInteraction::WithItem);
            }
        }
        // Mob.checkAndHandleImportantInteractions: only the player the lead
        // is tied to unties it (`getLeashHolder() == player`).
        let acting = self.acting_player_uuid();
        if held_by_acting_player(
            self.mobs[index].extra.leash,
            self.mobs[index].extra.leash_player,
            acting,
        ) {
            let creative = self.creative;
            self.break_leash_used(id, !creative);
            self.lead_game_event("minecraft:entity_interact", at);
            self.lead_sound("untied", at);
            return Some(LeashInteraction::WithoutItem);
        }
        if item == "minecraft:lead" {
            let center = self.player_center();
            if self.can_attach_lead(index, center) {
                if self.mobs[index].extra.leash.is_some() {
                    self.break_leash_used(id, true);
                }
                let holder = self.acting_player_uuid();
                // LeashData.setLeashHolder: a holder still waiting in the save
                // is forgotten, so it can neither take over nor drop a lead.
                let extra = &mut self.mobs[index].extra;
                extra.leash = Some(Holder::Player);
                extra.leash_pending = None;
                extra.leash_player = Some(holder);
                self.lead_sound("tied", at);
                // `Player.interactOn` restores the count of a player with
                // infinite materials (creative); others spend the lead.
                self.consume_held();
                return Some(LeashInteraction::WithItem);
            }
        }
        None
    }
    pub(in crate::game) fn lead_sound(&mut self, sound: &str, at: [f64; 3]) {
        self.audio
            .0
            .play(&format!("minecraft:item.lead.{sound}"), Some(at), 1.0, 1.0);
    }
    /// `LeashFeatureRenderer` for every lead and `LeashKnotRenderer` for the knots.
    pub(in crate::game) fn leash_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
    ) {
        let atlas = &self.gui.atlas;
        let white = atlas.font.white();
        let uv = [
            [white.u0, white.v1],
            [white.u1, white.v1],
            [white.u1, white.v0],
            [white.u0, white.v0],
        ];
        // A Polytone pack's `entity/lead.png`: each of the 24 segments, from the leashed end,
        // takes its row of the texture, unshaded (measured on the real mod).
        let lead = atlas.sprites.get("entity/lead").copied();
        let lerp3 = |a: sunrise_core::geometry::Vec3, b: sunrise_core::geometry::Vec3| {
            glam::DVec3::new(
                a.x + (b.x - a.x) * alpha,
                a.y + (b.y - a.y) * alpha,
                a.z + (b.z - a.z) * alpha,
            )
        };
        let mut knots: Vec<[i32; 3]> = Vec::new();
        // EntityRenderer.extractRenderState: a leashed mob or boat (an
        // `AbstractBoat` is `Leashable` too) with where it hangs.
        let body_of = |previous: f32, current: f32| {
            f64::from(
                previous + sunrise_client::mobs::living::wrap(current - previous) * alpha as f32,
            )
            .to_radians()
        };
        let mut ropes: Vec<Rope> = Vec::new();
        for m in &self.mobs {
            if let Some(holder) = m.extra.leash {
                // A mob a mod's pre hook canceled draws no lead (it is part of the mob's own
                // render), but the fence knot it hangs from is an entity of its own.
                if self.render_canceled(m.id) || self.hides_as_camera(m.id) {
                    if let Holder::Knot(p) = holder
                        && !knots.contains(&p)
                    {
                        knots.push(p);
                    }
                    continue;
                }
                ropes.push(Rope {
                    holder,
                    here: lerp3(m.previous, m.position),
                    body: body_of(m.previous_body_yaw, m.body_yaw),
                    offset: leash::leash_offset(m),
                    quad: leash::supports_quad_leash(m.species())
                        .then(|| leash::quad_leash_offsets(m)),
                });
            }
        }
        for b in &self.boats {
            if let Some(holder) = b.leash {
                if self.hides_as_camera(b.id) {
                    if let Holder::Knot(p) = holder
                        && !knots.contains(&p)
                    {
                        knots.push(p);
                    }
                    continue;
                }
                ropes.push(Rope {
                    holder,
                    here: lerp3(b.previous, b.position),
                    // Entity.getPreciseBodyRotation: the boat's yaw.
                    body: body_of(b.previous_yaw, b.yaw),
                    offset: leash::boat_leash_offset(),
                    quad: Some(leash::boat_quad_leash_offsets()),
                });
            }
        }
        for rope in ropes {
            let Rope {
                holder,
                here,
                body,
                offset,
                quad,
            } = rope;
            // One lead from the leash offset to the holder's rope hold
            // position, or, when the holder is a happy ghast and the leashee
            // supports a quad lead, four from `getQuadLeashOffsets` to
            // `getQuadLeashHolderOffsets`.
            let mut ends: Vec<(glam::DVec3, glam::DVec3, bool)> = Vec::new();
            let quad = match (holder, quad) {
                (Holder::Mob(id), Some(offsets)) => self
                    .mobs
                    .iter()
                    .find(|o| o.id == id)
                    .and_then(|o| leash::quad_holder_offsets(o).map(|h| (o, h, offsets))),
                _ => None,
            };
            if let Some((roper, holder_points, own)) = quad {
                let roper_body = body_of(roper.previous_body_yaw, roper.body_yaw);
                let holder_pos = lerp3(roper.previous, roper.position);
                for (mine, theirs) in own.iter().zip(holder_points) {
                    let start = here + rot_y(glam::DVec3::from_array(*mine), -body);
                    let end = holder_pos + rot_y(glam::DVec3::from_array(theirs), -roper_body);
                    ends.push((start, end, false));
                }
            } else {
                let offset = rot_y(glam::DVec3::from_array(offset), -body);
                let start = here + offset;
                let end = match holder {
                    Holder::Knot(p) => {
                        if !knots.contains(&p) {
                            knots.push(p);
                        }
                        glam::DVec3::new(
                            f64::from(p[0]) + 0.5,
                            f64::from(p[1]) + 0.375 + 0.2,
                            f64::from(p[2]) + 0.5,
                        )
                    }
                    Holder::Mob(id) => {
                        let Some(o) = self.mobs.iter().find(|o| o.id == id) else {
                            continue;
                        };
                        lerp3(o.previous, o.position)
                            + glam::DVec3::new(0.0, o.eye_height() * 0.7, 0.0)
                    }
                    Holder::Boat(id) => {
                        let Some(o) = self.boats.iter().find(|o| o.id == id) else {
                            continue;
                        };
                        lerp3(o.previous, o.position) + boat_hold()
                    }
                    Holder::Player => {
                        let Some(p) = self.player.as_ref() else {
                            continue;
                        };
                        self.player_rope_hold(p, alpha)
                    }
                };
                ends.push((start, end, true));
            }
            let light = |p: glam::DVec3| {
                self.world
                    .light(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
            };
            // Block and sky light averaged between the two ends.
            let (a, b) = (light(ends[0].0), light(ends[0].1));
            let packed = pack_light((((a >> 4) + (b >> 4)) / 2) << 4 | (((a & 15) + (b & 15)) / 2));
            for (start, end, slack) in ends {
                let d = end - start;
                let k = 1.0 / (d.x * d.x + d.z * d.z).sqrt().max(1e-6) * 0.05 / 2.0;
                let (dx_off, dz_off) = (d.z * k, d.x * k);
                let point = |step: i32, fudge: f64| {
                    let progress = f64::from(step) / 24.0;
                    // LeashFeatureRenderer.addVertexPair: a quad lead is straight.
                    let y = if !slack {
                        d.y * progress
                    } else if d.y > 0.0 {
                        d.y * progress * progress
                    } else {
                        d.y - d.y * (1.0 - progress) * (1.0 - progress)
                    };
                    let base = start + glam::DVec3::new(d.x * progress, y, d.z * progress);
                    (
                        base + glam::DVec3::new(-dx_off, fudge, dz_off),
                        base + glam::DVec3::new(dx_off, 0.05 - fudge, -dz_off),
                    )
                };
                for (fudge, backwards) in [(0.05, false), (0.0, true)] {
                    for step in 0..24 {
                        let shade = |s: i32| {
                            let dark = s % 2 == i32::from(backwards);
                            let f = if dark { 0.7 } else { 1.0 };
                            abgr(0.5 * f, 0.4 * f, 0.3 * f)
                        };
                        let (a0, b0) = point(step, fudge);
                        let (a1, b1) = point(step + 1, fudge);
                        let (color, rows) = match lead {
                            Some(r) => {
                                let v = |s: i32| r.v0 + (r.v1 - r.v0) * s as f32 / 24.0;
                                let (top, bottom) = (v(step), v(step + 1));
                                let near =
                                    [[r.u0, top], [r.u1, top], [r.u1, bottom], [r.u0, bottom]];
                                let far =
                                    [[r.u0, bottom], [r.u1, bottom], [r.u1, top], [r.u0, top]];
                                (0xFFFF_FFFF, [near, far])
                            }
                            None => (shade(step), [uv; 2]),
                        };
                        // Both faces: the leash is not culled.
                        for (quad, uv) in [[a0, b0, b1, a1], [a1, b1, b0, a0]].into_iter().zip(rows)
                        {
                            for (v, uv) in quad.iter().zip(uv) {
                                out.push(Vertex {
                                    normal: [0; 4],
                                    pos: (*v - camera).as_vec3().to_array(),
                                    uv,
                                    layer: 0,
                                    color,
                                    light: packed,

                                    glint_uv: [0.0, 0.0],
                                });
                            }
                        }
                    }
                }
            }
        }
        // The knots: a 6 by 8 by 6 pixel block on the fence post.
        let Some(model) = self.client.entity_models.leash_knot.as_ref() else {
            return;
        };
        let Some(r) = atlas.sprites.get("entity/lead_knot/lead_knot").copied() else {
            return;
        };
        for p in knots {
            if self
                .client
                .remote_knots
                .iter()
                .any(|(&id, &pos)| pos == p && self.hides_as_camera(id))
            {
                continue;
            }
            let state = leash::knot_state(p);
            let at =
                glam::DVec3::new(state.position.x, state.position.y, state.position.z) - camera;
            let base = glam::Mat4::from_translation(at.as_vec3())
                * glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0));
            let poses = sunrise_client::mobs::anim::Poses::new(model);
            let light = pack_light(self.world.light(p[0], p[1], p[2]));
            sunrise_client::mobs::render::draw(
                model,
                base,
                &poses,
                [r.u0, r.v0, r.u1, r.v1],
                0xFFFF_FFFF,
                light,
                0,
                out,
            );
        }
    }
    /// `Player.getRopeHoldPosition` (and `LocalPlayer`'s in first person).
    pub(in crate::game) fn player_rope_hold(
        &self,
        p: &sunrise_client::motion::PlayerMotion,
        alpha: f64,
    ) -> glam::DVec3 {
        // Player.getRopeHoldPosition: `0.22 * (RIGHT ? -1 : 1)`.
        let hand_dir = if self.settings.main_hand_left {
            1.0
        } else {
            -1.0
        };
        if self.client.camera_type.is_first_person() {
            let (yaw, pitch) = rope_hold_angles(
                (self.yaw, self.pitch),
                self.client.rope_rot_old,
                alpha as f32,
            );
            let v = glam::DVec3::new(0.39 * hand_dir, -0.6, 0.3);
            // Vec3.xRot(-pitch) then yRot(-yaw).
            let (s, c) = ((-pitch).sin(), (-pitch).cos());
            let v = glam::DVec3::new(v.x, v.y * c + v.z * s, v.z * c - v.y * s);
            // Entity.getEyePosition(partialTick).
            let feet = self.previous_position.lerp(p.position, alpha);
            let eye = glam::DVec3::new(feet.x, feet.y + f64::from(p.dimensions.eye_height), feet.z);
            return rot_y(v, -yaw) + eye;
        }
        // Player.getRopeHoldPosition: the view's pitch eased toward `xRotO`,
        // the body's yaw between `yBodyRotO` and `yBodyRot`, from
        // `getPosition(partialTick)`.
        let (_, x_rot) = rope_hold_angles(
            (self.yaw, self.pitch),
            self.client.rope_rot_old,
            alpha as f32,
        );
        let t = alpha as f32;
        let y_rot = self.client.player_model.as_ref().map_or(
            f64::from(self.yaw * (std::f32::consts::PI / 180.0)),
            |m| {
                f64::from(
                    (m.previous_body_yaw + t * (m.body_yaw - m.previous_body_yaw))
                        * (std::f32::consts::PI / 180.0),
                )
            },
        );
        let feet = self.previous_position.lerp(p.position, alpha);
        let pos = glam::DVec3::new(feet.x, feet.y, feet.z);
        let x_rotated = |v: glam::DVec3| {
            // Vec3.xRot(-xRot).
            let (s, c) = ((-x_rot).sin(), (-x_rot).cos());
            glam::DVec3::new(v.x, v.y * c + v.z * s, v.z * c - v.y * s)
        };
        if p.fall_flying || p.auto_spin_attack_ticks > 0 {
            // getViewVector(partialTick) against getDeltaMovement, flat.
            let look = sunrise_world::view_direction::view_vector(
                self.client.rope_rot_old.1 + t * (self.pitch - self.client.rope_rot_old.1),
                self.client.rope_rot_old.0 + t * (self.yaw - self.client.rope_rot_old.0),
            );
            let movement = p.velocity;
            let speed_len = movement.x * movement.x + movement.z * movement.z;
            let look_len = look.x * look.x + look.z * look.z;
            let z_rot = if speed_len > 0.0 && look_len > 0.0 {
                let dot =
                    (movement.x * look.x + movement.z * look.z) / (speed_len * look_len).sqrt();
                let sign = movement.x * look.z - movement.z * look.x;
                // Math.signum: 0 for 0.
                let signum = if sign > 0.0 {
                    1.0
                } else if sign < 0.0 {
                    -1.0
                } else {
                    0.0
                };
                f64::from((signum * sunrise_core::jmath::acos(dot)) as f32)
            } else {
                0.0
            };
            // Vec3(xOff, -0.11, 0.85).zRot(-zRot).xRot(-xRot).yRot(-yRot).
            let v = glam::DVec3::new(0.22 * hand_dir, -0.11, 0.85);
            let (s, c) = ((-z_rot).sin(), (-z_rot).cos());
            let v = glam::DVec3::new(v.x * c + v.y * s, v.y * c - v.x * s, v.z);
            return pos + rot_y(x_rotated(v), -y_rot);
        }
        // LivingEntity.isVisuallySwimming.
        if p.pose == Pose::Swimming || (!p.fall_flying && p.pose == Pose::FallFlying) {
            let v = glam::DVec3::new(0.22 * hand_dir, 0.2, -0.15);
            return pos + rot_y(x_rotated(v), -y_rot);
        }
        let crouching = self.held_mapping(Action::Sneak);
        let v = glam::DVec3::new(
            0.22 * hand_dir,
            f64::from(p.dimensions.height) - 1.0,
            if crouching { -0.2 } else { 0.07 },
        );
        pos + rot_y(v, -y_rot)
    }
    /// A click at the fence at `knot_pos`: `knot_hit` for `LeashFenceKnotEntity.interact`,
    /// otherwise `LeadItem.useOn`. Returns whether it was used.
    pub(in crate::game) fn use_knot(&mut self, knot_pos: [i32; 3], knot_hit: bool) -> bool {
        let knot = leash::knot_state(knot_pos).center;
        let center = glam::DVec3::new(knot.x, knot.y, knot.z);
        let at = [center.x, center.y, center.z];
        let existing: Vec<usize> = (0..self.mobs.len())
            .filter(|&i| self.mobs[i].extra.leash == Some(Holder::Knot(knot_pos)))
            .collect();
        let existing_boats = self.boats_on(Holder::Knot(knot_pos));
        let held_item = self.held().map(|s| s.item.to_string());
        let sneaking = self.held_mapping(Action::Sneak);
        // Shears snip every lead on the knot.
        if knot_hit && held_item.as_deref() == Some("minecraft:shears") {
            for i in existing {
                let id = self.mobs[i].id;
                self.break_leash_used(id, true);
            }
            self.drop_boat_leashes(&existing_boats);
            self.audio
                .0
                .play("minecraft:item.shears.snip", Some(at), 1.0, 1.0);
            self.damage_held_tool();
            return true;
        }
        let in_hand = self.player_leashed(center);
        let boats_in_hand = self.player_led_boats(center);
        // The knot is only made for the lead item, or when mobs already hang on it.
        if held_item.as_deref() != Some("minecraft:lead")
            && existing.is_empty()
            && existing_boats.is_empty()
            && in_hand.is_empty()
            && boats_in_hand.is_empty()
        {
            return false;
        }
        // A knot is made when a lead is tied to a fence that has none.
        if existing.is_empty()
            && existing_boats.is_empty()
            && (!in_hand.is_empty() || !boats_in_hand.is_empty())
            && self.hanging_place_cancelled("minecraft:leash_knot", knot_pos)
        {
            return true;
        }
        self.forget_untied_knots();
        let mut attached = self.tie_player_boats(center, Holder::Knot(knot_pos));
        let mut attached_mobs = Vec::new();
        for j in in_hand {
            if self.can_attach_lead(j, center) {
                self.mobs[j].extra.leash = Some(Holder::Knot(knot_pos));
                attached_mobs.push(j);
                attached = true;
            }
        }
        if attached {
            let knot_id = self.tie_knot(knot_pos);
            for j in attached_mobs {
                self.mobs[j].extra.leash_knot_id = Some(knot_id);
            }
            for i in self.boats_on(Holder::Knot(knot_pos)) {
                if self.boats[i].leash_knot_id.is_none() {
                    self.boats.update(i, |b| b.leash_knot_id = Some(knot_id));
                }
            }
        }
        if !attached && !sneaking && knot_hit {
            let player = self.player_center();
            let holder = self.acting_player_uuid();
            for i in existing {
                if self.can_attach_lead(i, player) {
                    self.mobs[i].extra.leash = Some(Holder::Player);
                    self.mobs[i].extra.leash_player = Some(holder);
                    attached = true;
                }
            }
            attached |= self.take_knot_boats(knot_pos);
        }
        if attached {
            let event_at = if knot_hit {
                [knot.x, knot.y - 0.25, knot.z]
            } else {
                knot_pos.map(|v| f64::from(v) + 0.5)
            };
            self.lead_game_event("minecraft:block_attach", event_at);
            self.lead_sound("tied", at);
            self.swing_arm();
        }
        attached
    }
}
