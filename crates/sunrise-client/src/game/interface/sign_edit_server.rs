#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game,
    commands::{Target, execute::Source},
};
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
    /// `ServerPlayer.openTextEdit` for the guest whose action opened a sign:
    /// the block, then the editor, on its client, and the guest may write on it.
    pub(in crate::game) fn lan_open_sign_editor(
        &mut self,
        conn: u32,
        edit: crate::game::server_side::acting_state::PendingSignEditor,
    ) {
        assert!(
            self.is_logical_server(),
            "lan_open_sign_editor called on Client role"
        );
        let pos = [edit.pos.0, edit.pos.1, edit.pos.2];
        let Some(guest_uuid) = self.lan.as_ref().and_then(|host| {
            let g = host.hub.guest(conn)?;
            (g.placed && g.participates_online() && g.within_block_range(pos, 4.0))
                .then_some(g.uuid)
        }) else {
            return;
        };
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get(&edit.pos) else {
            return;
        };
        if sign.waxed
            || sign
                .allowed_editor
                .is_some_and(|holder| holder != guest_uuid)
        {
            return;
        }
        self.world
            .block_entities
            .set_sign_editor(edit.pos, Some(guest_uuid));
        if let Some(host) = self.lan.as_mut() {
            host.hub.send_authoritative_block(conn, &self.world, pos);
            host.hub.send_to(
                conn,
                &C::OpenSignEditor(play::OpenSignEditor {
                    pos,
                    front: edit.front,
                }),
            );
        }
    }
    /// `ServerGamePacketListenerImpl.handleSignUpdate`: the lines, formatting
    /// codes stripped, go on the sign the guest may write on unless it is
    /// waxed (`SignBlockEntity.updateSignText`); a guest gone out of reach
    /// may no longer write (`playerIsTooFarAwayToEdit`).
    pub(in crate::game) fn lan_sign_update(&mut self, conn: u32, mut p: play::SignUpdate) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let Some(g) = host.hub.guest(conn) else {
            return;
        };
        let (uuid, name) = (g.uuid, g.name.clone());
        let key = (p.pos[0], p.pos[1], p.pos[2]);
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get(&key) else {
            return;
        };
        if sign.waxed || sign.allowed_editor != Some(uuid) {
            log::warn!("{name} just tried to change non-editable sign");
            return;
        }
        // Everything vanilla checks has passed: the mods hear what is about to
        // go on the sign, with the player's state swapped in, and may rewrite
        // or refuse it.
        if let Some((uuid, name, dimension)) = self
            .lan
            .as_ref()
            .and_then(|host| host.hub.guest(conn))
            .map(|g| (g.uuid, g.name.clone(), g.dimension.clone()))
        {
            let mut lines = p.lines.to_vec();
            let mut cancel = false;
            self.as_guest_messaged(conn, |game| {
                game.native_sign_changed(mod_api::__private::SignChangedCall {
                    player: uuid,
                    name: &name,
                    dimension: &dimension,
                    pos: p.pos,
                    front: p.front,
                    lines: &mut lines,
                    cancel: &mut cancel,
                });
            });
            if cancel {
                // The sign keeps its text: the editor closes, and the player's
                // client, which shows what it sent, gets the sign as it is.
                self.world.block_entities.set_sign_editor(key, None);
                self.mark_block_entity_updated(key);
                return;
            }
            lines.resize(4, String::new());
            for (line, new) in p.lines.iter_mut().zip(lines) {
                *line = new;
            }
        }
        if self
            .lan
            .as_ref()
            .and_then(|host| host.play.get(&conn))
            .is_none()
        {
            return;
        }
        // A hook may have broken the sign.
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get_mut(&key) else {
            return;
        };
        // A hook may have broken the sign.
        if sign.waxed {
            self.world.block_entities.set_sign_editor(key, None);
            return;
        }
        let side = if p.front {
            &mut sign.front
        } else {
            &mut sign.back
        };
        for (line, new) in side.lines.iter_mut().zip(&p.lines) {
            *line = sunrise_client::clipboard::strip_formatting(new);
        }
        self.world.block_entities.set_sign_editor(key, None);
        self.mark_block_entity_updated(key);
    }
    #[inline(always)]
    pub(in crate::game) fn open_sign_editor_server(&mut self, pos: (i32, i32, i32), front: bool) {
        if let (Some(player), Some(name)) = (
            self.server.lan_acting_uuid,
            self.server.lan_acting_name.clone(),
        ) {
            self.native_player_open_sign(player, &name, [pos.0, pos.1, pos.2], front);
        }
    }
    /// `SignBlock.otherPlayerIsEditingSign`: the sign at `key` belongs to
    /// someone else's editor right now.
    pub(in crate::game) fn other_player_is_editing_sign(&mut self, key: (i32, i32, i32)) -> bool {
        match self.world.block_entities.sign_editor(&key) {
            Some(holder) => holder != self.sign_editor_uuid(),
            None => false,
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
        self.server_send_overlay_message(
            sunrise_client::text::Text::translate("sign.click_actions_disabled", &[])
                .with_color(sunrise_client::text::RED),
        );
    }
    /// `SignBlockEntity.playerIsTooFarAwayToEdit`: a player who left is no
    /// player at all (`Level.getPlayerByUUID` finds nobody).
    pub(in crate::game::server_side) fn sign_editor_in_range(
        &self,
        who: u128,
        pos: (i32, i32, i32),
    ) -> bool {
        let at = [pos.0, pos.1, pos.2];
        let me = self
            .level
            .player_uuid
            .map(sunrise_client::lan::playerdata::ints_uuid);
        if me == Some(who) && self.player.is_some() {
            return self.eye_position().is_some_and(|eye| {
                sunrise_client::lan::guest::within_block_range(eye, self.reach() + 4.0, at)
            });
        }
        self.lan.as_ref().is_some_and(|host| {
            host.hub
                .guests
                .iter()
                .any(|g| g.uuid == who && g.within_block_range(at, 4.0))
        })
    }
    /// `Player.getUUID` of the acting player: the one a sign remembers as its
    /// editor.
    pub(in crate::game::server_side) fn sign_editor_uuid(&mut self) -> u128 {
        sunrise_client::lan::playerdata::ints_uuid(self.acting_player_uuid())
    }
    /// `SignText.hasEditableText` of the side the player faces: if every line is
    /// plain text, the editor rewrites nothing a player would miss.
    pub(in crate::game) fn sign_side_has_editable_text(
        &self,
        key: (i32, i32, i32),
        front: bool,
    ) -> bool {
        match self.world.block_entities.get(&key) {
            Some(BlockEntity::Sign(sign)) => {
                let side = if front { &sign.front } else { &sign.back };
                side.has_editable_text()
            }
            _ => false,
        }
    }
    /// `SignBlockEntity.tick` (`clearInvalidPlayerWhoMayEdit`): a sign keeps its
    /// editor only while that player is here and within the block interaction
    /// range plus four blocks of it.
    pub(in crate::game) fn tick_sign_editors(&mut self) {
        let held: Vec<(i32, i32, i32)> =
            self.world.block_entities.sign_editors().copied().collect();
        for pos in held {
            match self.world.block_entities.sign_editor(&pos) {
                Some(who) if self.sign_editor_in_range(who, pos) => {}
                Some(_) => self.world.block_entities.set_sign_editor(pos, None),
                // A sign that is gone, or one no editor holds.
                None => self.world.block_entities.forget_sign_editor(pos),
            }
        }
    }
}
