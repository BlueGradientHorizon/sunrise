use crate::game::ServerGame;
use sunrise_sim::lan::guest::Guest;

impl ServerGame<'_> {
    /// The leave lines of a guest the hub removed and broadcast the leave line of, and whether
    /// the mods hear that they logged out.
    pub(in crate::game) fn announce_guest_left(&mut self, guest: &Guest) -> bool {
        // A fake silently displaced by a real login was never announced as online.
        if guest.is_fake() && !guest.disconnect_pending {
            return false;
        }
        // A held player was recorded as joined at its login: it leaves too.
        if self.common.is_dedicated_server() && (guest.joined() || guest.held) {
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
                    &sunrise_sim::lan::arrivals::dropped_line(&guest.name, &guest.left_reason),
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
                .server
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
        let Some(host) = self.server.lan.as_mut() else {
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
        let Some(host) = self.server.lan.as_mut() else {
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
