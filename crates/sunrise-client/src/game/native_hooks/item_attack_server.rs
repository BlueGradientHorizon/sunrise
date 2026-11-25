use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_, crate::game::server_side::ServerState> {
    /// `ItemStack.onLeftClickEntity` for the acting player's blow at `entity`
    /// of `kind`, once `attack_entity` let it through: true when the held
    /// item's Part canceled the attack. Nothing runs for a hand holding an
    /// item without the Part.
    pub(in crate::game::server_side) fn native_left_click_entity_event(
        &mut self,
        entity: i32,
        kind: &str,
    ) -> bool {
        if !mod_api::__private::has_part_hook(mod_api::HookId::OnLeftClickEntity) {
            return false;
        }
        let Some(stack) = self.held() else {
            return false;
        };
        if !mod_api::__private::item_listens(mod_api::HookId::OnLeftClickEntity, &stack.item) {
            return false;
        }
        let stack = stack.clone();
        let player = self.native_acting_id();
        let mut canceled = false;
        self.native_on_left_click_entity(player, &stack, entity, kind, &mut canceled);
        canceled
    }
}
