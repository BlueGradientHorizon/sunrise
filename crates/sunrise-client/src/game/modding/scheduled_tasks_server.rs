#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::scheduled_tasks::PendingTask;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::scheduled_tasks::ScheduledTasks;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::scheduled_tasks::ServerTask;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::scheduled_tasks::dispose_pending_task;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::scheduled_tasks::report_failure;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::scheduled_tasks::run_all;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::scheduled_tasks::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    __private::game::{CallbackOwner, allocate_task_id},
    MAX_PENDING_TASKS, ScheduleError, ServerHost, TaskId,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    collections::{BTreeMap, HashMap},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

impl ScheduledTasks {
    /// A server tick starts: "now" moves to it before any hook of the tick runs, so tasks
    /// scheduled during this tick are measured from it (`delay_ticks == 0` is due in this tick).
    pub(in crate::game) fn begin_tick(&mut self) {
        self.now += 1;
    }
}
