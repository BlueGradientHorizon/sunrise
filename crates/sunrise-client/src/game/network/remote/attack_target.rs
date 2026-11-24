#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// `AttackRange.isInRange` measures from the player's current eyes, not the pick ray's origin.
    pub(in crate::game) fn attack_hit_distance(&self, [x, y, z]: [f64; 3]) -> Option<f64> {
        let [eye_x, eye_y, eye_z] = self.eye_position()?;
        Some(Vec3::new(x, y, z).distance(Vec3::new(eye_x, eye_y, eye_z)))
    }

    /// `Minecraft.startAttack`'s range check and `MultiPlayerGameMode.attack`'s send and prediction.
    pub(in crate::game) fn remote_attack_entity(&mut self, id: i32, distance: f64) -> bool {
        if !self.held_range_allows(distance) {
            return false;
        }
        self.send(S::Attack(play::Attack { id }));
        if let Some(index) = self.mobs.iter().position(|mob| mob.id as i32 == id) {
            let kind = format!(
                "minecraft:{}",
                self.mobs[index].kind.id.trim_start_matches("minecraft:")
            );
            self.remote_sweep_event(&kind, true);
            self.pvp_attacked(id, distance);
            self.remote_player_attack(index);
        } else if let Some(projectile) = self
            .creatures
            .projectiles
            .iter()
            .find(|p| p.id as i32 == id)
        {
            if !crate::game::deflect::redirectable(projectile) {
                let bullet = matches!(
                    projectile.kind,
                    sunrise_client::projectiles::ProjectileKind::ShulkerBullet { .. }
                );
                self.remote_sweep_event("minecraft:shulker_bullet", false);
                if bullet {
                    self.remote_object_attack();
                }
            }
        } else if let Some((kind, living)) = self.remote_attacked_object(id) {
            self.remote_sweep_event(&kind, living);
            // A boat, a minecart or an End crystal; an armor stand is
            // living, and its hurtClient is false.
            if !living {
                self.remote_object_attack();
            }
        } else if let Some(kind) = self
            .remote
            .as_ref()
            .and_then(|r| r.general.rows.get(&id))
            .map(|row| row.kind())
        {
            // A row reaches Player.attack but hurtClient is false.
            self.remote_sweep_event(kind, false);
        }
        self.reset_attack_strength();
        if let Some(remote) = self.remote.as_mut()
            && remote.abilities.instabuild
        {
            remote.destroy_delay = 5;
        }
        true
    }
    /// The attacker's side of `causeExtraKnockback` after a hit whose
    /// `hurtClient` is true on this client (another player, a boat, a
    /// minecart, an End crystal, a shulker bullet): with any knockback, the
    /// 0.6 horizontal speed cut and the sprint stop, which that tick's
    /// `sendChanges` sends. Without it the next move outruns the server's
    /// player.
    pub(in crate::game) fn remote_attacker_knockback(&mut self, damage: f32, strength: f32) {
        let knockback = self
            .own_attribute("minecraft:attack_knockback")
            .map_or(0.0, |v| v as f32);
        let sprinting = self.player.as_ref().is_some_and(|p| p.sprinting);
        if sunrise_client::attack_strength::attacker_slows_after_hit(
            damage, knockback, strength, sprinting,
        ) {
            self.attacker_slowdown();
        }
    }
    /// A hit on a boat, minecart, End crystal or shulker bullet: their
    /// `hurtClient` is true (`VehicleEntity`, `EndCrystal` for any source but
    /// the dragon, `ShulkerBullet`), so `Player.attack` reaches
    /// `causeExtraKnockback` here as for a player.
    pub(in crate::game) fn remote_object_attack(&mut self) {
        let (damage, strength) = self.remote_attack_damage();
        self.remote_attacker_knockback(damage, strength);
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    use crate::game::boats::Boat;
    #[allow(unused_imports)]
    use crate::game::dragon::Crystal;
    #[allow(unused_imports)]
    use crate::game::minecarts::{CartKind, Minecart};
    #[allow(unused_imports)]
    use crate::game::native_entity_refs::tests::with_client_game;
    #[allow(unused_imports)]
    use crate::game::native_hotbar_slot::hotbar_slot_tests::{logged_in, sent};
    #[allow(unused_imports)]
    use std::sync::Arc;
    #[allow(unused_imports)]
    use sunrise_client::projectiles::{Projectile, ProjectileKind};
    #[allow(unused_imports)]
    use sunrise_core::geometry::Vec3;
    #[allow(unused_imports)]
    use sunrise_protocol::packets::play::Serverbound as S;

    #[test]
    #[ignore = "daily"]
    fn a_sprint_hit_on_a_boat_minecart_crystal_or_bullet_slows_the_attacker_and_stops_its_sprint() {
        // The resource loader sets process-wide tables (the cauldron interactions): own process.
        crate::game::in_own_process(
            "game::remote::attack_target::tests::a_sprint_hit_on_a_boat_minecart_crystal_or_bullet_slows_the_attacker_and_stops_its_sprint",
            sprint_hits_isolated,
        );
    }

    #[allow(unused_imports)]
    pub(in crate::game) use crate::game::client_side::remote_client::attack_target::tests::sprint_hits_isolated;
}
