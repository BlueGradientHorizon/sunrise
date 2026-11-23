//! `PlayerRef::container_counter` and `PlayerRef::next_container_counter`
//! (`ServerPlayer.containerCounter` and `nextContainerCounter`), through the handles a mod calls.

use super::tests::sent;
use crate::game::{
    Game,
    native_entity_refs::tests::with_game,
    native_item_places::tests::{guest, open_to_lan},
};
use mod_api::{EntityRef, Host, MenuReplace, ServerHost, WorldEpoch};
use std::sync::{Arc, mpsc};
use sunrise_client::lan::{Out, PlayerSender};

/// Guest 1 (UUID 123) on a world open to LAN, and what their client is sent.
fn watched_guest(game: &mut Game<'_>) -> mpsc::Receiver<Out> {
    open_to_lan(game);
    let (tx, rx) = mpsc::channel();
    let mut player = guest(1);
    player.sender = PlayerSender::new(tx, Arc::default());
    game.lan.as_mut().unwrap().hub.guests.push(player);
    rx
}

/// A one-row chest display for UUID 123 (`Host::open_menu`), replacing the open menu: its window.
fn open(game: &mut Game<'_>) -> i32 {
    let host: &mut dyn ServerHost = game;
    host.open_menu(
        123,
        1,
        mod_api::Text::literal("chest"),
        Vec::new(),
        MenuReplace::CloseFirst,
    )
    .unwrap()
    .unwrap()
}

/// The counter is the window the last open took; an advance moves it as an open does
/// (`counter % 100 + 1`, so 100 wraps to 1) without opening or closing anything, and the next
/// open takes the id after it.
#[test]
#[ignore = "daily"]
fn the_container_counter_is_the_last_window_and_advances_as_an_open_does() {
    crate::game::in_own_process(
        "game::native_menus::container_counter_tests::the_container_counter_is_the_last_window_and_advances_as_an_open_does",
        || {
            with_game(|game| {
                let rx = watched_guest(game);
                let player = game.player(123).unwrap();
                let counter = |game: &Game<'_>| player.container_counter(game as &dyn Host);
                assert_eq!(counter(game), Some(0), "no menu has opened yet");
                let first = open(game);
                assert_eq!((first, counter(game)), (1, Some(1)));
                sent(&rx);

                let host: &mut dyn ServerHost = game;
                assert_eq!(player.next_container_counter(host), Some(2));
                assert!(
                    host.menu(123, first).is_some(),
                    "the open menu keeps its window"
                );
                assert!(sent(&rx).is_empty(), "an advance sends nothing");
                assert_eq!(counter(game), Some(2));
                assert_eq!(
                    open(game),
                    3,
                    "the next open takes the id after the advance"
                );

                let host: &mut dyn ServerHost = game;
                let ids: Vec<i32> = (0..98)
                    .map(|_| player.next_container_counter(host).unwrap())
                    .collect();
                assert_eq!(ids, (4..=100).chain([1]).collect::<Vec<_>>());
                assert_eq!(open(game), 2);

                let stale = mod_api::__private::game::player_ref(
                    EntityRef::new(
                        WorldEpoch::new(player.entity().epoch().get() + 1),
                        player.entity().id(),
                    ),
                    player.uuid(),
                );
                let host: &mut dyn ServerHost = game;
                assert_eq!(stale.container_counter(host), None);
                assert_eq!(stale.next_container_counter(host), None);
                assert_eq!(counter(game), Some(2), "a stale handle advances nothing");
            });
        },
    );
}
