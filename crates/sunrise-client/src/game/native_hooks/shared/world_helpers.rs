use super::*;

#[allow(dead_code)]
impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// This `Game` is the window's, not a server's.
    pub(in crate::game) fn native_window(&self) -> bool {
        self.options.execution_role == ExecutionRole::Client
    }
}
