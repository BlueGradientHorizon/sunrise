//! Delayed server tasks (`ModStateServerHost::schedule_task`), modeled on vanilla's
//! `MinecraftServer.schedule(TickTask)` taking a callable.

// no wire: only the running world's scheduler can use its cancellation ids.
/// A scheduled task's opaque capability id: unique across worlds and never reused.
/// Holding it suffices to cancel the task while it is still waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustive: an opaque id the game builds and mods only compare and hand back
pub struct TaskId(u64);

impl TaskId {
    pub(crate) const fn of(id: u64) -> Self {
        Self(id)
    }

    pub(crate) const fn raw(self) -> u64 {
        self.0
    }
}

// no wire: only returned to the caller of the process-local scheduler.
/// Why a task was not scheduled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScheduleError {
    /// The scheduling mod already has `MAX_PENDING_TASKS` tasks waiting; a warning naming the mod
    /// is logged.
    TooMany,
    /// The current phase disallows world access, or the server world is not open or opening.
    Unavailable,
}

impl crate::AccessRefusal for ScheduleError {
    fn unavailable() -> Self {
        Self::Unavailable
    }
}

/// How many tasks one mod may have waiting at once.
pub const MAX_PENDING_TASKS: usize = 4096;
