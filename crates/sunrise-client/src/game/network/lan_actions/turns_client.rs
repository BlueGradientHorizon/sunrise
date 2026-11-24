#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::turns::CombatPlayer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::turns::DisplacedPlayer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::turns::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::stats::Stats;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{actions::Action, lan::guest::Guest};

impl Game<'_> {
    /// `LivingEntity.playHurtSound` for a guest: the server's `Entity.playSound`
    /// reaches every player around but the hurt one, whose own client plays it
    /// from `handleDamageEvent` (#4149).
    pub(in crate::game) fn guest_hurt_sound(&mut self, conn: u32, sound: &str) {
        let Some(at) = self
            .lan
            .as_ref()
            .and_then(|host| host.hub.guest(conn))
            .map(|g| g.position)
        else {
            return;
        };
        self.audio.0.play_unshared(sound, Some(at), 1.0, 1.0);
        self.send_level_sound(LevelSound::new(sound, at, 1.0, 1.0), Some(conn));
    }
    #[inline(always)]
    pub(in crate::game) fn guest_turn_client_enter_input(
        &mut self,
        sneaking: bool,
    ) -> (
        sunrise_client::held::HeldKeys,
        sunrise_client::keymapping::VanillaMappings,
    ) {
        let host_keys = std::mem::take(&mut self.client.keys);
        let saved_mappings = std::mem::take(&mut self.client.vanilla_mappings);
        if sneaking {
            self.insert_device_key(KeyCode::ShiftLeft);
        }
        (host_keys, saved_mappings)
    }
    #[inline(always)]
    pub(in crate::game) fn guest_turn_client_enter_screens(
        &mut self,
    ) -> (
        Screen,
        Option<crate::game::sign_edit::SignEditor>,
        Option<crate::game::books::BookScreen>,
        Option<sunrise_client_gameplay::command_block_screen::CommandBlockScreen>,
        crate::game::mount_menu::MountMenu,
    ) {
        let screen = std::mem::replace(&mut self.client.screen, Screen::Playing);
        let host_sign = self.client.sign_edit.take();
        let host_book = self.client.book.take();
        let host_command_block = self.command_block_screen.0.take();
        let host_mount_menu = std::mem::take(&mut self.client.mount_menu);
        (
            screen,
            host_sign,
            host_book,
            host_command_block,
            host_mount_menu,
        )
    }
    #[inline(always)]
    pub(in crate::game) fn guest_turn_client_leave_input(
        &mut self,
        host_keys: sunrise_client::held::HeldKeys,
        saved_mappings: sunrise_client::keymapping::VanillaMappings,
    ) {
        self.client.keys = host_keys;
        self.client.vanilla_mappings = saved_mappings;
    }
    #[inline(always)]
    pub(in crate::game) fn guest_turn_client_leave_screens(
        &mut self,
        screen: Screen,
        host_sign: Option<crate::game::sign_edit::SignEditor>,
        host_book: Option<crate::game::books::BookScreen>,
        host_command_block: Option<
            sunrise_client_gameplay::command_block_screen::CommandBlockScreen,
        >,
        host_mount_menu: crate::game::mount_menu::MountMenu,
    ) -> (
        Option<crate::game::sign_edit::SignEditor>,
        Option<(i32, i32, i32)>,
        Option<(i32, i32, i32)>,
        Option<u32>,
    ) {
        self.client.screen = screen;
        // A sign the guest's placement opened (SignBlock.openTextEdit).
        let guest_sign = std::mem::replace(&mut self.client.sign_edit, host_sign);
        // A lectern the guest's use opened (LecternBlock.openScreen).
        let guest_lectern = match std::mem::replace(&mut self.client.book, host_book) {
            Some(crate::game::books::BookScreen::View {
                lectern: Some(key), ..
            }) => Some(key),
            _ => None,
        };
        // A command block the guest's use opened (Player.openCommandBlock).
        let guest_command_block =
            std::mem::replace(&mut self.command_block_screen.0, host_command_block)
                .map(|s| s.pos());
        // A mount inventory the guest's action opened (shift + use, or the
        // inventory key while riding).
        let guest_mount =
            std::mem::replace(&mut self.client.mount_menu, host_mount_menu).opened_mob();
        (guest_sign, guest_lectern, guest_command_block, guest_mount)
    }
    #[inline(always)]
    pub(in crate::game) fn guest_turn_client_replace_message(
        &mut self,
        message: Option<(String, std::time::Instant)>,
    ) -> Option<(String, std::time::Instant)> {
        std::mem::replace(&mut self.client.message, message)
    }
}
