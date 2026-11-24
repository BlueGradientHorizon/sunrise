#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::book_screen::placement::*;

impl Game<'_> {
    pub(in crate::game) fn apply_screen_with_mod_book(
        &mut self,
        screen: Screen,
        owner: Option<&str>,
    ) {
        // Publish the accepted native screen before building its layout. A
        // staged screen is not shown, even on an in-place Inventory open.
        if screen == Screen::Inventory
            && let Some(native) = self.mod_screen.0.as_mut()
        {
            native.shown = true;
        }
        self.apply_screen(screen, owner);
        if self.native_screen_shown() {
            self.begin_mod_book_placement();
            self.ensure_mod_book_catalog();
        }
    }
}
