#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, chat::PLAYER_NAME, screen_info};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    containers::{BlockEntity, Lectern},
    first_person::Hand,
    hud::{HudBatch, rgba},
    item_data::{StackData, WrittenBook},
    items::ItemStack,
    menu::{Kind, Menu},
    mobs::Who,
    text::Text,
    ui,
    widgets::{self, Narratable, ScreenInfo},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::lectern::lectern_signal;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::{
    self, Clientbound as C, Serverbound as S,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::level::flags as level_flags;
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::keyboard::KeyCode;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `ServerGamePacketListenerImpl.handleEditBook`: a guest's book in a
    /// hotbar slot or its off hand (`updateBookContents`, `signBook`).
    pub(in crate::game::server_side) fn lan_edit_book(&mut self, conn: u32, p: play::EditBook) {
        if !((0..9).contains(&p.slot) || p.slot == super::shield::OFFHAND_SLOT as i32) {
            return;
        }
        let menu_slot = if p.slot == super::shield::OFFHAND_SLOT as i32 {
            45
        } else {
            36 + p.slot as usize
        };
        let cancelled = self.as_guest(conn, |g| {
            let mut cancel = false;
            let writable = g
                .stack_in(p.slot as usize)
                .is_some_and(|stack| &*stack.item == "minecraft:writable_book");
            if writable
                && let (Some(player), Some(name)) =
                    (g.side.lan_acting_uuid, g.side.lan_acting_name.clone())
            {
                g.native_player_edit_book(
                    player,
                    &name,
                    p.pages.len() as i32,
                    p.title.is_some(),
                    &mut cancel,
                );
            }
            if cancel {
                return true;
            }
            g.write_book(p.slot as usize, p.pages, p.title);
            false
        });
        // A cancelled edit leaves the client's own copy of the book: forget
        // what was sent so the next broadcast resends the server's stack.
        if cancelled == Some(true)
            && let Some(host) = self.lan.as_mut()
        {
            let _ = host
                .hub
                .guests
                .update_connection(conn, |g| g.menu_sent[menu_slot] = None);
        }
    }
    /// The lectern menu's book and page back in the block entity: a new page
    /// is `LecternBlock.signalPageChange` (the 2-tick pulse, level event
    /// 1043), a book gone `onBookItemRemove` (`resetBookState`).
    pub(in crate::game) fn lectern_menu_done(&mut self, key: (i32, i32, i32), menu: &Menu) {
        let Some(BlockEntity::Lectern(l)) = self.world.block_entities.get_mut(&key) else {
            return;
        };
        let (taken_book, before) = (l.book.clone(), l.page);
        l.book = menu.slot(0).cloned();
        l.page = menu.data[0];
        if let Some(book) = taken_book.filter(|_| l.book.is_none()) {
            self.set_lectern_powered(key, false);
            self.set_lectern_has_book(key, false, None);
            self.container_changed(key);
            if let (Some(player), Some(name)) =
                (self.side.lan_acting_uuid, self.side.lan_acting_name.clone())
            {
                self.native_player_take_lectern_book(
                    player,
                    &name,
                    [key.0, key.1, key.2],
                    &book.item,
                );
            }
        } else if menu.data[0] != before {
            self.set_lectern_powered(key, true);
            self.side.lectern_pulses.push((key, 2));
            self.server_level_event_at(1043, [key.0, key.1, key.2], 0);
            self.container_changed(key);
            if let (Some(player), Some(name)) =
                (self.side.lan_acting_uuid, self.side.lan_acting_name.clone())
            {
                self.native_player_lectern_page_change(
                    player,
                    &name,
                    [key.0, key.1, key.2],
                    menu.data[0],
                );
            }
        }
    }
    /// The 2-tick pulses of turned lecterns.
    pub(in crate::game) fn tick_lecterns(&mut self) {
        let mut done = Vec::new();
        for (key, left) in &mut self.side.lectern_pulses {
            *left -= 1;
            if *left <= 0 {
                done.push(*key);
            }
        }
        self.side.lectern_pulses.retain(|(_, left)| *left > 0);
        for key in done {
            self.set_lectern_powered(key, false);
        }
    }
    /// `WritableBookItem.use` / `WrittenBookItem.use` (`Player.openItemGui`).
    /// A server's player writes in its own client's editor
    /// (`LocalPlayer.openItemGui`), and reads on its own client once told to
    /// (`ServerPlayer.openItemGui`'s ClientboundOpenBookPacket).
    pub(in crate::game::server_side) fn use_book(&mut self, stack: &ItemStack) -> bool {
        assert!(self.is_logical_server(), "use_book called on Client role");
        let written = match &*stack.item {
            "minecraft:writable_book" => false,
            "minecraft:written_book" => true,
            _ => return false,
        };
        // `ServerPlayer.openItemGui` resolves a written book for the player
        // opening it before the screen reads it.
        let held;
        let stack = if written && self.resolve_held_book() {
            held = self.held().cloned();
            held.as_ref().unwrap_or(stack)
        } else {
            stack
        };
        if self.side.lan_acting.is_some() && written {
            let hand = i32::from(self.hand == Hand::Off);
            self.side
                .guest_packets
                .push(C::OpenBook(play::OpenBook { hand }));
        }
        // WritableBookItem.use / WrittenBookItem.use count every open.
        self.award_stat("used", &stack.item, 1);
        self.swing_arm();
        true
    }
    /// `LecternBlock.useItemOn`: a book goes onto an empty lectern.
    pub(in crate::game::server_side) fn use_lectern(
        &mut self,
        pos: [i32; 3],
        held: Option<&ItemStack>,
    ) -> bool {
        let key = (pos[0], pos[1], pos[2]);
        if &*self
            .res
            .facts()
            .get(self.world.block(pos[0], pos[1], pos[2]))
            .name
            != "minecraft:lectern"
        {
            return false;
        }
        let has_book = matches!(self.world.block_entities.get(&key), Some(BlockEntity::Lectern(l)) if l.book.is_some());
        // `LecternBlock.useItemOn`: `ItemTags.LECTERN_BOOKS`, which a data
        // pack can add its own books to, not only the two vanilla items.
        let Some(book) = held.filter(|h| {
            world_items!(self)
                .item_tags
                .get("minecraft:lectern_books")
                .is_some_and(|books| books.contains(&*h.item))
        }) else {
            return false;
        };
        if has_book {
            return false;
        }
        let mut one = book.clone();
        one.count = 1;
        self.world.block_entities.insert(
            key,
            BlockEntity::Lectern(Lectern {
                book: Some(one),
                page: 0,
            }),
        );
        self.set_lectern_has_book(key, true, self.is_logical_server().then_some(Who::Player));
        if !self.creative {
            self.consume_held();
        }
        let at = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        self.server_play_sound("minecraft:item.book.put", at, 1.0, 1.0);
        self.container_changed(key);
        if let (Some(player), Some(name)) =
            (self.side.lan_acting_uuid, self.side.lan_acting_name.clone())
        {
            self.native_player_insert_lectern_book(player, &name, pos, &book.item);
        }
        true
    }
    /// The book and quill at `slot` takes the pages, or with a title becomes
    /// the written book its writer signed.
    pub(in crate::game) fn write_book(
        &mut self,
        slot: usize,
        pages: Vec<String>,
        title: Option<String>,
    ) {
        let author = self
            .side
            .lan_acting_name
            .clone()
            .unwrap_or_else(|| super::chat::PLAYER_NAME.to_owned());
        self.write_book_with_author(slot, pages, title, author);
    }

    /// `ServerPlayer.openItemGui` -> `WrittenBookContent.resolveForItem`: resolve selectors/scores.
    pub(in crate::game::server_side) fn resolve_held_book(&mut self) -> bool {
        let Some(mut book) = self
            .held()
            .and_then(|s| s.data())
            .and_then(|d| d.written.clone())
        else {
            return false;
        };
        if book.resolved {
            return false;
        }
        let string = |v: &Tag| match v {
            Tag::String(s) => Some(s.to_string_lossy()),
            _ => None,
        };
        let changed = book.resolve(|tag| {
            let Tag::Compound(c) = tag else { return None };
            if let Some(Tag::String(selector)) = c.get(&"selector".into()) {
                let selector = selector.to_string_lossy();
                return Some(self.resolve_selector_text(&selector));
            }
            let score = c.get(&"score".into())?;
            let name = score.get("name").and_then(string).unwrap_or_default();
            let objective = score.get("objective").and_then(string).unwrap_or_default();
            let name = if name == "*" {
                PLAYER_NAME
            } else {
                name.as_str()
            };
            Some(
                self.score
                    .board
                    .score(name, &objective)
                    .map_or_else(String::new, |s| s.value.to_string()),
            )
        });
        if let Some(stack) = self.held_mut() {
            stack.data_mut().written = Some(book);
        }
        changed
    }

    pub(in crate::game::server_side) fn set_lectern_powered(
        &mut self,
        key: (i32, i32, i32),
        powered: bool,
    ) {
        self.set_lectern_property(key, "powered", powered);
        let p = BlockPos::new(key.0, key.1, key.2);
        let state = self.world.block(key.0, key.1, key.2);
        self.with_level(|b, level, ticks| {
            b.lectern_update_below(level, ticks, p, state);
        });
    }

    /// `LecternBlock.resetBookState`: the property with the block change event.
    pub(in crate::game) fn set_lectern_has_book(
        &mut self,
        key: (i32, i32, i32),
        has_book: bool,
        who: Option<Who>,
    ) {
        self.set_lectern_property(key, "has_book", has_book);
        let after = self.world.block(key.0, key.1, key.2);
        self.post_block_game_event(
            "minecraft:block_change",
            [key.0, key.1, key.2],
            who,
            Some(after),
        );
    }

    pub(in crate::game) fn set_lectern_property(
        &mut self,
        key: (i32, i32, i32),
        name: &str,
        value: bool,
    ) {
        let state = self.world.block(key.0, key.1, key.2);
        let registry = self.pack.assets.registry;
        let Some(mut s) = registry.state(state).cloned() else {
            return;
        };
        if !s.properties.contains_key(name) {
            return;
        }
        s.properties.insert(name.into(), value.to_string());
        if let Some(new) = registry.id(&s) {
            let p = BlockPos::new(key.0, key.1, key.2);
            self.with_level(|b, level, ticks| {
                b.set_block(level, ticks, p, new, level_flags::UPDATE_ALL)
            });
        }
    }

    /// `LecternMenu` over the lectern at `key`: its book and page.
    pub(in crate::game) fn lectern_menu(&self, key: (i32, i32, i32)) -> Option<Menu> {
        let Some(BlockEntity::Lectern(l)) = self.world.block_entities.get(&key) else {
            return None;
        };
        let mut menu = Menu::new(Kind::Lectern);
        menu.set_registry_id(Some("minecraft:lectern"));
        menu.set_slot(&self.menu_env(), 0, l.book.clone());
        menu.data[0] = l.page;
        Some(menu)
    }
}

#[cfg(test)]
pub(in crate::game) mod lectern_book_removal_tests {
    use super::super::game_tests::tests::server_game;
    use sunrise_client::{
        containers::{BlockEntity, Lectern},
        items::ItemStack,
    };

    /// `LecternBlock.resetBookState`: removing a book clears `has_book` and posts `block_change`.
    #[test]
    #[ignore = "daily"]
    fn a_lectern_book_removal_posts_block_change() {
        let mut game_resources = None;
        let mut game = server_game(&mut game_resources);
        let key = (2, 4, 3);
        let state = game
            .pack
            .assets
            .registry
            .default_state("minecraft:lectern")
            .unwrap();
        game.world.set(key.0, key.1, key.2, state).unwrap();
        game.world.block_entities.insert(
            key,
            BlockEntity::Lectern(Lectern {
                book: Some(ItemStack::new("minecraft:written_book", 1)),
                page: 0,
            }),
        );
        game.set_lectern_property(key, "has_book", true);
        crate::game::sculk::TEST_POSTED_EVENTS.with(|e| *e.borrow_mut() = Some(Vec::new()));

        game.set_lectern_has_book(key, false, None);

        let has_book = game
            .pack
            .assets
            .registry
            .state(game.world.block(key.0, key.1, key.2))
            .and_then(|s| s.properties.get("has_book").cloned());
        assert_eq!(
            has_book.as_deref(),
            Some("false"),
            "the lectern has no book"
        );
        crate::game::sculk::TEST_POSTED_EVENTS.with(|e| {
            assert!(
                e.borrow().as_ref().unwrap().contains(&(
                    "minecraft:block_change",
                    [2.5, 4.5, 3.5],
                    None
                )),
                "{:?}",
                e.borrow()
            );
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod lectern_books_tag_tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::super::game_tests::tests::server_game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::{
        containers::BlockEntity, items::ItemStack,
    };

    /// `LecternBlock.useItemOn` checks `ItemTags.LECTERN_BOOKS`; a world's
    /// data pack adds its own book to that tag, and the lectern takes it.
    #[test]
    #[ignore = "daily"]
    fn a_packs_lectern_books_entry_goes_onto_the_lectern() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        let dir =
            std::env::temp_dir().join(format!("sunrise-lectern-books-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let tags = dir.join("datapacks/books/data/minecraft/tags/item");
        std::fs::create_dir_all(&tags).unwrap();
        std::fs::write(
            dir.join("datapacks/books/pack.mcmeta"),
            r#"{"pack":{"description":"fixture"}}"#,
        )
        .unwrap();
        std::fs::write(
            tags.join("lectern_books.json"),
            r#"{"values":["minecraft:written_book","minecraft:writable_book","minecraft:book"]}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("datapacks.json"),
            r#"{"enabled":["file/books"],"disabled":[]}"#,
        )
        .unwrap();
        game.side.world_root = dir;
        let stack = game.reload_datapacks(false, &[]).unwrap();
        let root = game.options.resources.clone();
        game.side.world_tables.0 = game
            .stage_world_tables(&stack, false, &root)
            .unwrap()
            .tables;

        let state = game
            .pack
            .assets
            .registry
            .default_state("minecraft:lectern")
            .unwrap();
        let key = (8, 64, 8);
        game.world.set(key.0, key.1, key.2, state).unwrap();

        // A book the pack put in the tag is placed; one it did not is not.
        let stone = ItemStack::new("minecraft:stone", 1);
        assert!(!game.use_lectern([8, 64, 8], Some(&stone)));
        let book = ItemStack::new("minecraft:book", 1);
        assert!(
            game.use_lectern([8, 64, 8], Some(&book)),
            "the pack's `#minecraft:lectern_books` entry was refused"
        );
        let Some(BlockEntity::Lectern(lectern)) = game.world.block_entities.get(&key) else {
            panic!("no lectern block entity");
        };
        assert_eq!(
            lectern.book.as_ref().map(|b| &*b.item),
            Some("minecraft:book")
        );
    }
}
