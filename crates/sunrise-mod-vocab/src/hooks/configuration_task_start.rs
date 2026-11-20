//! A configuration task's start callback.
pub trait Hooks {
    /// Start this task once. It may send payloads, finish synchronously, or wait for a reply.
    /// connection.finish(host, task_type) verifies the active type. An error or
    /// panic fails this connecting client with the configuration-error reason.
    #[hook(
        target = "sunrise_client::game::native_hooks::register_configuration_tasks::Game<'_>::native_configuration_task_start",
        dispatch = "flat",
        side = "server",
        contract = "listen"
    )]
    fn configuration_task_start(
        connection: mod_api::ConfigurationConnection,
        task_type: &Identifier,
    );
}
