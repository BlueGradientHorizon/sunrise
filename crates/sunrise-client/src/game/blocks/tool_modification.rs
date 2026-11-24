//! A tool acting on a block (an axe strips, a shovel flattens or douses, a hoe tills): the
//! block's parts say what the block becomes, then the global hook hears the result.
use super::{Game, Hit};
use sunrise_client::items::ItemStack;
use sunrise_world::registry::BlockStateId;

impl Game<'_> {
    /// [`Self::mod_tool_modification`] for this player's `stack` doing `ability` to the block
    /// `hit` names, `state` being vanilla's answer: what the block becomes, `None` when nothing
    /// does or the action was cancelled. Without a listener, `state` stands.
    pub(in crate::game) fn hit_tool_modification(
        &self,
        hit: &Hit,
        stack: &ItemStack,
        ability: mod_api::ToolAbility,
        state: Option<BlockStateId>,
    ) -> Option<BlockStateId> {
        let block = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        if !mod_api::__private::listened(mod_api::HookId::BlockToolModification)
            && !crate::game::api_values::block_state::block_listens(
                mod_api::HookId::ToolModifiedState,
                block,
            )
        {
            return state;
        }
        let tool = mod_api::ToolUse::from(mod_api::__private::built::ToolUse {
            player: self.player.is_some().then(|| self.own_entity_id() as u32),
            hand: self.hand,
            pos: hit.pos,
            face: hit.face,
            click: hit.point,
            ability,
            simulated: false,
        });
        let mut state = state;
        let mut cancel = false;
        self.mod_tool_modification(stack, block, &tool, &mut state, &mut cancel);
        if cancel { None } else { state }
    }

    /// `FlintAndSteelItem.useOn` / `FireChargeItem.useOn`'s `getToolModifiedState(context,
    /// FIRESTARTER_LIGHT)`: the lit state of the campfire, candle or candle cake `hit` names
    /// (`canLight`), then the block's parts and `block_tool_modification`. `None` places fire
    /// at the face instead.
    pub(in crate::game) fn fire_starter_lit(
        &self,
        hit: &Hit,
        stack: &ItemStack,
    ) -> sunrise_client::Result<Option<BlockStateId>> {
        let clicked = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        let name = self.res.facts().get(clicked).name.clone();
        let vanilla = if self.can_light(clicked, &name) {
            let registry = self.pack.assets.registry;
            let mut state = registry
                .state(clicked)
                .cloned()
                .ok_or("missing lit block state")?;
            state.properties.insert("lit".into(), "true".into());
            Some(registry.resolve(&state)?)
        } else {
            None
        };
        let light = mod_api::ToolAbility::FirestarterLight;
        Ok(self.hit_tool_modification(hit, stack, light, vanilla))
    }

    /// What the tool action `tool` makes of the block `block`: `state` starts with vanilla's
    /// answer. The block's parts answer first (`tool_modified_state`,
    /// `IBlockExtension.getToolModifiedState`), and `block_tool_modification` hears their result
    /// unless they cancelled the action.
    pub(in crate::game) fn mod_tool_modification(
        &self,
        stack: &ItemStack,
        block: BlockStateId,
        tool: &mod_api::ToolUse,
        state: &mut Option<BlockStateId>,
        cancel: &mut bool,
    ) {
        let template = *state;
        let mut inert = false;
        self.native_tool_modified_state(block, stack, tool, state, cancel, &mut inert);
        if *cancel {
            return;
        }
        let part_state = *state;
        self.native_block_tool_modification(stack, tool, state, cancel);
        // Suppress only an unchanged template answer. Leaving no result (rather than
        // cancelling) lets the tool try its next ability and notify listeners of it.
        if inert && part_state == template && *state == part_state {
            *state = None;
        }
    }
}
