#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game,
    execution_role::ExecutionRole,
    scheduled_tasks::{ScheduledTasks, ServerTask, dispose_pending_task, report_failure, run_all},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_lifecycle::NEXT_EPOCH;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_lifecycle::NativeLifecycle;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_lifecycle::OpenAttempt;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_lifecycle::OpenState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_lifecycle::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    Authority, LifecycleContext, ServerTickPhase, TickId, WorldEpoch, WorldPhase,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{ScheduleError, TaskId};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::atomic::{AtomicU64, Ordering};

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn client_lifecycle_open_started(&mut self) {
        self.client_reset_entity_residency();
        self.entity_initialization = Default::default();
        if let Some(remote) = &mut self.remote {
            remote.clear_attachments();
        }
        self.attachment_indexes.clear();
        self.passages.retire_world();
        self.native_lifecycle.begin_open(Authority::RemoteClient);
    }

    pub(in crate::game) fn client_lifecycle_world_ready(&mut self) {
        let _ = self.native_lifecycle.world_open_ready();
    }

    /// Client epochs retire without dispatching server lifecycle hooks.
    pub(in crate::game) fn client_lifecycle_world_closed(&mut self) {
        self.side.instance_groups.borrow_mut().clear();
        let _ = self.native_lifecycle.world_close_final();
        self.finish_world_closed_common();
    }
}
