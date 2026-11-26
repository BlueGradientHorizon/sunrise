//! Room beside an open container screen's window for mods' own panels (`screen_room`): the
//! window, and the recipe book open beside it, move right by half the room left of them less half
//! the room right of them when all of it fits across the screen, as the book moves the window
//! right to make room for itself (`RecipeBookComponent.updateScreenPosition`).
use super::*;

/// A mod's answer as room: not finite or below 0 counts as 0.
fn room(answer: f32) -> f32 {
    if answer.is_finite() {
        answer.max(0.0)
    } else {
        0.0
    }
}

/// How far `[left, right]` room beside a window `image` wide moves the window right (negative:
/// left), its left edge at `x`, and the book open beside it, its visible left extent (including
/// tabs) at `book`, on a screen `width` wide: half the left room less half the right room, each
/// rounded down, when the left room, the book, the window and the right room all fit across the
/// screen; else nothing.
pub(super) fn room_offset(
    width: f32,
    x: f32,
    image: f32,
    book: Option<f32>,
    rooms: [f32; 2],
) -> f32 {
    let [left, right] = rooms.map(room);
    if left == 0.0 && right == 0.0 {
        return 0.0;
    }
    let offset = (left / 2.0).floor() - (right / 2.0).floor();
    let first = book.map_or(x, |book| book.min(x));
    if first + offset - left >= 0.0 && x + offset + image + right <= width {
        offset
    } else {
        0.0
    }
}

impl Game<'_> {
    /// The room the mods ask left and right of the open container screen's window
    /// (`screen_room`), asked once a frame: none when no mod listens, for the creative
    /// inventory, a merchant's or a beacon's screen, and while the recipe book covers the window
    /// of a narrow screen.
    pub(super) fn mod_room(&self, batch: &HudBatch) -> [f32; 2] {
        let kind = self.station.as_ref().map(|s| s.kind);
        if self.screen != Screen::Inventory
            || matches!(
                kind,
                Some(stations::Kind::Merchant | stations::Kind::Beacon)
            )
            || self.book_covers_menu(batch.width)
            || !mod_api::__private::listened(mod_api::HookId::ScreenRoom)
        {
            return [0.0; 2];
        }
        if let Some(rooms) = self
            .screen_state
            .build
            .room_answer(self.frames, self.screen)
        {
            return rooms;
        }
        let image = self.native_screen_width().unwrap_or(176.0);
        let height = self.screen_height();
        let (x, y) = batch.centered_origin(image, height);
        let rect = [x + self.recipe_book_shift(batch.width), y, image, height];
        // Asked while the screen is laid out: nothing is drawn beside it yet.
        let view =
            super::screen_backgrounds::without_regions(&self.screen_view_at(self.screen, rect));
        // No room while the mods are asked, so a host call of theirs that lays the screen out
        // doesn't ask them again.
        let build = &self.screen_state.build;
        build.keep_room_answer(self.frames, self.screen, [0.0; 2]);
        let (mut left, mut right) = (0.0, 0.0);
        self.native_screen_room(&view, &mut left, &mut right);
        let rooms = [room(left), room(right)];
        build.keep_room_answer(self.frames, self.screen, rooms);
        rooms
    }

    /// How far the mods' room moves the open container screen's window, and the recipe book
    /// open beside it, right (negative: left; `room_offset`).
    pub(super) fn screen_room_offset(&self, batch: &HudBatch) -> f32 {
        let rooms = self.mod_room(batch);
        if rooms == [0.0; 2] {
            return 0.0;
        }
        let image = self.native_screen_width().unwrap_or(176.0);
        let (x, _) = batch.centered_origin(image, self.screen_height());
        let x = x + self.recipe_book_shift(batch.width);
        room_offset(batch.width, x, image, self.open_book_left(batch), rooms)
    }
}

#[cfg(test)]
mod tests {
    use super::room_offset;

    #[test]
    fn room_on_the_right_moves_the_window_left_by_half_when_it_all_fits() {
        // 1440p at the automatic GUI scale: 427 wide, the window at 125. A panel of 147 and its
        // gap of 2 fit right of it once the window moves 74 left.
        assert_eq!(room_offset(427.0, 125.0, 176.0, None, [0.0, 149.0]), -74.0);
        // Just wide enough, and a pixel short.
        assert_eq!(room_offset(325.0, 74.0, 176.0, None, [0.0, 149.0]), -74.0);
        assert_eq!(room_offset(320.0, 72.0, 176.0, None, [0.0, 149.0]), 0.0);
        // No room asked, or none that counts.
        assert_eq!(room_offset(427.0, 125.0, 176.0, None, [0.0, 0.0]), 0.0);
        assert_eq!(room_offset(427.0, 125.0, 176.0, None, [0.0, -20.0]), 0.0);
        assert_eq!(room_offset(427.0, 125.0, 176.0, None, [0.0, f32::NAN]), 0.0);
        assert_eq!(
            room_offset(427.0, 125.0, 176.0, None, [0.0, f32::INFINITY]),
            0.0
        );
    }

    #[test]
    fn room_on_the_left_moves_the_window_right_by_half_when_it_all_fits() {
        assert_eq!(room_offset(427.0, 125.0, 176.0, None, [149.0, 0.0]), 74.0);
        // Just wide enough; a pixel short on the left, then on the right.
        assert_eq!(room_offset(325.0, 75.0, 176.0, None, [149.0, 0.0]), 74.0);
        assert_eq!(room_offset(325.0, 74.0, 176.0, None, [149.0, 0.0]), 0.0);
        assert_eq!(room_offset(324.0, 75.0, 176.0, None, [149.0, 0.0]), 0.0);
        // None that counts.
        assert_eq!(room_offset(427.0, 125.0, 176.0, None, [-10.0, 0.0]), 0.0);
        assert_eq!(room_offset(427.0, 125.0, 176.0, None, [f32::NAN, 0.0]), 0.0);
        assert_eq!(
            room_offset(427.0, 125.0, 176.0, None, [f32::INFINITY, 0.0]),
            0.0
        );
    }

    #[test]
    fn room_on_both_sides_moves_the_window_by_the_difference_of_the_halves() {
        // 50 right less 74 left, with 1 pixel spare on the right and none on the left.
        assert_eq!(
            room_offset(427.0, 125.0, 176.0, None, [100.0, 149.0]),
            -24.0
        );
        assert_eq!(
            room_offset(427.0, 125.0, 176.0, None, [101.0, 149.0]),
            -24.0
        );
        assert_eq!(
            room_offset(427.0, 125.0, 176.0, None, [102.0, 149.0]),
            -23.0
        );
        // A pixel too much on the left: nothing moves, the right room alone would.
        assert_eq!(room_offset(427.0, 125.0, 176.0, None, [104.0, 149.0]), 0.0);
        // Each half rounds down on its own.
        assert_eq!(room_offset(427.0, 125.0, 176.0, None, [3.0, 5.0]), -1.0);
        // A room that doesn't count leaves the other side's answer.
        assert_eq!(
            room_offset(427.0, 125.0, 176.0, None, [f32::NAN, 149.0]),
            -74.0
        );
        assert_eq!(room_offset(427.0, 125.0, 176.0, None, [149.0, -20.0]), 74.0);
    }

    #[test]
    fn the_book_beside_the_window_moves_with_it_or_keeps_it_in_place() {
        // The book open beside the window (the window 77 right of centre, the book left of it):
        // at 427 the three don't fit, so nothing moves; at 640 they do.
        assert_eq!(
            room_offset(427.0, 202.0, 176.0, Some(22.0), [0.0, 149.0]),
            0.0
        );
        assert_eq!(
            room_offset(640.0, 309.0, 176.0, Some(128.0), [0.0, 149.0]),
            -74.0
        );
        // Panel at 95 would fit after a shift of 90, but its selected tab at 63 would not.
        assert_eq!(
            room_offset(510.0, 244.0, 176.0, Some(63.0), [0.0, 180.0]),
            0.0
        );
        // The selected tab remains at 13 and the reservation ends at 470.
        assert_eq!(
            room_offset(510.0, 244.0, 176.0, Some(63.0), [0.0, 100.0]),
            -50.0
        );
        // The book's left edge is what runs out first on the left.
        assert_eq!(
            room_offset(640.0, 300.0, 176.0, Some(60.0), [0.0, 149.0]),
            0.0
        );
        // Room on the left is measured from the selected tab: 13 of it spare after moving 50
        // right, and refused when it would reach past the screen's edge.
        assert_eq!(
            room_offset(510.0, 244.0, 176.0, Some(63.0), [100.0, 0.0]),
            50.0
        );
        assert_eq!(
            room_offset(510.0, 244.0, 176.0, Some(63.0), [180.0, 0.0]),
            0.0
        );
        assert_eq!(
            room_offset(640.0, 309.0, 176.0, Some(128.0), [149.0, 0.0]),
            74.0
        );
        // Both sides around the book: 30 right less 74 left.
        assert_eq!(
            room_offset(640.0, 309.0, 176.0, Some(128.0), [60.0, 149.0]),
            -44.0
        );
        assert_eq!(
            room_offset(427.0, 202.0, 176.0, Some(22.0), [20.0, 149.0]),
            0.0
        );
    }
}
