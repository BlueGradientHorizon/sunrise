//! Role-neutral block-entity animation events, ownership tracking, and state reads.
use super::Game;
use super::block_entity_anims::AnimTick;
use sunrise_client::{
    block_entities::Pos,
    block_entity_anim::{EndGateway, Ticker},
    block_entity_render::BlockAnim,
    containers::BlockEntity,
    save::number,
};
use sunrise_core::nbt::Tag;

impl Game<'_> {
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

    /// Start a tick of animated block entities, dropping stale owners and removed pots.
    pub(in crate::game) fn begin_block_entity_anims(&mut self) -> AnimTick {
        let world = &self.world;
        self.be_anims.retain_owned(
            |p, g| world.block_entities.generation(&(p[0], p[1], p[2])) == Some(g),
            |p| world.block(p[0], p[1], p[2]).0,
        );
        AnimTick { frames: None }
    }

    /// End of the block-entity animation tick: sync client shulker collision snapshots.
    pub(in crate::game) fn finish_block_entity_anims(&mut self) {
        if self.remote.is_some() {
            self.sync_shulkers();
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

    /// Renderer's view of the animated block entity at `pos`, including pot sherds.
    pub(in crate::game) fn block_anim(&self, pos: [i32; 3], partial: f32) -> BlockAnim {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        // A replacement may draw before its first ConduitBlockEntity.clientTick.
        if matches!(self.be_anims.ticker(state.0), Some(Ticker::Conduit))
            && let Some(&owner) = self.be_anims.owners.get(&pos)
            && self
                .world
                .block_entities
                .generation(&(pos[0], pos[1], pos[2]))
                != Some(owner)
        {
            return BlockAnim::default();
        }
        let sherds = if &*self.res.facts().get(state).name == "minecraft:decorated_pot" {
            let sherds = self.saved_tag(pos).and_then(|t| t.get("sherds"));
            ["back", "left", "right", "front"].map(|side| match sherds.and_then(|s| s.get(side)) {
                // An item id, or the item stack the processed templates hold.
                Some(Tag::String(id)) => Some(id.to_string_lossy()),
                Some(stack) => {
                    sunrise_client::save::item_from_nbt(stack).map(|s| s.item.to_string())
                }
                None => None,
            })
        } else {
            Default::default()
        };
        self.be_anims
            .block_anim(pos, self.world.game_time, partial, sherds)
    }

    /// Client chest ticker, also used when replaying a moved chest.
    pub(in crate::game) fn tick_chest_lid(&mut self, pos: [i32; 3]) {
        self.be_anims.chests.entry(pos).or_default().tick();
    }

    /// `ChestLidController.getOpenness(partial)` at `pos`.
    pub(in crate::game) fn chest_lid(&self, pos: [i32; 3], partial: f32) -> f32 {
        self.be_anims.lid(pos, partial)
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
