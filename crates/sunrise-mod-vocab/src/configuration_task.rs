//! Configuration tasks and capabilities valid before the player joins.
use crate::callback_owner::CallbackOwner;
use crate::{Identifier, PayloadBytes, PayloadId, RegisterConfigurationTasksEvent, ServerHost};
use std::{
    any::{Any, TypeId},
    collections::{BTreeSet, HashMap},
    fmt,
    sync::{LazyLock, Mutex},
};

// The number of concrete native task types is fixed by the installed code, not by peers.
type TaskOwners = HashMap<(&'static str, TypeId), CallbackOwner>;
static TASK_OWNERS: LazyLock<Mutex<TaskOwners>> = LazyLock::new(Mutex::default);

// no wire: tasks are owned objects with server-thread callbacks and local state.
/// One task instance owned by a configuring connection. Register the instance with
/// [`RegisterConfigurationTasksEvent::register`]. The server runs it in queue order,
/// on the server thread without holding the connection's task lock, and keeps the
/// same instance while it waits. Callbacks run as the registering mod. A disabled
/// implementation is skipped; a panic during registration, start or tick closes that
/// peer and disables the task implementation for that mod for the session, without
/// disabling other mods' tasks. Dropping a task also runs as the registering mod;
/// a destructor panic is reported and disables the implementation, even if the
/// task was rejected or skipped.
pub trait ConfigurationTask: Send + 'static {
    /// The namespaced task type, read once as the registering mod. It must be
    /// nonempty and unique among all tasks registered for this connection. A phase
    /// refusal here rolls back the enclosing listener's task registrations.
    fn task_type(&self) -> &Identifier;

    /// Start this task exactly once. Call [`ConfigurationTaskContext::finish`] or
    /// [`ConfigurationTaskContext::fail`], or leave it pending for a tick or a reply.
    /// A finish request publishes only after the outermost transaction commits and
    /// publishes its deferred sends.
    fn start(&mut self, host: &mut dyn ServerHost, context: &mut ConfigurationTaskContext<'_>);

    /// Poll a pending task, at most once per server tick after its start. By default
    /// the task just waits. As in [`Self::start`], use the context to send, finish or
    /// fail. A failure or panic rolls back the callback and fails admission.
    fn tick(&mut self, _host: &mut dyn ServerHost, _context: &mut ConfigurationTaskContext<'_>) {}
}

// no wire: local registration diagnostics, not payload data.
/// An invalid submission to the connection's configuration task queue.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigurationTaskRegistrationError {
    /// The task type has an empty namespace or path.
    EmptyType,
    /// Another task already declared this type for this connection.
    DuplicateType(Identifier),
    /// The task type could not be read under its owner. If it panicked, later
    /// submissions of this implementation by the same mod are skipped.
    CallbackFailed(String),
}
impl fmt::Display for ConfigurationTaskRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyType => {
                f.write_str("Configuration task type has an empty namespace or path")
            }
            Self::DuplicateType(id) => write!(f, "Duplicate configuration task type: {id}"),
            Self::CallbackFailed(reason) => f.write_str(reason),
        }
    }
}
impl std::error::Error for ConfigurationTaskRegistrationError {}

impl RegisterConfigurationTasksEvent<'_> {
    /// Append a task to this connection's queue. An invalid or duplicate type, or a
    /// panic while reading it, aborts the registration transaction and admission even
    /// if the listener ignores this result. The first error supplies the diagnostic.
    /// A disabled implementation is not queued and its instance is dropped without
    /// calling `task_type`. A destructor panic does not replace the first diagnostic.
    /// If the listener or `task_type` hits a phase refusal, discard this listener's
    /// staged tasks and diagnostic.
    pub fn register(
        &mut self,
        task: impl ConfigurationTask,
    ) -> Result<(), ConfigurationTaskRegistrationError> {
        let owner = crate::content::crash::current_native_mod().map(|mod_id| {
            TASK_OWNERS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .entry((mod_id, (&task as &dyn Any).type_id()))
                .or_insert_with(|| CallbackOwner::for_mod(mod_id))
                .clone()
        });
        let refusal = match owner {
            Some(owner) => {
                if !owner.is_enabled() {
                    dispose_task(&owner, task);
                    return Ok(());
                }
                let refusal =
                    match owner.call("configuration task", "ConfigurationTask::task_type", || {
                        task.task_type()
                    }) {
                        Err(reason) => ConfigurationTaskRegistrationError::CallbackFailed(reason),
                        Ok(id) if id.namespace().is_empty() || id.path().is_empty() => {
                            ConfigurationTaskRegistrationError::EmptyType
                        }
                        Ok(id) if !self.registration.types.insert(id.clone()) => {
                            ConfigurationTaskRegistrationError::DuplicateType(id.clone())
                        }
                        Ok(id) => {
                            self.registration.tasks.push(RegisteredConfigurationTask {
                                kind: id.clone(),
                                task: Some(Box::new(task)),
                                owner,
                            });
                            return Ok(());
                        }
                    };
                dispose_task(&owner, task);
                refusal
            }
            None => ConfigurationTaskRegistrationError::CallbackFailed(
                "Configuration task registration has no owning mod".into(),
            ),
        };
        self.registration
            .refusal
            .get_or_insert_with(|| refusal.clone());
        Err(refusal)
    }
}

fn dispose_task<T>(owner: &CallbackOwner, task: T) {
    owner.dispose("configuration task", "ConfigurationTask::drop", task);
}

/// Engine staging for one registration transaction.
// no wire: owns pending server-thread tasks.
// hidden: the server stages registrations here; listeners use the scoped event.
// exhaustive: the server starts empty staging with Default; the queue and refusal fields are private.
#[doc(hidden)]
#[derive(Default)]
pub struct ConfigurationTaskRegistration {
    tasks: Vec<RegisteredConfigurationTask>,
    types: BTreeSet<Identifier>,
    refusal: Option<ConfigurationTaskRegistrationError>,
}
impl ConfigurationTaskRegistration {
    /// The first refusal, if any, even if the listener ignored the error.
    pub fn refusal(&self) -> Option<&ConfigurationTaskRegistrationError> {
        self.refusal.as_ref()
    }

    /// Checkpoint one listener's append-only staging before invoking it.
    pub fn checkpoint(&self) -> (usize, bool) {
        (self.tasks.len(), self.refusal.is_some())
    }

    /// Discard only a panicking listener's tasks, reserved types and diagnostic.
    pub fn rollback(&mut self, (len, had_refusal): (usize, bool)) {
        for task in self.tasks.drain(len..) {
            self.types.remove(&task.kind);
        }
        if !had_refusal {
            self.refusal = None;
        }
    }

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
    kind: Identifier,
    task: Option<Box<dyn ConfigurationTask>>,
    /// The registering mod, for Host calls that depend on the caller's identity.
    pub owner: CallbackOwner,
}
impl RegisteredConfigurationTask {
    /// Invoke the live connection-owned instance as its registering mod.
    pub fn call<T>(
        &mut self,
        operation: &'static str,
        invoke: impl FnOnce(&mut dyn ConfigurationTask) -> T,
    ) -> Result<T, String> {
        let task = self.task.as_deref_mut().expect("live configuration task");
        self.owner
            .call("configuration task", operation, || invoke(task))
    }
}
impl Drop for RegisteredConfigurationTask {
    fn drop(&mut self) {
        dispose_task(&self.owner, self.task.take());
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

/// The exact running task of one configuring connection. For delayed completion,
/// keep the instance from the original challenge in server-local state. Never
/// substitute the task that happens to be running when the reply arrives.
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
    /// The exact task instance, which can outlive this callback. When sending a
    /// challenge, save this value for the matching reply handler.
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
    /// Request completion after the outermost transaction commits and publishes its
    /// deferred sends. Repeated requests coalesce; failure, panic or disconnect prevents it.
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
