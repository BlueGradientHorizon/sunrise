//! Pufferfish.playerTouch in the real player's tick, independent of fish AI.
#[allow(unused_imports)]
use super::super::{
    Game,
    entity::{Entity, with_entity},
};
#[allow(unused_imports)]
use sunrise_client::{mobs::Species, scoreboard::teams_allow_damage};
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
use sunrise_protocol::packets::play::{Clientbound, GameEvent};

/// `ClientboundGameEventPacket.PUFFER_FISH_STING`.
pub(in crate::game) const PUFFER_FISH_STING: u8 = 9;

impl Game<'_> {
    /// `Pufferfish.playerTouch`: accepted player damage, the sting game event, then poison with
    /// this fish as source.
    pub(in crate::game) fn touch_pufferfish(&mut self) {
        if !self.is_logical_server() || self.spectator() {
            return;
        }
        let Some(player) = self.player.as_ref().filter(|p| !p.health.dead_or_dying()) else {
            return;
        };
        let mut fish = self
            .mobs
            .iter()
            .filter(|m| m.species() == Species::Pufferfish && m.alive() && m.extra.size > 0)
            .peekable();
        if fish.peek().is_none() {
            return;
        }
        let own = player.bounds();
        // Player.aiStep's contact query; the existing vehicle bounds are read,
        // without moving or simulating that vehicle.
        let vehicle = self.riding.and_then(|id| {
            self.entity_handles().find_map(
                |h| with_entity!(self,h,e => (e.id()==id && !e.is_removed()).then(||e.bounds())),
            )
        });
        let query = vehicle.map_or_else(
            || own.inflate(Vec3::new(1.0, 0.5, 1.0)),
            |b| own.union_bounds(b).inflate(Vec3::new(1.0, 0.0, 1.0)),
        );
        let touching: Vec<_> = fish
            .filter(|m| {
                query.intersects(m.bounds())
                    && teams_allow_damage(self.mob_team_of(m), self.player_team())
            })
            .map(|m| {
                (
                    m.id,
                    m.extra.size,
                    [m.position.x, m.position.y, m.position.z],
                    self.mob_entity(m),
                    m.silent,
                )
            })
            .collect();
        for (id, puff, at, source, silent) in touching {
            if !self.hurt_player_from(
                (1 + puff) as f32,
                Some(at),
                "minecraft:mob_attack",
                Some(Some(id)),
                false,
            ) {
                continue;
            }
            // `if (!this.isSilent())` inside the `hurtServer` branch: before the poison.
            if !silent {
                self.puffer_fish_sting();
            }
            let changes = self.potions.effects.changes;
            self.give_player_effect("minecraft:poison", 60 * puff, 0);
            if self.potions.effects.changes != changes {
                self.trigger_effects_changed_by(Some(source));
            }
        }
    }
}

#[cfg(test)]
#[path = "pufferfish_sting_tests.rs"]
pub(in crate::game) mod pufferfish_sting_tests;
