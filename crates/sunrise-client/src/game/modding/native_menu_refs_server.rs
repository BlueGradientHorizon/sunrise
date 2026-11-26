use super::{
    Game,
    lan_crafting::{load_player, machine_menu},
};
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

impl Game<'_, crate::game::server_side::ServerState> {
    /// `ServerPlayer.closeContainer` of a guest's menu: queue the client's close before
    /// running normal server teardown.
    pub(in crate::game) fn menu_close_container(&mut self, menu: MenuRef) -> bool {
        let window = menu.fields().window;
        if !self.is_logical_server() || window == 0 {
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
        if !self.is_logical_server() {
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
            self.side
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
        let host = self.side.lan.as_ref()?;
        (host.hub.holder_guest(guest.uuid, self.dim.id())?.conn == guest.conn).then_some(())?;
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
                self.side
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
        Some(if self.side.lan_acting == Some(guest.conn) {
            self.carried.clone()
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
        let acting = self.side.lan_acting == Some(guest.conn);
        Some(match backing {
            MenuSlotBacking::BlockEntity { .. } | MenuSlotBacking::Entity { .. } => {
                let world = self.native_level_world(self.level(&guest.dimension)?)?;
                guest.menu.as_ref()?.slots.get(world, guest, slot)
            }
            MenuSlotBacking::Player(n @ 0..36) => {
                if acting {
                    self.inventory.get(n)?.clone()
                } else {
                    guest.inventory.get(n)?.clone()
                }
            }
            MenuSlotBacking::Player(n @ 36..40) => {
                if acting {
                    self.armor[39 - n].clone()
                } else {
                    guest.armor[39 - n].clone()
                }
            }
            MenuSlotBacking::Player(_) => {
                if acting {
                    self.offhand.clone()
                } else {
                    guest.offhand.clone()
                }
            }
            MenuSlotBacking::CraftGrid(n) => guest.craft_grid.get(n)?.clone(),
            MenuSlotBacking::CraftResult => guest.craft_result.clone(),
            MenuSlotBacking::Ender(n) => {
                if acting {
                    self.side.ender_items.get(n)?.clone()
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
    pub(in crate::game) fn menu_slot_backing(
        &self,
        menu: MenuRef,
        slot: usize,
    ) -> Option<MenuSlotBacking> {
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
                        .held_block_slot(&self.world, guest, slot)
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
                    .world
                    .block_entities
                    .get_mut(&pos)
                    .and_then(|block| block.slot_mut(index))
                else {
                    return false;
                };
                *held = Some(stack);
                self.after_commit(super::transactions::After::Changed(pos));
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
            super::native_server::PlayerRead::Guest(guest) => Some(guest),
            super::native_server::PlayerRead::Acting { .. } => {
                self.side.lan.as_ref()?.hub.guest(self.side.lan_acting?)
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
                    .side
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
        if self.side.lan_acting == Some(guest.conn) {
            for (n, stack) in self.inventory.iter().enumerate().take(36) {
                rules.set_player_item(n, stack.clone());
            }
            for (n, stack) in self.armor.iter().enumerate() {
                rules.set_player_item(39 - n, stack.clone());
            }
            rules.set_player_item(40, self.offhand.clone());
            rules.selected = self.selected;
            rules.creative = self.instabuild();
            rules.levels = self.experience.level;
            rules.carried = self.carried.clone();
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
    pub(in crate::game) fn native_menu_storage(
        &mut self,
        menu: MenuRef,
        slot: usize,
    ) -> Option<&mut Option<ItemStack>> {
        let backing = self.menu_slot_backing(menu, slot)?;
        if let MenuSlotBacking::Ender(n) = backing
            && self.side.lan_acting_uuid == Some(menu.fields().player.uuid())
        {
            return self.side.ender_items.get_mut(n);
        }
        let guests = &mut self.side.lan.as_mut()?.hub.guests;
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

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::super::api_values::TryIntoApi;
    use super::super::{lan::LanSettings, native_entity_refs::tests::with_game};
    use super::*;
    use crate::game::client_side::native_menu_refs::tests::guest;
    use sunrise_sim::lan::{NetConfig, hub::HubConfig};

    #[test]
    #[ignore = "daily"]
    fn a_server_closes_a_guests_menu_and_sets_their_selected_slot() {
        with_game(|game| {
            let config = HubConfig {
                default_game_mode: 0,
                guest_commands: false,
                force_game_mode: false,
                hardcore: false,
                view_distance: 4,
                simulation_distance: 4,
                dimension: "minecraft:overworld".into(),
                allow_flight: true,
                seed: 0,
                max_players: 10,
                integrated: true,
                levels: game.side.level_shapes,
                level_seeds: game.side.levels.0.runtime.seeds(),
            };
            let mut net = NetConfig::lan(0, "test".into(), 10);
            net.listen = false;
            game.lan_host(
                LanSettings {
                    port: 0,
                    guest_commands: false,
                    force_game_mode: false,
                },
                net,
                config,
            )
            .unwrap();
            game.side.lan.as_mut().unwrap().hub.guests.push(guest());
            let player = game.player(123).unwrap();

            // The selected slot: a slot past the hotbar is refused, a real one is held.
            assert_eq!(player.selected_slot(game), Some(0));
            assert!(!player.set_selected_slot(game, 9));
            assert!(player.set_selected_slot(game, 4));
            assert!(
                player.set_selected_slot(game, 4),
                "the same slot is no error"
            );
            assert_eq!(player.selected_slot(game), Some(4));
            assert_eq!(game.side.lan.as_ref().unwrap().hub.guests[0].selected, 4);

            // A menu: closing it returns the carried stack and leaves no menu behind.
            let stone = ItemStack::new("minecraft:stone", 4);
            let host = game.side.lan.as_mut().unwrap();
            host.hub
                .open_native_menu(
                    1,
                    1,
                    sunrise_core::nbt::Tag::String("test".into()),
                    vec![Some(stone.clone())],
                )
                .unwrap();
            host.hub
                .guests
                .update(0, |guest| guest.carried = Some(stone.clone()));
            let menu = game.player_container_menu(player).unwrap();
            assert_ne!(menu.container_id(game), Some(0));
            game.side
                .lan
                .as_mut()
                .unwrap()
                .hub
                .guests
                .update(0, |guest| guest.disconnect_pending = true);
            assert!(!menu.close_container(game), "disconnect owns the teardown");
            assert_eq!(game.player_container_menu(player), Some(menu));
            let hub = &mut game.side.lan.as_mut().unwrap().hub;
            assert_eq!(hub.guests[0].carried.as_ref(), Some(&stone));
            hub.guests
                .update(0, |guest| guest.disconnect_pending = false);
            assert!(menu.close_container(game));
            let guest = &game.side.lan.as_ref().unwrap().hub.guests[0];
            assert!(guest.menu.is_none(), "the menu is gone");
            assert!(guest.carried.is_none(), "the cursor stack went back");
            assert_eq!(
                guest
                    .inventory
                    .iter()
                    .flatten()
                    .map(|s| s.count)
                    .sum::<u32>(),
                4,
                "to the inventory, none lost or doubled"
            );
            assert!(!menu.close_container(game), "a stale menu is refused");
            // The player's own inventory menu is never closed.
            let inventory = game.player_container_menu(player).unwrap();
            assert_eq!(inventory.container_id(game), Some(0));
            assert!(!inventory.close_container(game));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn menus_read_storage_and_invalidate_on_replacement_wrap_and_reconnect() {
        with_game(|game| {
            let config = HubConfig {
                default_game_mode: 0,
                guest_commands: false,
                force_game_mode: false,
                hardcore: false,
                view_distance: 4,
                simulation_distance: 4,
                dimension: "minecraft:overworld".into(),
                allow_flight: true,
                seed: 0,
                max_players: 10,
                integrated: true,
                levels: game.side.level_shapes,
                level_seeds: game.side.levels.0.runtime.seeds(),
            };
            let mut net = NetConfig::lan(0, "test".into(), 10);
            net.listen = false;
            game.lan_host(
                LanSettings {
                    port: 0,
                    guest_commands: false,
                    force_game_mode: false,
                },
                net,
                config,
            )
            .unwrap();
            game.side.lan.as_mut().unwrap().hub.guests.push(guest());
            let player = game.player(123).unwrap();
            let inventory = game.player_container_menu(player).unwrap();
            assert_eq!(inventory.container_id(game), Some(0));
            assert_eq!(inventory.kind(game), Some(None));
            assert_eq!(inventory.slot_count(game), Some(46));
            assert_eq!(inventory.slot(game, 0), Some(None));
            assert_eq!(
                game.menu_slot_backing(inventory, 36),
                Some(MenuSlotBacking::Player(0))
            );
            assert_eq!(
                game.menu_slot_backing(inventory, 45),
                Some(MenuSlotBacking::Player(40))
            );
            assert_eq!(inventory.slot(game, 46), None);
            let stone = ItemStack::new("minecraft:stone", 4);
            let host = game.side.lan.as_mut().unwrap();
            {
                let value = Some(stone.clone());
                host.hub.guests.update(0, |g| g.inventory[9] = value);
            }
            {
                let value = Some(stone.clone());
                host.hub.guests.update(0, |guest| guest.carried = value);
            }
            let title = sunrise_core::nbt::Tag::String("test".into());
            let window = host
                .hub
                .open_native_menu(1, 1, title.clone(), vec![Some(stone.clone())])
                .unwrap();
            assert_eq!(inventory.slot_count(game), None);
            let menu = game.player_container_menu(player).unwrap();
            assert_eq!(menu.container_id(game), Some(window));
            assert_eq!(menu.kind(game), Some(Some("minecraft:generic_9x1".into())));
            assert_eq!(menu.slot_count(game), Some(45));
            assert_eq!(
                menu.slot(game, 0),
                Some(Some(stone.try_into_api().unwrap()))
            );
            assert_eq!(menu.slot(game, 1), Some(None));
            assert_eq!(
                menu.slot(game, 9),
                Some(Some(stone.try_into_api().unwrap()))
            );
            // Like a new AbstractContainerMenu, this window starts with an empty cursor,
            // independent of the inventory menu's carried stack.
            assert_eq!(menu.carried(game), Some(None));
            let stick = ItemStack::new("minecraft:stick", 2);
            game.side
                .lan
                .as_mut()
                .unwrap()
                .hub
                .guests
                .update(0, |guest| {
                    guest.carried = Some(stick.clone());
                });
            assert_eq!(
                menu.carried(game),
                Some(Some(stick.try_into_api().unwrap()))
            );
            assert_eq!(
                game.menu_slot_backing(menu, 0),
                Some(MenuSlotBacking::Native(0))
            );
            assert_eq!(
                game.menu_slot_backing(menu, 9),
                Some(MenuSlotBacking::Player(9))
            );
            assert_eq!(
                game.menu_slot_backing(menu, 36),
                Some(MenuSlotBacking::Player(0))
            );
            assert_eq!(game.menu_slot_backing(menu, 45), None);
            let host = game.side.lan.as_mut().unwrap();
            for _ in 0..100 {
                host.hub
                    .open_native_menu(1, 1, title.clone(), vec![])
                    .unwrap();
            }
            assert_eq!(host.hub.guests[0].menu.as_ref().unwrap().window, window);
            assert_eq!(menu.slot_count(game), None);
            let newer = game.player_container_menu(player).unwrap();
            let host = game.side.lan.as_mut().unwrap();
            host.hub.close_container(1);
            assert_eq!(newer.slot_count(game), None);
            let inventory = game.player_container_menu(player).unwrap();
            assert_eq!(
                inventory.carried(game),
                Some(Some(stone.try_into_api().unwrap()))
            );
            let host = game.side.lan.as_mut().unwrap();
            *host.menus.entry(1).or_default().crafting.table = Some((
                7,
                0,
                Box::new(sunrise_sim::menu::Menu::new(
                    sunrise_sim::menu::Kind::Crafting,
                )),
                [0; 3],
            ));
            assert_eq!(game.player_container_menu(player), None);
            assert_eq!(inventory.slot_count(game), None);
            let host = game.side.lan.as_mut().unwrap();
            host.play.clear();
            host.menus.clear();
            host.hub.guests.replace(0, guest());
            assert_eq!(inventory.slot_count(game), None);

            use sunrise_level::containers::BlockEntity;
            use sunrise_sim::lan::menu::Slots;
            let a = (1, 2, 3);
            let b = (2, 2, 3);
            let stone = ItemStack::new("minecraft:stone", 3);
            game.world
                .block_entities
                .insert(a, BlockEntity::Chest(vec![None; 2]));
            game.world
                .block_entities
                .insert(b, BlockEntity::Chest(vec![Some(stone.clone()); 3]));
            game.side.lan.as_mut().unwrap().hub.open_container(
                1,
                "minecraft:generic_9x1",
                title.clone(),
                a,
                Slots::Double(a, b),
                vec![None; 5],
                None,
            );
            let menu = game.player_container_menu(player).unwrap();
            assert_eq!(
                game.menu_slot_backing(menu, 2),
                Some(MenuSlotBacking::BlockEntity {
                    level: game.level("minecraft:overworld").unwrap(),
                    pos: [b.0, b.1, b.2],
                    slot: 0
                })
            );
            assert_eq!(
                menu.slot(game, 2),
                Some(Some(stone.try_into_api().unwrap()))
            );
            let vehicle = (i32::MAX - 15, VEHICLE_KEY_Y, 42);
            game.world
                .block_entities
                .insert(vehicle, BlockEntity::Chest(vec![Some(stone.clone())]));
            game.side.lan.as_mut().unwrap().hub.open_container(
                1,
                "minecraft:generic_9x1",
                title,
                vehicle,
                Slots::Block(vehicle),
                vec![None],
                None,
            );
            let menu = game.player_container_menu(player).unwrap();
            assert_eq!(
                game.menu_slot_backing(menu, 0),
                Some(MenuSlotBacking::Entity {
                    entity: EntityRef::new(game.level("minecraft:overworld").unwrap().epoch(), 42),
                    slot: 0
                })
            );
            assert_eq!(
                menu.slot(game, 0),
                Some(Some(stone.try_into_api().unwrap()))
            );

            // Identical coordinates in different dimensions must not alias.
            let gold = ItemStack::new("minecraft:gold_ingot", 2);
            let emerald = ItemStack::new("minecraft:emerald", 3);
            game.world
                .block_entities
                .insert(a, BlockEntity::Chest(vec![Some(gold.clone())]));
            let title = sunrise_core::nbt::Tag::String("dimension".into());
            game.side.lan.as_mut().unwrap().hub.open_container(
                1,
                "minecraft:generic_9x1",
                title.clone(),
                a,
                Slots::Block(a),
                vec![Some(gold.clone())],
                None,
            );
            let overworld_menu = game.player_container_menu(player).unwrap();
            let overworld = game.level("minecraft:overworld").unwrap();
            assert!(
                game.enter_command_level(super::super::Dim::Nether)
                    .is_some()
            );
            game.world
                .block_entities
                .insert(a, BlockEntity::Chest(vec![Some(emerald.clone())]));
            let mut nether_guest = guest();
            nether_guest.conn = 2;
            nether_guest.uuid = 456;
            nether_guest.entity_id = 2;
            nether_guest.dimension = "minecraft:the_nether".into();
            game.side
                .lan
                .as_mut()
                .unwrap()
                .hub
                .guests
                .push(nether_guest);
            game.side.lan.as_mut().unwrap().hub.open_container(
                2,
                "minecraft:generic_9x1",
                title,
                a,
                Slots::Block(a),
                vec![Some(emerald.clone())],
                None,
            );
            let other_player = game.player(456).unwrap();
            let nether_menu = game.player_container_menu(other_player).unwrap();
            let nether = game.level("minecraft:the_nether").unwrap();
            assert_eq!(
                overworld_menu.slot(game, 0),
                Some(Some(gold.try_into_api().unwrap()))
            );
            assert_eq!(
                game.menu_slot_backing(overworld_menu, 0),
                Some(MenuSlotBacking::BlockEntity {
                    level: overworld,
                    pos: [a.0, a.1, a.2],
                    slot: 0
                })
            );
            assert!(
                game.enter_command_level(super::super::Dim::Overworld)
                    .is_some()
            );
            assert_eq!(
                nether_menu.slot(game, 0),
                Some(Some(emerald.try_into_api().unwrap()))
            );
            assert_eq!(
                game.menu_slot_backing(nether_menu, 0),
                Some(MenuSlotBacking::BlockEntity {
                    level: nether,
                    pos: [a.0, a.1, a.2],
                    slot: 0
                })
            );
        });
    }
}
