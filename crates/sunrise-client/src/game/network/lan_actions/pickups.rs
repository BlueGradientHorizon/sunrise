#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// `Player.aiStep` touching arrows in the guest's box grown by (1, 0.5, 1):
    /// `AbstractArrow.playerTouch` for a guest, as `touch_arrows` does for the
    /// host's player. The guest's own client plays the pickup sound
    /// (`handleTakeItemEntity`).
    pub(in crate::game) fn guest_arrows(&mut self, gi: usize) {
        use sunrise_client::arrows::Touch;
        let Some(host) = self.lan.as_ref() else {
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
        if unable || self.creatures.projectiles.is_empty() {
            return;
        }
        let w = f64::from(dims.width) / 2.0;
        let reach = Aabb::new(
            Vec3::new(at[0] - w, at[1], at[2] - w),
            Vec3::new(at[0] + w, at[1] + f64::from(dims.height), at[2] + w),
        )
        .inflate(Vec3::new(1.0, 0.5, 1.0));
        let mut projectiles = self.creatures.projectiles.lift();
        #[cfg(test)]
        crate::game::entity::taken_pass_tests::after_take(self);
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
        self.creatures.projectiles.restore(
            projectiles,
            sunrise_client::join_seq::RestoreOrder::SurvivorsFirst,
        );
        for shot in picked {
            if let Some(arrow) = shot.arrow.as_ref() {
                self.native_player_pickup_arrow(player, &name, shot.id, &arrow.pickup_item.item);
            }
            if let Some(host) = self.lan.as_ref() {
                host.hub.take_item(shot.id as i32, id, 1, at);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/pickup_post_tests.rs"]
pub(in crate::game) mod pickup_post_tests;

#[cfg(test)]
#[path = "tests/orb_merge_contract_tests.rs"]
pub(in crate::game) mod orb_merge_contract_tests;
