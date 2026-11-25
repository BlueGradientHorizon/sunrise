//! Command blocks: `CommandBlock`'s placement (`setPlacedBy`), its scheduled
//! tick (`execute`, `executeChain`), `BaseCommandBlock.performCommand`
//! through the command system with the block's output, and
//! `CommandBlockEditScreen` with what `handleSetCommandBlock` does with it.
#[allow(unused_imports)]
use super::{Game, Screen, commands::execute::Source};
#[allow(unused_imports)]
use sunrise_client::{
    chat::Commands,
    command_block::{self, CommandBlock, Mode},
    containers::BlockEntity,
    hud::{HudBatch, rgba},
    items::ItemStack,
    screen_draw::Measure,
    text::Text,
    ui,
};
#[allow(unused_imports)]
use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
use sunrise_protocol::packets::play::{self, Clientbound as C, Serverbound as S};
#[allow(unused_imports)]
use winit::keyboard::KeyCode;
#[allow(unused_imports)]
use {
    sunrise_core::nbt::Tag,
    sunrise_world::{chunk::BlockState, level::flags},
};

#[allow(unused_imports)]
pub(in crate::game) use sunrise_client_gameplay::command_block_screen::{
    Button, CommandBlockScreen, previous_y,
};
#[allow(unused_imports)]
pub(in crate::game) use sunrise_gameplay::command_blocks::{
    CommandBlockEdit, CommandBlockTransition, analog_signal, command_block_mut, conditional,
    facing_step, facing_yaw, mark_condition_met, offset, set_powered, state_of,
};

pub(in crate::game) use super::command_blocks_common::Pos;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `CommandBlock.useWithoutItem` on a server's client, which opens the
    /// screen itself (`Player.openCommandBlock`) while the server only
    /// answers the click: the block at `pos`, when it is a command block the
    /// player may edit.
    pub(in crate::game::client_side) fn open_command_block_if_editable(&mut self, pos: Pos) {
        let is_command_block = self
            .block_state_at(pos)
            .is_some_and(|state| Mode::of_block(&state.name).is_some());
        if is_command_block && self.can_use_game_master_blocks() {
            self.open_command_block(pos);
        }
    }
    /// The `BaseCommandBlock` an edit screen shows: the block entity's or the
    /// minecart's.
    pub(in crate::game::client_side) fn edit_base(
        &self,
        s: &CommandBlockScreen,
    ) -> Option<&CommandBlock> {
        match s.cart {
            Some(id) => self.carts.iter().find(|c| c.id == id).map(|c| &c.command),
            None => self.command_block(s.pos),
        }
    }
    #[inline(always)]
    pub(in crate::game::client_side) fn command_block_open_at(&self, pos: Pos) -> bool {
        self.side.command_block_screen
            .0
            .as_ref()
            .is_some_and(|s| s.cart.is_none() && s.pos == pos)
    }
    /// `ClientPacketListener.handleBlockEntityData` then
    /// `CommandBlockEditScreen.updateGui`: new data for the command block the
    /// edit screen shows refreshes its fields. The mode and conditional live in
    /// the block state here (vanilla's `CommandBlockEntity.Mode`).
    pub(in crate::game::client_side) fn command_block_data_arrived(&mut self, pos: Pos) {
        if self
            .side.command_block_screen
            .0
            .as_ref()
            .is_none_or(|s| s.cart.is_some() || s.pos != pos)
        {
            return;
        }
        let state = self.block_state_at(pos).map(|s| {
            (
                Mode::of_block(&s.name).unwrap_or(Mode::Redstone),
                conditional(s),
            )
        });
        let block = self.command_block(pos).cloned();
        let m = Measure(&self.gui.atlas.font);
        let c = Commands {
            dispatcher: super::chat::chat_tree(&self.remote),
            registries: &self.ui.registries,
        };
        let Some(screen) = self.side.command_block_screen.0.as_mut() else {
            return;
        };
        if let Some((mode, conditional)) = state {
            screen.mode = mode;
            screen.conditional = conditional;
        }
        if let Some(block) = block {
            screen.command.suggest_command(&m, &c, &block.command);
            screen.track_output = block.track_output;
            screen.auto = block.auto;
            screen.show_previous(&block);
        }
    }

    /// `LocalPlayer.openCommandBlock`.
    pub(in crate::game::client_side) fn open_command_block(&mut self, pos: Pos) {
        let Some(block) = self.command_block(pos).cloned() else {
            return;
        };
        let Some(state) = self.block_state_at(pos) else {
            return;
        };
        let mode = Mode::of_block(&state.name).unwrap_or(Mode::Redstone);
        self.open_edit_screen(pos, None, block, mode, conditional(state));
    }
    /// The screen over `block`: a command block's at `pos`, or a minecart's.
    pub(in crate::game) fn open_edit_screen(
        &mut self,
        pos: Pos,
        cart: Option<u32>,
        block: CommandBlock,
        mode: Mode,
        conditional: bool,
    ) {
        let (w, h) = self.gui_size();
        let m = Measure(&self.gui.atlas.font);
        let c = Commands {
            dispatcher: super::chat::chat_tree(&self.remote),
            registries: &self.ui.registries,
        };
        let screen = CommandBlockScreen::new(
            &m,
            &c,
            (w, h),
            (pos, cart),
            &block,
            (mode, conditional),
            self.settings.auto_suggestions,
            std::time::Instant::now(),
        );
        self.side.command_block_screen.0 = Some(screen);
        self.set_screen(Screen::CommandBlock);
        self.capture(false);
    }
    pub(in crate::game::client_side) fn close_command_block(&mut self) {
        self.side.command_block_screen.0 = None;
        self.set_screen(Screen::Playing);
        self.capture(true);
    }
    pub(in crate::game::client_side) fn draw_command_block(&mut self, batch: &mut HudBatch) {
        let Some(s) = self.side.command_block_screen.0.as_ref() else {
            self.draw_screen_phase(batch, sunrise_client::screen_draw::RenderPhase::Background);
            self.draw_screen_phase(batch, sunrise_client::screen_draw::RenderPhase::Foreground);
            return;
        };
        // AbstractCommandBlockEditScreen.tick: the block (or the cart) is gone.
        if self.edit_base(s).is_none() {
            self.draw_screen_phase(batch, sunrise_client::screen_draw::RenderPhase::Background);
            self.draw_screen_phase(batch, sunrise_client::screen_draw::RenderPhase::Foreground);
            self.close_command_block();
            return;
        }
        // AbstractCommandBlockEditScreen: `GameNarrator.NO_TITLE`; its usage
        // is the suggestions' while they show (`getUsageNarration`).
        let usage = s.command.suggestions_usage();
        sunrise_client::widgets::begin_screen(|| {
            let info = sunrise_client::widgets::ScreenInfo::new("command_block", "");
            match usage {
                Some(key) => info.usage(super::screen_info::tr(key, &[])),
                None => info,
            }
        });
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width, batch.height);
        let mid = (w / 2.0).floor();
        // Screen.extractBackground of an isInGameUi screen: the dimmed world,
        // with no blur and no menu background.
        batch.fill_gradient(
            atlas,
            (0.0, 0.0, w, h),
            rgba(0x10, 0x10, 0x10, 0xC0),
            rgba(0x10, 0x10, 0x10, 0xD0),
        );
        self.draw_deferred_subtitles(batch);
        self.draw_screen_phase(batch, sunrise_client::screen_draw::RenderPhase::Background);
        let title = Text::translate("advMode.setCommand", &[]);
        let tw = atlas.font.width_text(&title);
        batch.text_styled(
            atlas,
            &title,
            (mid - tw / 2.0).floor(),
            20.0,
            u32::MAX,
            true,
        );
        let grey = rgba(160, 160, 160, 255);
        let label = Text::translate("advMode.command", &[]);
        batch.text_styled(atlas, &label, mid - 149.0, 40.0, grey, true);
        let s = self.side.command_block_screen.0.as_mut().unwrap();
        let cart = s.cart.is_some();
        if !s.previous.value.is_empty() {
            let label = Text::translate("advMode.previousOutput", &[]);
            // 75 + 5 * 9 + 1 + getPreviousY() - 135, then the label's 4.
            let y = 125.0 + previous_y(s.cart.is_some()) - 135.0;
            batch.text_styled(atlas, &label, mid - 149.0, y, grey, true);
            s.previous.draw(batch, atlas, None);
        }
        let s = self.side.command_block_screen.0.as_ref().unwrap();
        let fallback;
        let buttons = match self.built_command_block(w, h) {
            Some(layout) => &layout.buttons,
            None => {
                fallback = CommandBlockScreen::buttons(w, h, cart);
                &fallback
            }
        };
        for &(b, x, y, bw) in buttons {
            let hover = ui::hovered(self.side.cursor, x, y, bw, 20.0);
            let action = match b {
                Button::Done => Some(sunrise_client::widgets::Action::Done),
                Button::Cancel => Some(sunrise_client::widgets::Action::Cancel),
                _ => None,
            };
            if let Some(action) = action {
                sunrise_client::widgets::declare_action(action, [x, y, bw, 20.0], true);
            }
            ui::button_as(
                batch,
                atlas,
                [x, y, bw, 20.0],
                &s.label(b),
                true,
                hover,
                None,
                |rect| s.narration(b, rect, &super::screen_info::tr),
            );
        }
        // The command box, then its suggestions over the widgets: the build
        // made it at this size.
        let m = Measure(&atlas.font);
        let mouse = (
            self.side.cursor.0.floor() as i32,
            self.side.cursor.1.floor() as i32,
        );
        let blink = s.opened.elapsed().as_millis() as u64;
        // The command box is added first.
        let mark = sunrise_client::widgets::narratables_mark();
        let s = self.side.command_block_screen.0.as_mut().unwrap();
        let draws = s.command.draw(&m, mouse, blink, 0);
        sunrise_client::widgets::move_narratables(mark, 0);
        let tooltip = sunrise_client::screen_draw::paint(batch, atlas, &draws);
        self.draw_screen_phase(batch, sunrise_client::screen_draw::RenderPhase::Foreground);
        sunrise_client::screen_draw::paint_tooltip(batch, atlas, &draws, tooltip);
    }
    /// The wheel over the suggestion list (`CommandSuggestions.mouseScrolled`).
    pub(in crate::game::client_side) fn command_block_scroll(&mut self, amount: f32) -> bool {
        let mouse = (
            self.side.cursor.0.floor() as i32,
            self.side.cursor.1.floor() as i32,
        );
        self.side.command_block_screen
            .0
            .as_mut()
            .is_some_and(|s| s.command.scroll(mouse, f64::from(amount)))
    }
    /// The command box's completion request for an `ask_server` node.
    pub(in crate::game) fn command_block_suggestion_request(&mut self) -> Option<(i32, String)> {
        self.side.command_block_screen.0.as_mut()?.command.take_request()
    }
    /// The server's completions for the command box's `ask_server` request.
    pub(in crate::game::client_side) fn command_block_server_suggestions(
        &mut self,
        id: i32,
        range: (usize, usize),
        list: Vec<(String, Option<String>)>,
    ) {
        let Some(mut s) = self.side.command_block_screen.0.take() else {
            return;
        };
        let m = Measure(&self.gui.atlas.font);
        let c = Commands {
            dispatcher: super::chat::chat_tree(&self.remote),
            registries: &self.ui.registries,
        };
        s.command.server_suggestions(&m, &c, id, range, list);
        self.side.command_block_screen.0 = Some(s);
    }
    pub(in crate::game::client_side) fn command_block_click(&mut self) -> bool {
        let Some(mut s) = self.side.command_block_screen.0.take() else {
            return false;
        };
        // CommandSuggestions.mouseClicked first, then the command box.
        {
            let m = Measure(&self.gui.atlas.font);
            let c = Commands {
                dispatcher: super::chat::chat_tree(&self.remote),
                registries: &self.ui.registries,
            };
            let (x, y) = (
                f64::from(self.side.cursor.0),
                f64::from(self.side.cursor.1),
            );
            let double_click = s.command.live_double_click(x, y);
            if s.command.click(&m, &c, x, y, double_click) {
                self.side.command_block_screen.0 = Some(s);
                return true;
            }
        }
        let size = self
            .side.renderer
            .0
            .as_ref()
            .map_or([1, 1], |r| [r.config.width, r.config.height]);
        let b = HudBatch::new(size, self.settings.gui_scale_inputs());
        let fallback;
        let buttons = match self.built_command_block(b.width, b.height) {
            Some(layout) => &layout.buttons,
            None => {
                fallback = CommandBlockScreen::buttons(b.width, b.height, s.cart.is_some());
                &fallback
            }
        };
        let hit = buttons
            .iter()
            .find(|(_, x, y, bw)| ui::hovered(self.side.cursor, *x, *y, *bw, 20.0))
            .map(|b| b.0);
        if hit.is_some() {
            self.side.audio.0.play_button_click();
        }
        match hit {
            Some(Button::Output) => {
                s.track_output = !s.track_output;
                if let Some(block) = self.edit_base(&s) {
                    s.show_previous(block);
                }
            }
            Some(Button::Mode) => s.mode = s.mode.next(),
            Some(Button::Conditional) => s.conditional = !s.conditional,
            Some(Button::Auto) => s.auto = !s.auto,
            Some(Button::Done) => {
                self.set_command_block(&s);
                self.close_command_block();
                return true;
            }
            Some(Button::Cancel) => {
                self.close_command_block();
                return true;
            }
            None => {}
        }
        self.side.command_block_screen.0 = Some(s);
        hit.is_some()
    }
    /// `AbstractCommandBlockEditScreen.keyPressed`: the suggestions first,
    /// then Enter is Done, Escape cancels, the rest edits the command.
    /// A key pressed on the command block screen, with the edit shortcut it
    /// makes and the text it types.
    pub(in crate::game::client_side) fn command_block_key_code(
        &mut self,
        key: KeyCode,
        shortcut: Option<sunrise_client::clipboard::EditShortcut>,
        text: Option<&str>,
    ) -> bool {
        let ctrl = self.has_control_down_with_quirk();
        let shift = self.has_shift_down();
        // Windows' AltGr is Ctrl+Alt, and its characters still type.
        let types_text = self.types_text();
        let Some(mut s) = self.side.command_block_screen.0.take() else {
            return false;
        };
        let m = Measure(&self.gui.atlas.font);
        let c = Commands {
            dispatcher: super::chat::chat_tree(&self.remote),
            registries: &self.ui.registries,
        };
        if s.command.suggestions_key(&m, &c, key, shift) {
            self.side.command_block_screen.0 = Some(s);
            return true;
        }
        let handled = match key {
            KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter => true,
            _ => {
                if let Some(shortcut) = shortcut {
                    s.command.shortcut(&m, &c, shortcut, &mut self.side.clipboard.0);
                    true
                } else {
                    s.command.field_key(&m, &c, key, shift, ctrl)
                }
            }
        };
        self.side.command_block_screen.0 = Some(s);
        if !handled
            && types_text
            && let Some(text) = text
        {
            self.command_block_type(text);
        }
        match key {
            KeyCode::Escape => self.close_command_block(),
            KeyCode::Enter | KeyCode::NumpadEnter => {
                if let Some(s) = self.side.command_block_screen.0.take() {
                    self.set_command_block(&s);
                }
                self.close_command_block();
            }
            _ => {}
        }
        handled
    }
    /// Characters typed into the command box (`StringUtil.isAllowedChatCharacter`).
    pub(in crate::game::client_side) fn command_block_type(&mut self, typed: &str) {
        let allowed: String = typed
            .chars()
            .filter(|&c| c != '\u{a7}' && c >= ' ' && c != '\u{7f}')
            .collect();
        if allowed.is_empty() {
            return;
        }
        let Some(mut s) = self.side.command_block_screen.0.take() else {
            return;
        };
        let m = Measure(&self.gui.atlas.font);
        let c = Commands {
            dispatcher: super::chat::chat_tree(&self.remote),
            registries: &self.ui.registries,
        };
        s.command.type_text(&m, &c, &allowed);
        self.side.command_block_screen.0 = Some(s);
    }
    /// The edit screen's Done: a server's client sends the packet, the
    /// window's own world applies it (`handleSetCommandBlock`).
    pub(in crate::game::client_side) fn set_command_block(&mut self, s: &CommandBlockScreen) {
        if !self.can_use_game_master_blocks() {
            self.client_system_message(Text::translate("advMode.notAllowed", &[]));
            return;
        }
        if let Some(id) = s.cart {
            self.set_command_minecart(id, s);
            return;
        }
        let edit = CommandBlockEdit {
            pos: s.pos,
            command: s.command.value().to_owned(),
            track_output: s.track_output,
            mode: s.mode,
            conditional: s.conditional,
            auto: s.auto,
        };
        assert!(
            self.remote.is_some(),
            "set_command_block called without a server connection"
        );
        self.send(S::SetCommandBlock(play::SetCommandBlock {
            pos: [edit.pos.0, edit.pos.1, edit.pos.2],
            command: edit.command,
            mode: edit.mode.ordinal(),
            flags: i8::from(edit.track_output)
                | (i8::from(edit.conditional) << 1)
                | (i8::from(edit.auto) << 2),
        }));
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;

    /// The edit screen's boxes are re-made at the build's size and its
    /// buttons laid out once per build: a GUI scale change moves them with no
    /// draw, and a minecart's screen opening over a block's at the same size
    /// gets its own buttons. The rectangles are
    /// `AbstractCommandBlockEditScreen.init` and `addExtraControls` worked by
    /// hand for the test's 1280x720 window.
    #[test]
    #[ignore = "daily"]
    fn the_edit_screen_is_laid_out_by_the_build() {
        super::super::native_entity_refs::tests::with_client_game(|game| {
            game.side.title.0 = None;
            // 1280x720 at GUI scale 3 is 427x240.
            game.settings.gui_scale = 3;
            let block = CommandBlock::default;
            game.open_edit_screen((0, 0, 0), None, block(), Mode::Redstone, false);
            assert_eq!(game.side.screen, Screen::CommandBlock);
            game.build_screen();
            let boxes = |game: &crate::game::ClientGame<'_>| {
                let s = game.side.command_block_screen.0.as_ref().unwrap();
                let (i, p) = (&s.command.input, &s.previous);
                ([i.x, i.y, i.width, i.height], [p.x, p.y, p.width, p.height])
            };
            let buttons = |game: &crate::game::ClientGame<'_>, w, h| {
                game.built_command_block(w, h)
                    .map(|layout| layout.buttons.clone())
            };
            assert_eq!(boxes(game), ([63, 50, 300, 20], [63.0, 135.0, 276.0, 20.0]));
            assert_eq!(
                buttons(game, 427.0, 240.0),
                Some(vec![
                    (Button::Output, 343.0, 135.0, 20.0),
                    (Button::Mode, 59.0, 165.0, 100.0),
                    (Button::Conditional, 163.0, 165.0, 100.0),
                    (Button::Auto, 267.0, 165.0, 100.0),
                    (Button::Done, 59.0, 192.0, 150.0),
                    (Button::Cancel, 217.0, 192.0, 150.0),
                ])
            );
            // A minecart's screen at the same size: no mode switches, and the
            // output toggle lower.
            game.open_edit_screen((0, 0, 0), Some(7), block(), Mode::Redstone, false);
            game.build_screen();
            assert_eq!(
                buttons(game, 427.0, 240.0),
                Some(vec![
                    (Button::Output, 343.0, 150.0, 20.0),
                    (Button::Done, 59.0, 192.0, 150.0),
                    (Button::Cancel, 217.0, 192.0, 150.0),
                ])
            );
            // GUI scale 1 (1280x720): the build moves both boxes and the buttons.
            game.settings.gui_scale = 1;
            game.build_screen();
            assert_eq!(
                boxes(game),
                ([490, 50, 300, 20], [490.0, 150.0, 276.0, 20.0])
            );
            assert_eq!(
                buttons(game, 1280.0, 720.0),
                Some(vec![
                    (Button::Output, 770.0, 150.0, 20.0),
                    (Button::Done, 486.0, 312.0, 150.0),
                    (Button::Cancel, 644.0, 312.0, 150.0),
                ])
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn mode_switch_preserves_command_entity_output_and_state() {
        super::super::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            let pos = (1, 64, 1);
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            let old = game
                .pack
                .assets
                .registry
                .default_state("minecraft:command_block")
                .unwrap();
            game.world.set(pos.0, pos.1, pos.2, old).unwrap();
            let mut command = CommandBlock::default();
            command.set_command("say preserved");
            command.powered = true;
            command.last_output = Some(Text::literal("previous output"));
            game.world
                .block_entities
                .insert(pos, BlockEntity::CommandBlock(Box::new(command)));
            for (mode, conditional) in [
                (Mode::Sequence, false),
                (Mode::Auto, false),
                (Mode::Redstone, false),
                (Mode::Redstone, true),
            ] {
                game.apply_command_block(&CommandBlockEdit {
                    pos,
                    command: "say updated".into(),
                    track_output: true,
                    mode,
                    conditional,
                    auto: false,
                });
                let entity = game
                    .command_block(pos)
                    .expect("state change kept the instance");
                assert_eq!(entity.command, "say updated");
                assert_eq!(entity.last_output_string(), "previous output");
                assert!(entity.powered);
                assert_eq!(game.block_state_at(pos).unwrap().name, mode.block());
                assert_eq!(
                    game.block_state_at(pos).unwrap().properties["conditional"],
                    conditional.to_string()
                );
            }
        });
    }
}
