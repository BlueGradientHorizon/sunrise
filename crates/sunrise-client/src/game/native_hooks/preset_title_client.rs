#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::preset_title::preset_editor_key_code;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::preset_title::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use winit::keyboard::PhysicalKey;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Give the bound editor its key before normal title-screen navigation.
    pub(in crate::game::client_side) fn preset_editor_key(&self, key: KeyCode) -> bool {
        let physical = PhysicalKey::Code(key);
        self.preset_editor_title()
            .and_then(|title| title.preset_editor_session())
            .is_some_and(|session| {
                session.key_pressed(
                    preset_editor_key_code(physical),
                    keybinds::raw_key_code(physical),
                    crate::game::glfw_modifiers(self.side.modifiers),
                )
            })
    }
    /// Forward a mouse press to the open bound preset editor.
    pub(in crate::game) fn preset_editor_mouse_clicked(
        &self,
        button: i32,
        double_click: bool,
    ) -> bool {
        let Some(session) = self
            .preset_editor_title()
            .and_then(|title| title.preset_editor_session())
        else {
            return false;
        };
        session.mouse_clicked(
            self.side.cursor.0,
            self.side.cursor.1,
            button,
            double_click,
        )
    }
    /// Forward printable characters to the open bound preset editor. Returns `true` if the editor
    /// consumed any character.
    pub(in crate::game::client_side) fn preset_editor_type_text(&self, text: &str) -> bool {
        let Some(session) = self
            .preset_editor_title()
            .and_then(|title| title.preset_editor_session())
        else {
            return false;
        };
        let modifiers = crate::game::glfw_modifiers(self.side.modifiers);
        let mut handled = false;
        for ch in text.chars().filter(|c| !c.is_control()) {
            handled |= session.char_typed(ch, modifiers);
        }
        handled
    }
    pub(in crate::game) fn stage_title(&mut self, mut title: sunrise_client::title::Title) {
        title.set_additional_data_packs(self.pack.additional_data.clone());
        title.set_preset_data_source(self.pack.data_changes.clone());
        self.side.screen_state.0.build.reopened();
        self.side.title.0 = Some(title);
    }
    pub(in crate::game) fn show_title_with_registry(
        &mut self,
        title: sunrise_client::title::Title,
    ) {
        let answer = self
            .screen_hooks_heard()
            .then(|| self.title_change(Some(title.mode.id())));
        self.stage_title(title);
        let Some(answer) = answer else {
            self.release_mappings_on_screen_open();
            self.commit_screen_transition();
            return;
        };
        match self.screen_decision(answer) {
            ScreenDecision::ModScreen(mod_id) => self.apply_screen(Screen::Mod, Some(mod_id)),
            ScreenDecision::Vanilla(target) => self.open_screen_target(target),
            // With no world nothing else can show: a `Cancel` doesn't keep it shut.
            ScreenDecision::Keep | ScreenDecision::Open => {
                self.release_mappings_on_screen_open();
                self.commit_screen_transition();
            }
        }
    }
    /// Render the open bound preset editor's custom UI right before `screen_render_post`.
    pub(in crate::game) fn draw_preset_editor(&self, batch: &mut HudBatch) {
        let Some(session) = self
            .preset_editor_title()
            .and_then(|title| title.preset_editor_session())
        else {
            return;
        };
        let rect = [0.0, 0.0, batch.width, batch.height];
        let paint = batch.paint_checkpoint();
        let rendered = {
            let mut draw = crate::game::host::draw::GameDrawContext::new(self, batch);
            session.render(&mut draw, rect)
        }; // Close any open clips before removing the editor's paint.
        if !rendered {
            paint.discard(batch);
        }
    }
}
