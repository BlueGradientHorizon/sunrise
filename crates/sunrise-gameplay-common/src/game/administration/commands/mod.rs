

#[cfg(any(test, feature = "test-support"))]
pub mod peaceful_tests {
    use super::{Arg, Context};
    use crate::game::{execution_role::ExecutionRole, native_entity_refs::tests as fixture};
    use std::sync::{Arc, OnceLock};
    use sunrise_command::ParsedArg;
    use sunrise_core::nbt::Tag;
    use sunrise_items::items::ItemStack;
    use sunrise_level::chunk::Chunk;
    use sunrise_mod_package::{Content, Package, enabled::EnabledMods};
    use sunrise_sim::mod_content::{self, ContentMod};

    pub const TRADER: &str = "peaceful_test:trader";

    #[cfg(test)]
pub(in crate::game) fn trader_egg() -> ItemStack {
        let mut stack = ItemStack::new("minecraft:cow_spawn_egg", 1);
        stack.data_mut().set_other(
            "minecraft:entity_data",
            Some(Tag::compound([("id", Tag::String(TRADER.into()))])),
        );
        stack
    }

    

    

    
}
use crate::game::chat_common::ChatCommonGameExt1 as _;

pub mod terrain_cmds;
