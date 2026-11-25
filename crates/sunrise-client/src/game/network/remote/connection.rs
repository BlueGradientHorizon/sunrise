//! The window's connection to a server, as the engine sees it: its record of the last server, the
//! hooks it calls as a connection starts, lets the player in and closes (`sunrise_hooks::connection`),
//! and the Disconnected screen's offer to connect again, which a crate above asks for
//! (`Hooks::retry`). With nothing asking, the screen is vanilla's `DisconnectedScreen`.
//!
//! A connection over the network is one session, from `connection_starts` until it closes, once,
//! in `connection_left` or `connection_ended`, whichever way: the player leaving, the game closing,
//! a transfer, the server, the connection or this client ending it. A world the window plays
//! itself never opens one.
use super::*;
use sunrise_client::{multiplayer, title::Mode};

/// The Disconnected screen's line about connecting again, while that screen shows.
pub(in crate::game) fn retry_slot(
    title: &mut Option<sunrise_client::title::Title>,
) -> Option<&mut Option<String>> {
    match &mut title.as_mut()?.mode {
        Mode::Multiplayer(multiplayer::Screen::Disconnected { retry, .. }) => Some(retry),
        _ => None,
    }
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// A stack to the server (`SetCreativeModeSlot`): the same component
    /// patch a Sunrise server sends its clients, with the registries' ids, so
    /// the server's echo doesn't hand back a bare item.
    pub(in crate::game) fn to_network(&self, s: &Option<ItemStack>) -> Option<Stack> {
        self.to_network_projecting(s, &|_| ())
    }
}
