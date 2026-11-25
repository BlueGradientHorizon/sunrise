#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::lan::guest::Guest;

impl Game<'_> {
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
        }
        guest.joined()
            && self
                .lan
                .as_ref()
                .is_some_and(|host| !host.players_told_left)
    }
    /// A network guest the hub removed leaves the world: the leave lines, then
    /// `player_logged_out` where `PlayerList.remove` starts (NeoForge's
    /// `PlayerLoggedOutEvent`), after their menu closed and before the leave statistic, the
    /// vehicle and any player save. They are listed again in join order while the mods hear it,
    /// as when the server stops (`native_players_leaving`). A listener's edits persist only if
    /// this server saves the player: a completed proxy hand-over or a crash writes no logout
    /// save. A guest who quit from the End credits counts as online (`Hub::departing`) without
    /// going back into a level. Their disconnect counts as begun, so a listener's own is refused.
    pub(in crate::game) fn native_guest_left(&mut self, mut guest: Box<Guest>) -> Box<Guest> {
        if !self.announce_guest_left(&guest) {
            return guest;
        }
        let (conn, uuid, name) = (guest.conn, guest.uuid, guest.name.clone());
        let Some(host) = self.lan.as_mut() else {
            return guest;
        };
        guest.disconnect_pending = true;
        // Right behind the last of the players listed ahead of them who are still here.
        let at = host
            .hub
            .guests
            .iter()
            .rposition(|g| guest.listed_behind.contains(&g.conn))
            .map_or(0, |i| i + 1);
        host.hub.guests.insert(at, *guest);
        host.hub.departing = Some(conn);
        self.native_player_logged_out(uuid, &name);
        let Some(host) = self.lan.as_mut() else {
            unreachable!("LanHost dropped inside a tick; closes are deferred")
        };
        host.hub.departing = None;
        let at = host
            .hub
            .guests
            .connection_index(conn)
            .expect("a leaving player stays listed while the mods hear it");
        Box::new(host.hub.guests.remove(at))
    }
}

#[cfg(test)]
pub(in crate::game) mod player_leave_tests {
    #[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
        Dim, Game, native_entity_refs::tests::run_fresh_cases,
        native_guest_turn_tests::with_guests, native_item_places::tests::guest,
    };
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
        Hooks, ModEntry, PlayerBedLeaveEvent, PlayerLoggedOutEvent, Registrar, ServerHost,
    };
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::lan::{NetEvent, playerdata};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::{Compound, Tag};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_mod_runtime::attachments::{
        AttachmentCodec, AttachmentError, AttachmentType, Holder,
    };
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::FLAG;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::SAVED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::ASLEEP;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::VALUE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::Heard;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::player_leave::player_leave_tests::Leave;

    /// Two guests, 12 then 13, a listener on both hooks, and guest 12's connection closing as
    /// the hub hears it (`NetEvent::Left`, `Hub::remove_guest`). `prepare` sets the guests up
    /// first; `check` gets the game and what the listener heard.
    pub(in crate::game) fn first_of_two_leaves(
        prepare: impl FnOnce(&mut Game<'_>),
        check: impl FnOnce(&mut Game<'_>, Vec<Heard>),
    ) {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![ModEntry::new("leave_test", Box::new(Leave(heard.clone())))].into();
        mod_platform::testing::with_mods(mods, || {
            with_guests(|game| {
                game.server.world_root = game.save.0.root.clone();
                game.options.dedicated = Some(Vec::new());
                game.native_level_opened(Dim::Overworld);
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
                prepare(game);
                let shared = &game.lan.as_ref().unwrap().hub.lan.shared;
                shared
                    .events
                    .send(NetEvent::Left(12, "Disconnected".into()))
                    .unwrap();
                game.tick_lan_packets();
                let told = std::mem::take(&mut *heard.lock().unwrap());
                check(game, told);
            });
        });
    }

    /// NeoForge's `PlayerLoggedOutEvent` at the head of `PlayerList.remove`: a listener finds
    /// the leaving player still listed in their place in join order, changes them, and the save
    /// that follows keeps it. One who quit from the End credits (out of every level) is listed,
    /// found and keeps the attachment too.
    #[test]
    #[ignore = "daily"]
    fn what_a_listener_changes_on_a_leaving_player_is_saved() {
        if run_fresh_cases(SAVED, FLAG, &["placed", "end_credits"]) {
            return;
        }
        let credits = std::env::var(FLAG).unwrap() == "end_credits";
        first_of_two_leaves(
            |game| {
                if credits {
                    // ServerPlayer.showEndCredits: out of the level, still joined.
                    game.lan.as_mut().unwrap().hub.remove_player_for_respawn(0);
                }
            },
            |game, told| {
                let [
                    Heard::LoggedOut {
                        listed,
                        found,
                        asleep: false,
                        attached,
                        healed,
                    },
                ] = &told[..]
                else {
                    panic!("one logout expected: {told:?}");
                };
                assert_eq!(*listed, [12, 13]);
                assert!(*found);
                assert_eq!(*attached, Ok(()));
                assert_eq!(*healed, !credits, "{told:?}");
                assert_eq!(game.guest_index(12), None);
                let mut saved = guest(12);
                saved.uuid = 12;
                assert!(playerdata::load(&mut saved, &game.server.world_root));
                assert_eq!(VALUE.get(&mut saved.attachments, Holder::Level), Ok(77));
                if !credits {
                    assert_eq!(saved.health, 7.0);
                }
            },
        );
    }
}
