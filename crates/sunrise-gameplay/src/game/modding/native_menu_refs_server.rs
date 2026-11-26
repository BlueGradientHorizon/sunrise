use super::lan_crafting::{load_player, machine_menu};
use mod_api::__private::game::*;
use mod_api::{EntityRef, MenuRef, MenuSlotBacking, PlayerRef};
use sunrise_items::items::ItemStack;
use sunrise_level::containers::BlockEntity;
use sunrise_sim::{
    lan::{
        guest::Guest,
        menu::{SlotBacking, Slots, VEHICLE_KEY_Y},
    },
    menu::{Env, Kind, Menu},
};

impl crate::game::ServerGame<'_> {
    /// `ServerPlayer.closeContainer` of a guest's menu: queue the client's close before
    /// running normal server teardown.
    pub(in crate::game) fn menu_close_container(&mut self, menu: MenuRef) -> bool {
        let window = menu.fields().window;
        if !self.common.is_logical_server() || window == 0 {
            return false;
        }
        let Some(guest) = self.native_menu_guest(menu) else {
            return false;
        };
        if guest.disconnect_pending {
            return false;
        }
        self.request_menu_close(guest.conn, window, guest.menu_generation);
        true
    }

    /// `DataSlot.set` of an owning mod's blockless menu, journaled by window generation.
    pub(in crate::game) fn menu_set_data_slot(
        &mut self,
        menu: MenuRef,
        index: usize,
        value: i32,
    ) -> Result<bool, mod_api::MenuDataRefusal> {
        use mod_api::MenuDataRefusal::*;
        if !self.common.is_logical_server() {
            return Err(Unsupported);
        }
        let guest = self.native_menu_guest(menu).ok_or(Stale)?;
        let open = guest
            .menu
            .as_ref()
            .filter(|m| m.slots == Slots::Native)
            .ok_or(Unsupported)?;
        let kind = open.mod_type.ok_or(Unsupported)?;
        if mod_api::content::crash::current_native_mod() != Some(kind.owner) {
            return Err(WrongOwner);
        }
        let &committed = open.blockless_data.get(index).ok_or(InvalidIndex)?;
        let conn = guest.conn;
        let key = (conn, guest.menu_generation, index);
        if self.pending_menu_data(key).unwrap_or(committed) == value {
            return Ok(false);
        }
        if !self.journal_menu_data(key, value) {
            self.server
                .lan
                .as_mut()
                .unwrap()
                .hub
                .update_guest(conn, |guest| {
                    guest.set_menu_data(index, value);
                });
        }
        Ok(true)
    }

    pub(in crate::game) fn native_menu_guest(&self, menu: MenuRef) -> Option<&Guest> {
        let guest = self.native_menu_player(menu.fields().player)?;
        let host = self.server.lan.as_ref()?;
        (host
            .hub
            .holder_guest(guest.uuid, self.common.dim.id())?
            .conn
            == guest.conn)
            .then_some(())?;
        (guest.menu_generation == menu.fields().generation).then_some(())?;
        if let Some(open) = &guest.menu {
            (open.window == menu.fields().window).then_some(guest)
        } else {
            let play = host.play.get(&guest.conn);
            let menus = host.menus.get(&guest.conn);
            if menu.fields().window != 0
                || menus.is_some_and(|m| {
                    m.crafting.table.is_some() || m.station.is_some() || m.merchant.is_some()
                })
                || play.is_some_and(|p| p.lectern.is_some() || p.mount_window.is_some())
            {
                return None;
            }
            Some(guest)
        }
    }

    pub(in crate::game) fn server_player_container_menu(
        &self,
        player: PlayerRef,
    ) -> Option<MenuRef> {
        self.player_read_both(player)?;
        let guest = self.native_menu_player(player)?;
        let menu = mod_api::__private::game::menu_ref(
            player,
            guest.menu.as_ref().map_or(0, |m| m.window),
            guest.menu_generation,
        );
        self.native_menu_guest(menu)?;
        Some(menu)
    }

    /// Container id (0 for the player's inventory menu).
    pub(in crate::game) fn menu_container_id(&self, menu: MenuRef) -> Option<i32> {
        self.native_menu_guest(menu)?;
        Some(menu.fields().window)
    }

    pub(in crate::game) fn server_menu_kind(&self, menu: MenuRef) -> Option<Option<String>> {
        let guest = self.native_menu_guest(menu)?;
        match &guest.menu {
            Some(m) => Some(Some(
                self.server
                    .lan
                    .as_ref()?
                    .hub
                    .registry_name("minecraft:menu", m.kind)?
                    .to_owned(),
            )),
            None => Some(None),
        }
    }

    pub(in crate::game) fn server_menu_slot_count(&self, menu: MenuRef) -> Option<usize> {
        let guest = self.native_menu_guest(menu)?;
        Some(
            guest
                .menu
                .as_ref()
                .map_or(46, sunrise_sim::lan::menu::Menu::slot_count),
        )
    }

    /// Carried (cursor) stack; outer None means unavailable.
    pub(in crate::game) fn server_menu_carried(&self, menu: MenuRef) -> Option<Option<ItemStack>> {
        let guest = self.native_menu_guest(menu)?;
        Some(if self.server.lan_acting == Some(guest.conn) {
            self.common.carried.clone()
        } else {
            guest.carried.clone()
        })
    }

    /// Full stack at a menu index. Outer None means unavailable or out of
    /// range; Some(None) means an empty slot.
    pub(in crate::game) fn server_menu_slot(
        &self,
        menu: MenuRef,
        slot: usize,
    ) -> Option<Option<ItemStack>> {
        let backing = self.menu_slot_backing(menu, slot)?;
        let guest = self.native_menu_guest(menu)?;
        let acting = self.server.lan_acting == Some(guest.conn);
        Some(match backing {
            MenuSlotBacking::BlockEntity { .. } | MenuSlotBacking::Entity { .. } => {
                let world = self.native_level_world(self.level(&guest.dimension)?)?;
                guest.menu.as_ref()?.slots.get(world, guest, slot)
            }
            MenuSlotBacking::Player(n @ 0..36) => {
                if acting {
                    self.common.inventory.get(n)?.clone()
                } else {
                    guest.inventory.get(n)?.clone()
                }
            }
            MenuSlotBacking::Player(n @ 36..40) => {
                if acting {
                    self.common.armor[39 - n].clone()
                } else {
                    guest.armor[39 - n].clone()
                }
            }
            MenuSlotBacking::Player(_) => {
                if acting {
                    self.common.offhand.clone()
                } else {
                    guest.offhand.clone()
                }
            }
            MenuSlotBacking::CraftGrid(n) => guest.craft_grid.get(n)?.clone(),
            MenuSlotBacking::CraftResult => guest.craft_result.clone(),
            MenuSlotBacking::Ender(n) => {
                if acting {
                    self.server.ender_items.get(n)?.clone()
                } else {
                    guest.ender_items.get(n)?.clone()
                }
            }
            MenuSlotBacking::Native(n) | MenuSlotBacking::Owned(n) => {
                guest.native_menu.get(n)?.clone()
            }
            // The place's mirror, refreshed after each click and each tick.
            MenuSlotBacking::Place(_) => guest.native_menu.get(slot)?.clone(),
            _ => unreachable!("MenuSlotBacking: unhandled variant"),
        })
    }

    /// The storage menu slot `slot` shows (`Slot.container` and `getContainerSlot`).
    pub fn menu_slot_backing(&self, menu: MenuRef, slot: usize) -> Option<MenuSlotBacking> {
        let guest = self.native_menu_guest(menu)?;
        let Some(open) = &guest.menu else {
            return match slot {
                0 => Some(MenuSlotBacking::CraftResult),
                1..=4 => Some(MenuSlotBacking::CraftGrid(slot - 1)),
                5..=8 => Some(MenuSlotBacking::Player(39 - (slot - 5))),
                9..=35 => Some(MenuSlotBacking::Player(slot)),
                36..=44 => Some(MenuSlotBacking::Player(slot - 36)),
                45 => Some(MenuSlotBacking::Player(40)),
                _ => None,
            };
        };
        if slot >= open.slot_count() {
            return None;
        }
        let level = self.level(&guest.dimension)?;
        let world = self.native_level_world(level)?;
        if open
            .slots
            .block_slots(world)
            .is_some_and(|n| n != open.size)
        {
            return None;
        }
        if slot >= open.size {
            return match slot - open.size {
                n @ 0..=26 => Some(MenuSlotBacking::Player(n + 9)),
                n @ 27..=35 => Some(MenuSlotBacking::Player(n - 27)),
                _ => None,
            };
        }
        Some(match open.slots.backing(world, slot)? {
            SlotBacking::Block(pos, n) if pos.1 == VEHICLE_KEY_Y => MenuSlotBacking::Entity {
                entity: EntityRef::new(level.epoch(), pos.2 as u32),
                slot: n,
            },
            SlotBacking::Block(pos, n) => MenuSlotBacking::BlockEntity {
                level,
                pos: [pos.0, pos.1, pos.2],
                slot: n,
            },
            SlotBacking::Ender(n) => MenuSlotBacking::Ender(n),
            SlotBacking::Native(n) => match self.native_menu_place(guest.conn, open.window, n) {
                Some(place) => MenuSlotBacking::Place(place),
                None if open.owned_slots().is_some() => MenuSlotBacking::Owned(n),
                None => MenuSlotBacking::Native(n),
            },
        })
    }

    pub(in crate::game) fn menu_slot_holds(
        &self,
        menu: MenuRef,
        slot: usize,
        stack: &ItemStack,
    ) -> Option<u32> {
        let (rules, env) = self.native_menu_rules(menu, slot)?;
        Some(rules.slot_holds(&env, slot, stack))
    }

    pub(in crate::game) fn menu_slot_gives(&self, menu: MenuRef, slot: usize) -> Option<bool> {
        let (rules, env) = self.native_menu_rules(menu, slot)?;
        Some(rules.slot_gives(&env, slot))
    }

    pub(in crate::game) fn menu_slot_gives_with_stack(
        &self,
        menu: MenuRef,
        slot: usize,
        stack: &ItemStack,
    ) -> Option<bool> {
        let (mut rules, env) = self.native_menu_rules(menu, slot)?;
        rules.load_slot(slot, Some(stack.clone()));
        Some(rules.slot_gives(&env, slot))
    }

    pub(in crate::game) fn menu_owned_slot_limit(&self, menu: MenuRef, slot: usize) -> Option<u32> {
        let open = self.native_menu_guest(menu)?.menu.as_ref()?;
        let limit = *open.owned_slots()?.get(slot)?;
        Some(u32::from(limit))
    }

    pub(in crate::game) fn replace_crafted_menu_slot(
        &mut self,
        menu: MenuRef,
        slot: usize,
        stack: ItemStack,
    ) -> bool {
        let player = menu.fields().player;
        match self.menu_slot_backing(menu, slot) {
            Some(MenuSlotBacking::Player(index)) => self.native_as_player_handle(player, |game| {
                game.journal_player(player.uuid());
                let held = match index {
                    0..36 => &mut game.inventory[index],
                    36..40 => &mut game.armor[39 - index],
                    _ => &mut game.offhand,
                };
                *held = Some(stack);
            }),
            Some(MenuSlotBacking::BlockEntity { .. } | MenuSlotBacking::Entity { .. }) => {
                let Some((pos, index)) = self.native_menu_guest(menu).and_then(|guest| {
                    guest
                        .menu
                        .as_ref()?
                        .slots
                        .held_block_slot(&self.common.world, guest, slot)
                }) else {
                    return false;
                };
                if pos.1 == VEHICLE_KEY_Y {
                    let entity = pos.2 as u32;
                    self.journal_entity(entity);
                    return self
                        .edit_entity_slots(entity, |slots, _| {
                            let Some(held) = slots.get_mut(index) else {
                                return false;
                            };
                            *held = Some(stack);
                            true
                        })
                        .unwrap_or(false);
                }
                self.journal_block_entity_slot(pos, index);
                let Some(held) = self
                    .common
                    .world
                    .block_entities
                    .get_mut(&pos)
                    .and_then(|block| block.slot_mut(index))
                else {
                    return false;
                };
                *held = Some(stack);
                self.after_commit(sunrise_gameplay_common::game::transactions::After::Changed(
                    pos,
                ));
                true
            }
            Some(
                MenuSlotBacking::Ender(_)
                | MenuSlotBacking::CraftGrid(_)
                | MenuSlotBacking::Native(_)
                | MenuSlotBacking::Owned(_),
            ) => {
                self.journal_menus(player);
                let Some(held) = self.native_menu_storage(menu, slot) else {
                    return false;
                };
                *held = Some(stack);
                true
            }
            _ => false,
        }
    }
    /// Resolve the menu's owner through the shared player boundary. The menu metadata,
    /// crafting grid and native slots stay on the guest during a turn; inventory,
    /// equipment, cursor and ender contents are swapped into Game.
    pub(in crate::game) fn native_menu_player(&self, player: PlayerRef) -> Option<&Guest> {
        match self.player_handle_read(player)? {
            sunrise_gameplay_common::game::native_server::PlayerRead::Guest(guest) => Some(guest),
            sunrise_gameplay_common::game::native_server::PlayerRead::Acting { .. } => {
                self.server.lan.as_ref()?.hub.guest(self.server.lan_acting?)
            }
        }
    }
    /// The menu engine's model of `menu`, holding slot `slot`, for its slots' rules: the
    /// inventory menu, a furnace's, brewing stand's, crafter's or content mod's menu over its
    /// block entity, a shulker box's, or a plain container of the menu's size. None for a slot
    /// with no backing (`menu_slot_backing`), and for one the model hasn't.
    pub(in crate::game) fn native_menu_rules(
        &self,
        menu: MenuRef,
        slot: usize,
    ) -> Option<(Box<Menu>, Env<'_>)> {
        self.menu_slot_backing(menu, slot)?;
        let guest = self.native_menu_guest(menu)?;
        let (kind, disabled, menu_id) = match &guest.menu {
            None => (Kind::Inventory, None, None),
            Some(open) => {
                let world = self.native_level_world(self.level(&guest.dimension)?)?;
                let name = self
                    .server
                    .lan
                    .as_ref()?
                    .hub
                    .registry_name("minecraft:menu", open.kind);
                let entity = match open.slots {
                    Slots::Block(pos) => world.block_entities.get(&pos),
                    _ => None,
                };
                let kind = match entity.and_then(|e| machine_menu(e, open.mod_type)) {
                    Some((kind, _)) => kind,
                    None if open.slots == Slots::Native
                        && let Some(layout) = open.mod_type =>
                    {
                        Kind::Mod(layout)
                    }
                    None if name == Some("minecraft:shulker_box") => Kind::Shulker,
                    None if open.size == 5 => Kind::Hopper,
                    None if open.size == 9 => Kind::Dispenser,
                    None => Kind::Chest(open.size / 9),
                };
                // `CrafterSlot.mayPlace`: a disabled slot takes nothing.
                let disabled = match entity {
                    Some(BlockEntity::Crafter(c)) => Some(c.disabled),
                    _ => None,
                };
                (kind, disabled, name)
            }
        };
        let env = Env {
            player: Some(guest.uuid),
            player_entity_id: Some(guest.entity_id),
            ..self.menu_env()
        };
        let mut rules = Box::new(Menu::new(kind));
        rules.set_registry_id(menu_id);
        if let Some(open) = guest.menu.as_ref() {
            let world = self.native_level_world(self.level(&guest.dimension)?)?;
            let limits = open.slots.stack_limits(world, open.size);
            rules.set_container_limits(limits);
            rules.set_block_pos(
                (open.slots != Slots::Native)
                    .then_some(open.pos)
                    .flatten()
                    .map(|(x, y, z)| [x, y, z]),
            );
            if open.owned_slots().is_some() {
                rules.load_container(&env, &guest.native_menu, None);
            } else if open.slots == Slots::Native && open.mod_type.is_some() {
                for (i, stack) in guest.native_menu.iter().take(open.size).enumerate() {
                    rules.load_slot(i, stack.clone());
                }
            }
        }
        (slot < rules.slot_count()).then_some(())?;
        if self.server.lan_acting == Some(guest.conn) {
            for (n, stack) in self.common.inventory.iter().enumerate().take(36) {
                rules.set_player_item(n, stack.clone());
            }
            for (n, stack) in self.common.armor.iter().enumerate() {
                rules.set_player_item(39 - n, stack.clone());
            }
            rules.set_player_item(40, self.common.offhand.clone());
            rules.selected = self.common.selected;
            rules.creative = self.instabuild();
            rules.levels = self.common.experience.level;
            rules.carried = self.common.carried.clone();
        } else {
            load_player(&mut rules, guest);
        }
        for (k, off) in disabled.iter().flatten().enumerate() {
            rules.data[k] = i32::from(*off);
        }
        Some((rules, env))
    }
    /// The game's own storage menu slot `slot` shows, for a write: the ender chest's, the
    /// crafting grid's or a mod's chest menu's slot, kept on the guest. None for any other.
    pub fn native_menu_storage(
        &mut self,
        menu: MenuRef,
        slot: usize,
    ) -> Option<&mut Option<ItemStack>> {
        let backing = self.menu_slot_backing(menu, slot)?;
        if let MenuSlotBacking::Ender(n) = backing
            && self.server.lan_acting_uuid == Some(menu.fields().player.uuid())
        {
            return self.server.ender_items.get_mut(n);
        }
        let guests = &mut self.server.lan.as_mut()?.hub.guests;
        let index = guests.iter().position(|g| {
            g.placed
                && g.uuid == menu.fields().player.uuid()
                && g.entity_id as u32 == menu.fields().player.entity().id()
        })?;
        match backing {
            MenuSlotBacking::Ender(n) => guests.ender_items_mut(index)?.get_mut(n),
            MenuSlotBacking::CraftGrid(n) => guests.craft_grid_mut(index)?.get_mut(n),
            MenuSlotBacking::Native(n) | MenuSlotBacking::Owned(n) => {
                guests.native_menu_mut(index)?.get_mut(n)
            }
            _ => None,
        }
    }
}

use sunrise_gameplay_common::game::tables::WorldTablesExt as _;

use sunrise_gameplay_common::game::transactions::TransactionsGameExt as _;

use sunrise_gameplay_common::game::game_mode_common::GameModeCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_level_refs_common::NativeLevelRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_server::NativeServerGameExt1 as _;
