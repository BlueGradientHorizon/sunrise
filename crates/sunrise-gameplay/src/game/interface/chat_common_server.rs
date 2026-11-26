use crate::game::ServerGame;
use sunrise_gameplay_common::game::chat_common::owner_permission;

impl ServerGame<'_> {
    /// `IntegratedServer.setWorldAllowCommands`, then `updatePermissions`.
    pub(in crate::game) fn set_allow_commands(&mut self, allow: bool) {
        self.common.level.allow_commands = Some(allow);
        self.common.ui.registries.permission = owner_permission(allow);
    }
}
