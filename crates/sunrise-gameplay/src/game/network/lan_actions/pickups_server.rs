use super::*;

impl crate::game::ServerGame<'_> {
    /// `Player.aiStep` touching arrows in the guest's box grown by (1, 0.5, 1):
    /// `AbstractArrow.playerTouch` for a guest, as `touch_arrows` does for the
    /// host's player. The guest's own client plays the pickup sound
    /// (`handleTakeItemEntity`).
    pub fn guest_arrows(&mut self, gi: usize) {
        use sunrise_sim::arrows::Touch;
        let Some(host) = self.server.lan.as_ref() else {
            return;
        };
        let (conn, id, at, creative, unable, dims, player, name) = {
            let g = &host.hub.guests[gi];
            (
                g.conn,
                g.entity_id,
                g.position,
                g.creative(),
                g.spectator() || g.dead || g.health <= 0.0,
                g.dimensions(),
                g.uuid,
                g.name.clone(),
            )
        };
        if unable || self.common.creatures.projectiles.is_empty() {
            return;
        }
        let w = f64::from(dims.width) / 2.0;
        let reach = Aabb::new(
            Vec3::new(at[0] - w, at[1], at[2] - w),
            Vec3::new(at[0] + w, at[1] + f64::from(dims.height), at[2] + w),
        )
        .inflate(Vec3::new(1.0, 0.5, 1.0));
        let mut projectiles = self.common.creatures.projectiles.lift();
        #[cfg(any(test, feature = "test-support"))]
        crate::game::server_side::entity_server::taken_pass_tests::after_take(self);
        let mut picked = Vec::new();
        projectiles.retain(|shot| {
            let Some(a) = shot.arrow.as_ref().filter(|a| !a.removed) else {
                return true;
            };
            let p = shot.position;
            let bounds = Aabb::new(
                Vec3::new(p.x - 0.25, p.y, p.z - 0.25),
                Vec3::new(p.x + 0.25, p.y + 0.5, p.z + 0.25),
            );
            if !bounds.intersects(reach) {
                return true;
            }
            let taken = match a.player_touch(creative) {
                Touch::Ignore => false,
                Touch::Take => true,
                // Inventory.add: a creative player takes what does not fit.
                Touch::Give(stack) => {
                    self.as_guest(conn, |g| g.add_stack_to_inventory(stack))
                        .flatten()
                        .is_none()
                        || creative
                }
            };
            if taken {
                picked.push(shot.clone());
            }
            !taken
        });
        self.common.creatures.projectiles.restore(
            projectiles,
            sunrise_sim::join_seq::RestoreOrder::SurvivorsFirst,
        );
        for shot in picked {
            if let Some(arrow) = shot.arrow.as_ref() {
                self.native_player_pickup_arrow(player, &name, shot.id, &arrow.pickup_item.item);
            }
            if let Some(host) = self.server.lan.as_ref() {
                host.hub.take_item(shot.id as i32, id, 1, at);
            }
        }
    }
    /// `Player.tick`'s takeXpDelay and `Player.aiStep`'s touch of experience orbs.
    pub fn guest_orbs(&mut self, gi: usize) {
        let Some(host) = self.server.lan.as_mut() else {
            return;
        };
        let g = &host.hub.guests[gi];
        let own = g.bounds();
        let vehicle = g.vehicle;
        let (conn, id, at, player, name) =
            (g.conn, g.entity_id, g.position, g.uuid, g.name.clone());
        let unable = g.spectator() || g.dead || g.health <= 0.0;
        let play = host.play.entry(conn).or_default();
        let cooldown = if play.xp_delay > 0 {
            play.xp_delay -= 1;
            Some(play.xp_delay)
        } else {
            None
        };
        let delayed = play.xp_delay > 0;
        if let Some(cooldown) = cooldown {
            self.native_player_exp_cooldown_change(player, &name, cooldown);
        }
        if unable || delayed {
            return;
        }
        let area = match vehicle.and_then(|v| self.guest_target_box(gi, v as i32)) {
            Some(vehicle) => own.union_bounds(vehicle).inflate(Vec3::new(1.0, 0.0, 1.0)),
            None => own.inflate(Vec3::new(1.0, 0.5, 1.0)),
        };
        let (min, max) = (
            [
                area.min_position().x,
                area.min_position().y,
                area.min_position().z,
            ],
            [
                area.max_position().x,
                area.max_position().y,
                area.max_position().z,
            ],
        );
        let touching: Vec<usize> = self
            .common
            .orbs
            .iter()
            .enumerate()
            .filter(|(_, o)| {
                o.position.x + 0.25 > min[0]
                    && o.position.x - 0.25 < max[0]
                    && o.position.y + 0.5 > min[1]
                    && o.position.y < max[1]
                    && o.position.z + 0.25 > min[2]
                    && o.position.z - 0.25 < max[2]
            })
            .map(|(i, _)| i)
            .collect();
        if touching.is_empty() {
            return;
        }
        let pick = self
            .common
            .world
            .random
            .next_int_bound(touching.len() as i32)
            .unwrap_or(0) as usize;
        let index = touching[pick];
        let mut orb = self.common.orbs[index].clone();
        let mut canceled = false;
        let listened = (self.server.serving.is_some() || self.common.options.dedicated.is_some())
            && mod_api::__private::listened(mod_api::HookId::PickupXp);
        if listened {
            let dimension = self.common.dim.id();
            let called = self.as_guest(conn, |game| {
                let player = game.native_acting_id();
                game.native_pickup_xp(
                    player,
                    dimension,
                    orb.id,
                    &mut orb.value,
                    &mut orb.count,
                    &mut canceled,
                );
            });
            if called.is_none() || canceled {
                return;
            }
            orb.count = orb.count.max(1);
            self.common.orbs[index].value = orb.value;
            self.common.orbs[index].count = orb.count;
        }
        if orb.count <= 1 {
            self.remove_orb(index);
        } else {
            self.common.orbs[index].count -= 1;
        }
        if let Some(host) = self.server.lan.as_mut() {
            host.play.entry(conn).or_default().xp_delay = 2;
        }
        self.native_player_exp_cooldown_change(player, &name, 2);
        if let Some(host) = self.server.lan.as_ref() {
            host.hub.take_item(orb.id as i32, id, 1, at);
        }
        let mut random = sunrise_level::random_source::create();
        let pitch = (random.next_float() - random.next_float()) * 0.35 + 0.9;
        let p = orb.position;
        self.server_play_sound(
            "minecraft:entity.experience_orb.pickup",
            [p.x, p.y, p.z],
            0.1,
            pitch,
        );
        let value = orb.value.max(0) as u32;
        self.as_guest(conn, |game| {
            let rest = game.mending(value);
            if rest > 0 {
                game.give_experience_points(rest.min(i32::MAX as u32) as i32);
            }
        });
    }
    /// Dropped items touching a guest go into their inventory
    /// (`ItemEntity.playerTouch`, then `ServerPlayer.onItemPickup`).
    pub fn guest_pickups(&mut self, gi: usize) {
        let Some(host) = self.server.lan.as_ref() else {
            return;
        };
        let (conn, mode, position, id, uuid) = {
            let g = &host.hub.guests[gi];
            (
                g.conn,
                g.game_mode,
                g.position,
                g.entity_id,
                sunrise_sim::lan::playerdata::uuid_ints(g.uuid),
            )
        };
        if mode == SPECTATOR || self.common.dropped.is_empty() {
            return;
        }
        // The test below takes items whose position is less than 1.5 from
        // the player's on x and z: their boxes reach these columns.
        if let Some(columns) = self.server.pickup_columns.as_mut()
            && !columns.may_reach_area(
                &self.common.dropped,
                (
                    (position[0] - 1.5).floor() as i32,
                    (position[0] + 1.5).floor() as i32,
                ),
                (
                    (position[2] - 1.5).floor() as i32,
                    (position[2] + 1.5).floor() as i32,
                ),
            )
        {
            return;
        }
        let listens = mod_api::__private::listened(mod_api::HookId::ItemPickedUp);
        let near: Vec<_> = self
            .common
            .dropped
            .iter()
            .filter(|d| {
                (listens || (d.pickup_delay == 0 && d.target.is_none_or(|t| t == uuid)))
                    && (d.position.x - position[0]).abs() < 1.5
                    && (d.position.z - position[2]).abs() < 1.5
                    && d.position.y > position[1] - 0.6
                    && d.position.y < position[1] + 2.4
            })
            .map(|d| d.id)
            .collect();
        let mut taken = Vec::new();
        for item_id in near.into_iter().rev() {
            let Some(mut i) = self.common.dropped.iter().position(|d| d.id == item_id) else {
                continue;
            };
            let answer = if listens {
                let answer = self
                    .as_guest(conn, |g| g.native_pickup_answer(item_id))
                    .unwrap_or_default();
                // Only a dispatched callback can invalidate the original index.
                let Some(at) = self.common.dropped.iter().position(|d| d.id == item_id) else {
                    continue;
                };
                i = at;
                answer
            } else {
                mod_api::TriState::Default
            };
            let d = &mut self.common.dropped[i];
            if !Self::pickup_decision(&mut d.pickup_delay, d.target, uuid, answer) {
                continue;
            }
            let stack = self.common.dropped[i].stack.clone();
            let before = stack.count;
            let kind = stack.item.clone();
            let thrower = self.common.dropped[i].thrower;
            let Some(rest) = self.as_guest(conn, |g| g.native_pickup_insert(&stack)) else {
                continue;
            };
            // Where the entity is and what it holds once the pickup is told: `None` when a mod
            // removed it.
            let after = match rest {
                Some(rest) if rest.count >= before => continue,
                // ItemEntityPickupEvent.Post, on a whole or partial pickup: the remainder is the
                // entity's while mods hear it, and the count after comes from the entity.
                rest if mod_api::__private::listened(mod_api::HookId::ItemPickedUpPost) => {
                    match &rest {
                        Some(rest) => self.common.dropped[i].stack = rest.clone(),
                        None => self.common.dropped[i].stack.count = 0,
                    }
                    let told = self.as_guest(conn, |g| g.native_pickup_post(item_id, &stack, rest));
                    let at = self.common.dropped.iter().position(|d| d.id == item_id);
                    if let (Some(at), Some(remaining)) = (at, told) {
                        match remaining {
                            Some(remaining) => self.common.dropped[at].stack = remaining,
                            None => self.common.dropped[at].stack.count = 0,
                        }
                    }
                    at
                }
                Some(rest) => {
                    self.common.dropped[i].stack = rest;
                    Some(i)
                }
                None => {
                    self.common.dropped[i].stack.count = 0;
                    Some(i)
                }
            };
            let left = after.map_or(0, |at| self.common.dropped[at].stack.count);
            let count = before.saturating_sub(left);
            if count == 0 {
                continue;
            }
            let picked = match after {
                Some(at) if left > 0 => self.common.dropped[at].stack.clone(),
                // ItemEntity.playerTouch: an emptied stack is discarded with the count taken.
                Some(at) => {
                    let mut picked = self.remove_dropped(at).stack;
                    picked.count = count;
                    picked
                }
                None => {
                    let mut picked = stack;
                    picked.count = count;
                    picked
                }
            };
            self.as_guest(conn, |g| {
                g.award_stat("picked_up", &kind, count as i32);
                if let Some(thrower) = thrower {
                    g.trigger_thrown_picked_up_by_player(&picked, thrower);
                }
            });
            if after.is_some() {
                taken.push((item_id, count));
            }
        }
        // Entity.take: the clients play the pickup sound themselves
        // (handleTakeItemEntity).
        if let Some(host) = self.server.lan.as_ref() {
            for (item, count) in taken {
                host.hub.take_item(item as i32, id, count as i32, position);
            }
        }
    }
}

use sunrise_gameplay_common::game::held_items::HeldItemsGameExt1 as _;

use sunrise_gameplay_common::game::native_hooks::ModGameExt1 as _;

use sunrise_gameplay_common::game::GameGameExt1 as _;
