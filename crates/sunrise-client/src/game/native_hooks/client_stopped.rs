use super::*;

/// The hook points. Each does nothing in Sunrise.
#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// The client shutdown has completed.
    pub(in crate::game) fn native_client_stopped(&mut self) {
        self.fire_client_stopped();
    }
}
