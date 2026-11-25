//! Role-neutral block-entity animation events, ownership tracking, and state reads.
#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use super::block_entity_anims::AnimTick;
#[allow(unused_imports)]
use sunrise_client::{
    block_entities::Pos,
    block_entity_anim::{EndGateway, Ticker},
    block_entity_render::BlockAnim,
    containers::BlockEntity,
    save::number,
};
#[allow(unused_imports)]
use sunrise_core::nbt::Tag;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// `BlockEntity.triggerEvent(a, b)` on the block entity at `pos`.
    pub(in crate::game) fn block_entity_event(&mut self, pos: [i32; 3], a: i32, b: i32) {
        let state = self.world.block(pos[0], pos[1], pos[2]).0;
        let time = self.world.game_time;
        // PotentSulfurBlock.triggerEvent.
        if self.potent_sulfur_eruption_event((pos[0], pos[1], pos[2])) {
            return;
        }
        self.claim_anim(pos);
        match self.be_anims.ticker(state) {
            Some(Ticker::Lid) => {
                self.be_anims
                    .chests
                    .entry(pos)
                    .or_default()
                    .trigger_event(a, b);
            }
            Some(Ticker::Shulker { .. }) => {
                let shulker = self.be_anims.shulkers.entry(pos).or_default();
                if shulker.trigger_event(a, b) {
                    // Publish shulker lid status change to the collision snapshot immediately.
                    self.world.shulkers.insert(pos, shulker.clone());
                }
            }
            Some(Ticker::Bell) => {
                let gathered = self.side_bell_gather(pos);
                let rung =
                    self.be_anims
                        .bells
                        .entry(pos)
                        .or_default()
                        .trigger_event(a, b, time, || gathered);
                // Server informs living entities within 32 blocks; client is a no-op.
                if rung {
                    self.side_bell_heard(pos, time);
                }
            }
            Some(Ticker::Gateway) => {
                let initial = self.gateway_loaded(pos);
                self.be_anims
                    .gateways
                    .entry(pos)
                    .or_insert(initial)
                    .trigger_event(a, b);
            }
            Some(Ticker::Pot) => {
                self.be_anims
                    .pots
                    .entry(pos)
                    .or_default()
                    .trigger_event(a, b, time);
            }
            _ => {}
        }
    }

    /// Bind animation state at `pos` to the current block entity generation.
    pub(in crate::game) fn claim_anim(&mut self, pos: [i32; 3]) {
        if let Some(generation) = self
            .world
            .block_entities
            .generation(&(pos[0], pos[1], pos[2]))
        {
            self.be_anims.claim(pos, generation);
        }
    }

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

    /// Copies the shulker box lids into the world, where the collision queries
    /// read them (`SceneCollisions::shulker_collision_state`), as
    /// `sync_moving_pistons` does for pistons.
    pub(in crate::game) fn sync_shulkers(&mut self) {
        self.world.shulkers.clear();
        self.world
            .shulkers
            .extend(self.be_anims.shulkers.iter().map(|(p, s)| (*p, s.clone())));
    }
}
