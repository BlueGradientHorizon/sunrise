#[allow(unused_imports)]
use super::*;
#[path = "book_screen/open.rs"]
pub(in crate::game) mod open;
#[path = "book_screen/placement.rs"]
pub(in crate::game) mod placement;

#[cfg(test)]
#[path = "book_screen/lifecycle_tests.rs"]
pub(in crate::game) mod lifecycle_tests;

impl Game<'_> {
    /// Whether a mod's screen places the current container and player slots.
    pub(in crate::game) fn native_screen_shown(&self) -> bool {
        self.shown_native_screen().is_some()
    }
    /// The open mod screen's height.
    pub(in crate::game) fn native_screen_height(&self) -> Option<f32> {
        self.shown_native_screen().map(|s| s.layout().height)
    }
    pub(in crate::game) fn native_screen_menu(&self) -> Option<&str> {
        Some(&self.shown_native_screen()?.menu.name)
    }
    pub(in crate::game) fn native_screen_toggle(&self) -> Option<mod_api::RecipeBookToggle> {
        self.shown_native_screen()?.layout().recipe_book_toggle
    }
    pub(in crate::game) fn native_screen_big_result(&self) -> bool {
        self.shown_native_screen()
            .is_some_and(|screen| screen.layout().recipe_book_big_result)
    }
    pub(in crate::game) fn native_screen_block(&self) -> Option<&str> {
        self.shown_native_screen()?.menu.block.as_deref()
    }
    /// The explicitly assigned book; declarations bind it to the actual menu.
    pub(in crate::game) fn native_screen_book(&self) -> Option<&mod_api::RecipeBookTypeKey> {
        self.shown_native_screen()?.layout().recipe_book.as_ref()
    }
}
