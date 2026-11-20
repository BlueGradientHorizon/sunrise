//! A configuration task's start callback.
pub trait Hooks {
    /// Start this task once. It may send payloads, request completion, or wait for a reply.
    /// `connection.finish(host, task_type)` verifies the active type and stages completion until
    /// commit, after queued sends. An error or panic fails the connecting client with the
    /// configuration-error reason.
    #[hook(side = "server", contract = "listen", version = 2)]
    fn configuration_task_start();
}
