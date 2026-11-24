#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::effects::*;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn lan_broken_block_positions(&self) -> Vec<[f64; 3]> {
        let Some(host) = self.lan.as_ref() else {
            return Vec::new();
        };
        host.broken
            .iter()
            .copied()
            .chain(
                self.server
                    .lan_events
                    .iter()
                    .filter(|event| event.kind == 2001)
                    .map(|event| event.pos.map(f64::from)),
            )
            .collect()
    }
    /// `LivingEntity.tick`'s arrows and bee stingers falling out of the
    /// players' bodies, and their counts for the players around
    /// (`DATA_ARROW_COUNT_ID`, `DATA_STINGER_COUNT_ID`).
    pub(in crate::game) fn lan_stuck(&mut self, paused: bool) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        if !paused {
            self.combat.0.stuck.tick();
        }
        host.hub.guests.update_all(|g| {
            g.shoulders = crate::game::shoulders::shoulder_variants(g.saved.as_ref());
            g.stuck = match self.guest_combat.0.get_mut(&g.conn) {
                Some(combat) => {
                    if !paused {
                        combat.stuck.tick();
                    }
                    (combat.stuck.arrows, combat.stuck.stingers)
                }
                None => (0, 0),
            };
        });
    }
}
