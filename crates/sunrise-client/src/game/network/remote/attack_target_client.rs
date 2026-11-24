#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::attack_target::*;

impl Game<'_> {
    /// `Player.attack`'s damage and `getAttackStrengthScale(0.5)` on this
    /// client.
    pub(in crate::game) fn remote_attack_damage(&mut self) -> (f32, f32) {
        // `getAttackStrengthScale(0.5)`: here the ticker is `hand_strength`'s
        // (`attack_ticks` only counts in the local world), and the attack
        // speed is the player's synced attribute, which a server may change.
        let speed = self
            .own_attribute("minecraft:attack_speed")
            .unwrap_or_else(|| self.main_hand_attack_speed());
        let strength = self.client.hand_strength.scale(0.5, speed);
        // Player.attack's damage: a riptide spin's, or this client's own
        // ATTACK_DAMAGE, which the held item and effects don't change here.
        let spin = self.player.as_ref().map_or(0.0, |p| p.auto_spin_attack_dmg);
        let damage = if spin > 0.0 {
            spin
        } else {
            self.own_attribute("minecraft:attack_damage")
                .map_or(1.0, |v| v as f32)
        };
        (damage, strength)
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::boats::Boat;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::dragon::Crystal;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::minecarts::{CartKind, Minecart};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::native_entity_refs::tests::with_client_game;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::native_hotbar_slot::hotbar_slot_tests::{
        logged_in, sent,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::remote::attack_target::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::sync::Arc;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::projectiles::{
        Projectile, ProjectileKind,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_protocol::packets::play::Serverbound as S;

    /// Each target, sprinting at full strength (knockback 0.5): the horizontal motion times 0.6,
    /// the vertical kept, the sprint stopped, and STOP_SPRINTING after the Attack on the wire.
    /// Walking, the same hit on a boat has no knockback and changes nothing.
    pub(in crate::game) fn sprint_hits_isolated() {
        with_client_game(|game| {
            let mut reader = logged_in(game);
            let at = Vec3::new(0.5, 65.0, 2.5);
            game.boats.push(Boat::new(101, Arc::from("oak"), at, 0.0));
            game.carts.push(Minecart::new(102, CartKind::Rideable, at));
            game.crystals.push(Crystal::new(103, at, false, 0));
            let bullet = ProjectileKind::ShulkerBullet {
                target: None,
                dir: None,
                steps: 0,
                delta: [0.0; 3],
                seed: 0,
            };
            let mut bullet = Projectile::new(bullet, at, None);
            bullet.id = 104;
            game.creatures.projectiles.push(bullet);
            let start = Vec3::new(0.1, -0.08, 0.25);
            for (id, sprinting) in [
                (101, true),
                (102, true),
                (103, true),
                (104, true),
                (101, false),
            ] {
                game.client.hand_strength.ticker = 100;
                let p = game.player.as_mut().unwrap();
                let _ = p.set_sprinting(sprinting);
                p.velocity = start;
                // The server has heard of the sprint.
                game.remote_send_state(0);
                sent(&mut reader);
                assert!(game.remote_attack_entity(id, 2.0), "entity {id}");
                game.remote_send_state(0);
                let packets = sent(&mut reader);
                let p = game.player.as_ref().unwrap();
                let attack = packets
                    .iter()
                    .position(|q| matches!(q, S::Attack(a) if a.id == id));
                let stop = packets
                    .iter()
                    .position(|q| matches!(q, S::PlayerCommand(c) if c.action == 2));
                assert!(attack.is_some(), "entity {id}: {packets:?}");
                if sprinting {
                    assert!(!p.sprinting, "entity {id}");
                    assert_eq!(
                        p.velocity,
                        Vec3::new(0.1 * 0.6, -0.08, 0.25 * 0.6),
                        "entity {id}"
                    );
                    assert!(stop.is_some() && attack < stop, "entity {id}: {packets:?}");
                } else {
                    assert_eq!(p.velocity, start, "a walking hit has no knockback");
                    assert_eq!(stop, None, "{packets:?}");
                }
            }
        });
    }
}
