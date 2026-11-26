

#![cfg(test)]
#[cfg(test)]
pub(crate) mod tests {

use sunrise_gameplay_common::game::native_player_death::tests::at;

use sunrise_gameplay_common::game::native_player_death::tests::new_guest;

use sunrise_gameplay::game::native_player_death::tests::add_guest;

use super::super::native_entity_refs::tests::with_game;

use mod_api::{GlobalPos, Identifier, PlayerRef};

use sunrise_core::position::BlockPos;

#[test]
    #[ignore = "daily"]
    fn a_players_last_death_location_reads_from_their_record() {
        with_game(|game| {
            let player = add_guest(game);
            // Never died.
            assert_eq!(player.last_death_location(&*game), None);

            // A death records the level and block (`ServerPlayer.die`).
            game.side
                .lan
                .as_mut()
                .unwrap()
                .hub
                .guests
                .update(0, |guest| {
                    guest.last_death = Some(("minecraft:overworld".into(), [4, 70, -9]));
                });
            assert_eq!(player.last_death_location(&*game), Some(at([4, 70, -9])));

            // The setter stores it, and None forgets it.
            assert!(player.set_last_death_location(game, Some(at([1, 2, 3]))));
            assert_eq!(
                game.side.lan.as_ref().unwrap().hub.guests[0].last_death,
                Some(("minecraft:overworld".into(), [1, 2, 3]))
            );
            assert_eq!(player.last_death_location(&*game), Some(at([1, 2, 3])));
            assert!(player.set_last_death_location(game, None));
            assert_eq!(player.last_death_location(&*game), None);

            // A player who is not online here, and a handle of another
            // player's id, read nothing.
            let stale = mod_api::__private::game::player_ref(
                mod_api::EntityRef::new(
                    player.fields().entity.epoch(),
                    player.fields().entity.id() + 1,
                ),
                player.uuid(),
            );
            {
                let value = Some(("minecraft:overworld".into(), [1, 2, 3]));
                game.side
                    .lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .update(0, |g| g.last_death = value);
            }
            assert_eq!(stale.last_death_location(&*game), None);
            assert!(!stale.set_last_death_location(game, None));
            {
                let value = false;
                game.side
                    .lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .update(0, |guest| guest.placed = value);
            }
            assert_eq!(player.last_death_location(&*game), None);
            {
                let value = true;
                game.side
                    .lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .update(0, |guest| guest.placed = value);
            }

            // Raw saved data crosses the Identifier boundary only here.
            {
                let value = Some(("not a dimension key".into(), [1, 2, 3]));
                game.side
                    .lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .update(0, |g| g.last_death = value);
            }
            assert_eq!(player.last_death_location(&*game), None);
        });
    }
#[test]
    #[ignore = "daily"]
    fn the_acting_players_location_reads_and_writes_through_their_turn() {
        with_game(|game| {
            let player = add_guest(game);
            {
                let value = Some(("minecraft:overworld".into(), [4, 70, -9]));
                game.side
                    .lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .update(0, |g| g.last_death = value);
            }
            // During the guest's turn the acting player resolves, and the record is the one swapped into the level.
            let seen = game
                .as_guest(1, |game| {
                    let before = player.last_death_location(&*game);
                    let set = player.set_last_death_location(game, Some(at([1, 2, 3])));
                    (before, set, player.last_death_location(&*game))
                })
                .unwrap();
            assert_eq!(seen, (Some(at([4, 70, -9])), true, Some(at([1, 2, 3]))));
            assert_eq!(
                game.side.lan.as_ref().unwrap().hub.guests[0].last_death,
                Some(("minecraft:overworld".into(), [1, 2, 3]))
            );
        });
    }
#[test]
    #[ignore = "daily"]
    fn the_death_location_survives_a_playerdata_save_and_load() {
        use sunrise_sim::lan::playerdata;

        with_game(|game| {
            let player = add_guest(game);
            assert!(player.set_last_death_location(game, Some(at([4, 70, -9]))));

            let world = std::env::temp_dir().join(format!(
                "sunrise-death-location-{}-{:?}",
                std::process::id(),
                std::time::SystemTime::now()
            ));
            std::fs::create_dir_all(&world).unwrap();
            playerdata::save(
                &game.side.lan.as_ref().unwrap().hub.guests[0],
                &world,
                &Default::default(),
            );

            // A fresh record of the same player never died until it loads.
            let mut fresh = new_guest(2, 123);
            assert_eq!(fresh.last_death, None);
            assert!(playerdata::load(&mut fresh, &world));
            assert_eq!(
                fresh.last_death,
                Some(("minecraft:overworld".into(), [4, 70, -9]))
            );

            // Put in place of the saved one, it reads through the handle.
            game.side.lan.as_mut().unwrap().hub.guests.replace(0, fresh);
            assert_eq!(player.last_death_location(&*game), Some(at([4, 70, -9])));
            std::fs::remove_dir_all(&world).unwrap();
        });
    }
#[test]
    #[ignore = "daily"]
    fn a_death_records_the_floored_block_and_dimension() {
        with_game(|game| {
            let player = add_guest(game);
            {
                // Dying in the Nether at a fractional position.
                game.side
                    .lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .update(0, |guest| {
                        guest.dimension = "minecraft:the_nether".into();
                        guest.position = [-1.2, 64.9, 7.1];
                        guest.health = 0.0;
                    });
            }
            game.guest_die(0, 1);

            let guest = &game.side.lan.as_ref().unwrap().hub.guests[0];
            assert!(guest.dead);
            assert_eq!(
                guest.last_death,
                Some(("minecraft:the_nether".into(), [-2, 64, 7]))
            );
            // The Nether is not open here; the location still reads.
            assert_eq!(
                player.last_death_location(&*game),
                Some(GlobalPos::new(
                    Identifier::parse("minecraft:the_nether").unwrap(),
                    BlockPos::new(-2, 64, 7)
                ))
            );
            assert!(!player.set_last_death_location(game, Some(at([1, 2, 3]))));
            assert_eq!(
                player.last_death_location(&*game).unwrap().pos(),
                BlockPos::new(-2, 64, 7)
            );
        });
    }
}
