//! Dispatch each configuring connection on the server's ordinary tick.
use super::{Game, lan::LanHost};
use mod_api::{
    __private::{
        ConfigurationTaskOutcome, configuration_task_context, configuration_task_parts,
        configuration_task_ref,
    },
    ConfigurationConnection, ConfigurationTaskError, ConfigurationTaskRef, PayloadBytes, PayloadId,
};
use std::{io, sync::Arc};
use sunrise_client::lan::{Lan, NetConfig, configuration_tasks::Session, data::SyncData};

pub(super) fn start(sync: Arc<SyncData>, net: NetConfig) -> io::Result<Lan> {
    Lan::start_with_configuration_tasks(
        sync,
        net,
        mod_api::__private::listened(mod_api::HookId::RegisterConfigurationTasks),
    )
}
impl Game<'_> {
    fn configuration_callback(
        &mut self,
        session: &Arc<Session>,
        callback: impl FnOnce(&mut Self) -> Option<(usize, ConfigurationTaskOutcome)>,
    ) -> bool {
        let seal = self.transaction_open();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(self)));
        let commit = match result {
            Err(_) => {
                session.fail("Configuration task callback panicked");
                false
            }
            Ok(Some((_, ConfigurationTaskOutcome::Fail(reason)))) => {
                session.fail(reason);
                false
            }
            Ok(_) if session.closed() || matches!(session.result(), Some(Err(_))) => false,
            Ok(Some((ordinal, ConfigurationTaskOutcome::Finish))) => session
                .prepare_finish(ordinal, None)
                .is_ok_and(|completion| {
                    // A listener can leave a child open; its rollback must not drop this request.
                    self.journal_configuration_completion(Some(&seal), completion)
                }),
            Ok(None | Some((_, ConfigurationTaskOutcome::Pending))) => true,
        };
        self.transaction_close(&seal, commit);
        commit
    }
    pub(super) fn take_lan_for_packet_tick(&mut self) -> Option<LanHost> {
        if self.level_pass.first {
            let requests: Vec<_> = self
                .lan
                .as_ref()?
                .hub
                .lan
                .configuration_requests()
                .collect();
            for request in requests {
                if request.session.closed() {
                    continue;
                }
                self.lan
                    .as_mut()?
                    .hub
                    .lan
                    .configuration_tasks
                    .insert(&request);
                let mut tasks = Vec::new();
                if self.configuration_callback(&request.session, |game| {
                    game.native_register_configuration_tasks(
                        request.connection,
                        request.player,
                        &request.name,
                        &mut tasks,
                    );
                    None
                }) {
                    request.session.register(tasks);
                }
            }
            let mut after = None;
            while let Some((connection, session)) = self
                .lan
                .as_mut()?
                .hub
                .lan
                .configuration_tasks
                .next_running(after)
            {
                after = Some(connection);
                while let Some(turn) = session.turn() {
                    self.configuration_callback(&session, |game| {
                        let mut outcome = ConfigurationTaskOutcome::Pending;
                        let context = configuration_task_context(
                            configuration_task_ref(connection, turn.ordinal),
                            &mut outcome,
                        );
                        if turn.start {
                            game.native_configuration_task_start(context, &turn.task_type);
                        } else {
                            game.native_configuration_task_tick(context, &turn.task_type);
                        }
                        Some((turn.ordinal, outcome))
                    });
                    // Only a committed start can immediately open the next turn.
                    if !turn.start || !session.finished() {
                        break;
                    }
                }
            }
        }
        self.lan.take()
    }
    pub(in crate::game) fn configuration_session(
        &self,
        connection: ConfigurationConnection,
    ) -> Option<&Arc<Session>> {
        self.lan
            .as_ref()?
            .hub
            .lan
            .configuration_tasks
            .get(connection)
    }
    pub(in crate::game) fn configuration_has_channel(
        &self,
        connection: ConfigurationConnection,
        channel: &PayloadId,
    ) -> bool {
        self.configuration_session(connection)
            .is_some_and(|session| session.has_channel(channel))
    }
    pub(in crate::game) fn configuration_send_payload(
        &mut self,
        instance: ConfigurationTaskRef,
        channel: &PayloadId,
        bytes: PayloadBytes,
    ) -> Result<(), ConfigurationTaskError> {
        let (connection, ordinal) = configuration_task_parts(instance);
        let session = self
            .configuration_session(connection)
            .cloned()
            .ok_or(ConfigurationTaskError::Disconnected)?;
        session.require_running(ordinal)?;
        self.queue_current_payload(session.endpoint(), channel, bytes)
            .map(|_| ())
            .map_err(|error| {
                session.fail(format!("Configuration payload refused: {error:?}"));
                ConfigurationTaskError::Payload(error)
            })
    }
}
