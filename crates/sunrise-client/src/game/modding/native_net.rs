//! Private custom-book connection state and client network services.
use super::Game;
#[path = "native_net/packet_hold.rs"]
mod packet_hold;
use mod_api::{SendPacketError, ServerboundPlayPacket};
use std::collections::HashMap;
#[derive(Default)]
pub(super) struct NativeNet {
    pub(super) holds: std::cell::RefCell<packet_hold::Holds>,
    pub(super) placement: super::native_book_placement::State,
    pub(super) book_connections:
        HashMap<u32, sunrise_client::recipe_book::settings_connection::Connection>,
    pub(super) book_view: sunrise_client::recipe_book::settings_sync::ClientView,
    pub(super) book_request: u64,
    pub(super) book_ready: bool,
    pub(super) book_retry_at: Option<std::time::Instant>,
    /// A mod's `disconnect` (its reason, if any), carried out at the end of the frame.
    pub(super) leave: Option<super::native_disconnect::LeaveRequest>,
}

impl Game<'_> {
    /// Send directly through the play listener, not `ChatScreen` input. The
    /// server still applies signature validation, permissions and chat policy.
    pub(super) fn send_chat(&mut self, message: &str) -> Result<(), String> {
        let Some(remote) = self.remote.as_ref() else {
            return Err("not connected to a server".into());
        };
        // A server that reconfigures the player has them out of the play state.
        if !remote.joined || remote.reconfiguring.is_some() {
            return Err("the player has not joined yet".into());
        }
        if let Some(command) = message.strip_prefix('/') {
            self.send_command(command);
        } else {
            self.send_signed_chat(message);
        }
        Ok(())
    }

    /// Opens the chat screen with `text` typed in and the cursor at its end,
    /// as T and / open it (`ChatScreen` with its initial text): a command
    /// macro can fill in `/tp ` for the player to finish. Nothing is sent; the
    /// player edits and sends it, or presses Escape. The box keeps its limit
    /// (256 UTF-16 units) and its character filter. The ordinary screen
    /// opening and closing hooks govern the transition. API @API:open_chat@.
    pub(super) fn open_chat(&mut self, text: &str) -> Result<(), String> {
        if self.begin_chat_screen(text, false) {
            Ok(())
        } else {
            Err("no player in a world".into())
        }
    }

    /// Validate and reserve a vanilla packet for root-commit publication on this
    /// exact physical protocol incarnation. Registered and engine-reserved
    /// channels require their payload/service API; unrelated plugin payloads
    /// retain vanilla's discard-codec limit. No local effects are applied. API 409.
    pub(super) fn send_packet(
        &mut self,
        packet: ServerboundPlayPacket,
    ) -> Result<(), SendPacketError> {
        let remote = self.remote.as_ref().ok_or(SendPacketError::NotConnected)?;
        if !remote.joined || remote.reconfiguring.is_some() {
            return Err(SendPacketError::NotInPlay);
        }
        let body = packet.clone();
        let prepared = remote.session.prepare_mod_packet(packet)?;
        self.after_commit(super::transactions::After::Packet(prepared, body));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::native_entity_refs::tests::{run_fresh_cases, with_client_game};
    use super::{SendPacketError, ServerboundPlayPacket};
    use mod_api::Identifier;
    use sunrise_core::vanilla_bytes::Writer;
    use sunrise_protocol::connection::Pipe;

    #[test]
    #[ignore = "daily"]
    fn open_chat_replaces_screens_but_a_veto_keeps_the_old_chat() {
        use crate::game::Screen;
        use mod_api::{ClientHost, Hooks, ScreenClosingEvent, ScreenOpening, ScreenOpeningEvent};
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        };
        use sunrise_client::motion::PlayerMotion;
        use sunrise_core::geometry::Vec3;

        if run_fresh_cases(
            "game::native_net::tests::open_chat_replaces_screens_but_a_veto_keeps_the_old_chat",
            "SUNRISE_OPEN_CHAT_CASE",
            &["transition"],
        ) {
            return;
        }
        #[derive(Default)]
        struct Probe {
            cancel: AtomicBool,
            events: Mutex<Vec<bool>>,
        }
        struct Listener(Arc<Probe>);
        #[mod_api::__subscriptions]
        impl Hooks for Listener {
            fn screen_opening(
                &self,
                _: &dyn ClientHost,
                _: &mut ScreenOpeningEvent<'_>,
            ) -> Option<ScreenOpening> {
                self.0.events.lock().unwrap().push(true);
                self.0
                    .cancel
                    .load(Ordering::Relaxed)
                    .then_some(ScreenOpening::Cancel)
            }

            fn screen_closing(&self, _: &dyn ClientHost, _: &mut ScreenClosingEvent<'_>) {
                self.0.events.lock().unwrap().push(false);
            }
        }
        let probe = Arc::new(Probe::default());
        let mods = vec![mod_api::ModEntry::new(
            "chat_screens",
            Box::new(Listener(probe.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_client_game(|game| {
                game.player =
                    Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
                game.screen = Screen::Paused;
                assert_eq!(game.open_chat("first draft"), Ok(()));
                assert!(game.screen == Screen::Chat);
                assert_eq!(game.ui.chat_screen.as_ref().unwrap().value(), "first draft");
                probe.cancel.store(true, Ordering::Relaxed);
                assert_eq!(game.open_chat("vetoed"), Ok(()));
                assert_eq!(game.ui.chat_screen.as_ref().unwrap().value(), "first draft");
                probe.cancel.store(false, Ordering::Relaxed);
                assert_eq!(game.open_chat("second draft"), Ok(()));
                assert_eq!(
                    game.ui.chat_screen.as_ref().unwrap().value(),
                    "second draft"
                );
                assert_eq!(game.ui.chat_draft.as_ref().unwrap().text, "first draft");
                assert_eq!(
                    *probe.events.lock().unwrap(),
                    [true, false, true, true, false]
                );
            });
        });
    }

    /// A joined flag cannot manufacture a physical transport. Packet codec,
    /// reservation, and framing cases live in net::mod_packet_tests.
    #[test]
    #[ignore = "daily"]
    fn mod_packets_require_joined_player_and_physical_transport() {
        if run_fresh_cases(
            "game::native_net::tests::mod_packets_require_joined_player_and_physical_transport",
            "SUNRISE_PACKET_GUARD_CHILD",
            &["guards"],
        ) {
            return;
        }
        with_client_game(|game| {
            let packet = || {
                let mut body = Writer::new();
                body.string("voicechat:request_secret");
                body.bytes(&[1, 2, 3, 4]);
                ServerboundPlayPacket::new(
                    Identifier::parse("minecraft:custom_payload").unwrap(),
                    body.data,
                )
            };
            assert_eq!(
                game.send_packet(packet()),
                Err(SendPacketError::NotConnected)
            );
            let (client, _server) = Pipe::pair();
            game.connect_local(client, "PacketTest").unwrap();
            assert_eq!(game.send_packet(packet()), Err(SendPacketError::NotInPlay));
            // Deliberately inconsistent actor state: no handshake has installed
            // a writer. Acceptance must still require the physical provider.
            game.remote.as_mut().unwrap().joined = true;
            assert_eq!(
                game.send_packet(packet()),
                Err(SendPacketError::NotConnected)
            );
        });
    }
}
