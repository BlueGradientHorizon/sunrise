#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::deflect::PICK_RADIUS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::deflect::attackable;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::deflect::redirectable;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::deflect::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    mobs::Who,
    projectiles::{Projectile, ProjectileKind, Turn},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;

impl Game<'_> {
    /// `Player.attack` on a redirectable projectile: `onAttack` resets the
    /// attack strength, then `Projectile.deflect(AIM_DEFLECT, player, player,
    /// true)`. When the projectile refuses (a wind charge in its first five
    /// ticks) the blow goes on as an ordinary attack, which a projectile does
    /// not take: `hurtOrSimulate` fails and `PLAYER_ATTACK_NODAMAGE` plays,
    /// the same sound. Either way the swing follows. True when it turned.
    pub(in crate::game) fn deflect(&mut self, i: usize) -> bool {
        self.reset_attack_strength();
        let turned = self.turn_projectile(i);
        self.swing_arm();
        let from = self
            .player
            .as_ref()
            .map(|p| [p.position.x, p.position.y, p.position.z]);
        if let Some(at) = from {
            self.server_play_sound("minecraft:entity.player.attack.nodamage", at, 1.0, 1.0);
        }
        turned
    }
    /// `ShulkerBullet.hurtServer`'s sound (unless silent) and fifteen crit particles
    /// (`sendParticles`, spread 0.2) at the bullet, before it is discarded.
    pub(in crate::game) fn shulker_bullet_hurt(&mut self, at: [f64; 3], silent: bool) {
        if !silent {
            self.server_play_sound("minecraft:entity.shulker_bullet.hurt", at, 1.0, 1.0);
        }
        if let Some(ty) = sunrise_client::particles::ParticleType::from_name("crit") {
            self.server_level_particles(
                &crate::lan::server_effects::ServerParticle::simple(ty),
                at,
                [0.2, 0.2, 0.2],
                0.0,
                15,
            );
        }
    }
}
