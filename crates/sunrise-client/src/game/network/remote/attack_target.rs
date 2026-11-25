#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// `AttackRange.isInRange` measures from the player's current eyes, not the pick ray's origin.
    pub(in crate::game) fn attack_hit_distance(&self, [x, y, z]: [f64; 3]) -> Option<f64> {
        let [eye_x, eye_y, eye_z] = self.eye_position()?;
        Some(Vec3::new(x, y, z).distance(Vec3::new(eye_x, eye_y, eye_z)))
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
