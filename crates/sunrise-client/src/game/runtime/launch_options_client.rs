use super::*;
use crate::game::launch_options::Options;

impl Options {
    /// Whether this `Game` is the process's client.
    pub(in crate::game) fn is_client_start(&self) -> bool {
        self.execution_role.is_client()
    }
    /// Explicit timeouts apply to every run; only frame-limited runs get a default.
    pub(in crate::game) fn time_limit(&self) -> Option<Duration> {
        self.timeout
            .or(self.frames.map(|_| Duration::from_secs(120)))
    }
}
