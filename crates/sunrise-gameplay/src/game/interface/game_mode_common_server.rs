use crate::game::ServerGame;

impl ServerGame<'_> {
    /// `ItemStack.useOn`'s adventure gate for the held item.
    pub(in crate::game) fn may_use_on(&self, clicked: [i32; 3]) -> bool {
        self.common.may_build() || self.common.can_place_on_in_adventure(clicked)
    }
}
