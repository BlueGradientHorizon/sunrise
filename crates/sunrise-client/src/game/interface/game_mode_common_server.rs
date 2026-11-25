#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Screen};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::game_mode_common::inventory_after_infinite_materials;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::game_mode_common::relative;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::game_mode_common::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{PlayerAbilities, PlayerRef};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    chat::{ChatAbilities, ChatVisibility},
    command::args::GameMode,
    items::ItemStack,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::game_modes::{
    loads_chunks, mode_from_id, projectile_predicate_supported,
};

impl Game<'_> {
    /// `ItemStack.useOn`'s adventure gate for the held item.
    pub(in crate::game) fn may_use_on(&self, clicked: [i32; 3]) -> bool {
        self.may_build() || self.can_place_on_in_adventure(clicked)
    }
}
