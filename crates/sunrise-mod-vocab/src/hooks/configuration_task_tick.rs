//! A configuration task's tick callback.
pub trait Hooks {
    /// Check this running task once per server tick. `task_type` identifies its registered type.
    /// Call `context.finish()` on success. The request takes effect on commit, after queued sends.
    /// Failure or panic aborts the callback and fails the peer with the configuration-error reason.
    #[hook(side = "server", contract = "listen", version = 3)]
    fn configuration_task_tick();
}
