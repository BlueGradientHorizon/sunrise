//! The `player_commands_sent` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0249

pub trait Hooks {
    /// Filter the command tree just before it is sent to a live server player,
    /// including on join, on respawn, on dimension changes and on command refresh.
    /// Use `player` with the host to read the recipient and `tree.hide_where(...)`
    /// to select roots to omit from the permission-filtered tree. Aliases that
    /// redirect to removed commands are omitted too. This only changes command
    /// highlighting and completion on that client; it does not prevent execution.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_player_commands_sent",
        dispatch = "api_carrier",
        host = "read",
        receiver = "read",
        side = "server",
        contract = "collect",
        out = "answers",
        answered_by = "mod",
        isolate = true,
        version = 2,
        neoforge = "none (NeoForge 26.3 has no send-time filter event)"
    )]
    fn player_commands_sent(
        player: PlayerRef,
        tree: CommandTreeFilter<'_>,
        answers: &mut dyn FnMut(Answered<CommandTreeSelection>),
    );
}
