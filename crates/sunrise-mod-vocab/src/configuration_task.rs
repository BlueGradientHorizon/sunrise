//! Configuration tasks and capabilities valid before the player joins.
use crate::{Identifier, PayloadBytes, PayloadId, RegisterConfigurationTasksEvent, ServerHost};

// no wire: tasks are owned objects with server-thread callbacks and local state.
/// One task instance owned by a configuring connection. Register the instance with
/// [`RegisterConfigurationTasksEvent::register`]. The server runs it in queue order,
/// on the server thread without holding the connection's task lock, and keeps the
/// same instance while it waits.
pub trait ConfigurationTask: Send + 'static {
    /// The namespaced task type, read once on registration.
    fn task_type(&self) -> &Identifier;

    /// Start this task exactly once. Call [`ConfigurationTaskContext::finish`] or
    /// [`ConfigurationTaskContext::fail`], or leave it pending for a tick or a reply.
    /// A finish request takes effect after the callback commits and publishes its
    /// deferred sends. A panic rolls back the callback and fails this peer's admission,
    /// naming the mod in the log and error toast. This mod’s task type is skipped
    /// for the rest of the session, including on later connections.
    fn start(&mut self, host: &mut dyn ServerHost, context: &mut ConfigurationTaskContext<'_>);

    /// Poll a pending task, at most once per server tick after its start. By default
    /// the task just waits. As in [`Self::start`], use the context to send, finish or
    /// fail. A failure or panic rolls back the callback and fails admission.
    /// Only a panic disables the mod’s task type for later peers.
    fn tick(&mut self, _host: &mut dyn ServerHost, _context: &mut ConfigurationTaskContext<'_>) {}
}

impl RegisterConfigurationTasksEvent<'_> {
    /// Append a task to this connection's FIFO configuration queue, matching
    /// NeoForge's `RegisterConfigurationTasksEvent.register`.
    pub fn register(&mut self, task: impl ConfigurationTask) {
        let task_type = task.task_type().clone();
        self.registration.tasks.push(RegisteredConfigurationTask {
            task_type,
            task: Box::new(task),
            owner: crate::content::crash::current_native_mod(),
        });
    }
}

/// Engine staging for one registration transaction.
// no wire: owns pending server-thread tasks.
// hidden: the server stages registrations here; listeners use the scoped event.
// exhaustive: the server starts empty staging with Default; the queue field is private.
#[doc(hidden)]
#[derive(Default)]
pub struct ConfigurationTaskRegistration {
    tasks: Vec<RegisteredConfigurationTask>,
}
impl ConfigurationTaskRegistration {
    /// Move the ordered queue to the server after registration commits.
    pub fn into_tasks(self) -> Vec<RegisteredConfigurationTask> {
        self.tasks
    }
}

/// Engine queue entry with the callback's mod scope resolved at registration.
// no wire: an owned task and its process-local mod identity.
// hidden: the server owns these queue entries; listeners register through the scoped event.
#[doc(hidden)]
#[non_exhaustive]
pub struct RegisteredConfigurationTask {
    /// The task type, captured once on registration.
    pub task_type: Identifier,
    /// The connection-owned instance.
    pub task: Box<dyn ConfigurationTask>,
    /// The registering mod, for Host calls that depend on the caller's identity.
    pub owner: Option<&'static str>,
}

/// One configuring connection, available while registering its tasks.
// no wire: a local capability for this connection, not payload data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConfigurationConnection {
    incarnation: u64,
    id: u64,
}
impl ConfigurationConnection {
    /// Whether this peer negotiated the registered configuration payload channel.
    pub fn has_channel(self, host: &dyn ServerHost, channel: &PayloadId) -> bool {
        host.configuration_has_channel(self, channel)
    }
}

/// The active task during one server-thread start or tick callback.
// no wire: borrows the callback's outcome.
#[non_exhaustive]
pub struct ConfigurationTaskContext<'a> {
    connection: ConfigurationConnection,
    outcome: &'a mut ConfigurationTaskOutcome,
}
impl ConfigurationTaskContext<'_> {
    /// Whether this task's peer negotiated the registered configuration channel.
    pub fn has_channel(&self, host: &dyn ServerHost, channel: &PayloadId) -> bool {
        self.connection.has_channel(host, channel)
    }
    /// Queue a payload in the current transaction on this connection's registered
    /// configuration channel. Another handler completing the task does not prevent
    /// sends on a connection that is still configuring. Returns
    /// [`ConfigurationTaskError::Disconnected`] if it has moved to Play.
    pub fn send_payload(
        &self,
        host: &mut dyn ServerHost,
        channel: &PayloadId,
        bytes: PayloadBytes,
    ) -> Result<(), ConfigurationTaskError> {
        host.configuration_send_payload(self.connection, channel, bytes)
    }
    /// Request completion when this callback finishes and publishes its deferred sends.
    /// Repeated requests coalesce; failure, panic or disconnect prevents it.
    pub fn finish(&mut self) {
        if matches!(self.outcome, ConfigurationTaskOutcome::Pending) {
            *self.outcome = ConfigurationTaskOutcome::Finish;
        }
    }
    /// Fail configuration for this peer and discard the callback's changes and any finish
    /// request. The first explicit failure supplies the diagnostic.
    pub fn fail(&mut self, reason: &str) {
        if !matches!(self.outcome, ConfigurationTaskOutcome::Fail(_)) {
            *self.outcome = ConfigurationTaskOutcome::Fail(reason.into());
        }
    }
}

/// A task operation was refused.
// no wire: local task and transport diagnostics, not payload data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigurationTaskError {
    /// The connection or its configuration epoch has ended.
    Disconnected,
    /// The running task on this connection has a different type.
    WrongType,
    /// A completion was requested but no configuration task is currently running
    /// on this connection (or this handler already requested completion).
    NotRunning,
    /// Completion requires a server configuration payload handler.
    WrongContext,
    /// The registered payload transport refused the send.
    Payload(crate::PayloadSendError),
}
impl crate::NoBackend for ConfigurationTaskError {
    fn no_backend() -> Self {
        Self::Disconnected
    }
}

// hidden: the engine reads the callback outcome when closing its transaction.
// no wire: the engine's local decision for one callback.
// exhaustive: matched only by the engine.
#[doc(hidden)]
pub enum ConfigurationTaskOutcome {
    Pending,
    Finish,
    Fail(String),
}

// hidden: the engine makes capabilities after allocating a connection identity.
#[doc(hidden)]
pub fn configuration_connection(incarnation: u64, id: u64) -> ConfigurationConnection {
    ConfigurationConnection { incarnation, id }
}
// hidden: the engine resolves and validates a capability before every operation.
#[doc(hidden)]
pub fn configuration_connection_parts(connection: ConfigurationConnection) -> (u64, u64) {
    (connection.incarnation, connection.id)
}
// hidden: the engine lends the callback its outcome slot.
#[doc(hidden)]
pub fn configuration_task_context(
    connection: ConfigurationConnection,
    outcome: &mut ConfigurationTaskOutcome,
) -> ConfigurationTaskContext<'_> {
    ConfigurationTaskContext {
        connection,
        outcome,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct StubTask {
        id: Identifier,
        reads: Arc<AtomicUsize>,
    }

    impl ConfigurationTask for StubTask {
        fn task_type(&self) -> &Identifier {
            self.reads.fetch_add(1, Ordering::Relaxed);
            &self.id
        }

        fn start(&mut self, _: &mut dyn ServerHost, _: &mut ConfigurationTaskContext<'_>) {}
    }

    #[test]
    fn register_accepts_duplicate_and_empty_types_in_fifo_order() {
        let mut registration = ConfigurationTaskRegistration::default();
        let reads = Arc::new(AtomicUsize::new(0));
        let first = Identifier::parse("demo:first").unwrap();
        let empty = Identifier::from_parts("demo", "").unwrap();
        let mut event = RegisterConfigurationTasksEvent {
            connection: configuration_connection(1, 1),
            player: 1,
            name: "Peer",
            registration: &mut registration,
        };
        event.register(StubTask {
            id: first.clone(),
            reads: reads.clone(),
        });
        event.register(StubTask {
            id: empty.clone(),
            reads: reads.clone(),
        });
        event.register(StubTask {
            id: first.clone(),
            reads: reads.clone(),
        });
        let tasks = registration.into_tasks();
        assert_eq!(reads.load(Ordering::Relaxed), 3);
        let types: Vec<_> = tasks.into_iter().map(|entry| entry.task_type).collect();
        assert_eq!(types, vec![first.clone(), empty, first]);
    }
}
