#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, view_vector};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::spear::OWN;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::spear::Target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::spear::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::spear::hits_in_reach;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    attack_strength::delay,
    collisions::SceneCollisions,
    first_person::Hand,
    items::ItemStack,
    mobs::Damage,
    spear::{HIT_FEEDBACK_TICKS, Kinetic, REACH},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3, traverse_blocks},
    position::BlockPos,
};

impl Game<'_> {
    /// `Player.damageStatsAndHearts`: the damage dealt statistic, and the
    /// damage indicator particles over a target that lost more than 2 health
    /// (`feet` and `height`: the target's).
    pub(in crate::game) fn damage_stats_and_hearts(
        &mut self,
        actual: f32,
        feet: [f64; 3],
        height: f64,
    ) {
        self.award_custom_stat("damage_dealt", (actual * 10.0).round() as i32);
        if actual > 2.0 {
            let indicator = sunrise_client::particles::ParticleOptions::simple(
                sunrise_client::particles::ParticleType::DamageIndicator,
            );
            self.particles.0.send_particles(
                &indicator,
                [feet[0], feet[1] + height * 0.5, feet[2]],
                [0.1, 0.0, 0.1],
                0.2,
                (actual * 0.5) as u32,
            );
            // The players a server serves are sent them too (the window of the server thread).
            self.lan_send_particles(super::lan_particles::QueuedParticle {
                dimension: self.dim.id(),
                options: indicator,
                state: None,
                at: [feet[0], feet[1] + height * 0.5, feet[2]],
                spread: [0.1, 0.0, 0.1],
                speed: [0.2; 3],
                count: (actual * 0.5) as u32 as i32,
                randomization: 0,
            });
        }
    }
    /// A sound at the player's feet.
    pub(in crate::game) fn play_at_feet(&mut self, sound: &str) {
        if let Some(p) = &self.player {
            let at = [p.position.x, p.position.y, p.position.z];
            self.audio.0.play(sound, Some(at), 1.0, 1.0);
        }
    }
}
