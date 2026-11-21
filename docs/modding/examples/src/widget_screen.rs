//! A mod's own screen with a text field and two buttons mounted during `screen_init_post`
//! (`mod_api::client::widgets`): F8 opens it, Tab walks the mounted widgets, typing edits the
//! field, Echo updates the button label, and Done (or Escape) closes it.

// ANCHOR: widget_screen
use mod_api::client::widgets::{Button, TextField};
use mod_api::{
    ClientHost, Hooks, InputKeyEvent, KeyAction, ScreenInitPostEvent, ScreenRenderPostEvent, Text,
};
use std::sync::{Arc, Mutex};

/// The mod's id, which names its screen.
const ID: &str = "widget_example";

/// The mod: remembers the current draft text across resizes while the screen is open.
#[derive(Default)]
pub struct WidgetScreen {
    draft: Arc<Mutex<String>>,
}

#[mod_api::sunrise_mod]
impl Hooks for WidgetScreen {
    fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
        if event.action == KeyAction::Press && event.key == "key.keyboard.f8" && host.playing() {
            self.draft.lock().unwrap().clear();
            host.mod_screen_open(ID);
        }
    }

    fn screen_init_post(&self, host: &dyn ClientHost, event: &mut ScreenInitPostEvent<'_>) {
        if host.mod_screen() != Some(ID) {
            return;
        }
        let width = event.screen.rect[2];
        let x = (width / 2.0 - 100.0).floor();
        let initial = self.draft.lock().unwrap().clone();

        let on_change_draft = Arc::clone(&self.draft);
        let field = event.widgets.add_text_field(
            TextField::new([x, 40.0, 200.0, 20.0], 64)
                .value(initial)
                .hint(Text::literal("Type a note"))
                .on_change(move |text, _host| {
                    *on_change_draft.lock().unwrap() = text.to_owned();
                }),
        );
        event.widgets.set_initial_focus(field);

        let echo_draft = Arc::clone(&self.draft);
        event.widgets.add_button(Button::new(
            [x, 70.0, 98.0, 20.0],
            Text::literal("Echo"),
            move |state, _host| {
                let text = echo_draft.lock().unwrap().clone();
                state.label = if text.is_empty() {
                    Text::literal("Echo")
                } else {
                    Text::literal(text)
                };
            },
        ));
        event.widgets.add_button(Button::new(
            [x + 102.0, 70.0, 98.0, 20.0],
            Text::literal("Done"),
            |_state, host| {
                host.mod_screen_close(ID);
            },
        ));
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if host.mod_screen() != Some(ID) {
            return;
        }
        let [width, _] = event.draw.size();
        event.draw.fill([0.0, 30.0, width, 70.0], 0x8000_0000);
    }
}
// ANCHOR_END: widget_screen
