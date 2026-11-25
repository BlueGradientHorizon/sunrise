use crate::game::client_side::screen_widgets::MountedScreenWidgets;
use crate::game::client_side::screen_widgets::MountedWidgetsInner;
use std::cell::{Cell, RefCell};

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
