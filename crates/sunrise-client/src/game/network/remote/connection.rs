//! The window's connection to a server, as the engine sees it: its record of the last server, the
//! hooks it calls as a connection starts, lets the player in and closes (`sunrise_hooks::connection`),
//! and the Disconnected screen's offer to connect again, which a crate above asks for
//! (`Hooks::retry`). With nothing asking, the screen is vanilla's `DisconnectedScreen`.
//!
//! A connection over the network is one session, from `connection_starts` until it closes, once,
//! in `connection_left` or `connection_ended`, whichever way: the player leaving, the game closing,
//! a transfer, the server, the connection or this client ending it. A world the window plays
//! itself never opens one.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use sunrise_client::{multiplayer, title::Mode};
#[allow(unused_imports)]
use sunrise_hooks::connection::{By, Ended, Left, Retry, Server};

/// The Disconnected screen's line about connecting again, while that screen shows.
pub(in crate::game) fn retry_slot(
    title: &mut Option<sunrise_client::title::Title>,
) -> Option<&mut Option<String>> {
    match &mut title.as_mut()?.mode {
        Mode::Multiplayer(multiplayer::Screen::Disconnected { retry, .. }) => Some(retry),
        _ => None,
    }
}

impl Game<'_> {
    /// Leave the title for a server (ConnectScreen): an empty world fed by the connection.
    pub(in crate::game) fn connect_server(&mut self, address: &str, name: &str) -> Result<()> {
        self.connect_server_with_options(address, name, None, false, false, None)
    }
    /// A stack to the server (`SetCreativeModeSlot`): the same component
    /// patch a Sunrise server sends its clients, with the registries' ids, so
    /// the server's echo doesn't hand back a bare item.
    pub(in crate::game) fn to_network(&self, s: &Option<ItemStack>) -> Option<Stack> {
        self.to_network_projecting(s, &|_| ())
    }
    // handle_command_suggestions: remote/commands.rs
    /// Another player of this level, whose copy passes every click (`Player.interactOn`).
    pub(in crate::game) fn is_level_player(&self, id: i32) -> bool {
        self.remote
            .as_ref()
            .is_some_and(|r| r.level_players.contains_key(&id))
    }
    /// `ConnectScreen`'s Cancel: a server's (single player's server isn't
    /// joined through `ConnectScreen`), until the login is done.
    pub(in crate::game) fn connect_cancel(&self, (w, h): (f32, f32)) -> Option<(f32, f32)> {
        let r = self.remote.as_ref()?;
        (self.player.is_none() && !r.joined && self.integrated.is_none())
            .then(|| sunrise_client::level_loading::connect_cancel(w, h))
    }
}
