#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::pickups::*;

impl Game<'_> {
    /// `Player.tick`'s takeXpDelay and `Player.aiStep`'s touch of the
    /// experience orbs in the guest's box grown by 1, 0.5, 1: one of them at
    /// random, at most every 2 ticks (`ExperienceOrb.playerTouch`), repairs
    /// with Mending and gives the rest as points. Everyone near hears it.
    pub(in crate::game) fn guest_orbs(&mut self, gi: usize) {
        let Some(host) = self.lan.as_mut() else {
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
        // Util.getRandom(orbs, random).
        let pick = self
            .world
            .random
            .next_int_bound(touching.len() as i32)
            .unwrap_or(0) as usize;
        // A merged orb gives its value once per count and goes at zero.
        let index = touching[pick];
        let mut orb = self.orbs[index].clone();
        let mut canceled = false;
        // Swapping the guest in costs: only when a mod hears the pickup.
        let listened = (self.serving.is_some() || self.options.dedicated.is_some())
            && mod_api::__private::listened(mod_api::HookId::PickupXp);
        if listened {
            let dimension = self.dim.id();
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
            self.orbs[index].value = orb.value;
            self.orbs[index].count = orb.count;
        }
        if orb.count <= 1 {
            self.remove_orb(index);
        } else {
            self.orbs[index].count -= 1;
        }
        if let Some(host) = self.lan.as_mut() {
            host.play.entry(conn).or_default().xp_delay = 2;
        }
        self.native_player_exp_cooldown_change(player, &name, 2);
        if let Some(host) = self.lan.as_ref() {
            host.hub.take_item(orb.id as i32, id, 1, at);
        }
        // ClientPacketListener.handleTakeItemEntity's sound, at the orb.
        let mut random = sunrise_client::random_source::create(); // the listener's own stream
        let pitch = (random.next_float() - random.next_float()) * 0.35 + 0.9;
        let p = orb.position;
        self.server_play_sound("minecraft:entity.experience_orb.pickup", [p.x, p.y, p.z], 0.1, pitch);
        let value = orb.value.max(0) as u32;
        self.as_guest(conn, |game| {
            let rest = game.mending(value);
            if rest > 0 {
                game.give_experience_points(rest.min(i32::MAX as u32) as i32);
            }
        });
    }
}

#[cfg(test)]
#[path = "tests/pickup_post_tests_client.rs"]
pub(in crate::game) mod pickup_post_tests;
