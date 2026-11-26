//! `IItemExtension.canEquip` on the game's side (`native_can_equip`). The dispenser, a mob's
//! pickup and the two inventory menus ask it, each with the game as the read-only `Host` and the
//! target as a handle. A stack can be converted once for the parts and lent to every slot and
//! entity asked about it in a synchronous check.
use super::*;
use crate::game::api_values::TryIntoApi;
use sunrise_sim::mobs::equip::CanEquip;

/// A stack converted once for the parts of its item, ready for any number of `can_equip`
/// questions in a synchronous check. `DataOnly` when no part of the item listens (the component's
/// answer stands and nothing is converted); `Refused` when a listening stack cannot be converted.
pub(in crate::game) enum EquipStack {
    DataOnly,
    Refused,
    Ready(Box<mod_api::ItemStack>),
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub fn equip_stack(&self, stack: &ItemStack) -> EquipStack {
        if !mod_api::__private::item_listens(mod_api::HookId::CanEquip, &stack.item) {
            return EquipStack::DataOnly;
        }
        #[cfg(test)]
        EQUIP_CONVERSIONS.with(|count| count.set(count.get() + 1));
        match stack.try_into_api() {
            Ok(stack) => EquipStack::Ready(Box::new(stack)),
            Err(_) => EquipStack::Refused,
        }
    }

    /// `LivingEntity.isEquippableInSlot` with `IItemExtension.canEquip`: `data` is the stack's
    /// `equippable` answer for the entity `entity` of type `kind` in `slot`, and the parts of
    /// `stack`'s item may change it. When a part would be asked and `entity` is no live entity of
    /// this side (a missing or stale handle), the slot is refused.
    pub fn can_equip(
        &self,
        stack: &EquipStack,
        slot: &str,
        entity: u32,
        kind: &str,
        data: bool,
    ) -> bool {
        let stack = match stack {
            EquipStack::DataOnly => return data,
            EquipStack::Refused => return false,
            EquipStack::Ready(stack) => stack,
        };
        let Some(entity) = self.entity(entity) else {
            return false;
        };
        let native = crate::game::native_hooks::NativeHookHost {
            host: crate::game::Side::as_host(self),
        };
        let mut can_equip = data;
        native.native_can_equip(
            stack,
            slot,
            mod_api::LivingRef::new(entity),
            kind,
            &mut can_equip,
        );
        can_equip
    }

    /// `can_equip` lent to a menu click that runs while the game is at hand (the player's own
    /// inventory menu, predicted or authoritative).
    pub fn menu_can_equip(&self) -> MenuEquip<'_, '_, __Side> {
        MenuEquip { game: self }
    }
}

/// The game lent to a menu click as its `CanEquip`.
pub struct MenuEquip<'g, 'a, S: crate::game::Side> {
    game: &'g Game<'a, S>,
}

/// Call-scoped `CanEquip` that lazily converts one unchanged `ItemStack` at most once across a
/// synchronous candidate loop. Rust's `&'s ItemStack` borrow prevents mutation during the loop.
struct ScopedMenuEquip<'g, 'a, 's, S: crate::game::Side> {
    game: &'g Game<'a, S>,
    stack: &'s ItemStack,
    prepared: std::cell::OnceCell<EquipStack>,
}

impl<S: crate::game::Side> CanEquip for ScopedMenuEquip<'_, '_, '_, S> {
    fn listens(&self, item: &str) -> bool {
        mod_api::__private::item_listens(mod_api::HookId::CanEquip, item)
    }
    fn can_equip(
        &self,
        stack: &ItemStack,
        slot: &str,
        entity: u32,
        kind: &str,
        data: bool,
    ) -> bool {
        if std::ptr::eq(stack, self.stack) {
            let prepared = self
                .prepared
                .get_or_init(|| self.game.equip_stack(self.stack));
            self.game.can_equip(prepared, slot, entity, kind, data)
        } else {
            let prepared = self.game.equip_stack(stack);
            self.game.can_equip(&prepared, slot, entity, kind, data)
        }
    }
    fn with_stack(&self, _stack: &ItemStack, body: &mut dyn FnMut(&dyn CanEquip)) {
        body(self);
    }
}

impl<S: crate::game::Side> CanEquip for MenuEquip<'_, '_, S> {
    fn listens(&self, item: &str) -> bool {
        mod_api::__private::item_listens(mod_api::HookId::CanEquip, item)
    }
    fn can_equip(
        &self,
        stack: &ItemStack,
        slot: &str,
        entity: u32,
        kind: &str,
        data: bool,
    ) -> bool {
        let prepared = self.game.equip_stack(stack);
        self.game.can_equip(&prepared, slot, entity, kind, data)
    }
    fn with_stack(&self, stack: &ItemStack, body: &mut dyn FnMut(&dyn CanEquip)) {
        let scoped = ScopedMenuEquip {
            game: self.game,
            stack,
            prepared: std::cell::OnceCell::new(),
        };
        body(&scoped);
    }
}

#[cfg(test)]
thread_local! { static EQUIP_CONVERSIONS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)}; }
#[cfg(any(test, feature = "test-support"))]
pub fn take_equip_conversions() -> usize {
    EQUIP_CONVERSIONS.with(|count| count.replace(0))
}
