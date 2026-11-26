//! Sniffers on the client side: digging particles, block hit sound, and searching sound.
use super::Game;
use sunrise_core::{geometry::Vec3, position::BlockPos};
#[allow(unused_imports)]
use sunrise_sim::mobs::{Mob, Species, sniffer};

impl Game<'_, crate::game::client_side::ClientState> {
    /// The sniffer's client-side effects: `Sniffer.emitDiggingParticles`
    /// and `playSearchingSound`.
    pub(in crate::game) fn tick_sniffer_client(&mut self) {
        assert!(
            !self.is_logical_server(),
            "tick_sniffer_client called on Server role"
        );
        let mut players: Vec<Vec3> = Vec::new();
        if let Some(p) = self.player.as_ref()
            && !p.health.dead_or_dying()
        {
            players.push(p.position);
        }
        for m in &self.mobs {
            if &*m.kind.id == "minecraft:player" && m.alive() && !m.removed {
                players.push(m.position);
            }
        }
        if players.is_empty() {
            return;
        }
        for i in 0..self.mobs.len() {
            let m = &self.mobs[i];
            if m.species() != Species::Sniffer
                || !m.alive()
                || !players
                    .iter()
                    .any(|&p| m.position.distance_squared(p) <= 32.0 * 32.0)
            {
                continue;
            }
            let id = m.id;
            let at = [m.position.x, m.position.y, m.position.z];
            if sniffer::state(m) == sniffer::SEARCHING && m.age.is_multiple_of(20) {
                self.side.audio.0.play_unshared(
                    "minecraft:entity.sniffer.searching",
                    Some(at),
                    1.0,
                    1.0,
                );
                continue;
            }
            if !sniffer::digging(m) {
                continue;
            }
            let age = m.age;
            if let Some(ms) = m.anim.states[3].millis(age as f32)
                && ms > 1700
                && ms < 6000
            {
                let head = sniffer::head_block(&self.mobs[i]);
                let ground = [head[0], head[1] - 1, head[2]];
                let below = self.world.block(ground[0], ground[1], ground[2]);
                if self.res.facts().get(below).render_shape
                    != sunrise_items::blocks::RenderShape::Invisible
                {
                    if self
                        .particle_sprite(below, [head[0], head[1] - 1, head[2]])
                        .is_some()
                    {
                        let dust = [
                            f64::from(head[0]) + 0.5,
                            f64::from(head[1]) + 0.5 - f64::from(0.65_f32),
                            f64::from(head[2]) + 0.5,
                        ];
                        self.client_landing_dust(
                            below,
                            Vec3::new(dust[0], dust[1], dust[2]),
                            30,
                            0.0,
                        );
                    }
                    if age.is_multiple_of(10) {
                        let sniffer = Some((id, self.mobs[i].kind.id.as_str()));
                        let hit = sunrise_sim::block_sounds::sound_type(
                            &self.loop_context,
                            self.res.facts(),
                            BlockPos::new(ground[0], ground[1], ground[2]),
                            below,
                            sniffer,
                            mod_api::BlockSoundCause::Dig,
                        )
                        .hit;
                        self.side.audio.0.play_unshared(&hit, Some(at), 0.5, 0.5);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/sniffers_tests.rs"]
pub(in crate::game) mod sniffers_tests;
