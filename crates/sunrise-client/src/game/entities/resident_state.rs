//! Game-owned residency state; no second membership directory.
use super::*;
pub(in crate::game) struct ResidentEntities {
    pub table: Box<RefCell<sunrise_client::join_seq::residency::table::EntityTable>>,
    pub local_player: sunrise_client::join_seq::residency::member::Member,
    pub knots: sunrise_client::join_seq::JoinList<leads::Knot>,
    /// The game's general entity rows (#792), beside the knots this store
    /// already keeps for the resident table's lookups.
    pub general: super::general::GeneralStore,
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// `Player.drop(stack, false)`, as `ItemUtils.createFilledResult` drops what no longer fits
    /// in the inventory: thrown from the eyes, with no thrower, statistic or drop hook.
    pub(in crate::game) fn drop_plain(&mut self, stack: ItemStack) -> Option<u32> {
        self.drop_stack_from_eyes(stack, false)
    }
}
