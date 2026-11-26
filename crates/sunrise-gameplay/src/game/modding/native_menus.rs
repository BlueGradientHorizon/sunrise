//! Blockless server menus for native mods: vanilla chest displays or declared mod menus.
//! Owned slots run ordinary clicks; display slots move nothing by default. Menu parts can
//! replace a click, and a close listener receives any detached owned contents.
use super::{Game, lan::LanHost, lan_menus::GuestMenus};
use mod_api::__private::game::*;
use mod_api::{MenuRef, MenuReplace, ModMenuOpen};
use sunrise_game_data::text::Text;
use sunrise_gameplay_common::game::api_values::TryIntoEngine;
use sunrise_gameplay_common::game::native_menu_data::{ContainerPos, HeardMenu};
use sunrise_gameplay_common::game::native_menu_ids::MOUNT_MENU;
use sunrise_items::items::ItemStack;
use sunrise_sim::lan::{
    data::component_nbt,
    menu::{Menu, NativeMenuEvent, Slots},
};

/// The inventory slot (0..9 the hotbar, 9..36 the rest) a click on `slot`
/// of a native menu of `size` slots landed on, when it is the player's
/// inventory below the menu (`ChestMenu`: the menu's slots, then the
/// inventory's 27, then the hotbar's 9).
#[allow(dead_code)]
pub(in crate::game::server_side) fn native_menu_inventory_slot(
    size: usize,
    slot: i32,
) -> Option<usize> {
    let n = usize::try_from(slot).ok()?.checked_sub(size)?;
    match n {
        0..27 => Some(n + 9),
        27..36 => Some(n - 27),
        _ => None,
    }
}

/// Adds every stack of `gets` to the 36 slots of `inventory`, onto matching
/// stacks first, then into empty slots, hotbar first; false when one doesn't
/// fit whole. `max` is an item's stack size.
pub(in crate::game::server_side) fn fit(
    inventory: &mut [Option<ItemStack>],
    gets: &[ItemStack],
    max: &impl Fn(&ItemStack) -> u32,
) -> bool {
    for stack in gets {
        let mut left = stack.count;
        let max = max(stack).max(1);
        for slot in inventory.iter_mut().take(36).flatten() {
            if left > 0 && slot.same_item_same_components(stack) && slot.count < max {
                let n = (max - slot.count).min(left);
                slot.count += n;
                left -= n;
            }
        }
        for slot in inventory.iter_mut().take(36) {
            if left > 0 && slot.is_none() {
                let n = max.min(left);
                *slot = Some(ItemStack {
                    count: n,
                    ..stack.clone()
                });
                left -= n;
            }
        }
        if left > 0 {
            return false;
        }
    }
    true
}

/// Server menus, for server mods. The players are named by UUID;
/// none of these runs while a player's state is swapped in (inside
/// a command executor, the menus can be opened and filled).
#[allow(dead_code)]
impl crate::game::ServerGame<'_> {
    pub(in crate::game::server_side) fn native_conn(&self, uuid: u128) -> Option<u32> {
        let host = self.server.lan.as_ref()?;
        let g = host
            .hub
            .guests
            .iter()
            .find(|g| g.placed && g.participates_online() && g.uuid == uuid)?;
        Some(g.conn)
    }
    /// Opens a chest menu of `rows` rows (1 to 6) titled `title` for the
    /// player, showing `items` (by slot, padded with empty ones), in place of
    /// the menu they had open: its window id, a new one each time. The old
    /// menu closes as a close does, its cursor stack going back to the
    /// inventory, and `replace` says whether the client is first sent a close.
    pub(in crate::game) fn open_menu(
        &mut self,
        uuid: u128,
        rows: u8,
        title: Text,
        items: Vec<Option<ItemStack>>,
        replace: MenuReplace,
    ) -> Option<i32> {
        let conn = self.native_conn(uuid)?;
        if self.menu_click_in_progress(conn) {
            return None;
        }
        self.close_for_open(conn, replace);
        let gi = self.guest_index(conn)?;
        // The first contents show the player's own inventory and cursor, in their turn too.
        let window = self.with_own_record(gi, |lan, _| {
            lan.hub
                .open_native_menu(conn, rows, component_nbt(&title.component()), items)
        })?;
        self.player_menu_opening(conn);
        Some(window)
    }
    /// `ServerPlayer.openMenu`'s close of the menu that was open: `closeContainer`, which then
    /// tells the client, or `doCloseContainer` for a provider that skips the client's close.
    /// Either way the old menu's `removed` runs and the mods hear it close.
    pub(in crate::game::server_side) fn close_for_open(&mut self, conn: u32, replace: MenuReplace) {
        self.journal_merchant_replacement(conn);
        match replace {
            MenuReplace::CloseFirst => self.guest_close_menus_and_tell(conn),
            MenuReplace::InPlace => self.guest_close_all(conn),
        }
    }
    /// `ServerPlayer.containerCounter`: the window id the player's last opened menu took.
    pub(in crate::game) fn player_container_counter(
        &self,
        player: mod_api::PlayerRef,
    ) -> Option<i32> {
        if !self.common.is_logical_server() {
            return None;
        }
        Some(self.online_player_record(player)?.next_window)
    }
    /// `ServerPlayer.nextContainerCounter` with no menu after it: the counter moves as an open
    /// moves it, and nothing opens or closes.
    pub(in crate::game) fn player_next_container_counter(
        &mut self,
        player: mod_api::PlayerRef,
    ) -> Option<i32> {
        if !self.common.is_logical_server() {
            return None;
        }
        let conn = self.online_player_record(player)?.conn;
        self.server
            .lan
            .as_mut()?
            .hub
            .update_guest(conn, |guest| guest.next_container_counter())
    }
    /// `ServerPlayer.openMenu(provider, extraDataWriter)` for a content mod menu type and no
    /// block: the type's container slots show empty and their clicks reach the mods, as
    /// `open_menu`'s do; `extra` and `pos` go first in its `sunrise:menu_extra` payload, and
    /// `replace` is the provider's choice of the client's close, as for `open_menu`.
    pub(in crate::game) fn player_open_mod_menu(
        &mut self,
        player: mod_api::PlayerRef,
        request: ModMenuOpen,
    ) -> Result<Option<i32>, mod_api::OpenMenuError> {
        use mod_api::OpenMenuError;
        use sunrise_protocol::menu_extra;

        let ModMenuOpen {
            menu,
            title,
            extra,
            data,
            pos,
            opening_hand,
            replace,
            ..
        } = request;
        let title = title.try_into_engine(&())?;
        let id = mod_api::__private::menu_key_id(menu).ok_or(OpenMenuError::NotModMenu)?;
        let kind =
            sunrise_level::containers::mod_menu_type(&id).ok_or(OpenMenuError::NotModMenu)?;
        if kind.owned_slots.is_some() && kind.crafting.is_some() {
            return Err(OpenMenuError::OwnedSlotsWithCrafting);
        }
        if extra.len() > menu_extra::MAX_DATA {
            return Err(OpenMenuError::TooLarge);
        }
        if data.len() != kind.data_slots {
            return Err(OpenMenuError::DataCountMismatch);
        }
        let title = component_nbt(&title.component());
        sunrise_protocol::buf::check_network_nbt(&title)
            .map_err(|_| OpenMenuError::TitleNotEncodable)?;
        // A live handle of a connected player; a fake player has no connection.
        let guest = self
            .online_player_record(player)
            .filter(|g| !g.dead && !g.disconnect_pending)
            .ok_or(OpenMenuError::PlayerUnavailable)?;
        let revision = if !data.is_empty() || !kind.player_inventory || kind.slots == 0 {
            menu_extra::LIVE_DATA_REVISION
        } else {
            1 // Client must have negotiated menu_extra revision 1 or higher.
        };
        if !sunrise_level::containers::menu_reaches(&guest.brand, &id)
            || guest.sender.menu_extra_rev < revision
        {
            return Err(OpenMenuError::PlayerUnavailable);
        }
        let conn = guest.conn;
        self.server
            .lan
            .as_ref()
            .and_then(|host| host.hub.registry_id("minecraft:menu", &id))
            .ok_or(OpenMenuError::NotModMenu)?;
        // An admitted click cannot send packets for a speculative screen replacement.
        if self.menu_click_in_progress(conn) {
            return Err(OpenMenuError::ClickInProgress);
        }
        if !self
            .guest_may_open(conn, &id, pos)
            .ok_or(OpenMenuError::PlayerUnavailable)?
        {
            return Ok(None);
        }
        let guest = self
            .online_player_record(player)
            .filter(|g| g.conn == conn && !g.dead && !g.disconnect_pending)
            .ok_or(OpenMenuError::PlayerUnavailable)?;
        let opening_item = opening_hand.map(|hand| (hand, self.held_by(guest, hand).1.cloned()));
        let pos = pos.map(|[x, y, z]| (x, y, z));
        self.close_for_open(conn, replace);
        let gi = self
            .guest_index(conn)
            .ok_or(OpenMenuError::PlayerUnavailable)?;
        // The first contents show the player's own inventory and cursor, in their turn too.
        let window = self
            .with_own_record(gi, |lan, _| {
                let window = lan.hub.open_menu_window(
                    conn,
                    &id,
                    title,
                    pos,
                    Slots::Native,
                    vec![None; kind.slots],
                    Some(extra),
                )?;
                lan.hub.guests.update(gi, |g| {
                    let open = g.menu.as_mut().expect("the window was just installed");
                    open.blockless_data = data;
                    open.opening_item = opening_item;
                    g.send_menu_data_snapshot();
                });
                Some(window)
            })
            .ok_or(OpenMenuError::PlayerUnavailable)?;
        self.player_menu_opening(conn);
        Ok(Some(window))
    }
    /// The native menu the player has open as window `window`, for its slots;
    /// None for another window, a menu the game opened, or a player not here.
    pub(in crate::game) fn menu(&self, uuid: u128, window: i32) -> Option<MenuRef> {
        let conn = self.native_conn(uuid)?;
        self.server
            .lan
            .as_ref()?
            .hub
            .guest(conn)?
            .menu
            .as_ref()
            .filter(|m| m.window == window && m.slots == sunrise_sim::lan::menu::Slots::Native)?;
        self.common
            .player_container_menu(self.common.player(uuid)?)
            .filter(|menu| menu.fields().window == window)
    }
    /// Replaces what the player's open native menu `window` shows, without reopening it
    /// (`AbstractContainerMenu.sendAllDataToRemote`): the whole contents in one packet.
    pub(in crate::game) fn menu_set_items(
        &mut self,
        uuid: u128,
        window: i32,
        items: Vec<Option<ItemStack>>,
    ) -> Result<(), mod_api::MenuItemsRefusal> {
        use mod_api::MenuItemsRefusal;
        if !self.common.is_logical_server() {
            return Err(MenuItemsRefusal::Client);
        }
        let conn = self.native_conn(uuid).ok_or(MenuItemsRefusal::NoPlayer)?;
        let transactional = !self.common.transactions.is_empty();
        if transactional && let Some(player) = self.common.player(uuid) {
            self.journal_menus(player);
        }
        let host = self.server.lan.as_mut().ok_or(MenuItemsRefusal::NoPlayer)?;
        host.hub
            .set_native_menu_items(conn, window, items, !transactional)?;
        // The scoped click reconciles its own window on both override and refusal.
        if transactional
            && !self
                .common
                .transactions
                .iter()
                .any(|level| level.menu_click_active == Some(conn))
        {
            self.reply_menu_click(
                conn,
                sunrise_gameplay_common::game::lan_menu_data::ClickReply::Resend(window),
            );
        }
        Ok(())
    }
    /// Closes the player's native menu (a `native_menu_closed` follows). An active click
    /// stages the close for its current window and sends it only once the transaction commits.
    pub(in crate::game) fn close_menu(&mut self, uuid: u128) {
        if let Some(conn) = self.native_conn(uuid)
            && let Some(guest) = self
                .server
                .lan
                .as_ref()
                .and_then(|host| host.hub.guest(conn))
            && let Some(open) = guest
                .menu
                .as_ref()
                .filter(|m| m.slots == sunrise_sim::lan::menu::Slots::Native)
        {
            self.request_menu_close(conn, open.window, guest.menu_generation);
        }
    }
}

impl GuestMenus {
    pub(in crate::game::server_side) fn menu_holds(
        &self,
        menu: Option<&Menu>,
        pos: ContainerPos,
        generation: u64,
    ) -> bool {
        self.menu_heard
            .as_ref()
            .zip(menu)
            .is_some_and(|(opened, live)| {
                opened.window == live.window && opened.openers.contains(&Some((pos, generation)))
            })
    }
}

/// The menu `conn` has open, whichever way the game keeps it: a block's
/// container, a trade, a crafting table, a workstation, a lectern or a
/// mount's inventory. Not the player's own inventory menu.
pub(in crate::game) fn open_menu_of(
    host: &LanHost,
    conn: u32,
) -> Option<(i32, &str, Option<[i32; 3]>)> {
    let g = host.hub.guest(conn)?;
    let play = host.play.get(&conn);
    let menus = host.menus.get(&conn);
    if let Some(m) = &g.menu {
        let menu = host.hub.registry_name("minecraft:menu", m.kind)?;
        // A vehicle's menu is keyed by a stand-in, not a block.
        let pos = m
            .pos
            .filter(|p| p.1 != sunrise_sim::lan::menu::VEHICLE_KEY_Y)
            .map(|p| [p.0, p.1, p.2]);
        return Some((m.window, menu, pos));
    }
    let (window, menu, pos) = if let Some(gm) = menus.and_then(|m| m.merchant.as_ref()) {
        (gm.window, "minecraft:merchant", None)
    } else if let Some((window, _, _, pos)) = menus.and_then(|m| m.crafting.table.as_ref()) {
        (*window, "minecraft:crafting", Some(*pos))
    } else if let Some(st) = menus.and_then(|m| m.station.as_ref()) {
        (
            st.window,
            sunrise_gameplay_common::game::lan_station_data::menu_of(st.kind)?.0,
            Some(st.pos),
        )
    } else if let Some(l) = play.and_then(|p| p.lectern.as_ref()) {
        (
            l.window,
            "minecraft:lectern",
            Some([l.key.0, l.key.1, l.key.2]),
        )
    } else {
        let m = play.and_then(|p| p.mount_window.as_ref())?;
        (m.window, MOUNT_MENU, None)
    };
    Some((window, menu, pos))
}

/// Where the game calls the menu hook points.
impl crate::game::ServerGame<'_> {
    /// `ServerPlayer.openMenu` / `openHorseInventory`: called only after
    /// constructing the menu, queuing its open packet and sending full contents.
    /// Remember it before dispatch so a hook can close it synchronously.
    pub(in crate::game) fn player_menu_opening(&mut self, conn: u32) {
        let Some(host) = self.server.lan.as_mut() else {
            return;
        };
        let Some((window, menu, pos)) = open_menu_of(host, conn) else {
            return;
        };
        let openers = host
            .hub
            .guest(conn)
            .and_then(|g| g.menu.as_ref())
            .map_or([None; 2], |m| {
                let positions = match m.slots {
                    Slots::Block(at) => [Some(at), None],
                    Slots::Double(first, second) => [Some(first), Some(second)],
                    Slots::Ender => [m.pos, None],
                    Slots::Native => [None; 2],
                };
                positions.map(|pos| {
                    pos.and_then(|pos| {
                        self.common
                            .world
                            .block_entities
                            .generation(&pos)
                            .map(|generation| (pos, generation))
                    })
                })
            });
        let menu = HeardMenu {
            window,
            menu: menu.to_owned(),
            pos,
            openers,
        };
        host.menus.entry(conn).or_default().menu_heard = Some(menu.clone());
        self.player_menu_changed(conn, menu, true);
    }

    /// `conn`'s tracked menu closes with the guest's state in place
    /// (`ServerPlayer.doCloseContainer`). Close listeners are checked independently
    /// of open listeners.
    pub(in crate::game) fn player_menu_closing(&mut self, conn: u32) {
        let heard = self
            .server
            .lan
            .as_mut()
            .and_then(|host| host.menus.get_mut(&conn))
            .and_then(|p| {
                p.owned_menu = None;
                p.menu_heard.take()
            });
        if let Some(old) = heard {
            self.player_menu_changed(conn, old, false);
        }
    }

    fn player_menu_changed(&mut self, conn: u32, menu: HeardMenu, open: bool) {
        let hook = if open {
            mod_api::HookId::PlayerContainerOpen
        } else {
            mod_api::HookId::PlayerContainerClose
        };
        let heard = mod_api::__private::listened(hook);
        let openers = menu
            .openers
            .map(|opener| opener.filter(|&(pos, _)| self.common.mod_menu_at(pos).is_some()));
        if !heard && openers == [None; 2] {
            return;
        }
        let Some(guest) = self
            .server
            .lan
            .as_ref()
            .and_then(|host| host.hub.guest(conn))
        else {
            return;
        };
        let uuid = guest.uuid;
        let name = heard.then(|| guest.name.clone());
        let dimension = self.common.dim.id();
        self.player_hook(conn, move |g| {
            for (pos, generation) in openers.into_iter().flatten() {
                g.mod_container_player_opener_for(pos, open, Some(generation));
            }
            if let Some(name) = name {
                if open {
                    let handle = g
                        .player(uuid)
                        .and_then(|player| g.player_container_menu(player));
                    g.native_player_container_open(
                        uuid, &name, dimension, &menu.menu, menu.pos, handle,
                    );
                } else {
                    g.native_player_container_close(uuid, &name, dimension, &menu.menu, menu.pos);
                }
            }
        });
    }

    /// Runs a player hook with the guest's state in place, or as it is when
    /// the guest has no body to swap in.
    fn player_hook(&mut self, conn: u32, f: impl FnOnce(&mut Self)) {
        let mut f = Some(f);
        if self.server.lan_acting != Some(conn) {
            self.as_guest(conn, |g| {
                if let Some(f) = f.take() {
                    f(g);
                }
            });
        }
        if let Some(f) = f {
            f(self);
        }
    }

    /// The notices the hub collected, dispatched with the hub in `self.lan` and no guest turn active.
    pub(in crate::game) fn run_native_menu_events(&mut self) {
        loop {
            let Some(host) = self.server.lan.as_mut() else {
                return;
            };
            if host.hub.native_menu_events.is_empty() {
                return;
            }
            match host.hub.native_menu_events.remove(0) {
                NativeMenuEvent::ReturnCarried { conn, item } => {
                    if let Some(gi) = self.guest_index(conn) {
                        self.lan_place_back(gi, vec![item]);
                    }
                }
                NativeMenuEvent::ReturnProcessingInputs {
                    conn,
                    items,
                    drop_instead,
                } => {
                    if let Some(gi) = self.guest_index(conn) {
                        let dead = self
                            .server
                            .lan
                            .as_ref()
                            .is_some_and(|host| host.hub.guests[gi].dead);
                        if drop_instead || dead {
                            self.drop_guest_items(gi, items);
                        } else {
                            self.lan_place_back(gi, items);
                        }
                    }
                }
                NativeMenuEvent::Closed {
                    uuid,
                    window,
                    opening_hand,
                    owned_slots,
                    ..
                } => {
                    self.menu_places_closed(uuid, window);
                    if mod_api::__private::listened(mod_api::HookId::MenuClosed) {
                        use sunrise_gameplay_common::game::api_values::TryIntoApi;
                        let owned_slots: Vec<_> = owned_slots
                            .iter()
                            .map(|slot| slot.as_ref().and_then(|stack| stack.try_into_api().ok()))
                            .collect();
                        self.native_menu_closed(uuid, window, opening_hand, &owned_slots);
                    }
                }
                // A write to a backed menu's storage (#3385): its slots looked at again.
                NativeMenuEvent::PlacesChanged { uuid, window } => {
                    self.menu_places_changed(uuid, window)
                }
            }
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
pub mod tests {
    use sunrise_gameplay_common::game::server_side::native_item_places::tests::guest;

    use sunrise_gameplay_common::game::server_side::native_menus::tests::MOD_MENU;

    use super::super::native_entity_refs::tests::with_game;
    use super::*;
    use mod_api::{HostTransaction, ItemPlace, ItemResource, ServerHost, TransactionError};
    use std::sync::{Arc, mpsc};
    use sunrise_gameplay_common::game::api_values::TryIntoApi;
    use sunrise_protocol::{
        buf::Reader,
        packets::play::{self, Clientbound as C, Serverbound as S},
    };
    use sunrise_sim::lan::{Out, PlayerSender};
    use {
        crate::game::server_side::native_item_places::tests::open_to_lan,
        sunrise_gameplay_common::game::server_side::native_item_places::tests::guest,
    };

    /// Guest 1 (UUID 123) on a world open to LAN, and what their client is sent.
    pub fn watched_guest(game: &mut crate::game::ServerGame<'_>) -> mpsc::Receiver<Out> {
        open_to_lan(game);
        let (tx, rx) = mpsc::channel();
        let mut player = guest(1);
        player.sender = PlayerSender::new(tx, Arc::default());
        game.side.lan.as_mut().unwrap().hub.guests.push(player);
        rx
    }

    pub fn open(
        game: &mut crate::game::ServerGame<'_>,
        rows: u8,
        title: &str,
        replace: MenuReplace,
    ) -> i32 {
        let host: &mut dyn ServerHost = game;
        host.open_menu(
            123,
            rows,
            mod_api::Text::literal(title),
            Vec::new(),
            replace,
        )
        .unwrap()
        .unwrap()
    }

    /// [`watched_guest`] on a Sunrise client that reads a menu opened with no block, and the
    /// key of [`MOD_MENU`].
    pub fn mod_menu_guest(
        game: &mut crate::game::ServerGame<'_>,
    ) -> (mpsc::Receiver<Out>, mod_api::MenuKey) {
        let rx = watched_guest(game);
        game.side.lan.as_mut().unwrap().hub.guests.update(0, |g| {
            g.brand = "sunrise".into();
            g.sender.menu_extra_rev = 1;
        });
        let key = super::super::native_guest_turn_tests::register_test_menu(game, MOD_MENU);
        rx.try_iter().for_each(drop); // Registry setup sent reload packets, not part of these opens.
        (rx, key)
    }

    #[cfg(test)]
    #[test]
    fn menu_clicks_map_to_the_inventory_below() {
        // A 6-row chest: 54 slots, then the inventory's 27, then the hotbar.
        assert_eq!(native_menu_inventory_slot(54, 3), None);
        assert_eq!(native_menu_inventory_slot(54, 54), Some(9));
        assert_eq!(native_menu_inventory_slot(54, 80), Some(35));
        assert_eq!(native_menu_inventory_slot(54, 81), Some(0));
        assert_eq!(native_menu_inventory_slot(54, 89), Some(8));
        assert_eq!(native_menu_inventory_slot(54, 90), None);
        assert_eq!(native_menu_inventory_slot(54, -999), None);
        assert_eq!(native_menu_inventory_slot(9, 9), Some(9));
    }
}

use sunrise_gameplay_common::game::tables::WorldTablesExt as _;

use sunrise_gameplay_common::game::transactions::TransactionsGameExt as _;
