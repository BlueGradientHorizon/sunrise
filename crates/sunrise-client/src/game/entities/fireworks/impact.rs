//! Rocket impacts dispatch only after the vanilla deflection decision.
use super::*;

impl Game<'_> {
    /// Let listeners inspect and remove the live rocket, preserving its identity
    /// across the temporary tick extraction. Refuse a stale hit after writes.
    pub(super) fn rocket_impact(
        &mut self,
        index: usize,
        r: Rocket,
        hit: ProjectileHit,
        hears: bool,
    ) -> Option<(Rocket, bool)> {
        if !hears || matches!(hit, ProjectileHit::None) {
            return Some((r, false));
        }
        let (id, join, position, velocity) = (r.id, r.join, r.position, r.velocity);
        let owner = self.rocket_owner(&r);
        let target = self.native_impact_target(hit);
        // The resident owner keeps its slot during the tick: publish the advanced
        // state for the listeners, then read it back as the next tick clone.
        if !self.rockets.store_back(index, r) {
            return None;
        }
        // Listeners resolve the rocket by id: it is not held out while they read it.
        let held = self.held_out.iter().rposition(|&held| held == id);
        if let Some(at) = held {
            self.held_out.remove(at);
        }
        let canceled =
            self.native_separate_projectile_impact(id, "minecraft:firework_rocket", owner, hit);
        if held.is_some() {
            self.held_out.push(id);
        }
        let at = self.rockets.locate(join)?;
        if self
            .persist
            .removals
            .contains(&super::super::commands::Target::Rocket(id))
        {
            drop(self.rockets.extract(join)?);
            self.rockets.retire_extracted(id, join);
            return None;
        }
        let r = self.rockets[at].clone();
        let changed = r.position != position
            || r.velocity != velocity
            || target != self.native_impact_target(hit);
        Some((r, canceled || changed))
    }
}

#[cfg(test)]
#[path = "impact_tests.rs"]
mod tests;
