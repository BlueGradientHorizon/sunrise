//! `ClientViewClientHost::set_overlay_message` (#4626): `Hud.setOverlayMessage` on the
//! window's one action-bar slot. Each call replaces the line and restarts the stock timer;
//! refused text leaves the previous line, and nothing reaches chat.
use mod_api::{ClientHost, Text, TextFormError};

/// What the action bar holds: its plain line, ticks left and whether its colour cycles.
fn shown(game: &super::Game<'_>) -> Option<(String, i32, bool)> {
    let titles = &game.titles;
    let line = titles.overlay.as_ref()?.plain();
    Some((line, titles.overlay_time, titles.overlay_animated))
}

#[test]
#[ignore = "daily"]
fn a_mods_overlay_line_replaces_the_action_bar_and_restarts_its_timer() {
    crate::game::native_entity_refs::tests::with_client_game(|game| {
        let chat = game.ui.chat.recent_lines(usize::MAX);
        let host: &mut dyn ClientHost = game;
        assert_eq!(
            host.set_overlay_message(Text::literal("Now Playing"), true),
            Ok(())
        );
        assert_eq!(shown(game), Some(("Now Playing".into(), 60, true)));
        for _ in 0..25 {
            game.titles.tick();
        }
        assert_eq!(shown(game), Some(("Now Playing".into(), 35, true)));

        let host: &mut dyn ClientHost = game;
        assert_eq!(
            host.set_overlay_message(Text::literal("Saved"), false),
            Ok(())
        );
        assert_eq!(shown(game), Some(("Saved".into(), 60, false)));
        game.titles.tick();

        let mut refused = Text::literal("lost");
        refused.metadata = sunrise_core::nbt::Tag::Int(7);
        let host: &mut dyn ClientHost = game;
        assert_eq!(
            host.set_overlay_message(refused, true),
            Err(TextFormError::Malformed)
        );
        assert_eq!(shown(game), Some(("Saved".into(), 59, false)));

        let host: &mut dyn ClientHost = game;
        assert_eq!(host.set_overlay_message(Text::literal(""), false), Ok(()));
        assert_eq!(shown(game), Some((String::new(), 60, false)));
        assert_eq!(game.ui.chat.recent_lines(usize::MAX), chat);
    });
}
