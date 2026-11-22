//! Particle creation and server particle-event admission.
use super::*;

impl ParticleEngine {
    /// `ClientPacketListener.handleParticleEvent` for `ServerLevel.sendParticles`.
    pub fn send_particles(
        &mut self,
        options: &ParticleOptions,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: u32,
    ) {
        if count == 0 {
            let v = [speed * spread[0], speed * spread[1], speed * spread[2]];
            self.add(options, at, v);
            return;
        }
        for _ in 0..count {
            let r = &mut self.random;
            let p = [
                at[0] + r.next_gaussian() * spread[0],
                at[1] + r.next_gaussian() * spread[1],
                at[2] + r.next_gaussian() * spread[2],
            ];
            let v = [
                r.next_gaussian() * speed,
                r.next_gaussian() * speed,
                r.next_gaussian() * speed,
            ];
            self.add(options, p, v);
        }
    }
    /// Admit a level spawn as in `ClientLevel.doAddParticle`, then construct it
    /// synchronously with the live level and any original mod options borrowed for the call.
    pub fn add_with_level(
        &mut self,
        options: &ParticleOptions,
        p: [f64; 3],
        v: [f64; 3],
        flags: ParticleSpawnFlags,
        level: &ParticleLevel<'_>,
        mod_options: Option<&mod_api::ParticleOptions>,
    ) {
        self.add_scoped(
            options,
            p,
            v,
            flags,
            SpawnScope {
                level: Some(level),
                mod_options,
            },
        );
    }

    pub(super) fn add_scoped(
        &mut self,
        options: &ParticleOptions,
        p: [f64; 3],
        v: [f64; 3],
        flags: ParticleSpawnFlags,
        scope: SpawnScope<'_, '_>,
    ) {
        let c = self.camera;
        let d2 = (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2);
        if let Some(requested) = &mut self.requested {
            requested.push((options.clone(), p, v));
            REQUESTED.set(REQUESTED.get() + 1);
            return;
        }
        let minimal = self.calculate_particle_level(flags.always_show) == 2;
        if flags.override_limiter || options.ty.override_limiter() || (d2 <= 1024.0 && !minimal) {
            self.create_scoped(options, p, v, scope);
        }
    }

    /// `ParticleEngine.createParticle`: bypass the level's distance and settings
    /// checks, but still apply `ParticleLimit.SPORE_BLOSSOM` when queuing the result.
    pub fn create(&mut self, options: &ParticleOptions, p: [f64; 3], v: [f64; 3]) {
        self.create_scoped(options, p, v, SpawnScope::BARE);
    }

    fn create_scoped(
        &mut self,
        options: &ParticleOptions,
        p: [f64; 3],
        v: [f64; 3],
        scope: SpawnScope<'_, '_>,
    ) {
        if let Some(requests) = &mut self.created_requests {
            requests.push((options.clone(), p, v));
            return;
        }
        let seed = self.seeds.next_long();
        if let Some(mut particle) = self.make(options, p, v, seed, scope) {
            *self.made.entry(options.ty.name()).or_default() += 1;
            // Resource packs' Polytone particle modifiers (#11486), once as it spawns.
            if let Some(registry) = &self.polytone
                && registry.modifies_particles()
            {
                let id = match &options.mod_particle {
                    Some(name) => name.clone(),
                    None => format!("minecraft:{}", options.ty.name()).into(),
                };
                polytone::modify(registry, &id, options.source.as_deref(), &mut particle);
            }
            // The provider has drawn, so rejecting here keeps vanilla's RNG order.
            // shortcut: counts live plus queued particles by scanning instead of
            // vanilla's counted map; use a counter if spore spawning shows in profiles.
            if particle.ty == ParticleType::SporeBlossomAir
                && self.particles.len() + self.pending.len() >= SPORE_BLOSSOM_LIMIT
                && self
                    .particles
                    .iter()
                    .chain(self.pending.iter().filter_map(PendingParticle::ordinary))
                    .filter(|q| q.ty == ParticleType::SporeBlossomAir)
                    .count()
                    >= SPORE_BLOSSOM_LIMIT
            {
                return;
            }
            self.pending.push(PendingParticle::Ordinary(particle));
        }
    }
}

/// `Mth.nextDouble(random, min, max)`.
pub(super) fn next_double_between(random: &mut impl RandomSource, min: f64, max: f64) -> f64 {
    if min >= max {
        min
    } else {
        random.next_double() * (max - min) + min
    }
}
