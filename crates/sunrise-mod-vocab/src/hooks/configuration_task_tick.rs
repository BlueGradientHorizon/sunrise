//! A configuration task's tick callback.
pub trait Hooks {
    /// Check this running task once per server tick. Set `finished` when its check succeeds.
    /// The flag requests completion when this callback's transaction commits, after queued sends.
    /// `connection.finish(host, task_type)` verifies the active type and stages completion in
    /// the current transaction. An error or panic fails the connecting client with the
    /// configuration-error reason.
    #[hook(side = "server", contract = "collect", out = "finished", version = 2)]
    fn configuration_task_tick();
}
