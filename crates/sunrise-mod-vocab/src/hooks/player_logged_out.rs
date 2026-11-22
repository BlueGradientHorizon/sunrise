//! The `player_logged_out` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0068

pub trait Hooks {
    /// `player` (`name`) left the game (NeoForge's `PlayerLoggedOutEvent`), by
    /// any path: a lost connection, a kick, the server stopping or failing.
    /// Once for each player `player_logged_in` told of, and also for a kicked
    /// fake player without a preceding login. Silent `remove_fake` sends no logout.
    /// It comes before they're saved, with them still in
    /// `host.players().server_players()`, so what a listener changes on them (an
    /// attachment, their health, a kill) is saved with them. Only when the server
    /// stops can they still get a last message. A kicked fake, which isn't saved,
    /// has already left the list.
    /// Server-side.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_player_logged_out",
        dispatch = "flat",
        side = "server",
        version = 2,
        contract = "listen",
        neoforge = "PlayerEvent.PlayerLoggedOutEvent"
    )]
    fn player_logged_out(player: u128, name: &str);
}
