#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game,
    commands::{Target, execute::Source},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sign_edit::SignEditor;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sign_edit::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    containers::BlockEntity,
    hud::{HudBatch, rgba},
    items::ItemStack,
    signs::{ClickAction, DYE_TEXT, click_action, editor_sprite},
    ui::EditBox,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::{
    self, Clientbound as C, Serverbound as S,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::keyboard::KeyCode;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn open_sign_editor_server(&mut self, pos: (i32, i32, i32), front: bool) {
        if let (Some(player), Some(name)) = (
            self.server.lan_acting_uuid,
            self.server.lan_acting_name.clone(),
        ) {
            self.native_player_open_sign(player, &name, [pos.0, pos.1, pos.2], front);
        }
    }
    /// Run a sign's `run_command` line as the prefixed command vanilla runs
    /// it as: a leading slash is allowed, errors are not reported (the NULL
    /// source).
    pub(in crate::game) fn run_sign_command(&mut self, pos: [i32; 3], command: &str) {
        let source = self.sign_command_source(pos);
        let line = command.strip_prefix('/').unwrap_or(command);
        self.exec.0 = Default::default();
        let permission = std::mem::replace(&mut self.ui.registries.permission, 2);
        let _ = self.execute_line(line, source);
        self.ui.registries.permission = permission;
        // InventoryChangeTrigger fires as slots change, not on the next tick.
        if self.player.is_some() {
            self.check_inventory_trigger();
        }
    }
    /// `Player.sendOverlayMessage(CLICK_ACTIONS_DISABLED)`: red text on the
    /// action bar. A guest gets a packet, so its own client translates it.
    pub(in crate::game) fn sign_click_actions_disabled(&mut self) {
        const KEY: &str = "sign.click_actions_disabled";
        if self.server.lan_acting.is_some() {
            let message = sunrise_client::text::Text::translate(KEY, &[])
                .with_color(sunrise_client::text::RED);
            self.server
                .guest_packets
                .push(C::SystemChat(play::SystemChat {
                    content: sunrise_client::lan::data::component_nbt(&message.component()),
                    overlay: true,
                }));
        } else {
            let message =
                self.translate_with_fallback(KEY, "Click actions are disabled for this sign");
            self.set_client_message(message);
        }
    }
}
