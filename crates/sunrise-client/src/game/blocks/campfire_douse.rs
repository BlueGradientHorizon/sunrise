//! `CampfireBlock.useItemOn`: an item in `#minecraft:douses_campfires` (the
//! shovels) put on a lit campfire puts it out.
#[allow(unused_imports)]
use super::{Game, Hit};
#[allow(unused_imports)]
use sunrise_client::{Result, animate_tick::make_campfire_smoke, items::ItemStack};
#[allow(unused_imports)]
use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
use sunrise_world::chunk::BlockState;

impl Game<'_> {
    /// The lit campfire at `pos` that `held` puts out, when the item is in
    /// `#minecraft:douses_campfires` (`CampfireBlock.useItemOn`).
    pub(in crate::game) fn campfire_to_douse(
        &self,
        pos: [i32; 3],
        held: Option<&ItemStack>,
    ) -> Option<BlockState> {
        held.filter(|s| {
            self.res
                .items()
                .item_tags
                .get("minecraft:douses_campfires")
                .is_some_and(|members| members.contains(&*s.item))
        })?;
        self.pack
            .assets
            .registry
            .state(self.world.block(pos[0], pos[1], pos[2]))
            .filter(|s| matches!(&*s.name, "minecraft:campfire" | "minecraft:soul_campfire"))
            .filter(|s| s.properties.get("lit").map(String::as_str) == Some("true"))
            .cloned()
    }

    pub(in crate::game) fn modified_campfire(
        &self,
        hit: &Hit,
        stack: &ItemStack,
        unlit: sunrise_world::registry::BlockStateId,
    ) -> Option<sunrise_world::registry::BlockStateId> {
        let block = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        if !mod_api::__private::listened(mod_api::HookId::BlockToolModification)
            && !crate::game::api_values::block_state::block_listens(
                mod_api::HookId::ToolModifiedState,
                block,
            )
        {
            return Some(unlit);
        }
        let tool = mod_api::ToolUse::from(mod_api::__private::built::ToolUse {
            player: self.player.is_some().then(|| self.own_entity_id() as u32),
            hand: self.hand,
            pos: hit.pos,
            face: hit.face,
            click: hit.point,
            ability: mod_api::ToolAbility::ShovelDouse,
            simulated: false,
        });
        let mut state = Some(unlit);
        let mut cancel = false;
        self.mod_tool_modification(stack, block, &tool, &mut state, &mut cancel);
        if cancel { None } else { state }
    }
}
