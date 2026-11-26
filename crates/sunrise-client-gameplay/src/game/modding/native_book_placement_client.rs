//! Client placement transport state for mod recipe books.
use crate::game::ClientGame;
use sunrise_protocol::packets::{configuration::CustomPayload, play::Serverbound as S};
use sunrise_sim::recipe_book::{
    placement_sync as sync, settings_codec,
    settings_sync::{CHANNEL, Message},
};

impl ClientGame<'_> {
    pub(in crate::game::client_side) fn send_mod_book_placement(&mut self, frame: sync::Frame) {
        if let Some(data) = settings_codec::encode(&Message::Placement(frame)) {
            self.common.native_net.placement.deadline =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(30));
            self.send(S::CustomPayload(CustomPayload {
                channel: CHANNEL.into(),
                data,
            }));
        }
    }

    pub fn begin_mod_book_placement(&mut self) {
        let Some(epoch) = self.common.native_net.book_view.epoch else {
            return;
        };
        if self.common.native_net.placement.client.epoch() == Some(epoch) {
            return;
        }
        let supported = mod_api::RecipeBookLayout::registered_menus()
            .iter()
            .any(|menu| {
                mod_api::RecipeBookLayout::registered(menu).is_some_and(|(book, _, _)| {
                    self.common.native_net.book_view.contains(book.as_str())
                })
            });
        if !supported {
            return;
        }
        let message = self.common.native_net.placement.client.hello(epoch);
        self.send_mod_book_placement(message);
    }

    pub fn check_mod_book_placement_timeout(&mut self) {
        if self
            .common
            .native_net
            .placement
            .client
            .window()
            .is_some_and(|window| !self.native_book_placement_window_matches(window))
        {
            self.clear_mod_book_placement_screen();
        }
        if self
            .common
            .native_net
            .placement
            .deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            self.common.native_net.placement.deadline = None;
            self.clear_mod_book_placement_screen();
            log::error!(
                "mod recipe-book placement request timed out; close and reopen the menu to retry"
            );
        }
    }

    pub(in crate::game::client_side) fn clear_mod_book_placement_screen(&mut self) {
        if self.common.native_net.placement.client.window().is_some() {
            self.common.native_net.placement.deadline = None;
        }
        self.common.native_net.placement.client.close();
        self.common.native_net.placement.view = None;
        self.reset_mod_book_placement_ui();
    }

    pub fn ensure_mod_book_catalog(&mut self) {
        let Some(window) = self.native_book_placement_window() else {
            return;
        };
        if self.common.native_net.placement.client.window() == Some(&window) {
            return;
        }
        if let Some(frame) = self.common.native_net.placement.client.open(window) {
            self.common.native_net.placement.view = None;
            self.reset_mod_book_placement_ui();
            self.send_mod_book_placement(frame);
        }
    }

    pub(in crate::game) fn client_invalidate_mod_book_placement_catalogues(&mut self) {
        self.clear_mod_book_placement_screen();
        self.ensure_mod_book_catalog();
    }
}
