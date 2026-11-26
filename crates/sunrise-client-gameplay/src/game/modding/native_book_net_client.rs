//! Client custom-book synchronization, admitted by its own configuration capability.
use crate::game::ClientGame;
use std::collections::BTreeSet;
use sunrise_protocol::packets::{configuration::CustomPayload, play::Serverbound as S};
use sunrise_sim::recipe_book::{
    settings_codec,
    settings_sync::{CHANNEL, Message},
};

fn registered() -> BTreeSet<String> {
    mod_api::RecipeBookTypeKey::registered()
        .into_iter()
        .map(|key| key.as_str().to_owned())
        .collect()
}

impl ClientGame<'_> {
    pub(in crate::game::client_side) fn tick_book_snapshot_retry(&mut self) {
        self.check_mod_book_placement_timeout();
        if self
            .common.native_net
            .book_retry_at
            .is_some_and(|at| std::time::Instant::now() >= at)
        {
            let message = if self.common.native_net.book_view.epoch.is_none() {
                Message::Offer(registered().into_iter().collect())
            } else {
                Message::Resnapshot
            };
            self.common.native_net.book_retry_at = None;
            if let Some(data) = settings_codec::encode(&message) {
                self.send(S::CustomPayload(CustomPayload {
                    channel: CHANNEL.into(),
                    data,
                }));
            }
        }
    }

    pub(in crate::game) fn reset_book_view(&mut self) {
        self.common.native_net.placement.client = Default::default();
        self.common.native_net.placement.deadline = None;
        self.clear_mod_book_placement_screen();
        self.common.native_net.book_view = Default::default();
        self.common.native_net.book_request = 0;
        self.common.native_net.book_ready = false;
        self.common.native_net.book_retry_at = None;
        self.build_screen();
    }

    /// Authoritative view for a custom menu panel; None until negotiation and snapshot finish.
    pub(in crate::game::client_side) fn custom_book_settings(
        &self,
        book: &mod_api::RecipeBookTypeKey,
    ) -> Option<mod_api::RecipeBookSettings> {
        let view = &self.common.native_net.book_view;
        if !view.ready || !view.contains(book.as_str()) {
            return None;
        }
        let flags = view.books.get(book.as_str());
        Some(mod_api::RecipeBookSettings {
            open: flags.open,
            filtering: flags.filtering,
        })
    }

    /// Queue intent without changing the authoritative or saved pair.
    pub fn request_custom_book_settings(
        &mut self,
        book: &mod_api::RecipeBookTypeKey,
        flags: mod_api::RecipeBookSettings,
    ) -> bool {
        if self.custom_book_settings(book).is_none() {
            return false;
        }
        let Some(number) = self.common.native_net.book_request.checked_add(1) else {
            return false;
        };
        let Some(epoch) = self.common.native_net.book_view.epoch else {
            return false;
        };
        let message = Message::Request {
            epoch,
            number,
            key: book.as_str().into(),
            flags: sunrise_sim::recipe_book::BookFlags {
                open: flags.open,
                filtering: flags.filtering,
            },
        };
        let Some(data) = settings_codec::encode(&message) else {
            return false;
        };
        self.common.native_net.book_request = number;
        self.send(S::CustomPayload(CustomPayload {
            channel: CHANNEL.into(),
            data,
        }));
        true
    }

    pub(in crate::game::client_side) fn begin_book_negotiation(&mut self) {
        self.reset_book_view();
        if self
            .client
            .remote
            .as_ref()
            .is_some_and(|r| r.custom_books && r.joined)
            && let Some(data) = settings_codec::encode(&Message::Hello)
        {
            self.send(S::CustomPayload(CustomPayload {
                channel: CHANNEL.into(),
                data,
            }));
        }
    }

    pub(in crate::game) fn remote_book_settings(&mut self, data: &[u8]) {
        if !self
            .client
            .remote
            .as_ref()
            .is_some_and(|remote| remote.custom_books && remote.joined)
        {
            return;
        }
        let Some(message) = settings_codec::decode(data) else {
            return;
        };
        if let Message::Placement(frame) = message {
            if self.common.native_net.book_ready && self.common.native_net.book_view.ready {
                self.remote_mod_book_placement(frame);
            }
            return;
        }
        if message == Message::Busy && self.common.native_net.book_ready {
            self.clear_mod_book_placement_screen();
            self.common.native_net.book_view.ready = false;
            self.common.native_net.book_retry_at.get_or_insert_with(|| {
                std::time::Instant::now() + std::time::Duration::from_secs(1)
            });
            self.build_screen();
            return;
        }
        if message == Message::Ready {
            if self.common.native_net.book_ready {
                return;
            }
            self.common.native_net.book_ready = true;
            if let Some(data) =
                settings_codec::encode(&Message::Offer(registered().into_iter().collect()))
            {
                self.send(S::CustomPayload(CustomPayload {
                    channel: CHANNEL.into(),
                    data,
                }));
            }
            return;
        }
        if !self.common.native_net.book_ready
            || !matches!(message, Message::Snapshot { .. } | Message::Update { .. })
        {
            return;
        }
        let previous_epoch = self.common.native_net.book_view.epoch;
        if !self.common.native_net.book_view.receive(message, &registered())
            && !self.common.native_net.book_view.ready
            && let Some(data) = settings_codec::encode(&Message::Resnapshot)
        {
            self.send(S::CustomPayload(CustomPayload {
                channel: CHANNEL.into(),
                data,
            }));
        }
        if previous_epoch != self.common.native_net.book_view.epoch {
            self.common.native_net.placement.client = Default::default();
            self.common.native_net.placement.deadline = None;
            self.clear_mod_book_placement_screen();
        }
        // Both accepted settings and readiness loss change the visible panel.
        self.build_screen();
        if self.common.native_net.book_view.ready {
            self.common.native_net.book_retry_at = None;
            self.begin_mod_book_placement();
            self.ensure_mod_book_catalog();
        }
    }
}
