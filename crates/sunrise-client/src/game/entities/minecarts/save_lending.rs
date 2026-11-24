//! Save visibility while authoritative rows are lent out.
#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// Accepted damage is distinct from survival, destruction, or TNT priming.
    pub(in crate::game) fn hurt_cart_result(
        &mut self,
        i: usize,
        mut damage: f32,
        creative: bool,
        fire: bool,
        hit: CartHit,
    ) -> bool {
        let id = self.carts[i].id;
        let join = self.carts[i].join;
        let target = crate::game::commands::Target::Cart(id);
        if self.persist.destroying_vehicles.contains(&target)
            || self.persist.preparing_vehicle_removals.contains(&target)
        {
            return false;
        }
        let kind = self.carts[i].kind.id();
        let mut cancel = false;
        self.native_vehicle_damage(kind, &mut damage, &mut cancel);
        if cancel {
            return false;
        }
        let Some(i) = self.carts.iter().position(|c| {
            c.id == id
                && c.join == join
                && !self
                    .persist
                    .removals
                    .contains(&crate::game::commands::Target::Cart(id))
        }) else {
            return true;
        };
        let destroyed = self.carts.update(i, |c| {
            c.hurt_dir = -c.hurt_dir;
            c.hurt_time = 10;
            c.damage += damage * 10.0;
            // MinecartTNT.shouldSourceDestroy: fire and explosions destroy
            // (so ignite) a TNT cart whatever damage it has taken.
            cart_is_destroyed(creative, c.damage, fire && c.kind == CartKind::Tnt)
        });
        // VehicleEntity.hurtServer: `gameEvent(ENTITY_DAMAGE, source.getEntity())`.
        let hurt_at = self.carts[i].position.components();
        self.entity_event_by("minecraft:entity_damage", hurt_at, hit.by);
        if destroyed {
            let speed = self.carts[i].velocity.x * self.carts[i].velocity.x
                + self.carts[i].velocity.z * self.carts[i].velocity.z;
            let tnt_prime =
                self.carts[i].kind == CartKind::Tnt && (fire || speed >= f64::from(0.01f32));
            if tnt_prime {
                let _save_lease = sunrise_client::join_seq::visibility::PassLease::enter();
                let mut c = self.carts.extract(self.carts[i].join).unwrap();
                // MinecartTNT.destroy: prime instead of breaking.
                if c.fuse < 0 {
                    self.prime_cart(&mut c);
                    c.fuse = c.random.next_int_bound(20).unwrap_or(0)
                        + c.random.next_int_bound(20).unwrap_or(0);
                }
                self.carts.put_back(i, c);
            } else {
                let join = self.carts[i].join;
                let at = self.carts[i].position.components();
                self.native_vehicle_destroy(self.carts[i].kind.id(), at);
                if !self.carts.iter().any(|c| {
                    c.id == id && c.join == join && !self.persist.removals.contains(&target)
                }) {
                    return true;
                }
                let Some(cart) = self.carts.iter().find(|c| c.id == id && c.join == join) else {
                    return true;
                };
                let (kind, slots) = (cart.kind, cart.items.len());
                let _save_lease = sunrise_client::join_seq::visibility::PassLease::enter();
                if self.kill_vehicle_object(target) && !creative {
                    self.entity_killed_event(at, false);
                    if self
                        .ui
                        .rules
                        .bool_at(sunrise_client::gamerules::vanilla::ENTITY_DROPS)
                    {
                        self.spawn_item_own(self.cart_drop(kind, id), at, 10);
                        self.drop_vehicle_contents(at, &vec![None; slots]);
                        // ...and angers the piglins that see the player who broke it.
                        self.cart_destroy_angers(hit, kind);
                    }
                }
            }
        }
        true
    }
}
