

#[cfg(any(test, feature = "test-support"))]
pub mod loot_mine_component_tests {

use super::*;

use sunrise_core::nbt::{Compound, Tag};

use sunrise_sim::save;

pub fn decorations(stack: &ItemStack) -> Option<Tag> {
        save::item_to_nbt(stack, &Default::default())
            .get("components")?
            .get("minecraft:pot_decorations")
            .cloned()
    }
}
