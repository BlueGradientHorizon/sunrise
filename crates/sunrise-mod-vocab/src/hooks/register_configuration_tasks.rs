//! Tasks registered before admitting a connecting player.
pub trait Hooks {
    /// Append tasks for this connecting profile, on the server thread after the
    /// built-in configuration steps. Runs for every peer, including vanilla.
    /// The profile has no player handle yet. Tasks run in registration order.
    /// A task checks its required channels, then finishes, or fails this peer.
    /// There is no task timeout: answering keep-alives can hold configuration
    /// indefinitely. A silent or disconnected connection still ends normally.
    #[hook(
        dispatch = "flat",
        side = "server",
        contract = "collect",
        out = "tasks",
        neoforge = "RegisterConfigurationTasksEvent"
    )]
    fn register_configuration_tasks(
        connection: mod_api::ConfigurationConnection,
        player: u128,
        name: &str,
        tasks: &mut Vec<mod_api::ConfigurationTask>,
    );
}
