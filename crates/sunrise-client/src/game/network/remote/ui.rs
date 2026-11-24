#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// The inventory key when the vehicle owns its screen.
    pub(in crate::game) fn remote_request_mount_inventory(&self) {
        if let Some(r) = &self.remote {
            self.send(S::PlayerCommand(play::PlayerCommand {
                id: r.entity_id,
                action: 5,
                data: 0,
            }));
        }
    }
}
