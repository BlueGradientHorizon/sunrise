use crate::ClientHost;
use std::sync::Arc;

type Task = Box<dyn FnOnce(&mut dyn ClientHost) + Send>;
type Submit = dyn Fn(Task) + Send + Sync;

/// A handle to the client's task drain, obtained when `host.client_executor()` returns `Some`.
/// Clone it into workers; it never borrows a host or a connection.
// no wire: submits closures to the live client in this process.
// exhaustive: its field is private, so mods cannot build or destructure it.
#[derive(Clone)]
pub struct ClientExecutor(Arc<Submit>);

impl ClientExecutor {
    /// Game-side constructor for the calling mod's executor.
    #[doc(hidden)]
    pub fn new(submit: impl Fn(Task) + Send + Sync + 'static) -> Self {
        Self(Arc::new(submit))
    }

    /// Queues `task` for the next client-thread drain, even when called on the client
    /// thread. Tasks run in submission order with a fresh host. Move owned data into
    /// the task, not the caller's host or payload context. The task is dropped if the
    /// client stops before it can run. A task that panics is reported with the mod's name;
    /// later tasks, including those from the same mod, still run.
    pub fn execute(&self, task: Box<dyn FnOnce(&mut dyn ClientHost) + Send>) {
        (self.0)(task);
    }
}
