#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use winit::keyboard::PhysicalKey;

pub(in crate::game) fn preset_editor_key_code(physical: PhysicalKey) -> u32 {
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
