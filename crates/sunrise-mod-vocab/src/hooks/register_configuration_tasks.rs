//! Tasks registered before admitting a connecting player.
pub trait Hooks {
    /// Append tasks for this connecting profile, on the server thread after the
    /// built-in configuration steps. Runs for every peer, including vanilla.
    /// The profile has no player handle yet. Use `event.register(task)` to give
    /// this connection an owned task. Tasks start in registration order and check
    /// their required channels before finishing, waiting or failing this peer.
    /// There is no task timeout: answering keep-alives can hold configuration
    /// indefinitely. A silent or disconnected connection still ends normally.
    #[hook(
        side = "server",
        contract = "listen",
        version = 2,
        neoforge = "RegisterConfigurationTasksEvent"
    )]
    fn register_configuration_tasks();
}
