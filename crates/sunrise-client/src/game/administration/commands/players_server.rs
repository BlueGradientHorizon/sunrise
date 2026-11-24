#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Target};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::players::mode_name;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::players::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    command::args::Selector, lan::guest::Guest, text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::{self, Clientbound as C};

impl Game<'_> {
    /// `PlayerList.getPlayers` but the player in place: the guests who
    /// joined the world.
    pub(in crate::game) fn command_players(&self) -> impl Iterator<Item = &Guest> {
        let acting = self.server.lan_acting;
        self.lan
            .iter()
            .flat_map(|h| h.hub.online_players())
            .filter(move |g| Some(g.conn) != acting && self.native_command_player_available(g))
    }
    /// Run `f` for the player `t` (None for anything else): in place, or
    /// with the guest's state swapped in, in their level, as their own turn.
    /// What `f` tells the player (`system_message`) goes to their chat; the
    /// command's own feedback is the caller's to send.
    pub(in crate::game) fn for_player<T>(
        &mut self,
        t: Target,
        f: impl FnOnce(&mut Self) -> T,
    ) -> Option<T> {
        let conn = match t {
            Target::Player => return Some(f(self)),
            Target::Guest(conn) if self.server.lan_acting == Some(conn) => return Some(f(self)),
            Target::Guest(conn) => conn,
            _ => return None,
        };
        let back = self.dim;
        let theirs = self
            .lan
            .as_ref()?
            .hub
            .guest(conn)
            .map(|g| super::super::Dim::from_id(&g.dimension));
        if let Some(dim) = theirs.filter(|d| *d != back)
            && let Err(e) = self.enter_level(dim)
        {
            log::error!("could not open {}: {e}", dim.id());
        }
        let Some((out, told)) = self.with_guest_swap(conn, back, |game| {
            let source = game.server.chat_capture.replace(Vec::new());
            let out = f(game);
            let told = std::mem::replace(&mut game.server.chat_capture, source).unwrap_or_default();
            (out, told)
        }) else {
            if self.dim != back
                && let Err(e) = self.enter_level(back)
            {
                log::error!("could not reopen {}: {e}", back.id());
            }
            return None;
        };
        for line in told {
            self.lan.as_ref()?.hub.send_to(
                conn,
                &C::SystemChat(play::SystemChat {
                    content: sunrise_client::lan::data::component_nbt(&line.component()),
                    overlay: false,
                }),
            );
        }
        if self.dim != back
            && let Err(e) = self.enter_level(back)
        {
            log::error!("could not reopen {}: {e}", back.id());
        }
        Some(out)
    }
    /// Native console calls cannot reach an outer turn's displaced player.
    /// Ordinary command resolution is unchanged, including nested turns.
    pub(in crate::game) fn native_command_player_available(&self, guest: &Guest) -> bool {
        !self.server.native_command_filter || self.entity(guest.entity_id as u32).is_some()
    }
    /// The guests a selector looks at, in join order (`PlayerList.getPlayers`),
    /// the one in place as `Target::Player`: all of them, or those in the
    /// source's level when it limits the search to a place
    /// (`isWorldLimited`); with only the `Entity::isAlive` ones when
    /// `alive_only` (the built-in `@e` and `@n`).
    pub(in crate::game) fn player_candidates(&self, s: &Selector, alive_only: bool) -> Vec<Target> {
        let limited = s.distance.is_some()
            || s.pos.iter().any(Option::is_some)
            || s.delta.iter().any(Option::is_some);
        let here = self.dim.id();
        let in_place = self.server.lan_acting.filter(|_| self.player.is_some());
        self.lan
            .iter()
            .flat_map(|h| h.hub.online_players())
            .filter(|g| self.native_command_player_available(g))
            .filter(|g| !limited || g.dimension == here || Some(g.conn) == in_place)
            .filter(|g| !alive_only || (!g.dead && g.health > 0.0))
            .map(|g| {
                if Some(g.conn) == in_place {
                    Target::Player
                } else {
                    Target::Guest(g.conn)
                }
            })
            .collect()
    }
    /// `PlayerList.getPlayerByName`: names are not case sensitive. Sunrise:
    /// on a server where a mod gave players nicknames
    /// (`native_set_nickname`), a name no player has finds the player going
    /// by it, without its `~` or with it (quoted), so `/msg` and `/tell`
    /// reach a player by the name chat shows. No nickname, no change.
    pub(in crate::game) fn player_named(&self, name: &str) -> Option<Target> {
        if !self.server.lan_acting_fake
            && self.player.is_some()
            && name.eq_ignore_ascii_case(self.acting_player_name())
        {
            return Some(Target::Player);
        }
        self.command_players()
            .find(|g| g.name.eq_ignore_ascii_case(name))
            .map(|g| Target::Guest(g.conn))
            .or_else(|| self.player_nicknamed(name))
    }
    /// The name of the server's own source: "Rcon" for a command an RCON client sent
    /// (`RconConsoleSource`), "Server" otherwise (`MinecraftServer.createCommandSourceStack`).
    pub(in crate::game) fn server_name(&self) -> Text {
        Text::literal(if self.server.rcon_source {
            "Rcon"
        } else {
            "Server"
        })
    }
    /// `CommandSourceStack.getDisplayName`: the executing entity's name, the
    /// command block's or minecart's own name when the source has no entity,
    /// or the server's.
    pub(in crate::game) fn source_display_name(&self) -> Text {
        match self.server.command_source {
            Some(source) => match source.entity {
                Some(target) => self.target_name(target),
                None => source
                    .block
                    .and_then(|pos| match self.world.block_entities.get(&pos) {
                        Some(sunrise_client::containers::BlockEntity::CommandBlock(block)) => {
                            Some(block.name())
                        }
                        _ => None,
                    })
                    .or_else(|| {
                        source.cart.and_then(|id| {
                            self.carts
                                .iter()
                                .find(|cart| cart.id == id)
                                .map(|cart| cart.command.name())
                        })
                    })
                    .unwrap_or_else(|| self.server_name()),
            },
            None => self.player_display_name(),
        }
    }
    /// The entity the command runs as (`CommandSourceStack.getEntity`): a
    /// command typed by the player in place runs as them, the server
    /// console's as nobody.
    pub(in crate::game) fn source_entity(&self) -> Option<Target> {
        match self.server.command_source {
            Some(s) => s.entity,
            None => self.player.is_some().then_some(Target::Player),
        }
    }
    /// `EntitySelector.findEntities` with a UUID: the player with it, else
    /// the entity.
    pub(in crate::game) fn target_by_uuid(&self, uuid: u128) -> Option<Target> {
        let own = self.server.lan_acting_uuid.or_else(|| {
            self.level.player_uuid.map(|[a, b, c, d]| {
                (u128::from(a as u32) << 96)
                    | (u128::from(b as u32) << 64)
                    | (u128::from(c as u32) << 32)
                    | u128::from(d as u32)
            })
        });
        if self.player.is_some() && own == Some(uuid) {
            return (!self.server.lan_acting_fake).then_some(Target::Player);
        }
        if let Some(g) = self.command_players().find(|g| g.uuid == uuid) {
            return Some(Target::Guest(g.conn));
        }
        let words = [96, 64, 32, 0].map(|shift| (uuid >> shift) as u32 as i32);
        self.target_with_uuid(words)
    }
}
