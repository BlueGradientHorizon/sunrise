#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::book_screen::*;

#[path = "book_screen/open_client.rs"]
pub(in crate::game) mod open;

#[path = "book_screen/placement_client.rs"]
pub(in crate::game) mod placement;

impl Game<'_> {
    /// The mod screen showing now, if any, including a server-closed screen
    /// kept displayed through `screen_closing` or a veto until removal runs.
    pub(in crate::game) fn shown_native_screen(&self) -> Option<&OpenNativeScreen> {
        let screen = self.mod_screen.0.as_ref()?;
        if !screen.shown
            || self.client.screen != Screen::Inventory
            || self.container != Some(screen.menu.key)
        {
            return None;
        }
        let remote = &self.remote.as_ref()?.menu;
        let active = !screen.detached
            && remote.generation == screen.menu.generation
            && remote.as_ref().is_some_and(|m| m.0 == screen.menu.window);
        (active || (screen.detached && remote.as_ref().is_none())).then_some(screen)
    }
}

#[cfg(test)]
#[path = "book_screen/lifecycle_tests_client.rs"]
pub(in crate::game) mod lifecycle_tests;
