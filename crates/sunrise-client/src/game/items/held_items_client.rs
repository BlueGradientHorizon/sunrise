//! Client-side held-item block-break reset check (`MultiPlayerGameMode.sameDestroyTarget`).
use crate::game::Game;
use sunrise_client::items::ItemStack;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `MultiPlayerGameMode.sameDestroyTarget`'s item half for `began`.
    pub(in crate::game::client_side) fn same_destroying_item(
        &self,
        began: &Option<ItemStack>,
    ) -> bool {
        let held = self.held();
        let same = match (began, held) {
            (None, None) => true,
            (Some(a), Some(b)) => a.item == b.item && a.damage == b.damage && a.extra == b.extra,
            _ => false,
        };
        let Some(began) = began else {
            return same;
        };
        if !mod_api::__private::item_listens(
            mod_api::HookId::ShouldCauseBlockBreakReset,
            &began.item,
        ) {
            return same;
        }
        let mut reset = !same;
        self.native_should_cause_block_break_reset(began, held, &mut reset);
        !reset
    }
}
