#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::screen_widgets::MountedScreenWidgets;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::screen_widgets::MountedWidgetEntry;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::screen_widgets::MountedWidgetKind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::screen_widgets::MountedWidgetsInner;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::screen_widgets::ScreenWidgetMount;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::screen_widgets::call_owner;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::screen_widgets::point_in_rect;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::screen_widgets::valid_widget_rect;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::screen_widgets::widget_callback_owner;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::screen_widgets::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
    Game, api_values::TryIntoEngine, host::draw::GameDrawContext,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    __private::game::CallbackOwner,
    ScreenView, ScreenWidget, ScreenWidgetId, ScreenWidgets,
    client::widgets::{
        Button, ButtonPress, ButtonState, ScreenChild, TextEdit, TextField, TextFieldCallback,
        is_allowed_chat_character,
    },
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::cell::{Cell, RefCell};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::time::Instant;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    clipboard::EditShortcut,
    hud::HudBatch,
    keybinds,
    preedit::PreeditEvent,
    text::{DARK_GRAY, Style, Text},
    ui::{self, EditBox},
    widgets::{self, Kind, Narratable, Target},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::keyboard::KeyCode;

impl Default for MountedScreenWidgets {
    fn default() -> Self {
        Self {
            generation: Cell::new(0),
            next_id: Cell::new(1),
            inner: RefCell::new(MountedWidgetsInner {
                entries: Vec::new(),
                removed_pause_actions: Vec::new(),
                focused: None,
                dragging: None,
            }),
        }
    }
}

impl Drop for MountedScreenWidgets {
    fn drop(&mut self) {
        self.clear();
    }
}
