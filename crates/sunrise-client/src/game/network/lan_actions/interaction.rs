#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    pub(in crate::game) fn guest_tick_death(&mut self, gi: usize) {
        let removed = self.lan.as_mut().and_then(|host| {
            host.hub.guests.update(gi, |guest| {
                if !guest.tick_death() || guest.death_time != 20 {
                    return None;
                }
                let dimensions = guest.dimensions();
                Some((
                    guest.entity_id,
                    guest.position,
                    dimensions,
                    guest.death_canceled,
                ))
            })
        });
        if let Some((entity, position, dimensions, death_canceled)) = removed {
            self.lan_entity_event(entity, 60, position);
            self.death_poof(
                position,
                f64::from(dimensions.width),
                f64::from(dimensions.height),
            );
            if !death_canceled {
                self.guest_drop_carried(gi);
            }
        }
    }

    pub(in crate::game) fn guest_drop_carried(&mut self, gi: usize) {
        let carried = self.with_own_record(gi, |host, _| {
            let conn = host.hub.guests[gi].conn;
            let mut stacks = host.hub.take_closed_menu_cursors(conn).collect::<Vec<_>>();
            let (to_return, to_drop) = host.hub.take_closed_processing_inputs(conn);
            stacks.extend(to_return);
            stacks.extend(to_drop);
            host.hub
                .guests
                .update(gi, |g| stacks.extend(g.take_menu_cursors()));
            Some(stacks)
        });
        if let Some(carried) = carried.filter(|stacks| !stacks.is_empty()) {
            self.drop_guest_items(gi, carried);
        }
    }

    pub(in crate::game) fn drop_guest_items(&mut self, gi: usize, stacks: Vec<ItemStack>) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let g = &host.hub.guests[gi];
        let eye = [
            g.position[0],
            g.position[1] + g.eye_height() - f64::from(0.3f32),
            g.position[2],
        ];
        let (yaw, pitch) = (g.yaw, g.pitch);
        let mut r = sunrise_client::random_source::entity(); // the guest's Entity.random
        for stack in stacks {
            let v = sunrise_client::entities::drop_motion(&mut r, yaw, pitch, false);
            self.spawn_item(stack, eye, v, 40);
        }
    }

    /// Whether a block outline occludes the straight line from `eye` to `point`.
    pub(in crate::game) fn block_between(&self, eye: [f64; 3], point: [f64; 3]) -> bool {
        let d = [point[0] - eye[0], point[1] - eye[1], point[2] - eye[2]];
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if dist < 1e-9 {
            return false;
        }
        let dir = [d[0] / dist, d[1] / dist, d[2] / dist];
        self.clip_ray(eye, dir, dist).is_some_and(|h| {
            let hd = [
                h.point[0] - eye[0],
                h.point[1] - eye[1],
                h.point[2] - eye[2],
            ];
            let hit_dist = (hd[0] * hd[0] + hd[1] * hd[1] + hd[2] * hd[2]).sqrt();
            hit_dist < dist - 1e-6
        })
    }

    /// Aim the swapped-in player at an entity; returns where it aims and the
    /// eye. A boat, minecart, armor stand, item frame or painting is aimed at
    /// the point the client hit (the interact packet's location) when that
    /// is on it, else at its centre.
    pub(in crate::game) fn look_at_entity(
        &mut self,
        gi: usize,
        id: i32,
        location: Option<[f64; 3]>,
    ) -> Option<([f64; 3], [f64; 3])> {
        let eye = {
            let g = &self.lan.as_ref()?.hub.guests[gi];
            [g.position[0], g.position[1] + g.eye_height(), g.position[2]]
        };
        let object = || {
            let (lo, hi, origin) = self.object_target(id)?;
            let middle = [0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0);
            let Some(l) = location else {
                // A boat's mobs sit over its middle: aim low at its near
                // side, so the ray meets the boat before them (the server
                // attacks the entity it is sent).
                let carries = u32::try_from(id).is_ok_and(|v| {
                    self.boats.iter().any(|b| b.id == v) && !self.boat_mobs(v).is_empty()
                });
                if carries {
                    let near = |k: usize| eye[k].clamp(lo[k] + 0.01, hi[k] - 0.01);
                    return Some([near(0), lo[1] + 0.05, near(2)]);
                }
                return Some(middle);
            };
            let point = [0, 1, 2].map(|k| origin[k] + l[k]);
            // A little inside the box, so the ray from the eye meets it.
            let inside = [0, 1, 2].map(|k| {
                let margin = ((hi[k] - lo[k]) / 2.0).min(0.01);
                point[k].clamp(lo[k] + margin, hi[k] - margin)
            });
            let on = (0..3).all(|k| (point[k] - inside[k]).abs() < 0.1);
            Some(if on { inside } else { middle })
        };
        let mob_box = self.mobs.iter().find(|m| m.id as i32 == id).map(|m| {
            let b = m.bounds();
            let (mn, mx) = (b.min_position(), b.max_position());
            ([mn.x, mn.y, mn.z], [mx.x, mx.y, mx.z])
        });
        let centre = match mob_box {
            Some((lo, hi)) => visible_aim_point(eye, lo, hi, |p| self.block_between(eye, p)),
            None if self.object_target(id).is_some() => object()?,
            // A dragon part or an end crystal (game/mobs/dragon.rs).
            None => {
                let b = self.tracked_special_box(id)?;
                let (lo, hi) = (b.min_position(), b.max_position());
                [
                    (lo.x + hi.x) / 2.0,
                    (lo.y + hi.y) / 2.0,
                    (lo.z + hi.z) / 2.0,
                ]
            }
        };
        let d = [centre[0] - eye[0], centre[1] - eye[1], centre[2] - eye[2]];
        let horizontal = (d[0] * d[0] + d[2] * d[2]).sqrt();
        self.lan.as_mut()?.hub.guests.update(gi, |g| {
            g.yaw = (-d[0].atan2(d[2])).to_degrees() as f32;
            g.pitch = (-d[1].atan2(horizontal)).to_degrees() as f32;
        });
        Some((centre, eye))
    }

    /// `Level.getEntityOrPart(id).getBoundingBox()` for a guest's attack or
    /// interaction: the host, another player, a mob, a vehicle or object, a
    /// dragon part or crystal, a projectile. None for an id the level doesn't
    /// have (the handlers then find nothing either).
    /// `WorldBorder.isWithinBounds(target.blockPosition())` for an entity of this box:
    /// the block its feet are in (the box's bottom centre).
    pub(in crate::game) fn inside_border(&self, target: &Aabb) -> bool {
        let feet = target.bottom_center();
        self.border.contains(feet.x.floor(), feet.z.floor())
    }

    pub(in crate::game) fn guest_target_box(&self, gi: usize, id: i32) -> Option<Aabb> {
        let host = self.lan.as_ref()?;
        let conn = host.hub.guests[gi].conn;
        if let Some(g) = host
            .hub
            .guests
            .simulation_iter()
            .find(|g| g.entity_id == id && g.conn != conn && host.hub.here(g))
        {
            return Some(g.bounds());
        }
        if let Some(m) = self.mobs.iter().find(|m| m.id as i32 == id) {
            return Some(m.bounds());
        }
        if let Some((lo, hi, _)) = self.object_target(id) {
            return Some(Aabb::new(
                Vec3::new(lo[0], lo[1], lo[2]),
                Vec3::new(hi[0], hi[1], hi[2]),
            ));
        }
        if let Some(b) = self.tracked_special_box(id) {
            return Some(b);
        }
        if let Some(knot) = self.knot_at_id(id) {
            let knot = sunrise_client::mobs::leash::knot_state(knot);
            let p = knot.position;
            let half = knot.width / 2.0;
            return Some(Aabb::new(
                Vec3::new(p.x - half, p.y, p.z - half),
                Vec3::new(p.x + half, p.y + knot.height, p.z + half),
            ));
        }
        self.creatures
            .projectiles
            .iter()
            .find(|p| p.id != 0 && p.id as i32 == id)
            .map(|p| p.bounds())
    }

    /// `ServerGamePacketListenerImpl.handleAttack` for guest `conn` (at `gi`) hitting
    /// entity `id`: the border and the attack range, the native mods' `attack_entity`, then
    /// `Player.attack` for a player, a mob or another entity, and the deflection of a fireball,
    /// a wind charge or a shulker bullet. True when the blow landed. A fake player's
    /// `PlayerRef::perform_action` runs it as the packet does.
    pub(in crate::game) fn guest_attack(
        &mut self,
        conn: u32,
        gi: usize,
        id: i32,
    ) -> Result<bool, ActionRefused> {
        let Some(host) = self.lan.as_ref() else {
            return Err(ActionRefused::Stale);
        };
        if host.hub.guests[gi].spectator() {
            return Ok(false);
        }
        // handleAttack: a target inside the world border, then
        // `isWithinAttackRange(mainHandItem, box, 3.0)` before anything else, players,
        // mobs, objects and fireballs alike.
        if let Some(target) = self.guest_target_box(gi, id) {
            if !self.inside_border(&target) {
                return Err(ActionRefused::Protected);
            }
            if self.as_guest_at(gi, conn, |g| g.within_attack_range(&target)) != Ok(true) {
                return Err(ActionRefused::OutOfReach);
            }
        }
        if self.guest_attack_stops(gi, id) {
            return Ok(false);
        }
        // Native mods, where Player.attack starts: the global `attack_entity`,
        // then the held item's `on_left_click_entity` Part. A canceled blow
        // does nothing (game/native_hooks.rs).
        let Some(host) = self.lan.as_ref() else {
            return Err(ActionRefused::Stale);
        };
        let aimed = if let Some(g) = host
            .hub
            .guests
            .iter()
            .find(|g| g.entity_id == id && g.conn != conn && host.hub.here(g))
        {
            Some(("minecraft:player".to_owned(), g.position))
        } else {
            // The mod hooks hear no natural fence knot (#13036).
            self.level_entity(id)
                .filter(|t| !matches!(t, crate::game::commands::Target::Knot(_)))
                .map(|t| {
                    (
                        self.target_type(t),
                        self.target_position(t).unwrap_or_default(),
                    )
                })
        };
        if let Some((kind, at)) = aimed
            && self.as_guest_at(gi, conn, |g| {
                let mut canceled = false;
                g.native_attack_entity(id, &kind, at, &mut canceled);
                canceled || g.native_left_click_entity_event(id, &kind)
            }) == Ok(true)
        {
            return Ok(false);
        }
        if let Some(victim) = self.pvp_victim(conn, id) {
            if let Ok(Some(strike)) = self.as_guest_at(gi, conn, |g| g.pvp_strike(id as u32))
                && self.pvp_land(victim, &strike)
            {
                let on_ground = self.pvp_victim_on_ground(victim);
                let stops_sprint = strike.stops_sprint();
                let sent = self
                    .as_guest_at(gi, conn, |g| {
                        g.remember_player_target(Some(id as u32));
                        g.pvp_hit_after(on_ground, stops_sprint)
                    })
                    .ok()
                    .flatten();
                self.send_guest_motion(conn, sent);
                return Ok(true);
            }
            return Ok(false);
        }
        // A ghast's fireball or a wind charge: turned along the guest's own
        // view (ServerGamePacketListenerImpl.handleInteract, then
        // Player.attack's AIM_DEFLECT).
        if let Some(i) = self
            .creatures
            .projectiles
            .iter()
            .position(|p| p.id != 0 && p.id as i32 == id && crate::game::deflect::attackable(p))
        {
            return Ok(self.as_guest_at(gi, conn, |g| {
                if !g.within_interaction_range(i) {
                    return false;
                }
                if matches!(
                    g.creatures.projectiles[i].kind,
                    sunrise_client::projectiles::ProjectileKind::ShulkerBullet { .. }
                ) {
                    g.hit_shulker_bullet(i);
                } else {
                    g.deflect(i);
                }
                true
            }) == Ok(true));
        }
        if let Some(index) = self
            .mobs
            .iter()
            .position(|m| m.id as i32 == id && m.alive())
            .filter(|_| self.dragon.as_ref().is_none_or(|d| d.id as i32 != id))
        {
            let Some(host) = self.lan.as_ref() else {
                return Err(ActionRefused::Stale);
            };
            if host.hub.guests[gi].vehicle == Some(self.mobs[index].id) {
                return Ok(false);
            }
            // A mace's smash sets the attacker's velocity and sends it at once, before the
            // Wind Burst's push (MaceItem.hurtEnemy, then the post-attack effects).
            let sent = self
                .as_guest_at(gi, conn, |g| {
                    let marked = g
                        .player
                        .as_mut()
                        .is_some_and(|p| std::mem::take(&mut p.health.sync_velocity));
                    g.attack_mob(index);
                    g.swing_arm();
                    let p = g.player.as_mut()?;
                    let sent = std::mem::replace(&mut p.health.sync_velocity, marked);
                    sent.then_some(p.velocity)
                })
                .ok()
                .flatten();
            self.send_guest_motion(conn, sent);
            // The mob now answers to this guest (game/network/lan_mobs.rs).
            if let Ok(id) = u32::try_from(id)
                && let Some(host) = self.lan.as_mut()
                && host.hub.guests[gi].participates_in_player_simulation()
            {
                host.mob_players.insert(id, conn);
            }
            return Ok(true);
        }
        let Some(host) = self.lan.as_ref() else {
            return Err(ActionRefused::Stale);
        };
        let (old_yaw, old_pitch) = (host.hub.guests[gi].yaw, host.hub.guests[gi].pitch);
        let boat = self.boats.iter().position(|boat| boat.id as i32 == id);
        let tnt = self
            .tnts
            .iter()
            .find(|tnt| tnt.id as i32 == id)
            .map(|tnt| tnt.id);
        let falling = self.falling.iter().any(|block| block.id as i32 == id);
        if falling {
            // FallingBlockEntity.isAttackable is false: Player.attack exits at cannotAttack.
            return Ok(false);
        }
        let general = match u32::try_from(id).ok().and_then(|id| self.general_row(id)) {
            Some(row) => {
                let catalog = crate::game::general::general_type_catalog();
                if !catalog.pickable(row.type_slot) {
                    return Ok(false);
                }
                Some((row.id, catalog.key(row.type_slot)))
            }
            None => None,
        };
        // A packet names its target already. Boats, primed TNT and general rows use that target
        // with the attacker's actual aim instead of casting a new ray at its center.
        if boat.is_none()
            && tnt.is_none()
            && general.is_none()
            && self.look_at_entity(gi, id, None).is_none()
        {
            return Err(ActionRefused::InvalidTarget);
        }
        // A mace's smash sets the attacker's velocity and sends it at once, before the
        // Wind Burst's push (MaceItem.hurtEnemy, then the post-attack effects).
        let sent = self
            .as_guest_at(gi, conn, |g| {
                let marked = g
                    .player
                    .as_mut()
                    .is_some_and(|p| std::mem::take(&mut p.health.sync_velocity));
                if let Some(index) = boat {
                    g.strike_boat(index);
                    g.swing_arm();
                } else if let Some(tnt_id) = tnt {
                    g.strike_general_row(tnt_id, "minecraft:tnt");
                } else if let Some((id, kind)) = general {
                    g.strike_general_row(id, kind);
                } else {
                    g.attack();
                }
                let p = g.player.as_mut()?;
                let sent = std::mem::replace(&mut p.health.sync_velocity, marked);
                sent.then_some(p.velocity)
            })
            .ok()
            .flatten();
        self.send_guest_motion(conn, sent);
        if tnt.is_some() || general.is_some() {
            // Primed TNT and plain rows refuse damage, even though the attack spent its charge.
            return Ok(false);
        }
        // The mob now answers to this guest (game/network/lan_mobs.rs).
        let Some(host) = self.lan.as_mut() else {
            return Err(ActionRefused::Stale);
        };
        if host.hub.guests[gi].participates_in_player_simulation()
            && let Ok(id) = u32::try_from(id)
        {
            host.mob_players.insert(id, conn);
        }
        {
            let value = old_yaw;
            host.hub.guests.update(gi, |guest| guest.yaw = value);
        }
        {
            let value = old_pitch;
            host.hub.guests.update(gi, |guest| guest.pitch = value);
        }
        Ok(true)
    }
}
