//! A configuration task's start callback.
pub trait Hooks {
    /// Start this task once. It may send payloads, request completion, or wait for a reply.
    /// `task_type` identifies the registered type; `context` addresses only this instance.
    /// `context.finish()` takes effect on commit, after queued sends. Failure or panic aborts
    /// the callback and fails the connecting client with the configuration-error reason.
    #[hook(side = "server", contract = "listen", version = 3)]
    fn configuration_task_start();
}
