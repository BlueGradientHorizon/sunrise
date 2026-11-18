use super::*;

impl Game<'_> {
    /// The container slots (of `size`) that the mod's screen for the block at
    /// `pos` does not show: the server's menu does not have them either, so a
    /// shift-click never moves an item where the player cannot see it.
    pub(in crate::game) fn native_hidden_slots(&self, pos: Pos, size: usize) -> Vec<usize> {
        let Some(menu) = self.mod_menu_at(pos) else {
            return Vec::new();
        };
        self.native_hidden_slots_for_menu(menu, Some(pos), size)
    }

    /// The container slots a registered or block-provided mod screen does not
    /// show. A blockless content menu has no block screen to consult, so its
    /// registered menu layout is the sole source of truth.
    pub(in crate::game) fn native_hidden_slots_for_menu(
        &self,
        menu: &str,
        pos: Option<Pos>,
        size: usize,
    ) -> Vec<usize> {
        let block = pos.map(|pos| {
            self.res
                .facts()
                .get(self.world.block(pos.0, pos.1, pos.2))
                .name
                .to_string()
        });
        let mut screen = mod_api::RecipeBookLayout::registered_screen(menu);
        if screen.is_none() {
            let Some(block) = block.as_deref() else {
                return Vec::new();
            };
            self.native_screen(block, &mut screen);
        }
        let Some(screen) = screen else {
            return Vec::new();
        };
        (0..size)
            .filter(|&i| screen.slots.get(i).is_none_or(Option::is_none))
            .collect()
    }
    /// A menu opened on this client with its slots under `key`, on block
    /// `target`: a book-enabled menu uses its startup-validated registered screen;
    /// other mod menus obtain their screen through `native_screen`.
    /// `extra` is what the server sent for this window first.
    pub(in crate::game) fn open_native_screen(
        &mut self,
        key: Pos,
        target: Option<Pos>,
        window: i32,
        menu: &str,
        extra: Option<sunrise_protocol::menu_extra::Message>,
    ) {
        self.clear_mod_book_placement_screen();
        self.mod_screen = None;
        let target_block = target.map(|t| {
            self.res
                .facts()
                .get(self.world.block(t.0, t.1, t.2))
                .name
                .to_string()
        });
        let mut screen = mod_api::RecipeBookLayout::registered_screen(menu);
        if screen.is_none() {
            let Some(block) = target_block.as_deref() else {
                return;
            };
            if block.starts_with("minecraft:") {
                return;
            }
            // Only when this is the menu the clicked block opens: `target` outlives the
            // use that set it, so another menu opened later (a command's, another
            // mod's) would otherwise get the last clicked machine's screen. Asked
            // here, as the window's world has no mod containers.
            let mut container = None;
            self.native_block_entity_container(block, &mut container);
            let opens = container.and_then(|c| mod_menu(c.menu.as_deref()?).map(|(m, _)| m));
            if opens != Some(menu) {
                return;
            }
            self.native_screen(block, &mut screen);
        }
        if let Some(mut screen) = screen {
            let block = target_block.unwrap_or_else(|| menu.to_owned());
            // Resolve sprite identifiers once at the client declaration boundary.
            for slot in screen.slots.iter_mut().flatten() {
                if let Some(icon) = &mut slot.no_item_icon {
                    let key = sunrise_client::hud::atlas_key_of_gui_sprite(icon);
                    if !self.gui.atlas.sprites.contains_key(&key) {
                        log::error!("screen for {block}: unknown empty-slot GUI sprite {icon:?}");
                        return;
                    }
                    *icon = key;
                }
            }
            self.mod_screen = Some(OpenNativeScreen {
                slot_moves: Default::default(),
                layout_revision: {
                    static NEXT: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(1);
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                },
                key,
                window,
                block,
                screen,
                data: Vec::new(),
                extra,
            });
            self.begin_mod_book_placement();
            self.ensure_mod_book_catalog();
        }
    }
}
