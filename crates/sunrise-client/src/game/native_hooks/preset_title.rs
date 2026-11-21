use super::*;
use winit::keyboard::PhysicalKey;

fn preset_editor_key_code(physical: PhysicalKey) -> u32 {
    let PhysicalKey::Code(code) = physical else {
        return 0;
    };
    let Some(name) = keybinds::key_name(code) else {
        return 0;
    };
    sunrise_client::key_names::KEYBOARD
        .iter()
        .find(|(_, key_name)| *key_name == name)
        .and_then(|(id, _)| u32::try_from(*id).ok())
        .unwrap_or(0)
}

impl Game<'_> {
    pub(in crate::game) fn show_title_with_registry(
        &mut self,
        mut title: sunrise_client::title::Title,
    ) {
        title.set_additional_data_packs(self.pack.additional_data.clone());
        title.set_preset_data_source(self.pack.data_changes.clone());
        if !self.screen_hooks_heard() {
            self.screen_state.build.reopened();
            self.title = Some(title);
            self.build_screen();
            return;
        }
        let answer = self.title_change(Some(title.mode.id()));
        self.screen_state.build.reopened();
        self.title = Some(title);
        match self.screen_decision(answer) {
            ScreenDecision::ModScreen(mod_id) => self.apply_screen(Screen::Mod, Some(mod_id)),
            ScreenDecision::Vanilla(target) => self.open_screen_target(target),
            // With no world nothing else can show: a `Cancel` doesn't keep it shut.
            ScreenDecision::Keep | ScreenDecision::Open => self.build_screen(),
        }
        self.screen_state.observed = self.observed_now();
        self.shown_init_post();
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
        let mut draw = crate::game::host::draw::GameDrawContext::new(self, batch);
        session.render(&mut draw, rect);
    }

    /// Forward a mouse press to the open bound preset editor.
    pub(in crate::game) fn preset_editor_mouse_clicked(&self, button: MouseButton) -> bool {
        let Some(session) = self
            .preset_editor_title()
            .and_then(|title| title.preset_editor_session())
        else {
            return false;
        };
        let button = u8::try_from(crate::game::glfw_button(button)).unwrap_or(0);
        session.mouse_clicked(self.cursor.0, self.cursor.1, button)
    }

    /// Forward a key press to the open bound preset editor.
    pub(in crate::game) fn preset_editor_key_pressed(&self, physical: PhysicalKey) -> bool {
        let Some(session) = self
            .preset_editor_title()
            .and_then(|title| title.preset_editor_session())
        else {
            return false;
        };
        session.key_pressed(
            preset_editor_key_code(physical),
            keybinds::raw_key_code(physical),
            crate::game::glfw_modifiers(self.modifiers),
        )
    }

    /// Forward printable characters to the open bound preset editor. Returns whether a preset
    /// editor was open to receive them.
    pub(in crate::game) fn preset_editor_type_text(&self, text: &str) -> bool {
        let Some(session) = self
            .preset_editor_title()
            .and_then(|title| title.preset_editor_session())
        else {
            return false;
        };
        let modifiers = crate::game::glfw_modifiers(self.modifiers);
        for ch in text.chars().filter(|c| !c.is_control()) {
            session.char_typed(ch, modifiers);
        }
        true
    }
}
