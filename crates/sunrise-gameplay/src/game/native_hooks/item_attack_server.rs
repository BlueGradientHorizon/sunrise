use super::*;

/// The hook points. Each does nothing in Sunrise.
impl crate::game::ServerGame<'_> {
    /// `ItemStack.onLeftClickEntity` for the acting player's blow at `entity`
    /// of `kind`, once `attack_entity` let it through: true when the held
    /// item's Part canceled the attack. Nothing runs for a hand holding an
    /// item without the Part.
    pub(crate) fn native_left_click_entity_event(
        &mut self,
        entity: i32,
        kind: &str,
    ) -> bool {
        if !mod_api::__private::has_part_hook(mod_api::HookId::OnLeftClickEntity) {
            return false;
        }
        let Some(stack) = self.common.held() else {
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

/// The hook points. Each does nothing in Sunrise.
impl crate::game::ServerGame<'_> {
    /// `IItemExtension.onLeftClickEntity`: `player`, holding `stack`, is
    /// about to attack the entity `entity` of type `kind`, where
    /// `Player.attack` starts, once `attack_entity` let the blow through and
    /// before any damage, `hurt_enemy` or wear. Set `canceled` and the attack
    /// ends there.
    pub(in crate::game) fn native_on_left_click_entity(
        &mut self,
        player: u128,
        stack: &ItemStack,
        entity: i32,
        kind: &str,
        canceled: &mut bool,
    ) {
        self.fire_on_left_click_entity(player, stack, entity, kind, canceled);
    }
}

use sunrise_gameplay_common::game::native_hooks::ModGameExt1 as _;
