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
    /// The registered exclusions for a validated opener. Refuse the binding if
    /// any declared index is outside this opening's actual container backing.
    pub(in crate::game) fn native_hidden_slots(
        menu: &str,
        block: Option<&str>,
        backing_slots: usize,
    ) -> Option<&'static [u16]> {
        let hidden = mod_api::__private::menu_slot_exclusions(menu, block);
        if let Some(&last) = hidden.last()
            && usize::from(last) >= backing_slots
        {
            log::error!(
                "menu {menu} opener {block:?}: excluded slot {last} is outside {backing_slots} backing slots"
            );
            return None;
        }
        Some(hidden)
    }
    /// The mod block at `pos` when its container opens menu type `menu`, the
    /// `block` its screen hooks are told. Only when this is the menu that
    /// block opens: a position outlives the use that set it, so another menu
    /// opened later (a command's, another mod's) is not the last clicked
    /// machine's. Read the declaration directly, as a client's world has no
    /// server-side mod container table.
    pub(in crate::game) fn menu_opener_block(&self, pos: Pos, menu: &str) -> Option<&str> {
        let block = &self
            .res
            .facts()
            .get(self.world.block(pos.0, pos.1, pos.2))
            .name;
        if block.starts_with("minecraft:") {
            return None;
        }
        let container = mod_api::__private::parts_installation::block_container(block)?;
        let (opens, _) = mod_menu(container.menu.as_deref()?)?;
        (opens == menu).then_some(block.as_ref())
    }
    /// Stage the current remote opening's screen for the window. Its slots
    /// are stored under `key`; `extra` was already matched to this opening.
    /// False refuses the opening; an ordinary stock menu needs no mod screen.
    pub(in crate::game) fn open_native_screen(
        &mut self,
        key: Pos,
        target: Option<Pos>,
        backing_slots: usize,
        extra: Option<sunrise_protocol::menu_extra::Message>,
    ) -> bool {
        if self.is_logical_server() || self.lan_acting.is_some() {
            return false;
        }
        let Some(remote) = self.remote.as_ref() else {
            return false;
        };
        let Some((window, name)) = remote.menu.as_ref() else {
            return false;
        };
        let mod_type = sunrise_client::containers::mod_menu_type(name);
        let block = target.and_then(|pos| self.menu_opener_block(pos, name));
        if block.is_none() && mod_type.is_none() {
            return true;
        }
        let Some(hidden) = Self::native_hidden_slots(name, block, backing_slots) else {
            return false;
        };
        let of = block.unwrap_or(name);
        let Some(mut screen) = self.menu_screen(name, block) else {
            if !hidden.is_empty() {
                log::error!("screen for {of}: no layout for a menu with excluded slots");
            }
            return mod_type.is_none() && hidden.is_empty();
        };
        if screen.slots.len() > backing_slots {
            log::error!("screen for {of}: layout is longer than its {backing_slots} backing slots");
            return false;
        }
        // Every visible slot uses its backing index. An omitted entry is valid
        // only for a declared exclusion, including at the tail of the layout.
        let mut excluded = hidden.iter().copied().map(usize::from).peekable();
        for index in 0..backing_slots {
            let slot = screen.slots.get(index).and_then(Option::as_ref);
            let should_show = excluded.next_if_eq(&index).is_none();
            if slot.is_some() != should_show {
                log::error!(
                    "screen for {of}: slot {index} layout disagrees with its exclusion mask"
                );
                return false;
            }
            if let Some(icon) = slot.and_then(|slot| slot.no_item_icon.as_deref())
                && self.gui.atlas.gui_rect(icon).is_none()
            {
                log::error!("screen for {of}: unknown empty-slot GUI sprite {icon:?}");
                return false;
            }
        }
        screen.slots.resize_with(backing_slots, || None);
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
                block: block.map(str::to_owned),
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
        true
    }
}
