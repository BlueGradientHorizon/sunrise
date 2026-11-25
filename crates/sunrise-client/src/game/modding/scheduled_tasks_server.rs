use crate::game::scheduled_tasks::ScheduledTasks;

impl ScheduledTasks {
    /// A server tick starts: "now" moves to it before any hook of the tick runs, so tasks
    /// scheduled during this tick are measured from it (`delay_ticks == 0` is due in this tick).
    pub(in crate::game) fn begin_tick(&mut self) {
        self.now += 1;
    }
}
