#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::item_abilities::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::items::{ItemInfo, ItemStack};

impl Game<'_, crate::game::server_side::ServerState> {
    /// Whether the held stack shears a mob (`shears_harvest`): shears, or a stack whose
    /// item parts let it.
    pub(in crate::game) fn held_shears(&self) -> bool {
        self.held()
            .is_some_and(|stack| self.stack_shears(stack, ability::SHEARS_HARVEST))
    }
}
