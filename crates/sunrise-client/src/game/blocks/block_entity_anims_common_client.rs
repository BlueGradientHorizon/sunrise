#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::block_entity_anims::AnimTick;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::block_entity_anims_common::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    block_entities::Pos,
    block_entity_anim::{EndGateway, Ticker},
    block_entity_render::BlockAnim,
    containers::BlockEntity,
    save::number,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;

impl Game<'_> {
    /// Start a tick of animated block entities, dropping stale owners and removed pots.
    pub(in crate::game) fn begin_block_entity_anims(&mut self) -> AnimTick {
        let world = &self.world;
        self.be_anims.retain_owned(
            |p, g| world.block_entities.generation(&(p[0], p[1], p[2])) == Some(g),
            |p| world.block(p[0], p[1], p[2]).0,
        );
        AnimTick { frames: None }
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
    /// `ChestLidController.getOpenness(partial)` at `pos`.
    pub(in crate::game) fn chest_lid(&self, pos: [i32; 3], partial: f32) -> f32 {
        self.be_anims.lid(pos, partial)
    }
    /// End of the block-entity animation tick: sync client shulker collision snapshots.
    pub(in crate::game) fn finish_block_entity_anims(&mut self) {
        if !self.is_logical_server() {
            self.sync_shulkers();
        }
    }
    /// Client chest ticker, also used when replaying a moved chest.
    pub(in crate::game) fn tick_chest_lid(&mut self, pos: [i32; 3]) {
        self.be_anims.chests.entry(pos).or_default().tick();
    }
}
