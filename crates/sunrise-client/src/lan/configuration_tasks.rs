//! Server-thread task queues and progress shared with each connection worker.
use super::config_payloads::ConfigSender;
use crate::raw_payloads::{Endpoint, Transport, connection::Epoch};
use mod_api::{
    __private::RegisteredConfigurationTask, ConfigurationConnection, ConfigurationTaskError,
    PayloadId, PayloadSendError,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use sunrise_protocol::payload_negotiation::Phase;

pub struct ConfigurationRequest {
    pub connection: ConfigurationConnection,
    pub player: u128,
    pub name: String,
    pub session: Arc<Session>,
}
struct Entry {
    session: Arc<Session>,
    queue: Vec<Option<RegisteredConfigurationTask>>,
}

pub struct Tasks {
    incarnation: u64,
    sessions: BTreeMap<u64, Entry>,
}
impl Tasks {
    pub fn new(incarnation: u64) -> Self {
        Self {
            incarnation,
            sessions: BTreeMap::new(),
        }
    }
    fn local_id(&self, connection: ConfigurationConnection) -> Option<u64> {
        let (incarnation, id) = mod_api::__private::configuration_connection_parts(connection);
        (incarnation == self.incarnation).then_some(id)
    }
    pub fn insert(&mut self, request: &ConfigurationRequest) {
        let Some(id) = self.local_id(request.connection) else {
            return;
        };
        self.sessions.insert(
            id,
            Entry {
                session: request.session.clone(),
                queue: Vec::new(),
            },
        );
    }
    /// Publish a committed registration to the worker.
    pub fn register(
        &mut self,
        connection: ConfigurationConnection,
        tasks: Vec<RegisteredConfigurationTask>,
    ) {
        let Some(id) = self.local_id(connection) else {
            return;
        };
        let Some(entry) = self.sessions.get_mut(&id) else {
            return;
        };
        let task_types = tasks.iter().map(|task| task.task_type.clone()).collect();
        entry.queue = tasks.into_iter().map(Some).collect();
        entry.session.register(task_types);
    }
    pub fn get(&self, connection: ConfigurationConnection) -> Option<&Arc<Session>> {
        let id = self.local_id(connection)?;
        self.sessions.get(&id).map(|entry| &entry.session)
    }
    /// Take the object out of its slot before running or retiring it.
    pub fn take_task(
        &mut self,
        connection: ConfigurationConnection,
        ordinal: usize,
    ) -> Option<RegisteredConfigurationTask> {
        let id = self.local_id(connection)?;
        self.sessions.get_mut(&id)?.queue.get_mut(ordinal)?.take()
    }
    /// Put back a pending object only while its original session and turn are live.
    /// On refusal the caller still owns the object and retires it unlocked.
    pub fn restore_task(
        &mut self,
        connection: ConfigurationConnection,
        session: &Arc<Session>,
        ordinal: usize,
        task: &mut Option<RegisteredConfigurationTask>,
    ) {
        let Some(id) = self.local_id(connection) else {
            return;
        };
        let Some(entry) = self
            .sessions
            .get_mut(&id)
            .filter(|entry| Arc::ptr_eq(&entry.session, session))
        else {
            return;
        };
        if session.transport().closed() {
            return;
        }
        let state = session.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.running(ordinal)
            && let Some(slot) = entry.queue.get_mut(ordinal).filter(|slot| slot.is_none())
        {
            *slot = task.take();
        }
    }
    /// Take out a closed session's remaining tasks so the caller can retire them.
    pub fn remove(
        &mut self,
        connection: ConfigurationConnection,
    ) -> Vec<Option<RegisteredConfigurationTask>> {
        let Some(id) = self.local_id(connection) else {
            return Vec::new();
        };
        self.sessions
            .remove(&id)
            .map_or_else(Vec::new, |entry| entry.queue)
    }
    /// Visit the next session without retaining a borrow across a task callback.
    pub fn next_session(
        &self,
        after: Option<ConfigurationConnection>,
    ) -> Option<(ConfigurationConnection, Arc<Session>)> {
        use std::ops::Bound::{Excluded, Unbounded};
        let lower = match after {
            None => Unbounded,
            Some(connection) => Excluded(self.local_id(connection)?),
        };
        self.sessions
            .range((lower, Unbounded))
            .next()
            .map(|(&id, entry)| {
                (
                    mod_api::__private::configuration_connection(self.incarnation, id),
                    entry.session.clone(),
                )
            })
    }
}
#[derive(Default)]
struct State {
    task_types: Option<Vec<mod_api::Identifier>>,
    at: usize,
    started: bool,
    finished: bool,
    failure: Option<String>,
    closed: bool,
}
impl State {
    fn running(&self, ordinal: usize) -> bool {
        self.task_types
            .as_ref()
            .is_some_and(|types| ordinal < types.len())
            && self.at == ordinal
            && self.started
            && !self.finished
            && !self.closed
            && self.failure.is_none()
    }
}
pub struct Turn {
    pub ordinal: usize,
    pub start: bool,
}

pub struct Session {
    state: Mutex<State>,
    sender: ConfigSender,
}
impl Session {
    pub fn new(sender: ConfigSender) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State::default()),
            sender,
        })
    }
    pub(crate) fn register(&self, task_types: Vec<mod_api::Identifier>) {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .task_types = Some(task_types);
    }
    /// Advance past a published completion, returning its ordinal for retirement
    /// even when it was the final task. Then take the next eligible turn, if any.
    pub fn turn(&self) -> (Option<usize>, Option<Turn>) {
        if self.transport().closed() {
            return (None, None);
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(task_count) = state.task_types.as_ref().map(Vec::len) else {
            return (None, None);
        };
        if state.closed || state.failure.is_some() {
            return (None, None);
        }
        let completed = state.finished.then_some(state.at);
        if completed.is_some() {
            state.at += 1;
            state.started = false;
            state.finished = false;
        }
        let turn = (state.at < task_count).then(|| {
            let start = !state.started;
            state.started = true;
            Turn {
                ordinal: state.at,
                start,
            }
        });
        (completed, turn)
    }
    pub fn finished(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .finished
    }
    /// Verify that the currently running task on this session matches `task_type`
    /// and return its internal turn ordinal so deferred network publication can
    /// only finish that exact turn.
    pub fn running_ordinal_for_type(
        &self,
        task_type: &mod_api::Identifier,
    ) -> Result<usize, ConfigurationTaskError> {
        if self.transport().closed() {
            return Err(ConfigurationTaskError::Disconnected);
        }
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed || state.failure.is_some() {
            return Err(ConfigurationTaskError::Disconnected);
        }
        let Some(types) = &state.task_types else {
            return Err(ConfigurationTaskError::NotRunning);
        };
        if !(state.started && !state.finished && state.at < types.len()) {
            return Err(ConfigurationTaskError::NotRunning);
        }
        if &types[state.at] != task_type {
            return Err(ConfigurationTaskError::WrongType);
        }
        Ok(state.at)
    }
    /// Mark the currently running task finished immediately if its type matches `task_type`.
    pub fn finish_type(
        &self,
        task_type: &mod_api::Identifier,
    ) -> Result<(), ConfigurationTaskError> {
        if self.transport().closed() {
            return Err(ConfigurationTaskError::Disconnected);
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed || state.failure.is_some() {
            return Err(ConfigurationTaskError::Disconnected);
        }
        let Some(types) = &state.task_types else {
            return Err(ConfigurationTaskError::NotRunning);
        };
        if !(state.started && !state.finished && state.at < types.len()) {
            return Err(ConfigurationTaskError::NotRunning);
        }
        if &types[state.at] != task_type {
            return Err(ConfigurationTaskError::WrongType);
        }
        state.finished = true;
        Ok(())
    }
    /// Mark `ordinal` finished while the caller holds an active `Epoch::with_active`
    /// publication gate. Never acquires `transport()` (whose connection-state lock is
    /// ordered outside epoch invalidation).
    pub fn finish_ordinal_under_epoch(&self, ordinal: usize) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.running(ordinal) {
            state.finished = true;
        }
    }
    /// Mark the currently running task finished if this ordinal is still active.
    pub fn finish_current(&self, ordinal: usize) {
        if self.transport().closed() {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.running(ordinal) {
            state.finished = true;
        }
    }
    pub fn fail(&self, reason: impl Into<String>) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.failure.get_or_insert_with(|| reason.into());
    }
    pub fn has_channel(&self, channel: &PayloadId) -> bool {
        !self.closed()
            && self
                .transport()
                .epoch()
                .is_some_and(|epoch| epoch.peer.can_send(Phase::Configuration, channel).is_ok())
    }
    pub fn payload_target(&self) -> Result<(Endpoint, Arc<Epoch>), ConfigurationTaskError> {
        // The retained session may still be in the game thread's task map
        // after I/O publishes Play. Bind the send to this exact Configuration epoch.
        let epoch = self
            .transport()
            .epoch()
            .ok_or(ConfigurationTaskError::Payload(
                PayloadSendError::NotNegotiated,
            ))?;
        if epoch.phase != Phase::Configuration {
            return Err(ConfigurationTaskError::Disconnected);
        }
        Ok((Endpoint::configuration(self.sender.clone(), None), epoch))
    }
    pub fn transport(&self) -> &Arc<Transport> {
        self.sender.payload_transport()
    }
    pub fn result(&self) -> Option<Result<(), String>> {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(error) = &state.failure {
            return Some(Err(error.clone()));
        }
        (state.task_types.as_ref().map(Vec::len) == Some(state.at)).then_some(Ok(()))
    }
    pub fn close(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
    }
    pub fn closed(&self) -> bool {
        self.transport().closed() || self.state.lock().unwrap_or_else(|e| e.into_inner()).closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_configuration_session_refuses_the_play_epoch() {
        use crate::raw_payloads::Side;
        use mod_api::PayloadBytes;
        use sunrise_protocol::attachment_sync;

        let transport = Transport::capture(Side::Server).unwrap();
        transport
            .accept(transport.local.manifest.entries())
            .unwrap();
        let (sender, _peer) = ConfigSender::test_sender(transport.clone());
        let session = Session::new(sender);
        let (endpoint, configuring) = session.payload_target().unwrap();
        assert_eq!(configuring.phase, Phase::Configuration);

        // I/O closes the task session and advances the transport before the game
        // removes the saved connection or the configuration writer is retired.
        session.close();
        transport.finish_configuration().unwrap();
        let play = transport.epoch().unwrap();
        assert_eq!(play.phase, Phase::Play);
        let channel = PayloadId(attachment_sync::CHANNEL.into());
        assert_eq!(play.peer.can_send(Phase::Play, &channel), Ok(()));
        assert!(matches!(
            session.payload_target(),
            Err(ConfigurationTaskError::Disconnected)
        ));
        assert!(matches!(
            endpoint.prepare(&configuring, &channel, PayloadBytes(vec![1])),
            Err(PayloadSendError::StaleEpoch)
        ));
    }

    #[test]
    fn finish_type_and_finish_current_match_only_the_active_task() {
        use crate::lan::config_payloads::ConfigIo;
        use sunrise_protocol::connection::{Connection, Pipe};

        let (_peer, server) = Pipe::pair();
        let mut server = Connection::local(server);
        let transport = Transport::empty();
        let (events, _receiver) = crate::lan::event_channel_with_capacity(1 << 20);
        let mut io = ConfigIo::new(&mut server, transport.clone(), events).unwrap();
        let session = Session::new(io.task_sender(transport).unwrap());
        let first = mod_api::Identifier::parse("demo:first").unwrap();
        let second = mod_api::Identifier::parse("demo:second").unwrap();
        assert_eq!(
            session.running_ordinal_for_type(&first),
            Err(ConfigurationTaskError::NotRunning)
        );

        session.register(vec![first.clone(), second.clone()]);
        assert_eq!(
            session.running_ordinal_for_type(&first),
            Err(ConfigurationTaskError::NotRunning)
        );
        let (completed, turn) = session.turn();
        assert_eq!(completed, None);
        let turn = turn.unwrap();
        assert!(turn.start);
        assert_eq!(turn.ordinal, 0);
        assert_eq!(session.running_ordinal_for_type(&first), Ok(0));
        assert_eq!(
            session.running_ordinal_for_type(&second),
            Err(ConfigurationTaskError::WrongType)
        );
        assert_eq!(
            session.finish_type(&second),
            Err(ConfigurationTaskError::WrongType)
        );
        assert!(!session.finished());
        let (completed, tick) = session.turn();
        assert_eq!(completed, None);
        let tick = tick.unwrap();
        assert!(!tick.start);
        assert_eq!(tick.ordinal, 0);

        assert!(session.finish_type(&first).is_ok());
        assert!(session.finished());
        assert_eq!(
            session.running_ordinal_for_type(&first),
            Err(ConfigurationTaskError::NotRunning)
        );
        assert_eq!(
            session.finish_type(&first),
            Err(ConfigurationTaskError::NotRunning)
        );

        let (completed, next) = session.turn();
        assert_eq!(completed, Some(0));
        let next = next.unwrap();
        assert!(next.start);
        assert_eq!(next.ordinal, 1);
        session.finish_ordinal_under_epoch(turn.ordinal);
        assert!(!session.finished());
        assert_eq!(
            session.running_ordinal_for_type(&first),
            Err(ConfigurationTaskError::WrongType)
        );
        assert_eq!(session.running_ordinal_for_type(&second), Ok(1));
        session.finish_ordinal_under_epoch(next.ordinal);
        let (completed, turn) = session.turn();
        assert_eq!(completed, Some(1));
        assert!(turn.is_none());
        assert_eq!(session.result(), Some(Ok(())));
    }
}
