//! The `register_client_commands` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0017

pub trait Hooks {
    /// The client's own commands (NeoForge's `RegisterClientCommandsEvent`): push
    /// each usage with its executor attached by [`Command::executes_client`]
    /// (`Command::new("x <mode:word>").executes_client(...)`).
    /// Asked when a connection starts and again when the server sends its
    /// own tree, so the chat box colours, completes and lists them as the
    /// game's own; a name the game already has keeps that command's
    /// behaviour. A client command has no operator level: one with a nonzero
    /// `level` is left out, and server-only handle argument kinds (`player`,
    /// `players`, `entity`, `entities`, `dimension`) are refused.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_register_client_commands",
        dispatch = "flat",
        side = "client",
        contract = "collect",
        out = "commands",
        answered_by = "mod",
        owned_answer_batch = true,
        neoforge = "RegisterClientCommandsEvent",
        version = 2
    )]
    fn register_client_commands(
        commands: &mut Vec<mod_api::Answered<mod_api::ClientCommandRegistration>>,
    );
}
