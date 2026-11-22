//! The leave notification shared by closed connections and kicked fake players.

use super::Game;
use sunrise_client::{lan::guest::Guest, text::Text};

impl Game<'_> {
    /// A network guest the hub removed leaves the world: the leave lines, then
    /// `player_logged_out` where `PlayerList.remove` starts (NeoForge's
    /// `PlayerLoggedOutEvent`), after their menu closed and before the leave statistic, the
    /// vehicle and the save. They are listed again while the mods hear it, as when the server
    /// stops (`native_players_leaving`), so what a listener changes on them is saved with them;
    /// their disconnect counts as begun, so a listener's own is refused.
    pub(in crate::game) fn native_guest_left(&mut self, mut guest: Box<Guest>) -> Box<Guest> {
        if !self.announce_guest_left(&guest) {
            return guest;
        }
        let (conn, uuid, name) = (guest.conn, guest.uuid, guest.name.clone());
        let Some(host) = self.lan.as_mut() else {
            return guest;
        };
        guest.disconnect_pending = true;
        host.hub.guests.push(*guest);
        self.native_player_logged_out(uuid, &name);
        let Some(host) = self.lan.as_mut() else {
            unreachable!("LanHost dropped inside a tick; closes are deferred")
        };
        let at = host
            .hub
            .guests
            .connection_index(conn)
            .expect("a leaving player stays listed while the mods hear it");
        Box::new(host.hub.guests.remove(at))
    }

    /// The leave lines of a guest the hub removed and broadcast the leave line of, and whether
    /// the mods hear that they logged out.
    pub(in crate::game) fn announce_guest_left(&mut self, guest: &Guest) -> bool {
        // A fake silently displaced by a real login was never announced as online.
        if guest.is_fake() && !guest.disconnect_pending {
            return false;
        }
        // A held player was recorded as joined at its login: it leaves too.
        if self.is_dedicated_server() && (guest.joined() || guest.held) {
            crate::game::dedicated::events::record(
                "leave",
                &guest.name,
                guest.uuid,
                serde_json::json!({ "reason": guest.left_reason }),
            );
            if guest.held && !guest.joined() {
                // Never announced, so no "left the game" either.
                crate::game::dedicated::log(
                    "INFO",
                    &sunrise_client::lan::arrivals::dropped_line(&guest.name, &guest.left_reason),
                );
            } else {
                crate::game::dedicated::log(
                    "INFO",
                    &format!("{} lost connection: {}", guest.name, guest.left_reason),
                );
            }
            // A player moved to another server without `announce` was not
            // announced as leaving, so the line is not logged either; a held
            // player was never announced at all.
            if guest.joined() && !guest.quiet_leave {
                crate::game::dedicated::log(
                    "INFO",
                    &format!("System chat: {} left the game", guest.name),
                );
            }
        } else if self.player.is_some() && guest.joined() && !guest.quiet_leave {
            // ServerGamePacketListenerImpl.removePlayerFromWorld.
            let who = self.player_text(&guest.name, guest.uuid);
            self.system_message(
                Text::translate("multiplayer.player.left", &[who])
                    .with_color(sunrise_client::text::YELLOW),
            );
        }
        guest.joined()
            && self
                .lan
                .as_ref()
                .is_some_and(|host| !host.players_told_left)
    }
}

#[cfg(test)]
mod player_leave_tests {
    use crate::game::{
        Dim, native_guest_turn_tests::with_guests, native_item_places::tests::guest,
    };
    use mod_api::{Hooks, ModEntry, PlayerLoggedOutEvent, Registrar, ServerHost};
    use std::sync::{Arc, Mutex};
    use sunrise_client::lan::{hub::HostEvent, playerdata};
    use sunrise_core::nbt::{Compound, Tag};
    use sunrise_mod_runtime::attachments::{
        AttachmentCodec, AttachmentError, AttachmentType, Holder,
    };

    const VALUE: AttachmentType<i32> = AttachmentType {
        namespace: "leave_test",
        name: "value",
        default: || 0,
        codec: Some(AttachmentCodec {
            encode: |n| Ok(Compound::from([("n".into(), Tag::Int(*n))])),
            decode: |data| match data.get(&"n".into()) {
                Some(Tag::Int(n)) => Ok(*n),
                _ => Err(AttachmentError::Decode),
            },
        }),
        copy_on_death: false,
        sync: false,
    };

    /// What each call found: whether the player was still listed, the attachment write's
    /// answer and whether the health was written.
    type Heard = Arc<Mutex<Vec<(bool, Result<(), AttachmentError>, bool)>>>;
    struct Leave(Heard);
    #[mod_api::__subscriptions]
    impl Hooks for Leave {
        fn register(&self, registrar: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
            registrar
                .register_attachment_type("leave_test:value", false)
                .unwrap();
        }
        fn player_logged_out(
            &self,
            host: &mut dyn ServerHost,
            event: &mut PlayerLoggedOutEvent<'_>,
        ) {
            let listed = host
                .players()
                .server_players()
                .iter()
                .any(|player| player.uuid() == event.player);
            let attached = VALUE.set(host, Holder::Player(event.player), 77);
            let healed = host
                .player(event.player)
                .and_then(|player| host.living(player.entity()))
                .is_some_and(|living| living.set_health(host, 7.0));
            self.0.lock().unwrap().push((listed, attached, healed));
        }
    }

    /// NeoForge's `PlayerLoggedOutEvent` at the head of `PlayerList.remove`: a listener changes
    /// the leaving player while they are still listed, and the save that follows keeps it.
    #[test]
    #[ignore = "daily"]
    fn what_a_listener_changes_on_a_leaving_player_is_saved() {
        let heard = Heard::default();
        let mods = vec![ModEntry::new("leave_test", Box::new(Leave(heard.clone())))].into();
        mod_platform::testing::with_mods(mods, || {
            with_guests(|game| {
                game.world_root = game.save.root.clone();
                game.options.dedicated = Some(Vec::new());
                game.native_level_opened(Dim::Overworld);
                // The hub takes a leaving guest out of its list before it reports the leave
                // (`Hub::remove_guest`).
                let mut host = game.lan.take().unwrap();
                let mut gone = host.hub.guests.remove(0);
                gone.tracker.clear();
                game.lan = game.lan_process_events(
                    host,
                    [HostEvent::Left(Box::new(gone))],
                    &[],
                    [3.0, 64.0, 5.0],
                );
                assert_eq!(*heard.lock().unwrap(), [(true, Ok(()), true)]);
                assert_eq!(game.guest_index(12), None);
                let mut saved = guest(12);
                saved.uuid = 12;
                assert!(playerdata::load(&mut saved, &game.world_root));
                assert_eq!(saved.health, 7.0);
                assert_eq!(VALUE.get(&mut saved.attachments, Holder::Level), Ok(77));
            });
        });
    }
}
