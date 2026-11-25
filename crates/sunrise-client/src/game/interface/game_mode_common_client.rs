#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, Screen};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::game_mode_common::inventory_after_infinite_materials;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::game_mode_common::relative;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::game_mode_common::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{PlayerAbilities, PlayerRef};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    chat::{ChatAbilities, ChatVisibility},
    command::args::GameMode,
    items::ItemStack,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::game_modes::{
    loads_chunks, mode_from_id, projectile_predicate_supported,
};

impl Game<'_> {
    /// `MultiPlayerGameMode.canHurtPlayer()` and `hasExperience()` (`localPlayerMode.isSurvival()`).
    pub(in crate::game) fn can_hurt_player(&self) -> bool {
        matches!(self.mode, GameMode::Survival | GameMode::Adventure)
    }
    /// `ItemStack.canPlaceOnBlockInAdventureMode` of the stack a use names.
    pub(in crate::game::client_side) fn stack_can_place_on_in_adventure(
        &self,
        stack: &ItemStack,
        clicked: [i32; 3],
    ) -> bool {
        self.stack_adventure_predicate(stack, "minecraft:can_place_on", clicked)
    }
}
