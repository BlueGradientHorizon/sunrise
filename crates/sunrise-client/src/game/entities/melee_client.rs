#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::melee::*;

impl Game<'_> {
    /// `Player.attack` after a target that is not a mob took the hit:
    /// the sprint knockback sound, a living target's critical hit (`living`:
    /// its entity id; `crit`: whether `melee_damage` decided the hit critical,
    /// mods' `critical_hit` included), the `sweep` decided before the hit
    /// (`sweep_before_hit`) around the target's box `area` and the strong or weak sound.
    pub(in crate::game) fn attack_effects(
        &mut self,
        area: Aabb,
        sweep: Option<[f64; 3]>,
        living: Option<i32>,
        crit: bool,
        spared: mace::Spared,
        magic: Option<i32>,
    ) {
        let scale = self.attack_strength_scale();
        let full = scale > 0.9;
        let player = self.player.as_ref();
        let from = player.map(|p| [p.position.x, p.position.y, p.position.z]);
        if full && player.is_some_and(|p| p.sprinting) {
            self.audio
                .0
                .play("minecraft:entity.player.attack.knockback", from, 1.0, 1.0);
        }
        let crit = living.is_some() && crit;
        if let Some(at) = sweep {
            self.sweep_attack(at, from, Some((area, spared)), scale);
        }
        if crit {
            self.audio
                .0
                .play("minecraft:entity.player.attack.crit", from, 1.0, 1.0);
            if let Some(id) = living {
                self.area_critical_hit(id, area, false);
            }
        } else if sweep.is_none() {
            let sound = if full {
                "minecraft:entity.player.attack.strong"
            } else {
                "minecraft:entity.player.attack.weak"
            };
            self.audio.0.play(sound, from, 1.0, 1.0);
        }
        // Player.magicCrit: the enchantments added damage.
        if let Some(id) = magic {
            self.area_critical_hit(id, area, true);
        }
    }
    /// `Player.doSweepAttack` after a successful hit, in vanilla's order: the
    /// sweep sound, the victims around `target` (its box, and the mob that
    /// was hit, when there is one) and then the arc.
    pub(in crate::game) fn sweep_attack(
        &mut self,
        at: [f64; 3],
        from: Option<[f64; 3]>,
        target: Option<(Aabb, mace::Spared)>,
        scale: f32,
    ) {
        self.audio
            .0
            .play("minecraft:entity.player.attack.sweep", from, 1.0, 1.0);
        if let Some((area, mob)) = target {
            self.sweep_damage(area, mob, scale);
        }
        let arc = sunrise_client::particles::ParticleOptions::simple(
            sunrise_client::particles::ParticleType::SweepAttack,
        );
        self.particles.0.add(&arc, at, [0.0; 3]);
        // Player.sweepAttack's `sendParticles(SWEEP_ATTACK, ..., 0, dx, 0, dz, 0)`:
        // the offset is the one from the player to the particle.
        let origin = from.unwrap_or(at);
        let spread = [at[0] - origin[0], 0.0, at[2] - origin[2]];
        self.lan_send_particles(lan_particles::QueuedParticle {
            dimension: self.dim.id(),
            options: arc,
            state: None,
            at,
            spread,
            speed: [0.0; 3],
            count: 0,
            randomization: 0,
        });
    }
}
