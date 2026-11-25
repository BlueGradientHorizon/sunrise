#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, chat::PLAYER_NAME, screen_info};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::books::BLACK;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::books::BOOK_TEXT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::books::BookScreen;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::books::LINES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::books::MAX_PAGES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::books::PAGE_CHARS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::books::*;
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

impl Game<'_> {
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
                    (g.server.lan_acting_uuid, g.server.lan_acting_name.clone())
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
            if let (Some(player), Some(name)) = (
                self.server.lan_acting_uuid,
                self.server.lan_acting_name.clone(),
            ) {
                self.native_player_take_lectern_book(player, &name, [key.0, key.1, key.2], &book.item);
            }
        } else if menu.data[0] != before {
            self.set_lectern_powered(key, true);
            self.server.lectern_pulses.push((key, 2));
            self.server_level_event_at(1043, [key.0, key.1, key.2], 0);
            self.container_changed(key);
            if let (Some(player), Some(name)) = (
                self.server.lan_acting_uuid,
                self.server.lan_acting_name.clone(),
            ) {
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
        for (key, left) in &mut self.server.lectern_pulses {
            *left -= 1;
            if *left <= 0 {
                done.push(*key);
            }
        }
        self.server.lectern_pulses.retain(|(_, left)| *left > 0);
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
        if self.server.lan_acting.is_some() && written {
            let hand = i32::from(self.hand == Hand::Off);
            self.server
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
        if let (Some(player), Some(name)) = (
            self.server.lan_acting_uuid,
            self.server.lan_acting_name.clone(),
        ) {
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
            .server
            .lan_acting_name
            .clone()
            .unwrap_or_else(|| super::chat::PLAYER_NAME.to_owned());
        let Some(stack) = self
            .stack_in_mut(slot)
            .as_mut()
            .filter(|s| &*s.item == "minecraft:writable_book")
        else {
            return;
        };
        if let Some(title) = title {
            // `signBook` transmutes the carried stack, so a renamed book and
            // quill stays renamed and keeps its other components.
            let mut written = stack.transmute("minecraft:written_book", stack.count);
            let data = written.data_mut();
            data.writable_pages = Vec::new();
            data.writable_filtered = Vec::new();
            data.written = Some(WrittenBook {
                title,
                author,
                generation: 0,
                pages,
                page_tags: Vec::new(),
                // `signBook`'s `new WrittenBookContent(..., true)`: the pages
                // are literals, so the content is already resolved.
                resolved: true,
            });
            *stack = written;
        } else {
            let data = stack.data_mut();
            // `updateBookContents`: new `Filterable` pages, unfiltered while no text filter runs.
            data.writable_filtered = Vec::new();
            data.writable_pages = if pages.iter().all(String::is_empty) {
                Vec::new()
            } else {
                pages
            };
            stack.tidy();
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod lectern_books_tag_tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::super::game_tests::tests::server_game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::client_side::books::lectern_books_tag_tests::*;
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
        game.server.world_root = dir;
        let stack = game.reload_datapacks(false, &[]).unwrap();
        let root = game.options.resources.clone();
        game.world_tables.0 = game
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
