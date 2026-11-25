use super::Game;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `ItemStack.useOn`'s adventure gate for the held item.
    pub(in crate::game) fn may_use_on(&self, clicked: [i32; 3]) -> bool {
        self.may_build() || self.can_place_on_in_adventure(clicked)
    }
}
