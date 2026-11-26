fn head(game: &mut Game<'_, crate::game::server_side::ServerState>) {
    let ticks = game.side.__horizon_mixins_fnr_support_ticks();
    *ticks = crate::game::server_side::autosave_server::fnr_runtime::bumped(*ticks);
    fnr_support::note(&format!(
        "module and bridge: ticks_until_autosave {}",
        *ticks
    ));
}
