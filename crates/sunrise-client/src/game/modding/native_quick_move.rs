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

/// Admitted menu and resident block-entity identity for a single `menu_quick_move` click.
struct QuickMoveAdmission {
    conn: u32,
    window: i32,
    menu_generation: u64,
    pos: Pos,
    handle: mod_api::BlockEntityRef,
    state: sunrise_world::registry::BlockStateId,
    size: usize,
    menu_type: Option<&'static ModMenuType>,
    slot: usize,
    at: MenuSlot,
    kind: mod_api::BlockEntityTypeKey,
}

enum QuickMoveOutcome {
    /// No mod handled the shift-click (or the slot was empty): fall through to the game's routing.
    Unhandled { items_changed: bool },
    /// A mod handled the shift-click. Settle only on the admitted menu and holder.
    Handled,
    /// No listener handled the move after a callback failed, or the remainder was invalid:
    /// resend the menu without running the game's routing.
    Rejected,
}

impl Game<'_> {
    /// `ServerGamePacketListenerImpl.handleContainerClick` for a quick move
    /// in a guest's open mod container menu (on a vanilla menu type or the
    /// mod's own): the mods' `native_menu_quick_move` first, as the clicking
    /// player. An accepted remainder settles the clicked slot and resends
    /// the menu. An invalid remainder or a panic without an accepted answer
    /// also resends the menu rather than running the game's own routing.
    /// False for other clicks and ordinary unanswered moves.
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
        let (size, window, menu_type, menu_generation) =
            (menu.size, menu.window, menu.mod_type, g.menu_generation);
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
                    if mod_api::__private::block_entity_type_listens(
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
        let Some((handle, state)) = self.resident_block_entity_target(pos) else {
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
        let admission = QuickMoveAdmission {
            conn,
            window,
            menu_generation,
            pos,
            handle,
            state,
            size,
            menu_type,
            slot,
            at,
            kind,
        };
        let Some(outcome) = self.as_guest(conn, |g| g.run_mod_quick_move(&admission)) else {
            return false;
        };
        let menu_matches = self.quick_move_menu_matches(conn, window, menu_generation);
        let holder_matches = self.quick_move_holder_matches(handle);
        if let QuickMoveOutcome::Unhandled { items_changed } = outcome
            && menu_matches
            && holder_matches
        {
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
        // Handled, Rejected, or stale Unhandled (backing block removed/replaced or menu
        // closed/replaced): never route the old click to a replacement or close a new menu.
        let Some(host) = self.lan.as_mut() else {
            return true;
        };
        if menu_matches {
            // The mod menu type's engine starts over from the block entity next click.
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
            // that removed or replaced its own block leaves no matching holder to
            // send: close the old window and send the inventory on its own.
            if holder_matches {
                host.hub.resend_menu(conn, window, &self.world);
            } else if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
                host.hub.close_container(conn);
                host.hub.send_inventory(gi);
            }
        } else if let Some(gi) = host
            .hub
            .guests
            .iter()
            .position(|g| g.conn == conn && g.menu.is_none())
        {
            host.hub.send_inventory(gi);
        }
        true
    }

    fn quick_move_menu_matches(&self, conn: u32, window: i32, menu_generation: u64) -> bool {
        self.lan.as_ref().is_some_and(|host| {
            host.hub.guest(conn).is_some_and(|g| {
                g.menu_generation == menu_generation
                    && g.menu.as_ref().is_some_and(|m| m.window == window)
            })
        })
    }

    fn quick_move_holder_matches(&self, handle: mod_api::BlockEntityRef) -> bool {
        let Some(pos) = self.native_block_entity_write_key(handle) else {
            return false;
        };
        matches!(
            self.world.block_entities.get(&pos),
            Some(BlockEntity::Mod(_))
        )
    }

    /// Dispatch `MenuQuickMove` directly against the live resident block entity, validate its
    /// remainder, apply the clicked slot update, and fire `menu_slot_take` when a mod menu
    /// type's own slot moved items.
    fn run_mod_quick_move(&mut self, admission: &QuickMoveAdmission) -> QuickMoveOutcome {
        let QuickMoveAdmission {
            conn,
            window,
            menu_generation,
            pos,
            handle,
            state,
            size,
            menu_type,
            slot,
            at,
            kind,
        } = *admission;
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
        let original = Some(api_clicked);
        let mut held = original.clone();
        let mut handled = false;
        let ((items_changed, panicked), notices) = self.collect_quick_move_notices(|game| {
            let mut access = ResidentBlockEntityAccess::for_handle(game, handle, state);
            let scope = mod_api::BlockEntityScope::new(&mut access);
            let panicked =
                Game::native_menu_quick_move(kind, scope, size, slot, &mut held, &mut handled);
            (access.items_changed(), panicked)
        });
        let (outcome, settlement) = if handled {
            // Dispatch restores only the failed listener's outputs. An accepted answer can
            // settle even if another callback failed. Keep untouched engine stacks as-is.
            let remainder = if held == original {
                Ok(Some(clicked.clone()))
            } else {
                held.try_into_engine(self.items())
            };
            match remainder {
                Ok(remainder) => (QuickMoveOutcome::Handled, Some(remainder)),
                Err(error) => {
                    log::warn!("mod menu_quick_move write refused: {error:?}");
                    (QuickMoveOutcome::Rejected, None)
                }
            }
        } else if panicked {
            (QuickMoveOutcome::Rejected, None)
        } else {
            (QuickMoveOutcome::Unhandled { items_changed }, None)
        };

        let mut publish = items_changed;
        let mut take = None;
        if let Some(remainder) = settlement
            && self.quick_move_menu_matches(conn, window, menu_generation)
            && self.quick_move_holder_matches(handle)
        {
            match at {
                MenuSlot::Container(k) => {
                    self.journal_block_entity(pos);
                    if let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&pos) {
                        if m.items.len() <= k {
                            m.items.resize(k + 1, None);
                        }
                        if let Some(dest) = m.item_slot_mut(k) {
                            *dest = remainder.clone();
                        }
                        publish = true;
                    }
                    if let Some(menu) = menu_type.filter(|menu| menu.owns_slot(k)) {
                        let count = clicked
                            .count
                            .saturating_sub(remainder.as_ref().map_or(0, |l| l.count));
                        if count > 0 {
                            let mut moved = clicked;
                            moved.count = count;
                            take = Some((menu.id, (k, moved, remainder)));
                        }
                    }
                }
                MenuSlot::Inventory(i) => {
                    let player = self.native_acting_id();
                    self.journal_player(player);
                    if let Some(s) = self.inventory.get_mut(i) {
                        *s = remainder;
                    }
                }
            }
        }
        // AbstractContainerMenu.moveItemStackTo changes the source before notifying the
        // destination. Release committed capability notices only once the remainder is settled.
        // Surviving edits still notify after an unanswered or refused move.
        self.publish_quick_move_notices(notices);
        // Notify about any live source edits, but not at a replacement holder.
        if publish && self.quick_move_holder_matches(handle) {
            self.after_commit(After::Changed(pos));
        }
        if let Some((menu, take)) = take
            && self.quick_move_menu_matches(conn, window, menu_generation)
            && self.quick_move_holder_matches(handle)
        {
            let player = self.native_acting_id();
            let _ = self.mod_menu_slot_take(pos, player, menu, &take);
        }
        outcome
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
