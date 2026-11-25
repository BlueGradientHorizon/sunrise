//! Copy and paste in the text fields: `KeyboardHandler`'s clipboard and the
//! shortcuts of `EditBox`, `MultiLineEditBox` and `TextFieldHelper`.
use super::*;
use sunrise_client::clipboard::{self, EditShortcut};
use winit::event::KeyEvent;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Runs `f` with `user_input` saying whether it handles input, then restores it, so a
    /// frame drawn while a key is handled reads false.
    pub(in crate::game::client_side) fn with_user_input<R>(
        &mut self,
        input: bool,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let was = self.side.user_input.replace(input);
        let result = f(self);
        self.side.user_input.set(was);
        result
    }
    /// The clipboard's text (`KeyboardHandler.getClipboard`), empty when it holds none,
    /// while the game handles a key, mouse or typed-text event: a mod's key or screen hook
    /// pasting into its own field. None at any other moment (a tick, a frame, a packet),
    /// so no mod reads the clipboard without the player doing something, and None on a
    /// server.
    pub(in crate::game) fn clipboard_text(&self) -> Option<String> {
        (self.native_window() && self.side.user_input.get()).then(|| self.side.clipboard.0.text())
    }
    /// Puts `text` on the clipboard (`KeyboardHandler.setClipboard`) on the same terms as
    /// `clipboard_text` reads it. False, with nothing copied, at any other moment or
    /// on a server; empty text copies nothing, as vanilla's.
    pub(in crate::game) fn set_clipboard_text(&self, text: &str) -> bool {
        if !self.native_window() || !self.side.user_input.get() {
            return false;
        }
        self.side.clipboard.0.set_text(text);
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
        let m = self.side.modifiers;
        let held = |keys: [KeyCode; 2]| keys.iter().any(|k| self.side.physical_keys.contains(k));
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
        let held = |keys: [KeyCode; 2]| keys.iter().any(|k| self.side.physical_keys.contains(k));
        if cfg!(target_os = "macos") {
            self.side.modifiers.super_key() || held([KeyCode::SuperLeft, KeyCode::SuperRight])
        } else {
            self.side.modifiers.control_key()
                || held([KeyCode::ControlLeft, KeyCode::ControlRight])
        }
    }
    /// Whether the held keys let a key's text through to the focused field
    /// (`clipboard::types_text`): not with the edit modifier, unless it is
    /// part of AltGr.
    pub(in crate::game) fn types_text(&self) -> bool {
        let held = |keys: [KeyCode; 2]| keys.iter().any(|k| self.side.physical_keys.contains(k));
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
            || (self.side.screen == Screen::Chat && self.side.title.0.is_none())
            || self.side.sign_edit.is_some()
            || matches!(
                self.side.book,
                Some(super::books::BookScreen::Edit { .. })
            )
            || (self.side.screen == Screen::Creative && self.side.creative_screen.0.tab().search)
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
        let Some(title) = self.side.title.0.as_mut() else {
            return;
        };
        if title.has_caret_field() {
            return title.field_shortcut(shortcut, &mut self.side.clipboard.0);
        }
        if let Some((id, value, max)) = title.focused_field() {
            let changed = Self::append_shortcut(
                &mut self.side.text_selection,
                &mut self.side.clipboard.0,
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
        let Some(title) = self.side.title.0.as_mut() else {
            return;
        };
        let cleared = !title.has_caret_field()
            && title
                .focused_field()
                .is_some_and(|(id, value, _)| self.side.text_selection.take(id, value));
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
            .side.creative_screen
            .0
            .search
            .shortcut(shortcut, &mut self.side.clipboard.0)
        {
            self.side.creative_screen.0.scroll = 0.0;
        }
    }
    /// A backspace in the creative search box: with Ctrl, the whole word
    /// before the caret (`EditBox.deleteWords(-1)`), otherwise one character.
    /// A selection is what it deletes, as in `EditBox.keyPressed`.
    pub(in crate::game::client_side) fn creative_search_backspace(&mut self, ctrl: bool) {
        self.side.creative_screen
            .0
            .search
            .key(KeyCode::Backspace, ctrl, false);
        self.side.creative_screen.0.scroll = 0.0;
    }
    /// Typing in the creative search box.
    pub(in crate::game::client_side) fn creative_search_edit(&mut self, typed: Option<&str>) {
        if let Some(text) = typed {
            self.side.creative_screen.0.search.insert_text(text);
            self.side.creative_screen.0.scroll = 0.0;
        }
    }
}

/// `CreativeModeInventoryScreen`'s search box: `setMaxLength(50)`.
pub(in crate::game) const CREATIVE_SEARCH_CHARS: usize = 50;

#[cfg(test)]
mod tests {
    use mod_api::{
        ClientHost, ClientTickPreEvent, Flow, Hooks, ModEntry, MouseButtonPreEvent,
        RenderGuiPreEvent,
    };
    use mod_platform::testing::with_mods;
    use std::sync::{Arc, Mutex};

    /// What each hook read from the clipboard, and whether its copy was taken.
    #[derive(Clone, Default)]
    struct Reads(Arc<Mutex<Vec<(&'static str, Option<String>, bool)>>>);
    impl Reads {
        fn read(&self, at: &'static str, host: &dyn ClientHost) {
            let text = host.clipboard_text();
            let copied = host.set_clipboard_text(at);
            self.0.lock().unwrap().push((at, text, copied));
        }
    }
    struct Mod(Reads);
    /// A step of an event loop that is never asked to stop.
    struct NoEventLoop;
    impl crate::game::event_loop::EventLoopCtl for NoEventLoop {
        fn exit(&self) {}
        fn set_control_flow(&self, _: winit::event_loop::ControlFlow) {}
    }
    #[mod_api::__subscriptions]
    impl Hooks for Mod {
        fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
            self.0.read("tick", host);
        }
        fn render_gui_pre(&self, host: &dyn ClientHost, _: &mut RenderGuiPreEvent) -> Flow {
            self.0.read("frame", host);
            Flow::Continue
        }
        fn mouse_button_pre(&self, host: &mut dyn ClientHost, _: &mut MouseButtonPreEvent) -> Flow {
            self.0.read("click", host);
            // The click stops here: the game has no world to click in.
            Flow::Cancel
        }
    }

    /// A tick and a frame reach no clipboard, read or write; a click from the window does,
    /// and its copy is what the clipboard holds after.
    #[test]
    #[ignore = "daily"]
    fn a_mod_reaches_the_clipboard_only_while_input_is_handled() {
        let reads = Reads::default();
        let mods: mod_api::Mods = vec![ModEntry::new("paste", Box::new(Mod(reads.clone())))].into();
        with_mods(mods, || {
            crate::game::native_entity_refs::tests::with_game(|game| {
                game.options.execution_role = crate::game::execution_role::ExecutionRole::Client;
                game.options.headless = true;
                game.clipboard.0.set_text("pasted");

                game.native_client_tick_pre();
                game.native_render_gui_pre(&mut false);
                game.handle_window_event(
                    &NoEventLoop,
                    winit::window::WindowId::dummy(),
                    winit::event::WindowEvent::MouseInput {
                        device_id: winit::event::DeviceId::dummy(),
                        state: winit::event::ElementState::Pressed,
                        button: winit::event::MouseButton::Left,
                    },
                    crate::game::input_trace::InputOrigin::User,
                );
                game.native_client_tick_pre();
                let got = reads.0.lock().unwrap().clone();
                assert_eq!(
                    got,
                    [
                        ("tick", None, false),
                        ("frame", None, false),
                        ("click", Some("pasted".to_owned()), true),
                        ("tick", None, false),
                    ]
                );
                assert_eq!(game.clipboard.0.text(), "click");
            });
        });
    }
}
