use super::{Game, Screen};
use crate::game::game_mode_common::inventory_after_infinite_materials;
use sunrise_client::chat::{ChatAbilities, ChatVisibility};
use sunrise_command::args::GameMode;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `Minecraft.computeChatAbilities` from the Chat option.
    pub(in crate::game) fn chat_abilities(&self) -> ChatAbilities {
        ChatAbilities::new(ChatVisibility::from_option(
            self.side.settings.chat_visibility,
        ))
    }
    /// `MultiPlayerGameMode.setLocalMode(GameType)`: updates only the local mode and `Abilities`.
    pub(in crate::game) fn set_local_mode(&mut self, mode: GameMode) {
        if mode != self.mode {
            self.previous_mode = Some(self.mode);
        }
        self.mode = mode;
        self.update_world_options_personal_mode(mode.id());
        if let (Some(r), Some(p)) = (self.side.remote.as_mut(), self.player.as_ref()) {
            r.sync_body_flight(p.flying, p.flying_speed);
        }
        self.finish_local_mode_update(mode);
    }
    /// `MultiPlayerGameMode.setLocalMode(GameType, @Nullable GameType)` for `Login` and `Respawn`.
    pub(in crate::game::client_side) fn set_local_mode_with_previous(
        &mut self,
        mode: GameMode,
        previous: Option<GameMode>,
    ) {
        self.mode = mode;
        self.previous_mode = previous;
        self.finish_local_mode_update(mode);
    }
    pub(in crate::game::client_side) fn finish_local_mode_update(&mut self, mode: GameMode) {
        let r = self
            .side
            .remote
            .as_mut()
            .expect("set_local_mode requires a remote client");
        self.creative = r.update_local_mode_abilities(mode.id());
        self.apply_remote_abilities();
        let local_spectator = self.spectator();
        if let Some(player) = self.player.as_mut() {
            player.local_spectator = local_spectator;
        }
    }
    /// Whether `Screen::Inventory` currently shows the player's own `InventoryScreen`
    /// rather than a container, crafting table, workstation, mount, or server/mod menu.
    pub(in crate::game) fn is_player_inventory_screen(&self) -> bool {
        self.side.screen == Screen::Inventory
            && self.container.is_none()
            && self.station.is_none()
            && self.craft_size == 2
            && !self.mount_active()
            && self.side.mod_screen.0.is_none()
            && self.remote_menu_type().is_none()
    }
    /// Reconcile an open player inventory on its screen tick (`InventoryScreen.containerTick`
    /// and `CreativeModeInventoryScreen.containerTick`) against live `instabuild`, keeping
    /// the shared cursor (`carried`) stack across the switch.
    pub(in crate::game::client_side) fn tick_inventory_screen(&mut self) {
        if self.player.is_none() {
            return;
        }
        let next = inventory_after_infinite_materials(
            self.side.screen,
            self.is_player_inventory_screen(),
            self.instabuild(),
        );
        if next == self.side.screen {
            return;
        }
        match next {
            Screen::Creative => {
                self.set_screen(Screen::Creative);
                if self.side.screen == Screen::Creative {
                    self.refresh_creative_tabs();
                    self.side.creative_screen.0.reopen();
                    self.refresh_saved_hotbars();
                }
            }
            Screen::Inventory => {
                self.set_screen(Screen::Inventory);
            }
            _ => {}
        }
    }
}
