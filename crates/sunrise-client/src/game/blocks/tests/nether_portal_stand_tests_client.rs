#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::game_tests::tests::server_game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::stand_tests::put_portal;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::nether_portal::stand_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::entities::ArmorStand;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::motion::PlayerMotion;

#[test]
#[ignore = "daily"]
fn the_portal_effect_keeps_the_screens_that_allow_it() {
    let mut resources = None;
    let mut game = server_game(&mut resources);
    // `Screen.isAllowedInPortal` is `isPauseScreen` for these: the pause menu,
    // the options over it and the chat stay open while the effect grows.
    for screen in [Screen::Paused, Screen::Options, Screen::Chat] {
        game.client.screen = screen;
        game.portals.client_inside = true;
        assert!(game.client_portal_effect(), "the effect grows");
        assert!(game.client.screen == screen, "{} stayed open", screen.id());
    }
    // A container screen does not: `Gui.setScreen(null)` closes it.
    game.client.screen = Screen::Inventory;
    game.portals.client_inside = true;
    assert!(game.client_portal_effect(), "the effect grows");
    assert!(
        game.client.screen == Screen::Playing,
        "the inventory closed in the portal"
    );
    // And no screen at all is nothing to close.
    game.portals.client_inside = true;
    assert!(game.client_portal_effect(), "the effect grows");
    assert!(
        game.client.screen == Screen::Playing,
        "playing stays playing"
    );
}
