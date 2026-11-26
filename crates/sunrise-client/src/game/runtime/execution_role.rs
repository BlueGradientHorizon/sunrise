pub use sunrise_gameplay_common::execution_role::ExecutionRole;

use super::Game;
use mod_api::HostSide;

/// The hook host of `game`'s side.
pub(super) fn host_side<'a, S: super::Side>(game: &'a mut Game<'_, S>) -> HostSide<'a> {
    S::host_side(game)
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// This game's side: a server simulates the world, a client does not. The
    /// one source for a helper's side; `remote` says whether a client is
    /// connected, not which side this game is.
    pub(super) fn is_logical_server(&self) -> bool {
        self.options.execution_role.is_logical_server()
    }

    /// Whether this game is a dedicated server, not the integrated server thread (which
    /// keeps `options.dedicated` set too).
    pub(super) fn is_dedicated_server(&self) -> bool {
        self.options.execution_role.is_dedicated_server()
    }
}
