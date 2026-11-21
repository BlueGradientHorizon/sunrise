//! Tasks registered before admitting a connecting player.
pub trait Hooks {
    /// Append tasks for this connecting profile, on the server thread after the
    /// built-in configuration steps. Runs for every peer, including vanilla.
    /// The profile has no player handle yet. Use `event.connection.has_channel`
    /// to check what the peer negotiated and `event.register(task)` to give this
    /// connection an owned task. An empty or duplicate type refuses admission.
    /// If a listener hits a phase refusal, discard only its staged tasks and type checks;
    /// other listeners' registrations stand. Tasks start in registration order and can
    /// finish, wait or fail this peer.
    /// There is no task timeout: answering keep-alives can hold configuration
    /// indefinitely. A silent or disconnected connection still ends normally.
    #[hook(
        dispatch = "flat",
        side = "server",
        contract = "listen",
        version = 3,
        neoforge = "RegisterConfigurationTasksEvent"
    )]
    fn register_configuration_tasks(
        connection: mod_api::ConfigurationConnection,
        player: u128,
        name: &str,
        registration: &mut mod_api::__private::ConfigurationTaskRegistration,
    );
}
