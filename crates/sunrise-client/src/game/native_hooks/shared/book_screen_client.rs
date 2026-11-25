#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;

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
    /// Whether a mod's screen places the current container and player slots.
    pub(in crate::game) fn native_screen_shown(&self) -> bool {
        self.shown_native_screen().is_some()
    }
    /// The open mod screen's height.
    pub(in crate::game) fn native_screen_height(&self) -> Option<f32> {
        self.shown_native_screen().map(|s| s.layout().height)
    }
    pub(in crate::game::client_side) fn native_screen_menu(&self) -> Option<&str> {
        Some(&self.shown_native_screen()?.menu.name)
    }
    pub(in crate::game::client_side) fn native_screen_toggle(
        &self,
    ) -> Option<mod_api::RecipeBookToggle> {
        self.shown_native_screen()?.layout().recipe_book_toggle
    }
    pub(in crate::game::client_side) fn native_screen_big_result(&self) -> bool {
        self.shown_native_screen()
            .is_some_and(|screen| screen.layout().recipe_book_big_result)
    }
    pub(in crate::game::client_side) fn native_screen_block(&self) -> Option<&str> {
        self.shown_native_screen()?.menu.block.as_deref()
    }
    /// The explicitly assigned book; declarations bind it to the actual menu.
    pub(in crate::game) fn native_screen_book(&self) -> Option<&mod_api::RecipeBookTypeKey> {
        self.shown_native_screen()?.layout().recipe_book.as_ref()
    }
}

#[cfg(test)]
#[path = "book_screen/lifecycle_tests_client.rs"]
pub(in crate::game) mod lifecycle_tests;

#[cfg(test)]
#[path = "book_screen/lifecycle_tests.rs"]
pub(in crate::game::client_side) mod lifecycle_tests_shared;
