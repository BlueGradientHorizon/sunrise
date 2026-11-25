//! The sign editing screen (`SignEditScreen` / `HangingSignEditScreen`):
//! opened when a sign is placed, four lines limited to the sign's line
//! width, Up/Down/Enter move between lines, Escape or Done closes.
#[allow(unused_imports)]
use super::{
    Game,
    commands::{Target, execute::Source},
};
#[allow(unused_imports)]
use sunrise_client::{
    containers::BlockEntity,
    hud::{HudBatch, rgba},
    items::ItemStack,
    signs::{ClickAction, DYE_TEXT, click_action, editor_sprite},
    ui::EditBox,
};
#[allow(unused_imports)]
use sunrise_protocol::packets::play::{self, Clientbound as C, Serverbound as S};
#[allow(unused_imports)]
use winit::keyboard::KeyCode;

pub(in crate::game) struct SignEditor {
    pub pos: (i32, i32, i32),
    pub front: bool,
    pub line: usize,
    /// The line being edited (`AbstractSignEditScreen`'s text field).
    pub(in crate::game) edit: EditBox,
    /// `AbstractSignEditScreen.messages`: the side's lines as edited, sent
    /// when the editor closes, even if the sign is gone by then.
    pub(in crate::game) lines: [String; 4],
}

pub(in crate::game::client_side) fn sign_box() -> EditBox {
    let mut edit = EditBox::new(0.0, 0.0, 90.0, 9.0);
    edit.bordered = false;
    edit.max_length = 384;
    edit.focused = true;
    edit
}

/// The Done button's text, `CommonComponents.GUI_DONE`, in the selected language.
pub(in crate::game::client_side) fn done_label() -> String {
    super::screen_info::tr("gui.done", &[])
}
/// AbstractSignEditScreen.init's Done button: `width / 2 - 100`,
/// `height / 4 + 144`, int GUI pixels.
pub(in crate::game::client_side) fn done_button(w: f32, h: f32) -> (f32, f32) {
    (
        (w.ceil() / 2.0).floor() - 100.0,
        (h.ceil() / 4.0).floor() + 144.0,
    )
}

impl Game<'_> {
    /// `SignBlockEntity.getSlotPlayerIsFacing`: whether the player stands in
    /// front of the sign's text (within 90 degrees of its facing).
    pub(in crate::game::client_side) fn facing_sign_front(&self, pos: (i32, i32, i32)) -> bool {
        let Some(player) = self.player.as_ref() else {
            return true;
        };
        let id = self.world.block(pos.0, pos.1, pos.2);
        let Some(state) = self.pack.assets.registry.state(id) else {
            return true;
        };
        let form = self.res.facts().get(id).sign_form();
        let prop = |key: &str| state.properties.get(key).map(String::as_str);
        // getYRotationDegrees: a wall sign's `Direction.toYRot`, a standing or
        // ceiling sign's rotation segment.
        let y_rot = if form.is_some_and(|f| f.wall) {
            match prop("facing") {
                Some("west") => 90.0,
                Some("north") => 180.0,
                Some("east") => 270.0,
                _ => 0.0,
            }
        } else {
            prop("rotation")
                .and_then(|r| r.parse::<f32>().ok())
                .unwrap_or(0.0)
                * 22.5
        };
        // getSignHitboxCenterPosition: only a wall sign's board is off-centre,
        // against the wall it hangs on.
        let centre = if form.is_some_and(|f| f.wall && !f.hanging) {
            match y_rot as i32 {
                90 => (0.9375, 0.5),
                180 => (0.5, 0.9375),
                270 => (0.0625, 0.5),
                _ => (0.5, 0.0625),
            }
        } else {
            (0.5, 0.5)
        };
        let xd = player.position.x - (f64::from(pos.0) + centre.0);
        let zd = player.position.z - (f64::from(pos.2) + centre.1);
        let player_y_rot = (zd.atan2(xd).to_degrees() - 90.0) as f32;
        // Mth.degreesDifferenceAbs.
        let diff = (player_y_rot - y_rot).rem_euclid(360.0);
        let diff = if diff >= 180.0 { 360.0 - diff } else { diff };
        diff <= 90.0
    }
    /// `SignBlockEntity.canExecuteClickCommands` for the side the acting
    /// player faces: a waxed sign with a `run_command` click event on it.
    pub(in crate::game::client_side) fn sign_can_execute_click_commands(
        &self,
        pos: (i32, i32, i32),
    ) -> bool {
        // Only a waxed sign can; skip the facing maths for everything else.
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get(&pos) else {
            return false;
        };
        sign.waxed && sign.can_execute_click_commands(self.facing_sign_front(pos))
    }
    /// `WallHangingSignBlock.shouldTryToChainAnotherHangingSign` and
    /// `CeilingHangingSignBlock.shouldTryToChainAnotherHangingSign`: a hanging
    /// sign item clicked on a hanging sign's free side chains another sign onto
    /// it, unless the side the player faces carries click commands the click
    /// runs instead (`canExecuteClickCommands`).
    pub(in crate::game) fn chains_hanging_sign(
        &self,
        pos: [i32; 3],
        face: [i32; 3],
        held: Option<&ItemStack>,
    ) -> bool {
        // `itemStack.getItem() instanceof HangingSignItem`, by the item's class
        // and not its name.
        let hanging_sign = held.is_some_and(|s| {
            self.res
                .behaviours()
                .item_binding(&s.item)
                .is_some_and(|i| i.kind == sunrise_world::level::ItemKind::HangingSign)
        });
        let key = (pos[0], pos[1], pos[2]);
        if !hanging_sign {
            return false;
        }
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let Some(form) = self
            .res
            .facts()
            .get(state)
            .sign_form()
            .filter(|f| f.hanging)
        else {
            return false;
        };
        let free_side = if form.wall {
            // `isHittingEditableSide`: a wall hanging sign's board lies in its
            // FACING's plane, so its two faces are the editable text and only
            // its edges chain.
            let Some(facing) = self
                .pack
                .assets
                .registry
                .state(state)
                .and_then(|s| s.properties.get("facing"))
            else {
                return false;
            };
            let axis = match &**facing {
                "east" | "west" => 0,
                "up" | "down" => 1,
                _ => 2,
            };
            face.iter().position(|c| *c != 0) != Some(axis)
        } else {
            // `hitResult.getDirection().equals(Direction.DOWN)`.
            face == [0, -1, 0]
        };
        free_side && !self.sign_can_execute_click_commands(key)
    }
    /// `SignBlockEntity.createCommandSourceStack`: the acting player runs the
    /// command from the middle of the sign's block, rotation zero, at the
    /// game master level, with `CommandSource.NULL` (no feedback, no failure
    /// messages).
    pub(in crate::game) fn sign_command_source(&self, pos: [i32; 3]) -> Source {
        Source {
            entity: self.player.is_some().then_some(Target::Player),
            pos: pos.map(|v| f64::from(v) + 0.5),
            rot: [0.0, 0.0],
            eyes: false,
            silent: true,
            store: None,
            dimension: self.dim,
            origin_dimension: self.dim,
            origin_player: self.player.is_some(),
            level: None,
            block: None,
            cart: None,
        }
    }
    /// `SignBlockEntity.executeClickCommandsIfPresent`: every line of the
    /// side whose style has a `run_command`, `show_dialog` or `custom` click
    /// event runs it as the acting player when the sign allows op features;
    /// otherwise the player is told click actions are disabled. Whether any
    /// line had one.
    pub(in crate::game::client_side) fn execute_sign_click_commands(
        &mut self,
        pos: [i32; 3],
        front: bool,
    ) -> bool {
        let key = (pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get(&key) else {
            return false;
        };
        let allow_op_features = sign.allow_op_features;
        let side = if front { &sign.front } else { &sign.back };
        // The side as it was when the loop began: a command may change it.
        let events: Vec<_> = side.click.iter().flatten().cloned().collect();
        let mut any_click_command = false;
        for event in &events {
            match click_action(event) {
                Some(ClickAction::RunCommand(command)) => {
                    if allow_op_features {
                        self.run_sign_command(pos, &command);
                    }
                    any_click_command = true;
                }
                Some(ClickAction::ShowDialog(dialog)) => {
                    if allow_op_features {
                        self.show_click_dialog(dialog);
                    }
                    any_click_command = true;
                }
                // MinecraftServer.handleCustomClickAction only logs it.
                Some(ClickAction::Custom { id, payload }) => {
                    if allow_op_features {
                        log::debug!("Received custom click action {id} with payload {payload:?}");
                    }
                    any_click_command = true;
                }
                None => {}
            }
        }
        if !allow_op_features && any_click_command {
            self.sign_click_actions_disabled();
        }
        any_click_command
    }
    /// `SignBlock.useWithoutItem` (the block's `OpenMenu` effect): the side
    /// the player faces runs its click commands first; then a waxed sign
    /// plays the failed-interaction sound, a sign that ran one does nothing
    /// more, and otherwise a player who may build edits the side they face.
    pub(in crate::game) fn use_sign_empty_hand(&mut self, pos: [i32; 3]) {
        let key = (pos[0], pos[1], pos[2]);
        if !matches!(
            self.world.block_entities.get(&key),
            Some(BlockEntity::Sign(_))
        ) {
            return;
        }
        let front = self.facing_sign_front(key);
        let executed = self.execute_sign_click_commands(pos, front);
        // The commands may have changed the sign, or removed it.
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get(&key) else {
            return;
        };
        if sign.waxed {
            // `getSignInteractionFailedSoundEvent`: a hanging sign has its own.
            let sound = if sign.hanging {
                "minecraft:block.hanging_sign.waxed_interact_fail"
            } else {
                "minecraft:block.sign.waxed_interact_fail"
            };
            self.server_play_sound(sound, pos.map(|v| f64::from(v) + 0.5), 1.0, 1.0);
        } else if !executed
            && !self.other_player_is_editing_sign(key)
            && self.may_build()
            && self.sign_side_has_editable_text(key, front)
        {
            self.server_request_sign_editor(key, front);
        }
    }
    /// Whether `SignBlock.useWithoutItem` on the sign at `pos` consumes the
    /// click. Vanilla answers: a waxed sign (the failed-interaction sound), a
    /// side whose lines carry a click action (`executeClickCommandsIfPresent`,
    /// which runs it), and a player who may build (the text editor). Every
    /// other sign answers PASS, so `ServerPlayerGameMode.useItemOn` goes on to
    /// `ItemStack.useOn` and an adventure-mode `can_place_on` stack places
    /// against it. Vanilla also passes while another player is editing the
    /// sign and when its text has styled lines; neither is modelled here.
    pub(in crate::game) fn sign_consumes_click(&self, pos: [i32; 3]) -> bool {
        let key = (pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get(&key) else {
            return false;
        };
        if sign.waxed || self.may_build() {
            return true;
        }
        let side = if self.facing_sign_front(key) {
            &sign.front
        } else {
            &sign.back
        };
        side.click
            .iter()
            .flatten()
            .any(|event| click_action(event).is_some())
    }
    /// `SignBlock.useItemOn` with a `SignApplicator` in hand (dyes, ink sacs,
    /// honeycomb): the item changes the side the player faces and is used up.
    /// Whether it did; a sign it does not apply to goes on to the empty hand.
    pub(in crate::game) fn use_sign_applicator(
        &mut self,
        pos: [i32; 3],
        held: Option<&ItemStack>,
    ) -> bool {
        let Some(stack) = held else {
            return false;
        };
        let key = (pos[0], pos[1], pos[2]);
        let waxed = match self.world.block_entities.get(&key) {
            Some(BlockEntity::Sign(sign)) => sign.waxed,
            _ => return false,
        };
        if waxed || !self.may_build() {
            return false;
        }
        let item = &*stack.item;
        let dye = if item.ends_with("_dye") {
            self.res
                .items()
                .get(&stack.item)
                .and_then(|i| i.dye.clone())
        } else {
            None
        };
        let colour = dye.and_then(|d| DYE_TEXT.iter().position(|(n, _)| *n == d));
        if !matches!(
            item,
            "minecraft:honeycomb" | "minecraft:glow_ink_sac" | "minecraft:ink_sac"
        ) && colour.is_none()
        {
            return false;
        }
        // `SignBlock.useItemOn`: a sign another player's editor is on takes no
        // dye, ink sac or honeycomb.
        if self.other_player_is_editing_sign(key) {
            return false;
        }
        let front = self.facing_sign_front(key);
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get_mut(&key) else {
            return false;
        };
        let centre = pos.map(|v| f64::from(v) + 0.5);
        let sound = if item == "minecraft:honeycomb" {
            // HoneycombItem.tryApplyToSign: waxes any sign, empty or not.
            sign.waxed = true;
            "minecraft:item.honeycomb.wax_on"
        } else {
            let side = if front {
                &mut sign.front
            } else {
                &mut sign.back
            };
            // SignApplicator.canApplyToSign: the side must have text.
            if side.is_empty() {
                return false;
            }
            match colour {
                Some(c) if side.color != c as u8 => {
                    side.color = c as u8;
                    "minecraft:item.dye.use"
                }
                None if item == "minecraft:glow_ink_sac" && !side.glowing => {
                    side.glowing = true;
                    "minecraft:item.glow_ink_sac.use"
                }
                None if item == "minecraft:ink_sac" && side.glowing => {
                    side.glowing = false;
                    "minecraft:item.ink_sac.use"
                }
                _ => return false,
            }
        };
        if item == "minecraft:honeycomb" {
            self.level_event_at(3003, pos, 0);
        }
        self.server_play_sound(sound, centre, 1.0, 1.0);
        self.mark_block_entity_updated(key);
        self.execute_sign_click_commands(pos, front);
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.post_block_game_event(
            "minecraft:block_change",
            pos,
            self.sculk_actor(),
            Some(state),
        );
        self.award_stat("used", &stack.item, 1);
        self.consume_held();
        self.trigger_block("minecraft:item_used_on_block", pos, stack);
        true
    }
    /// `SignBlock.openTextEdit` for the front side after placement.
    pub(in crate::game) fn open_sign_editor(&mut self, pos: (i32, i32, i32), front: bool) {
        if self.fake_blocks(mod_api::FakeEffect::Menu) {
            return;
        }
        self.open_screen_overlay("sign_editor", |game| {
            // Minecraft.setScreen: the editor already open is removed first.
            game.close_sign_editor();
            let lines = match game.world.block_entities.get(&pos) {
                Some(BlockEntity::Sign(sign)) if front => sign.front.lines.clone(),
                Some(BlockEntity::Sign(sign)) => sign.back.lines.clone(),
                _ => Default::default(),
            };
            game.client.sign_edit = Some(SignEditor {
                pos,
                front,
                line: 0,
                edit: sign_box(),
                lines,
            });
            game.sign_load();
            game.clear_device_keys();
            game.set_mapping_held("key.use", false);
            game.capture(false);
        });
        self.open_sign_editor_server(pos, front);
    }
    /// `AbstractSignEditScreen.tick`: the editor closes (and `removed` sends
    /// the text) once its sign is removed or the player's eyes are further
    /// than 4 blocks beyond the block interaction range
    /// (`SignBlockEntity.playerIsTooFarAwayToEdit`).
    pub(in crate::game::client_side) fn tick_sign_editor(&mut self) {
        let Some(edit) = &self.client.sign_edit else {
            return;
        };
        let (x, y, z) = edit.pos;
        let near = self.eye_position().is_some_and(|eye| {
            sunrise_client::lan::guest::within_block_range(eye, self.reach() + 4.0, [x, y, z])
        });
        let sign = matches!(
            self.world.block_entities.get(&edit.pos),
            Some(BlockEntity::Sign(_))
        );
        if !sign || !near {
            self.close_sign_editor();
        }
    }
    pub(in crate::game::client_side) fn close_sign_editor(&mut self) {
        if self.remove_sign_editor() {
            self.refresh_screen_widgets();
        }
        self.capture(true);
    }

    /// `AbstractSignEditScreen.removed`: send what the client wrote, or
    /// keep it in the window's own world and send its guests the block update.
    pub(in crate::game::client_side) fn remove_sign_editor(&mut self) -> bool {
        let Some(edit) = self.client.sign_edit.take() else {
            return false;
        };
        if self.remote.is_some() {
            self.send(S::SignUpdate(play::SignUpdate {
                pos: [edit.pos.0, edit.pos.1, edit.pos.2],
                lines: edit.lines,
                front: edit.front,
            }));
        } else {
            self.mark_block_entity_updated(edit.pos);
        }
        // `SignBlockEntity.updateSignText`'s `setAllowedPlayerEditor(null)`:
        // the editor that closes gives the sign up.
        self.world.block_entities.set_sign_editor(edit.pos, None);
        true
    }
    /// `ClientPacketListener.handleOpenSignEditor`: the editor on a sign this
    /// client has.
    pub(in crate::game::client_side) fn remote_open_sign_editor(
        &mut self,
        p: play::OpenSignEditor,
    ) {
        let key = (p.pos[0], p.pos[1], p.pos[2]);
        if matches!(
            self.world.block_entities.get(&key),
            Some(BlockEntity::Sign(_))
        ) {
            self.open_sign_editor(key, p.front);
        } else {
            log::warn!(
                "ignoring openTextEdit on a block that is no sign at {:?}",
                p.pos
            );
        }
    }
    pub(in crate::game::client_side) fn sign_edit_key(
        &mut self,
        key: KeyCode,
        shortcut: Option<sunrise_client::clipboard::EditShortcut>,
    ) -> bool {
        if let Some(shortcut) = shortcut {
            self.sign_shortcut(shortcut);
            true
        } else if matches!(
            key,
            KeyCode::Escape
                | KeyCode::Enter
                | KeyCode::NumpadEnter
                | KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::Backspace
                | KeyCode::Delete
        ) {
            self.sign_input(Some(key), None);
            true
        } else {
            false
        }
    }
    /// A key and the text it typed on the sign being edited (the keyboard,
    /// or a controller's on-screen keyboard).
    pub(in crate::game::client_side) fn sign_input(
        &mut self,
        key: Option<KeyCode>,
        typed: Option<&str>,
    ) {
        if self.client.sign_edit.is_none() {
            return;
        }
        match key {
            Some(KeyCode::Escape) => self.close_sign_editor(),
            Some(KeyCode::ArrowUp) => self.sign_move_line(3),
            Some(KeyCode::ArrowDown | KeyCode::Enter | KeyCode::NumpadEnter) => {
                self.sign_move_line(1)
            }
            Some(
                key @ (KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::Backspace
                | KeyCode::Delete),
            ) => {
                let (ctrl, shift) = (self.has_control_down(), self.has_shift_down());
                self.sign_change(|edit| {
                    edit.key(key, ctrl, shift);
                });
            }
            _ => {
                let Some(typed) = typed else {
                    return;
                };
                self.sign_change(|edit| edit.insert_text(typed));
            }
        }
    }
    /// Up or down a line: the box writes the line it leaves and loads the next.
    pub(in crate::game::client_side) fn sign_move_line(&mut self, by: usize) {
        self.sign_write();
        if let Some(edit) = self.client.sign_edit.as_mut() {
            edit.line = (edit.line + by) & 3;
        }
        self.sign_load();
    }
    /// The edited line's text.
    pub(in crate::game::client_side) fn sign_current(&self) -> Option<String> {
        let edit = self.client.sign_edit.as_ref()?;
        let BlockEntity::Sign(sign) = self.world.block_entities.get(&edit.pos)? else {
            return None;
        };
        let side = if edit.front { &sign.front } else { &sign.back };
        Some(side.lines[edit.line].clone())
    }
    /// `EditBox.setValue` from the line now being edited.
    pub(in crate::game::client_side) fn sign_load(&mut self) {
        let Some(text) = self.sign_current() else {
            return;
        };
        if let Some(edit) = self.client.sign_edit.as_mut() {
            edit.edit.set_value(&text);
        }
    }
    /// The box's text onto the editor's lines and the sign
    /// (`AbstractSignEditScreen.setMessage`), when it still fits the line.
    pub(in crate::game::client_side) fn sign_write(&mut self) {
        let Some(edit) = self.client.sign_edit.as_ref() else {
            return;
        };
        let (pos, front, line, value) = (edit.pos, edit.front, edit.line, edit.edit.value.clone());
        if !self.sign_fits(&value) {
            return;
        }
        if let Some(edit) = self.client.sign_edit.as_mut() {
            edit.lines[line].clone_from(&value);
        }
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get_mut(&pos) else {
            return;
        };
        let side = if front {
            &mut sign.front
        } else {
            &mut sign.back
        };
        side.lines[line] = value;
    }
    /// TextFieldHelper: `font.width(s) <= maxTextLineWidth`.
    pub(in crate::game::client_side) fn sign_fits(&self, value: &str) -> bool {
        let Some(edit) = self.client.sign_edit.as_ref() else {
            return false;
        };
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get(&edit.pos) else {
            return false;
        };
        HudBatch::text_width(&self.gui.atlas, value) <= sign.max_line_width()
    }
    /// Edit the line, then keep the result only when it still fits.
    pub(in crate::game::client_side) fn sign_change(&mut self, f: impl FnOnce(&mut EditBox)) {
        let before = self
            .client
            .sign_edit
            .as_ref()
            .map(|e| (e.edit.value.clone(), e.edit.cursor, e.edit.highlight));
        if let Some(edit) = self.client.sign_edit.as_mut() {
            f(&mut edit.edit);
            let clean = sunrise_client::clipboard::strip_formatting(&edit.edit.value);
            if clean != edit.edit.value {
                let n = sunrise_client::ui::char_len(&clean);
                edit.edit.value = clean;
                edit.edit.cursor = edit.edit.cursor.min(n);
                edit.edit.highlight = edit.edit.highlight.min(n);
            }
        }
        self.sign_keep(before);
    }
    /// Write the box when the line still fits; otherwise put the old text back.
    pub(in crate::game::client_side) fn sign_keep(
        &mut self,
        before: Option<(String, usize, usize)>,
    ) {
        let value = self
            .client
            .sign_edit
            .as_ref()
            .map(|e| e.edit.value.clone())
            .unwrap_or_default();
        if self.sign_fits(&value) {
            self.sign_write();
        } else if let (Some(edit), Some((value, cursor, highlight))) =
            (self.client.sign_edit.as_mut(), before)
        {
            edit.edit.value = value;
            edit.edit.cursor = cursor;
            edit.edit.highlight = highlight;
        }
    }
    /// `TextFieldHelper.keyPressed`'s shortcuts on the edited line. Paste
    /// reads `getClipboardContents` before `insertText`. A paste that would
    /// pass the sign's width is refused whole.
    pub(in crate::game::client_side) fn sign_shortcut(
        &mut self,
        shortcut: sunrise_client::clipboard::EditShortcut,
    ) {
        if shortcut == sunrise_client::clipboard::EditShortcut::Paste {
            let raw = sunrise_client::clipboard::Clipboard::get(&mut self.clipboard.0);
            let text = sunrise_client::clipboard::helper_clipboard_contents(&raw);
            self.sign_change(|edit| edit.insert_text(&text));
            return;
        }
        let before = self
            .client
            .sign_edit
            .as_ref()
            .map(|e| (e.edit.value.clone(), e.edit.cursor, e.edit.highlight));
        if let Some(edit) = self.client.sign_edit.as_mut() {
            edit.edit.shortcut(shortcut, &mut self.clipboard.0);
            let clean = sunrise_client::clipboard::strip_formatting(&edit.edit.value);
            if clean != edit.edit.value {
                let n = sunrise_client::ui::char_len(&clean);
                edit.edit.value = clean;
                edit.edit.cursor = edit.edit.cursor.min(n);
                edit.edit.highlight = edit.edit.highlight.min(n);
            }
        }
        self.sign_keep(before);
    }
    /// Done button (`this.width / 2 - 100, this.height / 4 + 144`, 200x20),
    /// or a click on a line (`EditBox.onClick`).
    pub(in crate::game::client_side) fn sign_edit_click(&mut self, size: [u32; 2]) -> bool {
        let b = HudBatch::new(size, self.settings.gui_scale_inputs());
        let (x, y) = done_button(b.width, b.height);
        let (mx, my) = self.client.cursor;
        if mx >= x && mx < x + 200.0 && my >= y && my < y + 20.0 {
            self.audio.0.play_button_click();
            self.close_sign_editor();
            return true;
        }
        let Some((cx, top, line_height, max_width, lines)) = self.sign_lines(b.width) else {
            return false;
        };
        let shift = self.has_shift_down();
        for i in 0..lines.len() {
            let ly = top + i as f32 * line_height;
            let left = cx - max_width / 2.0;
            if mx < left || mx >= left + max_width || my < ly || my >= ly + line_height {
                continue;
            }
            let current = self.client.sign_edit.as_ref().map(|e| e.line);
            if current != Some(i) {
                self.sign_move_line((i + 4 - current.unwrap_or(0)) & 3);
            }
            let value = self
                .client
                .sign_edit
                .as_ref()
                .map(|e| e.edit.value.clone())
                .unwrap_or_default();
            let lw = HudBatch::text_width(&self.gui.atlas, &value);
            let text_x = cx - (lw as i32 / 2) as f32;
            if let Some(edit) = self.client.sign_edit.as_mut() {
                edit.edit.x = text_x;
                edit.edit.y = ly;
                edit.edit.width = max_width;
                edit.edit.click(&self.gui.atlas, mx, shift, false);
            }
            self.sign_write();
            return true;
        }
        false
    }
    /// Centre x, the first line's y, the line height, the width limit and the four lines.
    pub(in crate::game::client_side) fn sign_lines(
        &self,
        width: f32,
    ) -> Option<(f32, f32, f32, f32, Vec<String>)> {
        let edit = self.client.sign_edit.as_ref()?;
        let BlockEntity::Sign(sign) = self.world.block_entities.get(&edit.pos)? else {
            return None;
        };
        let text = if edit.front { &sign.front } else { &sign.back };
        let line_height = sign.line_height();
        let y = if sign.hanging { 125.0 } else { 90.0 };
        let top = y - (4.0 * line_height / 2.0).floor();
        Some((
            width / 2.0,
            top,
            line_height,
            sign.max_line_width(),
            text.lines.to_vec(),
        ))
    }
    pub(in crate::game::client_side) fn draw_sign_editor(&self, batch: &mut HudBatch) {
        let Some(edit) = &self.client.sign_edit else {
            return;
        };
        let Some(BlockEntity::Sign(sign)) = self.world.block_entities.get(&edit.pos) else {
            return;
        };
        self.render_transparent_background("sign_editor", batch);
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width, batch.height);
        let (key, fallback) = if sign.hanging {
            ("hanging_sign.edit", "Edit Hanging Sign Message")
        } else {
            ("sign.edit", "Edit Sign Message")
        };
        let title = self.translate_with_fallback(key, fallback);
        // SignEditScreen / HangingSignEditScreen: their title.
        sunrise_client::widgets::begin_screen(|| {
            sunrise_client::widgets::ScreenInfo::new("sign_edit", title.clone())
        });
        let tw = HudBatch::text_width(atlas, &title);
        batch.text(atlas, &title, (w - tw) / 2.0, 40.0, u32::MAX, true);
        let facts = self.res.facts();
        let block = facts.get(self.world.block(edit.pos.0, edit.pos.1, edit.pos.2));
        let wood = facts.wood_type(&block.name).unwrap_or("oak");
        let cx = w / 2.0;
        let y = if sign.hanging {
            // HangingSignEditScreen: 16x16 texture scaled 4.5 at (0, -13).
            let s = 4.5;
            batch.sprite_scaled(
                atlas,
                &editor_sprite("hanging_signs", wood),
                cx - 8.0 * s,
                125.0 - 13.0 - 8.0 * s,
                16.0 * s,
                16.0 * s,
                u32::MAX,
            );
            125.0
        } else {
            // SignEditScreen: 24x26 texture scaled 3.9 at (0, 27); wall signs show 12 rows.
            let s = 3.9;
            let wall = block.sign_form().is_some_and(|f| f.wall);
            let rows = if wall { 12.0 } else { 26.0 };
            if let Some(mut r) = atlas.sprites.get(&editor_sprite("signs", wood)).copied() {
                r.v1 = r.v0 + (r.v1 - r.v0) * rows / 26.0;
                batch.rect_uv(
                    cx - 12.0 * s,
                    90.0 + 27.0 - 13.0 * s,
                    24.0 * s,
                    rows * s,
                    r,
                    u32::MAX,
                );
            }
            90.0
        };
        let text = if edit.front { &sign.front } else { &sign.back };
        let color = if text.glowing {
            text.text_color()
        } else {
            text.dark_color()
        };
        let color = rgba((color >> 16) as u8, (color >> 8) as u8, color as u8, 255);
        let line_height = sign.line_height();
        let midpoint = (4.0 * line_height / 2.0).floor();
        for (i, line) in text.lines.iter().enumerate() {
            let shown = if i == edit.line {
                edit.edit.value.as_str()
            } else {
                line.as_str()
            };
            let lw = HudBatch::text_width(atlas, shown);
            let x = cx - (lw as i32 / 2) as f32;
            let ly = y + i as f32 * line_height - midpoint;
            batch.text(atlas, shown, x, ly, color, false);
            if i != edit.line {
                continue;
            }
            let value = &edit.edit.value;
            if edit.edit.cursor != edit.edit.highlight {
                let (a, b) = (
                    edit.edit.cursor.min(edit.edit.highlight),
                    edit.edit.cursor.max(edit.edit.highlight),
                );
                let pre: String = value.chars().take(a).collect();
                let sel: String = value.chars().skip(a).take(b - a).collect();
                batch.text_highlight(
                    atlas,
                    (
                        x + HudBatch::text_width(atlas, &pre),
                        ly,
                        HudBatch::text_width(atlas, &sel),
                        line_height,
                    ),
                    true,
                );
            }
            let pre: String = value.chars().take(edit.edit.cursor).collect();
            let caret_x = x + HudBatch::text_width(atlas, &pre);
            if edit.edit.cursor_visible() {
                batch.fill(atlas, caret_x, ly, 1.0, line_height, color);
            }
            sunrise_client::preedit::report_caret(caret_x, ly, line_height);
        }
        let (bx, by) = done_button(w, h);
        sunrise_client::widgets::declare_action(
            sunrise_client::widgets::Action::Done,
            [bx, by, 200.0, 20.0],
            true,
        );
        sunrise_client::widgets::narratable(|| {
            sunrise_client::widgets::Narratable::button(done_label(), [bx, by, 200.0, 20.0], true)
        });
        batch.sprite_scaled(atlas, "widget/button", bx, by, 200.0, 20.0, u32::MAX);
        let done = done_label();
        let tw = HudBatch::text_width(atlas, &done);
        batch.text(
            atlas,
            &done,
            bx + 100.0 - (tw / 2.0).floor(),
            by + 6.0,
            u32::MAX,
            true,
        );
        self.draw_overlay_mounted_widgets(batch, "sign_editor");
    }
}

#[cfg(test)]
#[path = "sign_edit/sign_edit_tests.rs"]
pub(in crate::game) mod sign_edit_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[test]
    fn done_button_sits_on_whole_gui_pixels() {
        // `width / 2 - 100, height / 4 + 144` in ints: 334 high is 83 + 144.
        assert_eq!(super::done_button(427.0, 334.0), (113.0, 227.0));
        assert_eq!(super::done_button(320.0, 240.0), (60.0, 204.0));
    }

    /// The Done button said "Done" in every language (#1927).
    #[test]
    fn done_button_reads_the_selected_language() {
        use sunrise_client::lang::{Language, ResourceSource, scoped};
        struct Vanilla;
        impl ResourceSource for Vanilla {
            fn read(&self, path: &str) -> Option<Vec<u8>> {
                match path {
                    "assets/minecraft/lang/en_us.json" => Some(br#"{"gui.done": "Done"}"#.to_vec()),
                    "assets/minecraft/lang/de_de.json" => {
                        Some(br#"{"gui.done": "Fertig"}"#.to_vec())
                    }
                    _ => None,
                }
            }
        }
        for (code, want) in [("de_de", "Fertig"), ("en_us", "Done")] {
            let (label, missed) = scoped(Language::load(&[&Vanilla], code), super::done_label);
            assert_eq!(label, want, "{code}");
            assert!(missed.is_empty(), "{code}: {missed:?}");
        }
    }
}
