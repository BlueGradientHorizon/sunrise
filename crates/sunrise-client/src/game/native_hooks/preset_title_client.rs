#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::preset_title::preset_editor_key_code;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::preset_title::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use winit::keyboard::PhysicalKey;

impl Game<'_> {
    /// Give the bound editor its key before normal title-screen navigation.
    pub(in crate::game) fn preset_editor_key(&self, key: KeyCode) -> bool {
        let physical = PhysicalKey::Code(key);
        self.preset_editor_title()
            .and_then(|title| title.preset_editor_session())
            .is_some_and(|session| {
                session.key_pressed(
                    preset_editor_key_code(physical),
                    keybinds::raw_key_code(physical),
                    crate::game::glfw_modifiers(self.client.modifiers),
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
            self.client.cursor.0,
            self.client.cursor.1,
            button,
            double_click,
        )
    }
    /// Forward printable characters to the open bound preset editor. Returns `true` if the editor
    /// consumed any character.
    pub(in crate::game) fn preset_editor_type_text(&self, text: &str) -> bool {
        let Some(session) = self
            .preset_editor_title()
            .and_then(|title| title.preset_editor_session())
        else {
            return false;
        };
        let modifiers = crate::game::glfw_modifiers(self.client.modifiers);
        let mut handled = false;
        for ch in text.chars().filter(|c| !c.is_control()) {
            handled |= session.char_typed(ch, modifiers);
        }
        handled
    }
    pub(in crate::game) fn stage_title(&mut self, mut title: sunrise_client::title::Title) {
        title.set_additional_data_packs(self.pack.additional_data.clone());
        title.set_preset_data_source(self.pack.data_changes.clone());
        self.screen_state.0.build.reopened();
        self.title.0 = Some(title);
    }
}
