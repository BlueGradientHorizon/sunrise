//! Role-neutral block-entity animation events, ownership tracking, and state reads.
use super::Game;
use sunrise_core::nbt::Tag;
use sunrise_level::block_entity_ticks::EndGateway;
use sunrise_level::{block_entities::Pos, containers::BlockEntity};
use sunrise_sim::save::number;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// End gateway at `pos`: live `Age`, saved update tag, or a fresh gateway.
    pub(in crate::game) fn gateway_loaded(&self, pos: [i32; 3]) -> EndGateway {
        let live = match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(BlockEntity::EndGateway(g)) => Some(g.age),
            _ => None,
        };
        let age = live.or_else(|| self.saved_tag(pos).and_then(|t| number(t.get("Age"))));
        EndGateway {
            age: age.unwrap_or(0),
            teleport_cooldown: 0,
        }
    }

    /// Saved block entity NBT tag at `pos`, if present on the loaded chunk.
    pub(in crate::game) fn saved_tag(&self, pos: [i32; 3]) -> Option<&Tag> {
        self.world
            .chunks
            .get(&(pos[0] >> 4, pos[2] >> 4))
            .and_then(|c| c.saved.as_deref())
            .and_then(|saved| {
                saved.block_entities.iter().find(|t| {
                    [t.get("x"), t.get("y"), t.get("z")]
                        .map(number)
                        .eq(&pos.map(|v| Some(i64::from(v))))
                })
            })
    }

    /// `PotentSulfurBlock.triggerEvent`: stamp the eruption tick on a potent sulfur block entity.
    pub(in crate::game) fn potent_sulfur_eruption_event(&mut self, pos: Pos) -> bool {
        let time = self.world.game_time;
        match self.world.block_entities.get_mut(&pos) {
            Some(BlockEntity::PotentSulfur(sulfur)) => {
                sulfur.eruption_tick = time;
                true
            }
            _ => false,
        }
    }
}
