//! World epochs for native handles, and lifecycle observations on authoritative servers.
//! A paused pass
//! observes nothing; a frozen world reports `world_running: false` except during a step.

use sunrise_gameplay_common::game::native_lifecycle_common::OpenState;

use super::{
    execution_role::ExecutionRole,
    scheduled_tasks::{ServerTask, dispose_pending_task, report_failure, run_all},
};
use mod_api::{ScheduleError, TaskId};
// Their old home, for mixin code that names them here.
#[allow(unused_imports)]
pub(in crate::game) use mod_api::{
    Authority, LifecycleContext, ServerTickPhase, TickId, WorldEpoch, WorldPhase,
};

impl crate::game::ServerGame<'_> {
    /// The role is fixed at construction.
    pub(in crate::game::server_side) fn native_lifecycle_authority(&self) -> Authority {
        match self.common.options.execution_role {
            ExecutionRole::Client => Authority::RemoteClient,
            ExecutionRole::IntegratedServer => Authority::IntegratedServer,
            ExecutionRole::DedicatedServer => Authority::DedicatedServer,
        }
    }

    pub(in crate::game) fn server_lifecycle_open_started(&mut self) {
        self.clear_composter_randoms();
        self.reset_entity_residency();
        self.common.entity_initialization = Default::default();
        self.common.attachment_indexes.clear();
        self.common.passages.retire_world();
        self.common.native_lifecycle
            .begin_open(self.native_lifecycle_authority());
    }

    pub(in crate::game) fn server_lifecycle_world_ready(&mut self) {
        // The saved scoreboard is there for the server-started hooks, before any tick.
        self.load_scoreboard();
        if let Some(context) = self.common.native_lifecycle.world_open_ready() {
            self.native_server_started(context.authority == Authority::DedicatedServer);
        }
    }

    /// After the final save, before the lock release on the server.
    pub(in crate::game) fn server_lifecycle_world_closed(&mut self) {
        // Each server level closes after the final save (`LevelEvent.Unload`).
        self.native_levels_closing();
        if let Some(context) = self.common.native_lifecycle.world_close_final() {
            self.native_world_lifecycle(&context, WorldPhase::WorldCloseFinal, "");
        }
        self.clear_composter_randoms();
        self.finish_world_closed_common();
    }

    /// `error_text` is the engine's own error text, unchanged.
    pub(in crate::game) fn native_lifecycle_world_failed(&mut self, error_text: &str) {
        // The server levels that opened before the failure close first.
        if self.common.is_logical_server() {
            self.native_levels_closing();
        }
        if let Some(context) = self.common.native_lifecycle.world_open_failed() {
            self.native_world_lifecycle(&context, WorldPhase::WorldOpenFailed, error_text);
        }
        self.common.attachment_indexes.clear();
        self.common.passages.retire_world();
        self.common.resident_entities.table.get_mut().reset();
    }

    /// Report a server-loop or shutdown failure once and retire its epoch.
    pub(in crate::game::server_side) fn native_lifecycle_close_failed(&mut self, error_text: &str) {
        if self.common.is_logical_server() {
            self.native_levels_closing();
        }
        if let Some(context) = self.common.native_lifecycle.world_close_failed() {
            self.native_world_lifecycle(&context, WorldPhase::WorldCloseFailed, error_text);
        }
        self.common.attachment_indexes.clear();
        self.common.passages.retire_world();
        self.common.resident_entities.table.get_mut().reset();
    }

    pub(in crate::game::server_side) fn drain_due_tasks(
        &mut self,
    ) -> Option<Box<dyn std::any::Any + Send>> {
        run_all(
            self,
            |game| game.native_lifecycle.tasks.pop_due(),
            |game, (owner, task)| {
                use mod_api::__private::game::CallbackOwner;

                let Some(mod_id) = owner else {
                    // Engine-owned work still follows the server's own failure policy.
                    let _: Result<(), std::convert::Infallible> =
                        game.storage_transaction(|game, _| {
                            task(game);
                            Ok(())
                        });
                    return;
                };
                if let Err(panic) =
                    CallbackOwner::call_scheduled_task(mod_id, game, |game| task(game))
                {
                    report_failure(mod_id, &panic);
                    if let Some(serving) = &game.server.serving {
                        serving.reports().report_scheduled_task_failure(mod_id);
                    }
                }
            },
        )
    }

    pub(in crate::game) fn native_lifecycle_server_tick_start(&mut self, world_running: bool) {
        if let Some((context, phase)) = self.common.native_lifecycle.server_tick_start(world_running) {
            self.native_console_lines();
            self.common.native_lifecycle.read_scope = true;
            self.native_server_tick(&context, phase);
            // Tasks come due after the start phase, still inside the tick's read scope. Only
            // an unattributed engine panic resumes once the scope is restored.
            let panic = self.drain_due_tasks();
            self.common.native_lifecycle.read_scope = false;
            if let Some(payload) = panic {
                std::panic::resume_unwind(payload);
            }
        }
    }

    /// `schedule_task`: a delayed callable attributed to the current mod when called inside one.
    pub(in crate::game) fn schedule_task(
        &mut self,
        delay_ticks: u32,
        task: ServerTask,
    ) -> Result<TaskId, ScheduleError> {
        let owner = mod_api::content::crash::current_native_mod();
        if self.common.native_lifecycle.attempt.is_none_or(|attempt| {
            attempt.authority == Authority::RemoteClient || attempt.session == OpenState::Closing
        }) {
            return Err(ScheduleError::Unavailable);
        }
        self.common.native_lifecycle
            .tasks
            .schedule(owner, delay_ticks, task)
    }

    /// `cancel_task`: withdraws a waiting task by its opaque `TaskId`, disposing of its captures
    /// under the scheduling owner's panic policy when attributed.
    pub(in crate::game) fn cancel_task(&mut self, id: TaskId) -> bool {
        let Some(task) = self.common.native_lifecycle.tasks.cancel(id) else {
            return false;
        };
        dispose_pending_task(task);
        true
    }

    /// An errored tick returned at `result?` before this site.
    pub(in crate::game) fn native_lifecycle_server_tick_end(&mut self, paused: bool) {
        if let Some((context, phase)) = self.common.native_lifecycle.server_tick_end(paused) {
            self.common.native_lifecycle.read_scope = true;
            self.native_server_tick(&context, phase);
            // Drain tasks due this tick (including delay 0 scheduled during the tick body or end
            // hook), still inside the tick's read scope.
            let panic = self.drain_due_tasks();
            self.common.native_lifecycle.read_scope = false;
            if let Some(payload) = panic {
                std::panic::resume_unwind(payload);
            }
        }
    }
}

use sunrise_gameplay_common::game::transactions::TransactionsGameExt as _;

use sunrise_gameplay_common::game::native_lifecycle_common::NativeLifecycleCommonGameExt1 as _;
