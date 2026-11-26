use super::*;

#[test]
#[ignore = "daily"]
fn the_portal_effect_keeps_the_screens_that_allow_it() {
    sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
        // `Screen.isAllowedInPortal` is `isPauseScreen` for these: the pause menu,
        // the options over it and the chat stay open while the effect grows.
        for screen in [Screen::Paused, Screen::Options, Screen::Chat] {
            game.side.screen = screen;
            game.side.client_inside = true;
            assert!(game.client_portal_effect(), "the effect grows");
            assert!(game.side.screen == screen, "{} stayed open", screen.id());
        }
        // A container screen does not: `Gui.setScreen(null)` closes it.
        game.side.screen = Screen::Inventory;
        game.side.client_inside = true;
        assert!(game.client_portal_effect(), "the effect grows");
        assert!(
            game.side.screen == Screen::Playing,
            "the inventory closed in the portal"
        );
        // And no screen at all is nothing to close.
        game.side.client_inside = true;
        assert!(game.client_portal_effect(), "the effect grows");
        assert!(game.side.screen == Screen::Playing, "playing stays playing");
    });
}
