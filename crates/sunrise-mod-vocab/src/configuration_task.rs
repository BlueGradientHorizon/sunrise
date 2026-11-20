//! Configuration tasks and capabilities valid before the player joins.
use crate::{Identifier, PayloadBytes, PayloadId, ServerHost};

/// A task type queued once for this joining connection.
#[derive(Clone, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
#[non_exhaustive]
pub struct ConfigurationTask {
    /// Namespaced type passed to the task start and tick callbacks.
    pub id: Identifier,
}
impl ConfigurationTask {
    /// Queue an instance of this task type.
    pub fn new(id: Identifier) -> Self {
        Self { id }
    }
}

/// One configuring connection, available while registering its tasks.
// no wire: a local capability for this connection, not payload data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConfigurationConnection {
    id: u64,
}
impl ConfigurationConnection {
    /// Whether this peer negotiated the registered configuration payload channel.
    pub fn has_channel(self, host: &dyn ServerHost, channel: &PayloadId) -> bool {
        host.configuration_has_channel(self, channel)
    }
}

/// The exact running task of one configuring connection. Keep this in local state
/// if a server main-thread payload handler will finish the task after a reply.
// no wire: a process-local capability, never supplied by a peer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConfigurationTaskRef {
    connection: ConfigurationConnection,
    ordinal: usize,
}

/// The active task during one server-thread start or tick callback.
// no wire: borrows the callback's outcome.
#[non_exhaustive]
pub struct ConfigurationTaskContext<'a> {
    instance: ConfigurationTaskRef,
    outcome: &'a mut ConfigurationTaskOutcome,
}
impl ConfigurationTaskContext<'_> {
    /// The exact task instance, which can outlive this callback.
    pub fn instance(&self) -> ConfigurationTaskRef {
        self.instance
    }
    /// Whether this task's peer negotiated the registered configuration channel.
    pub fn has_channel(&self, host: &dyn ServerHost, channel: &PayloadId) -> bool {
        self.instance.connection.has_channel(host, channel)
    }
    /// Queue a payload in the current transaction on this task's registered channel.
    /// A transport refusal fails the task.
    pub fn send_payload(
        &self,
        host: &mut dyn ServerHost,
        channel: &PayloadId,
        bytes: PayloadBytes,
    ) -> Result<(), ConfigurationTaskError> {
        host.configuration_send_payload(self.instance, channel, bytes)
    }
    /// Request completion when the callback commits, after its queued sends have been
    /// published. Repeated requests coalesce; failure, panic or disconnect prevents it.
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
    /// The instance is not the currently running task.
    StaleTask,
    /// The instance belongs to a different physical connection than the receiving peer.
    WrongConnection,
    /// Completion needs a server main-thread configuration handler with an open transaction.
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
pub fn configuration_connection(id: u64) -> ConfigurationConnection {
    ConfigurationConnection { id }
}
// hidden: the engine resolves and validates a capability before every operation.
#[doc(hidden)]
pub fn configuration_connection_id(connection: ConfigurationConnection) -> u64 {
    connection.id
}
// hidden: only the engine issues an instance for a queue entry.
#[doc(hidden)]
pub fn configuration_task_ref(
    connection: ConfigurationConnection,
    ordinal: usize,
) -> ConfigurationTaskRef {
    ConfigurationTaskRef {
        connection,
        ordinal,
    }
}
// hidden: the engine resolves the instance against its own queue.
#[doc(hidden)]
pub fn configuration_task_parts(
    instance: ConfigurationTaskRef,
) -> (ConfigurationConnection, usize) {
    (instance.connection, instance.ordinal)
}
// hidden: the engine lends the callback its outcome slot.
#[doc(hidden)]
pub fn configuration_task_context(
    instance: ConfigurationTaskRef,
    outcome: &mut ConfigurationTaskOutcome,
) -> ConfigurationTaskContext<'_> {
    ConfigurationTaskContext { instance, outcome }
}
