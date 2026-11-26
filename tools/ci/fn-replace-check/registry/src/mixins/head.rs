fn head(game: &mut Game<'_, crate::game::server_side::ServerState>) {
    fnr_registry::note(
        "HEAD on_tick_rate_changed, ticks_until_autosave",
        game.side.ticks_until_autosave,
    );
}
