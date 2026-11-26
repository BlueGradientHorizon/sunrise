//! A tool acting on a block (an axe strips, a shovel flattens or douses, a hoe tills): the
//! block's parts say what the block becomes, then the global hook hears the result.

use sunrise_items::items::ItemStack;
use sunrise_world::registry::BlockStateId;

pub trait ToolModificationGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// What the tool action `tool` makes of the block `block`: `state` starts with vanilla's
    /// answer. The block's parts answer first (`tool_modified_state`,
    /// `IBlockExtension.getToolModifiedState`), and `block_tool_modification` hears their result
    /// unless they cancelled the action.
    fn mod_tool_modification(
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
impl<'__sg, G: crate::game::SideGame<'__sg>> ToolModificationGameExt1<'__sg> for G {}

use crate::game::native_hooks::block_tool_modification::GameHooks as _;

use crate::game::native_hooks::tool_modified_state::GameHooks as _;
