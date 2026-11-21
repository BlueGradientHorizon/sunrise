//! A shift-click in a mod container's menu, routed by the mods
//! (`native_menu_quick_move`, `AbstractContainerMenu.quickMoveStack`) before
//! the game's own routing.
use super::{Game, decided_action::Epoch, transactions::After};
use crate::game::transactions::decision::{InvalidReason, Reply};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use sunrise_client::{
    block_entities::Pos,
    containers::{BlockEntity, ModBlockEntity, ModMenuType},
    items::ItemStack,
    lan::menu::{ModCraft, Slots},
    menu::Menu,
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

/// Storage behind the clicked slot in the admitted menu.
enum Source {
    Own(usize),
    Player(usize),
    Linked {
        pos: Pos,
        slot: usize,
        generation: Option<u64>,
    },
    CraftingResult {
        grid: Vec<Option<ItemStack>>,
    },
}

struct QuickMove {
    slot: usize,
    source: Source,
    held: ItemStack,
}

impl Game<'_> {
    /// `ServerGamePacketListenerImpl.handleContainerClick` for a quick move
    /// in a guest's open mod container menu (on a vanilla menu type or the
    /// mod's own): the mods' `native_menu_quick_move` first, as the clicking player.
    /// For a mod menu type, `live` is the prepared click menu that already allowed pickup.
    /// True when the move was handled or rejected; false leaves the click to the menu.
    pub(in crate::game) fn lan_mod_quick_move(
        &mut self,
        conn: u32,
        p: &play::ContainerClick,
        live: Option<&Menu>,
    ) -> bool {
        if p.input != QUICK_MOVE
            || !mod_api::__private::has_part_hook(mod_api::HookId::MenuQuickMove)
        {
            return false;
        }
        let Some(host) = self.lan.as_ref() else {
            return false;
        };
        let Some(gi) = host.hub.guests.connection_index(conn) else {
            return false;
        };
        let g = &host.hub.guests[gi];
        let Some(menu) = g.menu.as_ref().filter(|m| {
            p.container != 0
                && m.window == p.container
                && m.mod_type.is_some() == live.is_some()
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
        // A shift-click during a drag only ends the drag (`doClick`'s resetQuickCraft).
        // The machine route has checked the drag on its own Menu before calling us.
        if g.spectator() || menu.drag.is_some() {
            return false;
        }
        let (size, window, menu_type) = (menu.size, menu.window, menu.mod_type);
        let Some(BlockEntity::Mod(entity)) = self.world.block_entities.get(&pos) else {
            return false;
        };
        if entity.id().is_empty() {
            return false;
        }
        let slot = p.slot as usize;
        let held = match (live, at) {
            (Some(menu), _) => menu.slot(slot).cloned(),
            (None, MenuSlot::Container(k)) => entity.items.get(k).cloned().flatten(),
            (None, MenuSlot::Inventory(i)) => g.inventory[i].clone(),
        };
        let Some(held) = held.filter(|s| s.count > 0) else {
            return false;
        };
        let source = if let Some(live) = live
            && let Some(craft) = live.mod_crafting().filter(|craft| craft.result == slot)
        {
            Source::CraftingResult {
                grid: craft.grid_slots().map(|k| live.slot(k).cloned()).collect(),
            }
        } else if menu_type
            .and_then(|t| t.links)
            .is_some_and(|link| link.slots().contains(&slot))
        {
            let Some((pos, slot)) = menu.slots.held_block_slot(&self.world, g, slot) else {
                return false;
            };
            Source::Linked {
                pos,
                slot,
                generation: self.world.block_entities.generation(&pos),
            }
        } else {
            match at {
                MenuSlot::Container(k) => Source::Own(k),
                MenuSlot::Inventory(i) => Source::Player(i),
            }
        };
        let quick_move = QuickMove { slot, source, held };
        match self.decided_mod_quick_move(conn, pos, size, menu_type, window, quick_move) {
            QuickMoveOutcome::Handled => {}
            QuickMoveOutcome::Untaken => return false,
            QuickMoveOutcome::Rejected => {
                // The client predicted the game's routing: it gets the menu as it is.
                if let Some(host) = self.lan.as_mut() {
                    host.hub.resend_menu(conn, window, &self.world);
                }
                return true;
            }
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

    /// Dispatch `MenuQuickMove`: the block entity is taken out of the world (journaled
    /// first, so a refused move puts it back) and the mods answer on a copy of the slot's stack.
    fn mod_menu_quick_move_fire(&mut self, pos: Pos, size: usize, quick_move: QuickMove) -> Fire {
        self.journal_block_entity(pos);
        let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&pos) else {
            return Fire::Nothing;
        };
        let mut stack = Some(quick_move.held.clone());
        let mut taken = std::mem::take(&mut **m);
        let items = taken.items.clone();
        let mut handled = false;
        let valid = self.menu_quick_move_checked(
            [pos.0, pos.1, pos.2],
            &mut taken,
            size,
            quick_move.slot,
            &mut stack,
            &mut handled,
        );
        if !valid {
            self.put_back_mod_block_entity(pos, taken, &items);
            return Fire::Invalid;
        }
        if !handled {
            // No mod took the click: the game's own routing moves the stack next, so nothing the
            // listener did may stay. The block entity goes back into the world and the action
            // is cancelled, which undoes every write the listener made, to this block and to others.
            self.put_back_mod_block_entity(pos, taken, &items);
            return Fire::Untaken;
        }
        Fire::Done(Fired {
            quick_move,
            taken,
            items,
            stack,
        })
    }

    /// Check the mods' answer before applying it. An invalid answer leaves the block entity,
    /// stack and `handled` as they were and is refused by the decision.
    fn menu_quick_move_checked(
        &mut self,
        pos: [i32; 3],
        entity: &mut ModBlockEntity,
        container_slots: usize,
        slot: usize,
        stack: &mut Option<ItemStack>,
        handled: &mut bool,
    ) -> bool {
        use crate::game::api_values::{
            BlockEntitySnapshotError, TryIntoApi, TryIntoEngine, finish_block_entity_writeback,
        };
        let (Ok(mut api), Ok(mut held)) = (entity.try_into_api(), stack.try_into_api()) else {
            return true;
        };
        let revision = api.revision();
        let original = held.clone();
        let mut candidate_handled = *handled;
        self.native_menu_quick_move(
            pos,
            &mut api,
            container_slots,
            slot,
            &mut held,
            &mut candidate_handled,
        );
        let held = if held == original {
            Ok(None)
        } else {
            held.try_into_engine(self.items())
                .map(Some)
                .map_err(|_| BlockEntitySnapshotError::InvalidStack)
        };
        match finish_block_entity_writeback(
            entity,
            &api,
            revision,
            self.items(),
            "menu_quick_move",
            held,
        ) {
            Ok(held) => {
                if let Some(held) = held {
                    *stack = held;
                }
                *handled = candidate_handled;
                true
            }
            Err(_) => false,
        }
    }

    /// What `mod_menu_quick_move_fire` kept, applied: the slot, the block's items and the block
    /// entity back in the world. True when a mod took the click. A move from a mod menu type's
    /// own slot is then heard as a take (`menu_slot_take`) with the amount it moved; a failed
    /// listener undoes the move.
    fn mod_menu_quick_move_apply(
        &mut self,
        conn: u32,
        window: i32,
        pos: Pos,
        take_menu: Option<&'static ModMenuType>,
        fired: Fired,
    ) -> bool {
        let Fired {
            quick_move: QuickMove { slot, source, held },
            mut taken,
            items,
            stack,
        } = fired;
        let player = self.native_acting_id();
        let count = held
            .count
            .saturating_sub(stack.as_ref().map_or(0, |left| left.count));
        let notice = take_menu.filter(|_| count > 0).map(|menu| {
            let mut moved = held;
            moved.count = count;
            (menu, (slot, moved, stack.clone()))
        });
        let mut craft = None;
        match source {
            Source::Own(slot) => {
                if taken.items.len() <= slot {
                    taken.items.resize(slot + 1, None);
                }
                taken.items[slot] = stack;
            }
            Source::Player(slot) => {
                self.journal_player(player);
                self.inventory[slot] = stack;
            }
            Source::Linked { pos, slot, .. } => {
                self.journal_block_entity(pos);
                let Some(entity) = self.world.block_entities.get_mut(&pos) else {
                    return false;
                };
                let Some(left) = entity.slots().get(slot) else {
                    return false;
                };
                if *left != stack {
                    let Some(left) = entity.slot_mut(slot) else {
                        return false;
                    };
                    *left = stack;
                    self.after_commit(After::Changed(pos));
                }
            }
            Source::CraftingResult { grid } => {
                craft = Some(ModCraft {
                    grid,
                    result: stack,
                })
            }
        }
        if let Some((menu, take)) = notice
            && !self.api_menu_slot_take(&mut taken, player, menu.id, &take)
        {
            // The click's savepoint still owns both inventories, including everything
            // the quick-move listener wrote before the notice failed.
            return false;
        }
        if let Some(craft) = craft {
            if let Some(player) = self.player(player) {
                self.journal_menus(player);
            }
            if let Some(host) = self.lan.as_mut()
                && let Some(gi) = host.hub.guests.connection_index(conn)
            {
                host.hub.guests.update(gi, |guest| {
                    if let Some(open) = guest.menu.as_mut().filter(|open| open.window == window) {
                        open.craft = Some(craft);
                    }
                });
            }
        }
        self.put_back_menu_entity(pos, taken, &items);
        true
    }

    /// Route a mod menu quick move as one decided action, after asking the slot rules once.
    #[allow(clippy::too_many_arguments)]
    fn decided_mod_quick_move(
        &mut self,
        conn: u32,
        pos: Pos,
        size: usize,
        menu_type: Option<&'static ModMenuType>,
        window: i32,
        quick_move: QuickMove,
    ) -> QuickMoveOutcome {
        let (Some(actor), Some(route)) = (self.guest_actor(conn), self.guest_route(conn)) else {
            return QuickMoveOutcome::Rejected;
        };
        let Some(admitted) = Admitted::of(self, conn, pos, size, window) else {
            return QuickMoveOutcome::Rejected;
        };
        let fired = Rc::new(RefCell::new(None));
        let handled = Rc::new(Cell::new(None));
        let untaken = Rc::new(Cell::new(false));
        let (kept, taken, result, left) = (
            fired.clone(),
            fired.clone(),
            handled.clone(),
            untaken.clone(),
        );
        let take_menu = menu_type.filter(|menu| {
            menu.owns_slot(quick_move.slot)
                && matches!(self.world.block_entities.get(&pos), Some(BlockEntity::Mod(entity))
                    if mod_api::__private::block_entity_type_listens(mod_api::HookId::MenuSlotTake, entity.id()))
        });
        let linked = match &quick_move.source {
            Source::Linked {
                pos, generation, ..
            } => Some((*pos, *generation)),
            _ => None,
        };
        let decide = |game: &mut Self| {
            game.decided_action(
                "menu_quick_move",
                route,
                actor,
                Epoch::Player,
                // Everything the click was admitted on is still so.
                move |game| {
                    admitted.holds(game)
                        && linked.is_none_or(|(pos, generation)| {
                            game.world.block_entities.generation(&pos) == generation
                        })
                },
                move |game| match game
                    .as_guest(conn, |g| g.mod_menu_quick_move_fire(pos, size, quick_move))
                {
                    Some(Fire::Done(fired)) => {
                        *kept.borrow_mut() = Some(fired);
                        Reply::Allow
                    }
                    Some(Fire::Untaken) => {
                        left.set(true);
                        Reply::Cancel
                    }
                    Some(Fire::Invalid) => Reply::Invalid(InvalidReason::Plan),
                    Some(Fire::Nothing) | None => Reply::Failed,
                },
                move |game| {
                    if let Some(fired) = taken.take() {
                        result.set(Some(
                            game.as_guest(conn, |g| {
                                g.mod_menu_quick_move_apply(conn, window, pos, take_menu, fired)
                            })
                            .unwrap_or(false),
                        ));
                    }
                },
            )
        };
        let applied = if take_menu.is_some() || linked.is_some() {
            mod_api::run_transaction(self, |game, _| {
                // `decided_action` closes its own savepoint into this one. If the apply was
                // entered but the notice or source write-back failed, abort the earlier writes.
                let applied = decide(game);
                if applied && handled.get() != Some(false) {
                    Ok(true)
                } else {
                    Err(())
                }
            })
            .is_ok()
        } else {
            decide(self)
        };
        if applied && handled.get() == Some(true) {
            QuickMoveOutcome::Handled
        } else if untaken.get() {
            QuickMoveOutcome::Untaken
        } else {
            if handled.get() == Some(false) {
                log::warn!("menu_quick_move: could not apply the move at {pos:?}; nothing changed");
            }
            QuickMoveOutcome::Rejected
        }
    }
}

enum QuickMoveOutcome {
    /// A mod listener handled the shift-click and its writes stood.
    Handled,
    /// No mod took the click; continue with the admitted menu's quick-move.
    Untaken,
    /// The decided action failed validation, panicked, or went stale; resend the menu.
    Rejected,
}

/// What `lan_mod_quick_move` admitted the click on, read again before the move is applied: the
/// menu, the block entity behind it (a replaced one has another generation though its values may
/// be equal) and the inputs a listener's command could change (the guest's mode and life, a drag).
/// The validity hook and the slot rules are not asked again.
pub(super) struct Admitted {
    conn: u32,
    window: i32,
    pos: Pos,
    size: usize,
    hidden: Vec<usize>,
    generation: Option<u64>,
}

impl Admitted {
    pub(super) fn of(
        game: &Game<'_>,
        conn: u32,
        pos: Pos,
        size: usize,
        window: i32,
    ) -> Option<Self> {
        let menu = game.lan.as_ref()?.hub.guest(conn)?.menu.as_ref()?;
        Some(Self {
            conn,
            window,
            pos,
            size,
            hidden: menu.hidden.clone(),
            generation: game.world.block_entities.generation(&pos),
        })
    }

    /// Whether everything admitted is still so. A block entity that is gone is not a replaced
    /// one: the move's apply puts back or drops what it took.
    pub(super) fn holds(&self, game: &Game<'_>) -> bool {
        let Some(guest) = game.lan.as_ref().and_then(|host| host.hub.guest(self.conn)) else {
            return false;
        };
        guest.menu.as_ref().is_some_and(|m| {
            m.window == self.window
                && m.slots == Slots::Block(self.pos)
                && m.size == self.size
                && m.hidden == self.hidden
                && m.drag.is_none()
        }) && !guest.spectator()
            && !guest.dead
            && game
                .world
                .block_entities
                .generation(&self.pos)
                .is_none_or(|now| Some(now) == self.generation)
    }
}

/// What the mods' call left, before it is applied.
struct Fired {
    quick_move: QuickMove,
    taken: ModBlockEntity,
    items: Vec<Option<ItemStack>>,
    stack: Option<ItemStack>,
}

enum Fire {
    /// The block is gone before the listener can be reached.
    Nothing,
    /// The mods' answer was invalid.
    Invalid,
    /// The mods asked took nothing: the game routes the click.
    Untaken,
    Done(Fired),
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
