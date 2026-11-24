#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::menu_interactions::*;

impl Game<'_> {
    pub(in crate::game) fn creative_drop_features(&self) -> Option<&Arc<[String]>> {
        // Screen::Inventory covers the non-creative AbstractContainerScreens.
        (self.creative && self.remote.is_some() && self.client.screen != Screen::Inventory)
            .then(|| self.enabled_feature_ids())
    }
    /// `CreativeModeInventoryScreen.slotClicked` outside the window: the server knows nothing of
    /// a stack picked from the creative screen, so the carried stack (an item tab's right click:
    /// one of it) is dropped through `handleCreativeModeItemDrop`, a SetCreativeModeSlot to slot
    /// -1, and the hand swings as `Player.drop` predicts.
    pub(in crate::game) fn remote_creative_drop(&mut self, right: bool) {
        if !self.client.drop_spam_throttler.is_under_threshold() {
            return;
        }
        let one = right && !self.creative_inventory_open();
        let Some(carried) = self.carried.as_mut().filter(|s| !s.is_empty()) else {
            return;
        };
        let thrown = if one && carried.count > 1 {
            carried.count -= 1;
            ItemStack {
                count: 1,
                ..carried.clone()
            }
        } else {
            self.carried.take().expect("carried")
        };
        self.creative_screen_drop(thrown);
    }
    /// `MultiPlayerGameMode.handleCreativeModeItemDrop`: the click site, not
    /// this sender, checks the threshold. Only an accepted send incurs a charge.
    pub(in crate::game) fn remote_creative_throw(&mut self, thrown: ItemStack) {
        if self.try_send_creative_drop_packet(&thrown) {
            self.client.drop_spam_throttler.increment();
        }
    }
    /// `ClientPacketListener.handleMountScreenOpen`: only horses and nautiluses
    /// open a menu. A missing or different entity leaves the current menu alone.
    pub(in crate::game) fn remote_open_mount(&mut self, p: play::MountScreenOpen) -> Result<()> {
        use crate::game::{inventory_slots::MOD_MENU_SLOT_BASE, mount_menu::CHEST};
        // `new SimpleContainer(getInventorySize(inventoryColumns))` is built before
        // the entity is looked at, so it throws for any entity: a size that comes
        // out negative, as Java's `int` wraps, ends the connection. `HorseInventoryMenu`
        // lays out `inventoryColumns` chest slots only when that is positive, a
        // `NautilusInventoryMenu` has none; the chest's slots must end below the mod
        // menus' base, and a chest that does not fit ends the connection, as the
        // exception does.
        let size = p.columns.wrapping_mul(3);
        if size < 0 {
            return self.refuse_play_payload("the mount menu's chest has a negative size");
        }
        let columns = usize::try_from(p.columns).unwrap_or(0);
        if columns > (MOD_MENU_SLOT_BASE - CHEST) / 3 {
            return self.refuse_play_payload("the mount menu's chest does not fit its slot range");
        }
        let mut items = Vec::new();
        if items.try_reserve_exact(size as usize).is_err() {
            return self.refuse_play_payload("the mount menu's chest does not fit its slot range");
        }
        let entity = p.entity as u32;
        let Some(species) = self
            .mobs
            .iter()
            .find(|m| m.id == entity)
            .map(sunrise_client::mobs::Mob::species)
            .filter(|&s| crate::game::mount_menu::has_mount_menu(s))
        else {
            return Ok(());
        };
        if self.mod_screen.0.is_some() {
            self.remote_close_local();
        }
        // `HorseInventoryMenu` lays out its chest slots only `if (inventoryColumns > 0)`
        // and a `NautilusInventoryMenu` has none: no chest slot addresses the container then.
        let chest = if sunrise_client::mobs::nautilus::is_nautilus(species) || columns == 0 {
            0
        } else {
            size as usize
        };
        items.resize(chest, None);
        let columns = if chest == 0 { 0 } else { columns };
        // Install the window before opening its screen, including when a mod
        // replaces the requested screen with the player's own inventory.
        self.remote_set_menu(Some((p.container, format!("{MOUNT_MENU}{columns}"))), None);
        self.open_remote_mount_menu(entity, species, items);
        Ok(())
    }
    /// ClientboundOpenScreenPacket for the menus Sunrise draws.
    pub(in crate::game) fn remote_open_screen(&mut self, p: play::OpenScreen) {
        let menu = self
            .remote
            .as_ref()
            .and_then(|r| {
                r.builtin
                    .get("minecraft:menu")?
                    .name(p.menu)
                    .map(str::to_owned)
            })
            .unwrap_or_default();
        let target = self
            .remote
            .as_mut()
            .and_then(|r| r.use_target.take())
            .map(|t| (t[0], t[1], t[2]));
        // The server names a mod menu's block itself: it, not the last
        // clicked block, is the one the menu is of. Only for this window:
        // any other's is stale.
        let extra = self
            .remote
            .as_mut()
            .and_then(|r| r.menu_extra.take())
            .filter(|e| e.window == p.container);
        let target = extra
            .as_ref()
            .map_or(target, |e| e.pos.map(|p| (p[0], p[1], p[2])));
        let mod_type = sunrise_client::containers::mod_menu_type(&menu);
        // MenuScreens.create looks up the constructor before touching the active window.
        if mod_type.is_some() && !mod_api::__private::has_menu_screen_factory(&menu) {
            log::warn!("no screen factory for menu {menu}");
            return;
        }
        if self.remote.as_ref().and_then(|r| r.menu.as_ref()).is_some()
            || self.mod_screen.0.is_some()
        {
            // An in-place open has no intervening ContainerClose packet. Drop
            // the replaced menu's local state, not the screen or pointer grab.
            self.remote_close_local();
        } else {
            self.recipe_ui.0.reset_screen();
        }
        if menu == "minecraft:lectern" {
            // LecternScreen: the book and page come in the menu's content and data.
            self.remote_set_menu(Some((p.container, menu)), None);
            self.remote_open_lectern(target.unwrap_or_default());
            return;
        }
        if let Some(kind) = Self::remote_station_kind(&menu) {
            let title = menu_title(&p.title);
            let pos = target.map_or([0, 0, 0], |t| [t.0, t.1, t.2]);
            self.remote_open_station(p.container, menu, kind, title, pos);
            return;
        }
        if menu == "minecraft:merchant" {
            // A trading window: its offers follow in a MerchantOffers packet.
            let title = plain(
                &p.title,
                &self
                    .remote
                    .as_ref()
                    .map(|r| r.lang.clone())
                    .unwrap_or_default(),
            );
            self.remote_open_merchant(p.container, title, menu_title(&p.title));
            return;
        }
        let entity = match menu.as_str() {
            "minecraft:crafting" => None,
            "minecraft:furnace" | "minecraft:blast_furnace" | "minecraft:smoker" => {
                let kind = match menu.as_str() {
                    "minecraft:blast_furnace" => FurnaceKind::BlastFurnace,
                    "minecraft:smoker" => FurnaceKind::Smoker,
                    _ => FurnaceKind::Furnace,
                };
                Some(BlockEntity::Furnace(Furnace {
                    kind,
                    items: [None, None, None],
                    lit_remaining: 0,
                    lit_total: 0,
                    cooking_timer: 0,
                    cooking_total: 0,
                    speed: 1.0,
                    recipes_used: std::collections::HashMap::new(),
                }))
            }
            // HopperMenu, and DispenserMenu (MenuType.GENERIC_3x3: dispensers
            // and droppers).
            "minecraft:hopper" => Some(BlockEntity::Chest(vec![None; 5])),
            // BrewingStandMenu: its times come in ContainerSetData.
            "minecraft:brewing_stand" => Some(BlockEntity::Brewing(Box::new(
                sunrise_client::brewing::BrewingStand::new(),
            ))),
            "minecraft:generic_3x3" => Some(BlockEntity::Chest(vec![None; 9])),
            // CrafterMenu: the disabled slots and triggered come in ContainerSetData.
            "minecraft:crafter_3x3" => Some(BlockEntity::Crafter(Default::default())),
            m if m.starts_with("minecraft:generic_9x") || m == "minecraft:shulker_box" => {
                let rows = m
                    .strip_prefix("minecraft:generic_9x")
                    .and_then(|r| r.parse::<usize>().ok())
                    .unwrap_or(3);
                Some(BlockEntity::Chest(vec![None; rows * 9]))
            }
            // A content mod's menu type: its container's slots, which its
            // mod's screen places.
            _ if mod_type.is_some() => Some(BlockEntity::Mod(Box::new(
                sunrise_client::containers::ModBlockEntity::new(
                    String::new(),
                    sunrise_client::containers::ModBlockEntityFields {
                        items: vec![None; mod_type.map_or(0, |t| t.slots)],
                        ..Default::default()
                    },
                ),
            ))),
            _ => {
                // Unsupported menu: tell the server we closed it.
                self.remote_reject_container(p.container);
                self.add_chat_line(
                    sunrise_client::text::Text::literal(format!(
                        "(Sunrise cannot show {menu} screens yet)"
                    )),
                    sunrise_client::chat::ChatSource::Client,
                );
                return;
            }
        };
        let title = menu_title(&p.title);
        self.remote_set_menu(Some((p.container, menu.clone())), Some(title));
        self.craft_size = if menu == "minecraft:crafting" { 3 } else { 2 };
        self.container = None;
        if let Some(entity) = entity {
            // The clicked block shows the menu when it holds that container;
            // an ender chest's or a double chest's slots are no one block's,
            // so they go under a key no block has.
            let holds = |k: &(i32, i32, i32)| {
                let name = &self.res.facts().get(self.world.block(k.0, k.1, k.2)).name;
                BlockEntity::for_block(name).is_some_and(|e| {
                    e.id() == entity.id() && e.slots().len() == entity.slots().len()
                })
            };
            let key = target.filter(holds).unwrap_or((i32::MIN, 0, 0));
            let backing_slots = entity.slots().len();
            self.world.block_entities.insert(key, entity);
            self.container = Some(key);
            if !self.open_native_screen(key, target, backing_slots, extra) {
                if mod_type.is_some() || key.0 == i32::MIN {
                    self.world.block_entities.remove(&key);
                }
                self.remote_reject_container(p.container);
                return;
            }
        }
        let native = self.mod_screen.0.is_some();
        self.set_screen(Screen::Inventory);
        if !native || self.native_screen_shown() {
            self.capture(false);
        }
    }
    pub(in crate::game) fn remote_reject_container(&mut self, container: i32) {
        self.remote_close_local();
        if self.client.screen == Screen::Inventory {
            self.set_screen(Screen::Playing);
            self.capture(true);
        }
        self.send(S::ContainerClose(play::ContainerClose { container }));
    }
    pub(in crate::game) fn remove_remote_menu_screen(&mut self) {
        // `Gui.setScreen` clears the focus of the screen it takes down: the
        // container's keyboard focus must not outlive it (a Tab bound to the
        // inventory moves the focus and closes the container in one press).
        self.pad_clear_focus();
        self.recipe_ui.0.reset_screen();
        if self.mod_screen.0.take().is_some() {
            self.clear_mod_book_placement_screen();
            self.screen_state.0.build.reopened();
        }
        self.mount_menu_reset();
        // The client's menu has no level access: what it holds is the server's.
        self.station = None;
        self.craft_size = 2;
        if let Some(key) = self.container.take() {
            // Server containers stay renderable but lose their copied slots.
            if let Some(entity) = self.world.block_entities.get_mut(&key) {
                for s in entity.slots_mut() {
                    *s = None;
                }
            }
        }
    }
    /// Single admission path for `MultiPlayerGameMode.handleCreativeModeItemDrop`:
    /// resolves mode/screen/feature checks, `client_packet_send`, and transport acceptance
    /// before charging `drop_spam_throttler`.
    pub(in crate::game) fn try_send_creative_drop_packet(&self, thrown: &ItemStack) -> bool {
        let Some(flags) = self.creative_drop_features() else {
            return false;
        };
        if thrown.is_empty() || !self.creative_screen.0.catalog().item_enabled(thrown, flags) {
            return false;
        }
        let Some(item) = self.to_network(&Some(thrown.clone())) else {
            return false;
        };
        self.send_play(S::SetCreativeModeSlot(play::SetCreativeModeSlot {
            slot: -1,
            item: Some(item),
        }))
    }
}
