//! A shift-click in a mod container's menu, routed by the mods
//! (`native_menu_quick_move`, `AbstractContainerMenu.quickMoveStack`) before
//! the game's own routing.
use super::api_values::{TryIntoApi, TryIntoEngine};
use super::native_hooks::ResidentBlockEntityAccess;
use super::{Game, transactions::After};
use sunrise_client::{
    block_entities::Pos,
    containers::{BlockEntity, ModMenuType},
    items::ItemStack,
    lan::menu::Slots,
    menu::Asked,
};
use sunrise_protocol::packets::play;

/// `ClickType.QUICK_MOVE`'s id in `ServerboundContainerClickPacket`.
const QUICK_MOVE: i32 = 1;

/// Where a menu slot of a block container's menu lives: the container's own
/// slot `k`, or the player's inventory index (hotbar 0 to 8, then the 27
/// main slots), the menu listing the main slots before the hotbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MenuSlot {
    Container(usize),
    Inventory(usize),
}

/// The slot a quick move of `slot` on a menu with `size` container slots
/// asks a mod about, or None when the click is not one: another click type,
/// a button other than left or right (`doClick` takes 0 and 1), no slot,
/// a slot past the menu, or a container slot the mod's screen hides (the
/// menu has no such slot, so the click goes nowhere).
pub(super) fn quick_move_slot(
    input: i32,
    button: i32,
    slot: i32,
    size: usize,
    hidden: &[usize],
) -> Option<MenuSlot> {
    if input != QUICK_MOVE || !(0..=1).contains(&button) {
        return None;
    }
    let slot = usize::try_from(slot).ok()?;
    if slot < size {
        return (!hidden.contains(&slot)).then_some(MenuSlot::Container(slot));
    }
    let n = slot - size;
    match n {
        0..27 => Some(MenuSlot::Inventory(9 + n)),
        27..36 => Some(MenuSlot::Inventory(n - 27)),
        _ => None,
    }
}

/// Validates and writes back a `menu_quick_move` remainder onto `stack`, returning `false` when
/// the listener changed `held` to an invalid stack (leaving `stack` untouched). An untouched
/// `held` bypasses revalidation so pre-existing engine stacks round-trip cleanly.
fn apply_quick_move_remainder(
    items: &sunrise_client::items::Items,
    original: &Option<mod_api::ItemStack>,
    held: &Option<mod_api::ItemStack>,
    stack: &mut Option<ItemStack>,
) -> bool {
    if held == original {
        return true;
    }
    match held.try_into_engine(items) {
        Ok(updated) => {
            *stack = updated;
            true
        }
        Err(_) => false,
    }
}

enum QuickMoveOutcome {
    /// No mod handled the shift-click (or the slot was empty): fall through to the game's routing.
    Unhandled { items_changed: bool },
    /// A mod handled the shift-click and its remainder (and any `menu_slot_take` notice) ran.
    Handled,
    /// A listener panicked or returned an invalid remainder: resend the menu without running
    /// the game's routing.
    Rejected,
}

impl Game<'_> {
    /// `ServerGamePacketListenerImpl.handleContainerClick` for a quick move
    /// in a guest's open mod container menu (on a vanilla menu type or the
    /// mod's own): the mods' `native_menu_quick_move` first, as the clicking
    /// player. True when a mod took it (or a listener panicked / returned an
    /// invalid remainder): the slot keeps what the mod left and the guest's
    /// client gets the whole menu again. False for every other click, and for
    /// one no mod took, so the game's own routing runs.
    pub(super) fn lan_mod_quick_move(&mut self, conn: u32, p: &play::ContainerClick) -> bool {
        if !mod_api::__private::has_part_hook(mod_api::HookId::MenuQuickMove) {
            return false;
        }
        let Some(host) = self.lan.as_ref() else {
            return false;
        };
        let Some(g) = host.hub.guest(conn) else {
            return false;
        };
        let Some(menu) = g.menu.as_ref().filter(|m| {
            p.container != 0
                && m.window == p.container
                && usize::try_from(p.slot).is_ok_and(|slot| slot < m.slot_count())
        }) else {
            return false;
        };
        let Slots::Block(pos) = menu.slots else {
            return false;
        };
        let Some(at) = quick_move_slot(
            p.input,
            i32::from(p.button),
            i32::from(p.slot),
            menu.size,
            &menu.hidden,
        ) else {
            return false;
        };
        // A spectator's or a dead player's click and a menu no longer valid
        // change nothing, and a shift-click during a drag only ends the drag
        // (`doClick`'s resetQuickCraft): the game's own path answers them.
        if g.spectator()
            || g.dead
            || menu.drag.is_some()
            || !menu.still_valid(&host.hub.loop_context, &self.world, g)
        {
            return false;
        }
        let (size, window, menu_type) = (menu.size, menu.window, menu.mod_type);
        // A mod menu type keeps its drag in the connection's machine menu
        // (`lan_machine_click`), not in `menu.drag`: a shift-click during it
        // goes there too, to only end the drag.
        if host
            .menus
            .get(&conn)
            .and_then(|play| play.crafting.machine.as_ref())
            .is_some_and(|(w, m)| *w == window && m.quick_crafting())
        {
            return false;
        }
        let Some(kind) = self
            .world
            .block_entities
            .get(&pos)
            .and_then(|entity| match entity {
                BlockEntity::Mod(m)
                    if !m.id().is_empty()
                        && mod_api::__private::block_entity_type_listens(
                            mod_api::HookId::MenuQuickMove,
                            m.id(),
                        ) =>
                {
                    mod_api::__private::block_entity_type_key(m.id())
                }
                _ => None,
            })
        else {
            return false;
        };
        let slot = p.slot as usize;
        // The game's own routing reuses the pickup rule's answer asked before the hook, so the
        // click asks it once, as the client's prediction does.
        let Some((gives, asked)) = self.as_guest(conn, |g| {
            g.mod_menu_quick_move_gives(pos, menu_type, slot, at)
        }) else {
            return false;
        };
        if !gives {
            return !asked.is_empty() && self.lan_machine_click_asked(conn, p, asked);
        }
        let Some(outcome) = self.as_guest(conn, |g| {
            g.run_mod_quick_move(pos, size, menu_type, slot, at, kind)
        }) else {
            return false;
        };
        match outcome {
            QuickMoveOutcome::Unhandled { items_changed } => {
                if items_changed
                    && let Some(host) = self.lan.as_mut()
                    && let Some(play) = host.menus.get_mut(&conn)
                    && play
                        .crafting
                        .machine
                        .as_ref()
                        .is_some_and(|(w, _)| *w == window)
                {
                    *play.crafting.machine = None;
                }
                return !asked.is_empty() && self.lan_machine_click_asked(conn, p, asked);
            }
            QuickMoveOutcome::Handled | QuickMoveOutcome::Rejected => {}
        }
        // The mod menu type's engine starts over from the block entity next click.
        let Some(host) = self.lan.as_mut() else {
            return false;
        };
        if let Some(play) = host.menus.get_mut(&conn)
            && play
                .crafting
                .machine
                .as_ref()
                .is_some_and(|(w, _)| *w == window)
        {
            *play.crafting.machine = None;
        }
        // sendAllDataToRemote: the client predicted the game's routing. A mod
        // that removed its own block leaves no menu to send: it closes, and
        // the inventory goes out on its own.
        if matches!(
            self.world.block_entities.get(&pos),
            Some(BlockEntity::Mod(_))
        ) {
            host.hub.resend_menu(conn, window, &self.world);
        } else if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            host.hub.close_container(conn);
            host.hub.send_inventory(gi);
        }
        true
    }

    /// Dispatch `MenuQuickMove` directly against the live resident block entity, validate its
    /// remainder, apply the clicked slot update, and fire `menu_slot_take` when a mod menu
    /// type's own slot moved items.
    fn run_mod_quick_move(
        &mut self,
        pos: Pos,
        size: usize,
        menu_type: Option<&'static ModMenuType>,
        slot: usize,
        at: MenuSlot,
        kind: mod_api::BlockEntityTypeKey,
    ) -> QuickMoveOutcome {
        let held = match at {
            MenuSlot::Container(k) => match self.world.block_entities.get(&pos) {
                Some(BlockEntity::Mod(m)) => m.items.get(k).cloned().flatten(),
                _ => None,
            },
            MenuSlot::Inventory(i) => self.inventory.get(i).cloned().flatten(),
        };
        let Some(clicked) = held.filter(|s| s.count > 0) else {
            return QuickMoveOutcome::Unhandled {
                items_changed: false,
            };
        };
        let Ok(api_clicked) = clicked.try_into_api() else {
            return QuickMoveOutcome::Unhandled {
                items_changed: false,
            };
        };
        let Some((handle, state)) = self.resident_block_entity_target(pos) else {
            return QuickMoveOutcome::Unhandled {
                items_changed: false,
            };
        };
        let original = Some(api_clicked);
        let mut held = original.clone();
        let mut handled = false;
        let failures = mod_api::runtime::isolated_failures();
        let items_changed = {
            let mut access = ResidentBlockEntityAccess::for_handle(self, handle, state);
            let scope = mod_api::BlockEntityScope::new(&mut access);
            Game::native_menu_quick_move(kind, scope, size, slot, &mut held, &mut handled);
            access.items_changed()
        };
        if items_changed {
            self.after_commit(After::Changed(pos));
        }
        if mod_api::runtime::isolated_failures() != failures {
            return QuickMoveOutcome::Rejected;
        }
        if !handled {
            return QuickMoveOutcome::Unhandled { items_changed };
        }
        let mut remainder = Some(clicked.clone());
        if !apply_quick_move_remainder(self.items(), &original, &held, &mut remainder) {
            return QuickMoveOutcome::Rejected;
        }
        let player = self.native_acting_id();
        match at {
            MenuSlot::Container(k) => {
                if self.world.block_entities.get(&pos).is_some() {
                    self.journal_block_entity(pos);
                    if let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&pos) {
                        if m.items.len() <= k {
                            m.items.resize(k + 1, None);
                        }
                        if let Some(dest) = m.item_slot_mut(k) {
                            *dest = remainder.clone();
                        }
                        self.after_commit(After::Changed(pos));
                    }
                }
                if let Some(menu) = menu_type.filter(|menu| menu.owns_slot(k)) {
                    let count = clicked
                        .count
                        .saturating_sub(remainder.as_ref().map_or(0, |l| l.count));
                    if count > 0 {
                        let mut moved = clicked;
                        moved.count = count;
                        let take = (k, moved, remainder);
                        let _ = self.mod_menu_slot_take(pos, player, menu.id, &take);
                    }
                }
            }
            MenuSlot::Inventory(i) => {
                self.journal_player(player);
                if let Some(s) = self.inventory.get_mut(i) {
                    *s = remainder;
                }
            }
        }
        QuickMoveOutcome::Handled
    }

    /// `Slot.mayPickup` of the clicked slot, which `doClick` asks before the menu's
    /// `quickMoveStack`: a slot of a mod menu type its parts refuse (`menu_slot_rules`) asks no
    /// mod, and the game's own routing refuses it with the same answer. The answers go to that
    /// routing (`Menu::click_asked`), which does not ask again.
    fn mod_menu_quick_move_gives(
        &self,
        pos: Pos,
        menu_type: Option<&'static ModMenuType>,
        slot: usize,
        at: MenuSlot,
    ) -> (bool, Asked) {
        let Some(BlockEntity::Mod(m)) = self.world.block_entities.get(&pos) else {
            return (true, Asked::default());
        };
        let held = match at {
            MenuSlot::Container(k) => m.items.get(k).cloned().flatten(),
            MenuSlot::Inventory(i) => self.inventory.get(i).cloned().flatten(),
        };
        match (menu_type, held.filter(|s| s.count > 0)) {
            (Some(t), Some(stack)) => {
                self.mod_menu_slot_gives(t, slot, &stack, [pos.0, pos.1, pos.2], &m.items)
            }
            _ => (true, Asked::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MenuSlot, quick_move_slot};

    /// A quick move asks about the clicked slot only: a `generic_9x1` menu
    /// has the container's 9, then the main inventory, then the hotbar.
    #[test]
    fn a_quick_move_names_the_container_or_inventory_slot() {
        assert_eq!(
            quick_move_slot(1, 0, 0, 9, &[]),
            Some(MenuSlot::Container(0))
        );
        assert_eq!(
            quick_move_slot(1, 1, 8, 9, &[]),
            Some(MenuSlot::Container(8))
        );
        // The first main slot is inventory index 9, the last 35.
        assert_eq!(
            quick_move_slot(1, 0, 9, 9, &[]),
            Some(MenuSlot::Inventory(9))
        );
        assert_eq!(
            quick_move_slot(1, 0, 35, 9, &[]),
            Some(MenuSlot::Inventory(35))
        );
        // The hotbar comes last in the menu and first in the inventory.
        assert_eq!(
            quick_move_slot(1, 0, 36, 9, &[]),
            Some(MenuSlot::Inventory(0))
        );
        assert_eq!(
            quick_move_slot(1, 0, 44, 9, &[]),
            Some(MenuSlot::Inventory(8))
        );
    }

    /// Only a left or right shift-click on a slot the menu has is a quick
    /// move: no other click type, button, outside click or hidden slot.
    #[test]
    fn other_clicks_are_not_quick_moves() {
        // PICKUP, SWAP, CLONE, THROW, QUICK_CRAFT, PICKUP_ALL.
        for input in [0, 2, 3, 4, 5, 6] {
            assert_eq!(quick_move_slot(input, 0, 0, 9, &[]), None);
        }
        assert_eq!(quick_move_slot(1, 2, 0, 9, &[]), None);
        assert_eq!(quick_move_slot(1, -1, 0, 9, &[]), None);
        assert_eq!(quick_move_slot(1, 0, -999, 9, &[]), None);
        assert_eq!(quick_move_slot(1, 0, 45, 9, &[]), None);
        // A slot the mod's screen hides is not in the menu.
        assert_eq!(quick_move_slot(1, 0, 3, 9, &[3]), None);
        assert_eq!(
            quick_move_slot(1, 0, 4, 9, &[3]),
            Some(MenuSlot::Container(4))
        );
        // The inventory's slots are never hidden.
        assert_eq!(
            quick_move_slot(1, 0, 9, 9, &[0, 1, 2, 3, 4, 5, 6, 7, 8]),
            Some(MenuSlot::Inventory(9))
        );
    }
}
