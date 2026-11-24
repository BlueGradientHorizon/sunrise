//! The client nonce is tied to the actual shown mod screen, not just its menu type.
#[allow(unused_imports)]
use super::*;
impl Game<'_> {
    pub(in crate::game) fn native_book_placement_scope_matches(
        &self,
        window: i32,
        nonce: u64,
        menu: &str,
        book: &str,
    ) -> bool {
        self.shown_active_native_screen().is_some_and(|screen| {
            screen.menu.window == window
                && screen.menu.name == menu
                && screen.layout_revision == nonce
                && screen
                    .layout()
                    .recipe_book
                    .as_ref()
                    .is_some_and(|key| key.as_str() == book)
        })
    }
    pub(in crate::game) fn native_book_placement_window_matches(
        &self,
        window: &sunrise_client::recipe_book::placement_sync::Window,
    ) -> bool {
        self.native_book_placement_scope_matches(
            window.window,
            window.nonce,
            window.menu.as_str(),
            window.book.as_str(),
        )
    }
    pub(in crate::game) fn native_book_placement_catalog_matches(
        &self,
        identity: &sunrise_client::recipe_book::placement_sync::CatalogIdentity,
    ) -> bool {
        self.native_book_placement_scope_matches(
            identity.window,
            identity.nonce,
            identity.menu.as_str(),
            identity.book.as_str(),
        )
    }
    pub(in crate::game) fn native_book_placement_window(
        &self,
    ) -> Option<sunrise_client::recipe_book::placement_sync::Window> {
        let screen = self.shown_active_native_screen()?;
        let menu = screen.menu.name.as_str();
        mod_api::RecipeBookLayout::registered(menu)?;
        Some(sunrise_client::recipe_book::placement_sync::Window {
            window: screen.menu.window,
            nonce: screen.layout_revision,
            menu: menu.to_owned().try_into().ok()?,
            book: screen
                .layout()
                .recipe_book
                .as_ref()?
                .as_str()
                .to_owned()
                .try_into()
                .ok()?,
        })
    }
}
