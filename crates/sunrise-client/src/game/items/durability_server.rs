#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::durability::Wearer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::durability::World;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::durability::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::convert::Infallible;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::items::ItemStack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::item_durability::{
    self, DurabilityStack, DurabilityWorld,
};

impl Game<'_> {
    /// `damage_stack` for the stack the living entity `id` wears.
    pub(in crate::game) fn damage_living_stack(
        &mut self,
        stack: &ItemStack,
        amount: u32,
        id: u32,
    ) -> (Option<ItemStack>, Option<ItemStack>) {
        let amount = amount.min(i32::MAX as u32) as i32;
        self.damage_stack_worn(stack, amount, Wearer::Living(id))
    }
}
