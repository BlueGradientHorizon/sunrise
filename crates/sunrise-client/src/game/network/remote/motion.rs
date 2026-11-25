#[allow(unused_imports)]
use super::movement::stepped_move;
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::entity::Entity;

impl Game<'_> {
    /// `move_entity` with an explicit old pose (`setValuesFromPositionPacket`'s snapped old pose).
    pub(in crate::game) fn move_entity_with_old(
        &mut self,
        id: i32,
        path: Option<Path>,
        rotation: Option<(f32, f32)>,
        on_ground: bool,
        packet: MovePacket,
        snap_old: Option<PosRot>,
    ) {
        // The dragon is drawn from its own state (game/mobs/dragon.rs), which
        // tick_remote_dragon eases to the server's moves through `lerps`. Its
        // copy in the mob list draws nothing: given the move, it kept the
        // dragon where it spawned (#2083).
        let dragon = self.dragon.as_ref().is_some_and(|d| d.id as i32 == id);
        let driven = self.driven_mount().is_some_and(|m| m.id as i32 == id)
            || u32::try_from(id)
                .is_ok_and(|id| self.controls_boat(id) && self.boats.iter().any(|b| b.id == id));
        let mob = self
            .mob_index
            .get_mut(&mut self.mobs, id as u32)
            .filter(|_| !dragon);
        let tracked = self
            .remote
            .as_ref()
            .and_then(|r| r.entities.get(&id))
            .map(|t| t.position);
        let position = path
            .as_ref()
            .map(Path::end)
            .or(tracked)
            .or_else(|| {
                mob.as_ref()
                    .map(|m| [m.position.x, m.position.y, m.position.z])
            })
            .unwrap_or([0.0; 3]);
        if self.remote.is_some() {
            if let Some(r) = self.remote.as_mut() {
                if let Some(t) = r.entities.get_mut(&id) {
                    let d = [0, 1, 2].map(|i| position[i] - t.position[i]);
                    let snap = matches!(packet, MovePacket::PositionSyncSnap)
                        || (matches!(packet, MovePacket::Teleport | MovePacket::PositionSync)
                            && d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > 4096.0);
                    if path.is_some() {
                        t.position = position;
                    }
                    if let Some((yaw, pitch)) = rotation {
                        if yaw.is_finite() {
                            t.yaw = if snap { yaw } else { yaw % 360.0 };
                        }
                        if pitch.is_finite() {
                            t.pitch = (pitch % 360.0).clamp(-90.0, 90.0);
                        }
                    }
                    if snap {
                        if let Some(old) = snap_old {
                            t.previous = old.position;
                            t.previous_yaw = if old.yaw.is_finite() { old.yaw } else { t.yaw };
                            t.previous_pitch = if old.pitch.is_finite() {
                                (old.pitch % 360.0).clamp(-90.0, 90.0)
                            } else {
                                t.pitch
                            };
                        } else {
                            t.previous = t.position;
                            t.previous_yaw = t.yaw;
                            t.previous_pitch = t.pitch;
                        }
                    }
                }
                if driven {
                    // Authoritative client mounts/boats keep their own position and interpolation.
                    return;
                }
                if let Some(m) = mob {
                    // Shulker uses NO_OP interpolation: setPos immediately, keeping old block.
                    if m.species() == sunrise_client::mobs::Species::Shulker {
                        r.stepped.remove(&id);
                        if path.is_some() {
                            sunrise_client::mobs::shulker::client_set_pos(
                                m,
                                position[0],
                                position[1],
                                position[2],
                            );
                        }
                        if let Some((yaw, pitch)) = rotation {
                            m.yaw += wrap_degrees(yaw - m.yaw);
                            m.head_pitch = (pitch % 360.0).clamp(-90.0, 90.0);
                        }
                        let _ = m.update_retained_ground(
                            &self.world,
                            self.collision,
                            &world_items!(self).entity_type_tags,
                            on_ground,
                            None,
                        );
                        return;
                    }
                    // LivingEntity SteppedInterpolationHandler over its type's update interval.
                    let entity = PosRot {
                        position: [m.position.x, m.position.y, m.position.z],
                        yaw: m.yaw,
                        pitch: m.head_pitch,
                    };
                    let handler = r.stepped.entry(id).or_insert_with(|| {
                        let interval = sunrise_client::lan::tracking::settings(&m.kind.id).1;
                        SteppedInterpolation::new(interval as i32)
                    });
                    if let Some(at) =
                        stepped_move(handler, packet, entity, position, path, rotation)
                    {
                        place_interpolated(m, at);
                        if let Some(old) = snap_old {
                            m.previous =
                                Vec3::new(old.position[0], old.position[1], old.position[2]);
                            m.previous_yaw = old.yaw;
                            m.previous_head_pitch = (old.pitch % 360.0).clamp(-90.0, 90.0);
                        } else {
                            m.previous = m.position;
                            m.previous_yaw = m.yaw;
                            m.previous_head_pitch = m.head_pitch;
                        }
                    }
                    // ClientPacketListener sets onGround after moving or queuing interpolation.
                    let _ = m.update_retained_ground(
                        &self.world,
                        self.collision,
                        &world_items!(self).entity_type_tags,
                        on_ground,
                        None,
                    );
                    return;
                }
                // Armor stands use the same stepped handler in `tick_remote_entities`.
                if let Some(a) = self.armor_stands.iter_mut().find(|a| a.id == id as u32) {
                    let entity = PosRot {
                        position: [a.position.x, a.position.y, a.position.z],
                        yaw: a.yaw,
                        pitch: a.pitch,
                    };
                    let handler = r.stepped.entry(id).or_insert_with(|| {
                        let interval =
                            sunrise_client::lan::tracking::settings("minecraft:armor_stand").1;
                        SteppedInterpolation::new(interval as i32)
                    });
                    if let Some(at) =
                        stepped_move(handler, packet, entity, position, path, rotation)
                    {
                        a.snap(
                            Vec3::new(at.position[0], at.position[1], at.position[2]),
                            at.yaw,
                            at.pitch,
                        );
                        if let Some(old) = snap_old {
                            a.previous =
                                Vec3::new(old.position[0], old.position[1], old.position[2]);
                            a.previous_yaw = old.yaw;
                            a.previous_pitch = (old.pitch % 360.0).clamp(-90.0, 90.0);
                        }
                    }
                    let _ = a.update_retained_ground(
                        &self.world,
                        self.collision,
                        &world_items!(self).entity_type_tags,
                        on_ground,
                        None,
                    );
                    return;
                }
            }
            // Interaction keeps NO_OP interpolation: the server's position replaces its own at once.
            if self.side_move_remote_interaction(
                id,
                position,
                path.is_some(),
                rotation,
                packet,
                snap_old,
            ) {
                return;
            }
            // Projectiles keep Entity's NO_OP InterpolationHandler too: the
            // server's position replaces theirs at once and their own tick
            // flies them on (`tick_remote_projectiles`). Eased towards it
            // instead, they only moved when an update came, every 10 to 20
            // ticks.
            if let Some(i) = self
                .creatures
                .projectiles
                .iter()
                .position(|p| p.id == id as u32)
            {
                self.creatures.projectiles.update(i, |p| {
                    if path.is_some() {
                        p.position = Vec3::new(position[0], position[1], position[2]);
                    }
                    if let Some((yaw, pitch)) = rotation {
                        p.yaw = yaw;
                        p.pitch = pitch;
                    }
                });
                return;
            }
            if path.is_some()
                && (self.potions.move_remote_thrown(
                    id as u32,
                    Vec3::new(position[0], position[1], position[2]),
                ) || self.end.move_remote_eye(id as u32, position))
            {
                return;
            }
            // A fishing hook keeps a LinearInterpolationHandler: eased towards the
            // update over three ticks, on top of its own movement (`tick_remote_hooks`).
            if path.is_some() {
                if let Some(h) = self.hook.as_mut().filter(|h| h.id() == id as u32) {
                    h.move_to(position, on_ground);
                    return;
                }
                if self.side_move_remote_hook(id as u32, position, on_ground) {
                    return;
                }
            }
            // A thrown trident is an arrow: placed at once, flown on by its tick.
            if let Some(t) = self.tridents.iter_mut().find(|t| t.id == id as u32) {
                if path.is_some() {
                    t.position = Vec3::new(position[0], position[1], position[2]);
                }
                if let Some((yaw, pitch)) = rotation {
                    t.yaw = yaw;
                    t.pitch = pitch;
                }
                return;
            }
            // So does a firework rocket: its own tick flies it on.
            if path.is_some()
                && let Some(f) = self.rockets.iter_mut().find(|f| f.id == id as u32)
            {
                f.position = Vec3::new(position[0], position[1], position[2]);
                return;
            }
            if let Some(p) = self
                .remote
                .as_mut()
                .and_then(|r| r.pending_items.get_mut(&id))
            {
                p.position = position;
            }
            // ItemEntity, FallingBlockEntity and PrimedTnt keep Entity's
            // NO_OP InterpolationHandler: moveOrInterpolateTo puts them there
            // at once, and their own tick moves them on between the server's
            // updates (`tick_remote_entities`, `tick_remote_projectiles`).
            let at = Vec3::new(position[0], position[1], position[2]);
            if let Some(d) = self.dropped.iter_mut().find(|d| d.id == id as u32) {
                d.position = at;
                d.ground.on_ground = on_ground;
                return;
            }
            if let Some(i) = self.falling.iter().rposition(|f| f.id == id as u32) {
                self.falling.update(i, |f| f.position = at);
                return;
            }
            if let Some(t) = self.tnts.iter_mut().rev().find(|t| t.id == id as u32) {
                t.position = at;
                t.ground.on_ground = on_ground;
                return;
            }
            // ExperienceOrb.createInterpolationHandler is a
            // LinearInterpolationHandler: the correction is spread over 3
            // ticks (`tick_remote_entities`), while `ExperienceOrb.tick` keeps
            // moving it.
            if let Some(o) = self.orbs.iter_mut().find(|o| o.id == id as u32) {
                o.ground.on_ground = on_ground;
                if let Some(r) = self.remote.as_mut() {
                    r.lerps.insert(
                        id,
                        Lerp {
                            target: position,
                            rotation: None,
                            steps: 3,
                        },
                    );
                }
                return;
            }
            let has_cart_lerp = self
                .remote
                .as_ref()
                .is_some_and(|r| r.cart_lerps.contains_key(&id));
            if has_cart_lerp {
                // A minecart on NewMinecartBehavior keeps Entity's NO_OP
                // InterpolationHandler: a teleport puts it there at once.
                if let Some(i) = self.carts.iter().position(|c| c.id == id as u32) {
                    self.carts.update(i, |c| {
                        if path.is_some() {
                            c.position = at;
                        }
                        if let Some((yaw, pitch)) = rotation {
                            c.yaw = yaw;
                            c.pitch = pitch;
                        }
                    });
                }
            } else if !self.side_has_remote_text_display(id)
                && let Some(r) = self.remote.as_mut()
            {
                // AbstractInterpolationHandler.interpolateTo keeps the heading rotation when absent.
                let rotation = rotation.or_else(|| r.lerps.get(&id).and_then(|l| l.rotation));
                r.lerps.insert(
                    id,
                    Lerp {
                        target: position,
                        rotation,
                        steps: 3,
                    },
                );
            }
        }
        self.turn_remote_object(id, rotation.map(|(yaw, _)| yaw), on_ground);
        match packet {
            MovePacket::PositionSync | MovePacket::PositionSyncSnap => {
                self.position_sync_remote_text_display(id, position, rotation);
            }
            MovePacket::Move | MovePacket::Teleport => {
                self.move_remote_text_display_opt(id, path.is_some().then_some(position), rotation);
            }
        }
        self.turn_remote_projectile(id, rotation);
    }
}
