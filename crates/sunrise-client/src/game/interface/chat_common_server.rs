use super::Game;
use crate::game::chat_common::owner_permission;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `IntegratedServer.setWorldAllowCommands`, then `updatePermissions`.
    pub(in crate::game) fn set_allow_commands(&mut self, allow: bool) {
        self.level.allow_commands = Some(allow);
        self.ui.registries.permission = owner_permission(allow);
    }
}
