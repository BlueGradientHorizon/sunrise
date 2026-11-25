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
        self.audio
            .0
            .play("minecraft:entity.player.attack.nodamage", from, 1.0, 1.0);
        turned
    }
    /// `ShulkerBullet.hurtServer`'s sound (unless silent) and fifteen crit particles
    /// (`sendParticles`, spread 0.2) at the bullet, before it is discarded.
    pub(in crate::game) fn shulker_bullet_hurt(&mut self, at: [f64; 3], silent: bool) {
        if !silent {
            self.audio
                .0
                .play("minecraft:entity.shulker_bullet.hurt", Some(at), 1.0, 1.0);
        }
        if let Some(ty) = sunrise_client::particles::ParticleType::from_name("crit") {
            let options = sunrise_client::particles::ParticleOptions::simple(ty);
            self.particles
                .0
                .send_particles(&options, at, [0.2, 0.2, 0.2], 0.0, 15);
            self.lan_level_particles(&options, at, [0.2, 0.2, 0.2], 0.0, 15, None);
        }
    }
}
