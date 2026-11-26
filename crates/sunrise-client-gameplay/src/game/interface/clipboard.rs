//! Copy and paste in the text fields: `KeyboardHandler`'s clipboard and the
//! shortcuts of `EditBox`, `MultiLineEditBox` and `TextFieldHelper`.
use super::*;
use sunrise_client::clipboard::{self, EditShortcut};
use winit::event::KeyEvent;

impl crate::game::ClientGame<'_> {
    /// Runs `f` with `user_input` saying whether it handles input, then restores it, so a
    /// frame drawn while a key is handled reads false.
    pub(in crate::game::client_side) fn with_user_input<R>(
        &mut self,
        input: bool,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let was = self.client.user_input.replace(input);
        let result = f(self);
        self.client.user_input.set(was);
        result
    }
    /// The clipboard's text (`KeyboardHandler.getClipboard`), empty when it holds none,
    /// while the game handles a key, mouse or typed-text event: a mod's key or screen hook
    /// pasting into its own field. None at any other moment (a tick, a frame, a packet),
    /// so no mod reads the clipboard without the player doing something, and None on a
    /// server.
    pub(in crate::game) fn clipboard_text(&self) -> Option<String> {
        (self.common.native_window() && self.client.user_input.get()).then(|| self.client.clipboard.0.text())
    }
    /// Puts `text` on the clipboard (`KeyboardHandler.setClipboard`) on the same terms as
    /// `clipboard_text` reads it. False, with nothing copied, at any other moment or
    /// on a server; empty text copies nothing, as vanilla's.
    pub(in crate::game) fn set_clipboard_text(&self, text: &str) -> bool {
        if !self.common.native_window() || !self.client.user_input.get() {
            return false;
        }
        self.client.clipboard.0.set_text(text);
        true
    }
    /// `InputWithModifiers.isSelectAll`/`isCopy`/`isPaste`/`isCut` of a key
    /// press (or repeat): the keyboard's modifier state, or a modifier key an
    /// agent holds.
    pub(in crate::game::client_side) fn edit_shortcut(
        &self,
        event: &KeyEvent,
    ) -> Option<EditShortcut> {
        if event.state != ElementState::Pressed {
            return None;
        }
        self.edit_shortcut_for_letter(clipboard::shortcut_letter(
            &event.logical_key,
            event.physical_key,
        ))
    }

    pub(in crate::game::client_side) fn edit_shortcut_for_letter(
        &self,
        letter: Option<char>,
    ) -> Option<EditShortcut> {
        let m = self.client.modifiers;
        let held = |keys: [KeyCode; 2]| keys.iter().any(|k| self.client.physical_keys.contains(k));
        clipboard::edit_shortcut(
            letter,
            self.has_control_down_with_quirk(),
            m.shift_key() || held([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            m.alt_key() || held([KeyCode::AltLeft, KeyCode::AltRight]),
        )
    }
    /// `InputWithModifiers.hasControlDownWithQuirk`: the edit shortcuts'
    /// modifier (`InputQuirks.EDIT_SHORTCUT_KEY_MODIFIER`: Cmd on macOS, Ctrl
    /// elsewhere), held on the keyboard or by an agent. It reads the physical
    /// keys, never `keys`: a latched sprint or sneak toggle puts Control or
    /// Shift in `keys` without the key being down.
    pub(in crate::game::client_side) fn has_control_down_with_quirk(&self) -> bool {
        let held = |keys: [KeyCode; 2]| keys.iter().any(|k| self.client.physical_keys.contains(k));
        if cfg!(target_os = "macos") {
            self.client.modifiers.super_key() || held([KeyCode::SuperLeft, KeyCode::SuperRight])
        } else {
            self.client.modifiers.control_key() || held([KeyCode::ControlLeft, KeyCode::ControlRight])
        }
    }
    /// Whether the held keys let a key's text through to the focused field
    /// (`clipboard::types_text`): not with the edit modifier, unless it is
    /// part of AltGr.
    pub(in crate::game) fn types_text(&self) -> bool {
        let held = |keys: [KeyCode; 2]| keys.iter().any(|k| self.client.physical_keys.contains(k));
        let edit = if cfg!(target_os = "macos") {
            held([KeyCode::SuperLeft, KeyCode::SuperRight])
        } else {
            held([KeyCode::ControlLeft, KeyCode::ControlRight])
        };
        clipboard::types_text(
            edit,
            held([KeyCode::AltLeft, KeyCode::AltRight]),
            clipboard::ALTGR_IS_CTRL_ALT,
        )
    }
    /// A text field has keyboard focus: chat, a sign, a book being written,
    /// a search box, the anvil's name, the title screens' and the options'
    /// fields, the LAN port, a dialog's input, the command box and the edit
    /// screens of the jigsaw, structure and test blocks.
    pub(in crate::game::client_side) fn text_input_focused(&self) -> bool {
        self.mounted_widget_captures_input()
            || (self.client.screen == Screen::Chat && self.client.title.0.is_none())
            || self.client.sign_edit.is_some()
            || matches!(self.side.book, Some(super::books::BookScreen::Edit { .. }))
            || (self.client.screen == Screen::Creative && self.client.creative_screen.0.tab().search)
            || self.recipe_search_focused()
            || self.station_typing()
            || self.dialog_typing()
            || self.mod_browser_typing()
            || matches!(
                self.side.screen,
                Screen::CommandBlock | Screen::BlockEditor | Screen::Social | Screen::DebugOptions
            )
    }
    /// A shortcut in an append-only field limited to `max_chars` characters
    /// (an `EditBox` with `setMaxLength`). True when the value changed.
    pub(in crate::game::client_side) fn append_shortcut(
        selection: &mut clipboard::AppendSelection,
        board: &mut clipboard::SystemClipboard,
        id: clipboard::FieldId,
        value: &mut String,
        shortcut: EditShortcut,
        max_chars: usize,
    ) -> bool {
        selection.apply(id, value, shortcut, board, |v, t| {
            clipboard::append_limited(v, t, max_chars)
        })
    }
    /// Shortcuts on the title, world creation and server screens.
    pub(in crate::game::client_side) fn title_shortcut(&mut self, shortcut: EditShortcut) {
        let Some(title) = self.client.title.0.as_mut() else {
            return;
        };
        if title.has_caret_field() {
            return title.field_shortcut(shortcut, &mut self.client.clipboard.0);
        }
        if let Some((id, value, max)) = title.focused_field() {
            let changed = Self::append_shortcut(
                &mut self.client.text_selection,
                &mut self.client.clipboard.0,
                id,
                value,
                shortcut,
                max,
            );
            if changed {
                title.search_changed();
            }
        }
    }
    /// Typing or a backspace in a title-screen field. A field that still only
    /// appends replaces a Select All selection before the edit.
    pub(in crate::game::client_side) fn title_edit(
        &mut self,
        typed: Option<&str>,
        backspace: bool,
    ) {
        let Some(title) = self.client.title.0.as_mut() else {
            return;
        };
        let cleared = !title.has_caret_field()
            && title
                .focused_field()
                .is_some_and(|(id, value, _)| self.client.text_selection.take(id, value));
        if cleared {
            title.search_changed();
        }
        if backspace {
            if !cleared {
                title.backspace();
            }
        } else if let Some(text) = typed {
            title.type_text(text);
        }
    }
    /// Shortcuts in the creative inventory's search box (an `EditBox` of 50).
    pub(in crate::game::client_side) fn creative_search_shortcut(
        &mut self,
        shortcut: EditShortcut,
    ) {
        if self
            .client
            .creative_screen
            .0
            .search
            .shortcut(shortcut, &mut self.client.clipboard.0)
        {
            self.client.creative_screen.0.scroll = 0.0;
        }
    }
    /// A backspace in the creative search box: with Ctrl, the whole word
    /// before the caret (`EditBox.deleteWords(-1)`), otherwise one character.
    /// A selection is what it deletes, as in `EditBox.keyPressed`.
    pub(in crate::game::client_side) fn creative_search_backspace(&mut self, ctrl: bool) {
        self.client
            .creative_screen
            .0
            .search
            .key(KeyCode::Backspace, ctrl, false);
        self.client.creative_screen.0.scroll = 0.0;
    }
    /// Typing in the creative search box.
    pub(in crate::game::client_side) fn creative_search_edit(&mut self, typed: Option<&str>) {
        if let Some(text) = typed {
            self.client.creative_screen.0.search.insert_text(text);
            self.client.creative_screen.0.scroll = 0.0;
        }
    }
}

/// `CreativeModeInventoryScreen`'s search box: `setMaxLength(50)`.
pub(in crate::game) const CREATIVE_SEARCH_CHARS: usize = 50;
