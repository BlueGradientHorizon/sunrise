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

impl Game<'_> {
    /// After the final save, before the lock release. Client epochs retire
    /// without dispatching server lifecycle hooks.
    pub(in crate::game) fn native_lifecycle_world_closed(&mut self) {
        self.client.instance_groups.borrow_mut().clear();
        // Each server level closes after the final save (`LevelEvent.Unload`).
        if self.is_logical_server() {
            self.native_levels_closing();
        }
        if let Some(context) = self.native_lifecycle.world_close_final() {
            self.native_world_lifecycle(&context, WorldPhase::WorldCloseFinal, "");
        }
        self.attachment_indexes.clear();
        self.passages.retire_world();
        self.resident_entities.table.get_mut().reset();
        super::cauldron_interactions::clear();
        self.clear_composter_randoms();
        self.clear_world_parts();
    }
}
