#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::pickups::*;

impl Game<'_> {
    /// Dropped items touching a guest go into their inventory
    /// (`ItemEntity.playerTouch`, then `ServerPlayer.onItemPickup`).
    pub(in crate::game) fn guest_pickups(&mut self, gi: usize) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let (conn, mode, position, id, uuid) = {
            let g = &host.hub.guests[gi];
            (
                g.conn,
                g.game_mode,
                g.position,
                g.entity_id,
                sunrise_client::lan::playerdata::uuid_ints(g.uuid),
            )
        };
        if mode == SPECTATOR || self.dropped.is_empty() {
            return;
        }
        // The test below takes items whose position is less than 1.5 from
        // the player's on x and z: their boxes reach these columns.
        if let Some(columns) = self.server.pickup_columns.as_mut()
            && !columns.may_reach_area(
                &self.dropped,
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
            let Some(mut i) = self.dropped.iter().position(|d| d.id == item_id) else {
                continue;
            };
            let answer = if listens {
                let answer = self
                    .as_guest(conn, |g| g.native_pickup_answer(item_id))
                    .unwrap_or_default();
                // Only a dispatched callback can invalidate the original index.
                let Some(at) = self.dropped.iter().position(|d| d.id == item_id) else {
                    continue;
                };
                i = at;
                answer
            } else {
                mod_api::TriState::Default
            };
            let d = &mut self.dropped[i];
            if !Self::pickup_decision(&mut d.pickup_delay, d.target, uuid, answer) {
                continue;
            }
            let stack = self.dropped[i].stack.clone();
            let before = stack.count;
            let kind = stack.item.clone();
            let thrower = self.dropped[i].thrower;
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
                        Some(rest) => self.dropped[i].stack = rest.clone(),
                        None => self.dropped[i].stack.count = 0,
                    }
                    let told = self.as_guest(conn, |g| g.native_pickup_post(item_id, &stack, rest));
                    let at = self.dropped.iter().position(|d| d.id == item_id);
                    if let (Some(at), Some(remaining)) = (at, told) {
                        match remaining {
                            Some(remaining) => self.dropped[at].stack = remaining,
                            None => self.dropped[at].stack.count = 0,
                        }
                    }
                    at
                }
                Some(rest) => {
                    self.dropped[i].stack = rest;
                    Some(i)
                }
                None => {
                    self.dropped[i].stack.count = 0;
                    Some(i)
                }
            };
            let left = after.map_or(0, |at| self.dropped[at].stack.count);
            let count = before.saturating_sub(left);
            if count == 0 {
                continue;
            }
            let picked = match after {
                Some(at) if left > 0 => self.dropped[at].stack.clone(),
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
        if let Some(host) = self.lan.as_ref() {
            for (item, count) in taken {
                host.hub.take_item(item as i32, id, count as i32, position);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/pickup_post_tests_server.rs"]
pub(in crate::game) mod pickup_post_tests;
