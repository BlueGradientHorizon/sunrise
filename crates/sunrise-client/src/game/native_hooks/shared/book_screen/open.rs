use super::*;

impl Game<'_> {
    /// The screen of menu type `menu`: its startup-validated registered book
    /// layout, else what the menu type's parts answer `native_screen`. `block`
    /// is the mod block whose menu it is, when the menu is block-backed.
    fn menu_screen(&self, menu: &str, block: Option<&str>) -> Option<mod_api::MenuScreenLayout> {
        let mut screen = mod_api::RecipeBookLayout::registered_screen(menu);
        if screen.is_some() {
            // A code-opened menu may provide its grid through native slot
            // backing instead of a block entity.
            return screen;
        }
        self.native_screen(menu, block, &mut screen);
        screen
    }
    /// The mod screen of a menu of type `menu` opened at `target`, with the
    /// mod block whose menu it is: a mod menu type's screen is its type's
    /// (`menu_screen`) whatever opened it, and a vanilla type gets a mod's
    /// screen only as the menu of the mod block at `target` that opens it.
    fn opened_screen(
        &self,
        menu: &str,
        target: Option<Pos>,
    ) -> Option<(Option<String>, mod_api::MenuScreenLayout)> {
        let block = target.and_then(|t| self.menu_opener_block(t, menu));
        if block.is_none() && sunrise_client::containers::mod_menu_type(menu).is_none() {
            return None;
        }
        let screen = self.menu_screen(menu, block.as_deref())?;
        Some((block, screen))
    }
    /// The container slots (of `size`) that the mod's screen for a menu of
    /// type `menu` opened at `target` does not show: the server's menu does
    /// not have them either, so a shift-click never moves an item where the
    /// player cannot see it.
    pub(in crate::game) fn native_hidden_slots(
        &self,
        menu: &str,
        target: Option<Pos>,
        size: usize,
    ) -> Vec<usize> {
        let Some((_, screen)) = self.opened_screen(menu, target) else {
            return Vec::new();
        };
        (0..size)
            .filter(|&i| screen.slots.get(i).is_none_or(Option::is_none))
            .collect()
    }
    /// The mod block at `pos` when its container opens menu type `menu`, the
    /// `block` its screen hooks are told. Only when this is the menu that
    /// block opens: a position outlives the use that set it, so another menu
    /// opened later (a command's, another mod's) is not the last clicked
    /// machine's. Asked of the block's container, as a client's world has no
    /// mod containers.
    fn menu_opener_block(&self, pos: Pos, menu: &str) -> Option<String> {
        let block = &self
            .res
            .facts()
            .get(self.world.block(pos.0, pos.1, pos.2))
            .name;
        if block.starts_with("minecraft:") {
            return None;
        }
        let mut container = None;
        self.native_block_entity_container(block, &mut container);
        let opens = container.and_then(|c| mod_menu(c.menu.as_deref()?).map(|(m, _)| m));
        (opens == Some(menu)).then(|| block.to_string())
    }
    /// Stage the current remote opening's screen for the window. Its slots
    /// are stored under `key`; `extra` was already matched to this opening.
    pub(in crate::game) fn open_native_screen(
        &mut self,
        key: Pos,
        target: Option<Pos>,
        backing_slots: usize,
        extra: Option<sunrise_protocol::menu_extra::Message>,
    ) {
        if self.is_logical_server() || self.lan_acting.is_some() {
            return;
        }
        let Some(remote) = self.remote.as_ref() else {
            return;
        };
        let Some((window, name)) = remote.menu.as_ref() else {
            return;
        };
        let Some((block, mut screen)) = self.opened_screen(name, target) else {
            return;
        };
        let mod_type = sunrise_client::containers::mod_menu_type(name);
        if mod_type.is_some() {
            screen.slots.truncate(backing_slots);
        }
        // Check sprite identifiers once at the client declaration boundary.
        for slot in screen.slots.iter().flatten() {
            if let Some(icon) = &slot.no_item_icon
                && self.gui.atlas.gui_rect(icon).is_none()
            {
                let of = block.as_deref().unwrap_or(name);
                log::error!("screen for {of}: unknown empty-slot GUI sprite {icon:?}");
                return;
            }
        }
        // The server can explicitly say there is no position. Only without
        // its extra data do we use the clicked block the slots are under.
        let (pos, extra) = extra.map_or_else(
            || {
                (
                    (key.0 != i32::MIN).then_some([key.0, key.1, key.2]),
                    Vec::new(),
                )
            },
            |extra| (extra.pos, extra.data),
        );
        self.mod_screen = Some(OpenNativeScreen {
            menu: OpenNativeMenu {
                key,
                window: *window,
                generation: remote.menu.generation,
                name: name.clone(),
                block,
                pos,
                mod_type,
                data: Vec::new(),
                extra,
            },
            slot_moves: Default::default(),
            layout_revision: {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            },
            screen,
            shown: false,
        });
    }
}
