fn before(game: &Game<'_, crate::game::server_side::ServerState>) {
    fnr_beta::note(&format!(
        "BEFORE_CALL next_autosave_interval, rate {}",
        game.clock.rate()
    ));
}
