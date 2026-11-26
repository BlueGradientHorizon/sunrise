#![cfg(test)]
#[cfg(feature = "client")]
#[cfg(test)]
pub(crate) mod tests {
    use crate::game::server_side::native_entity_refs::tests::with_client_game;
    use std::sync::Arc;
    use sunrise_core::geometry::Vec3;
    use sunrise_gameplay_common::game::boats_common::Boat;
    use sunrise_gameplay_common::game::dragon_common::Crystal;
    use sunrise_protocol::packets::play::Serverbound as S;
    use sunrise_sim::projectiles::{Projectile, ProjectileKind};
    use {
        crate::game::client_side::native_hotbar_slot::hotbar_slot_tests::logged_in,
        crate::game::client_side::native_hotbar_slot::hotbar_slot_tests::sent,
    };
    use {
        sunrise_gameplay_common::game::minecarts::CartKind,
        sunrise_gameplay_common::game::minecarts::Minecart,
    };

    /// Each target, sprinting at full strength (knockback 0.5): the horizontal motion times 0.6,
    /// the vertical kept, the sprint stopped, and STOP_SPRINTING after the Attack on the wire.
    /// Walking, the same hit on a boat has no knockback and changes nothing.
    pub(crate) fn sprint_hits_isolated() {
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
