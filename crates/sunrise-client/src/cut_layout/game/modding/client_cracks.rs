

#![cfg(test)]
#[cfg(feature = "client")]
#[cfg(test)]
pub(crate) mod tests {
    use {sunrise_gameplay_common::game::Mining, sunrise_client_gameplay::game::client_side::native_client_session::tests::logged_client, crate::game::server_side::native_entity_refs::tests::with_client_game};
    use mod_api::{
        BlockPart, BlockPos, BreakProgressPositionsEvent, ClientHost, Hooks, ModEntry, Registrar,
        RegistrationContext,
    };
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use sunrise_level::chunk::Chunk;

    struct ExtraPositions {
        calls: Arc<Mutex<Vec<u8>>>,
        invalid: Arc<AtomicBool>,
        reenter: bool,
    }
    #[mod_api::sunrise_part]
    impl BlockPart for ExtraPositions {
        fn break_progress_positions(
            &self,
            host: &mut dyn ClientHost,
            event: &mut BreakProgressPositionsEvent<'_>,
        ) {
            let count = {
                let mut calls = self.calls.lock().unwrap();
                calls.push(event.progress);
                calls.len()
            };
            let p = event.pos;
            // Bound the fixture's recursion so a regression fails the assertion
            // rather than overflowing the test process's stack.
            if self.reenter && count == 1 {
                assert!(event.level.set_client_break_progress(
                    host,
                    -1,
                    [p.x, p.y, p.z],
                    i32::from(event.progress),
                ));
            }
            event.positions.extend([
                BlockPos::new(p.x + 1, p.y, p.z),
                BlockPos::new(p.x, p.y, p.z + 1),
                BlockPos::new(p.x + 1, p.y, p.z + 1),
            ]);
            if self.invalid.load(Ordering::Relaxed) {
                event.positions.extend([
                    p,
                    BlockPos::new(p.x + 1, p.y, p.z),
                    BlockPos::new(p.x + 8, p.y, p.z),
                    BlockPos::new(p.x + 9, p.y, p.z),
                    BlockPos::new(p.x + 8, p.y + 8, p.z),
                    BlockPos::new(i32::MIN, i32::MAX, i32::MIN),
                ]);
                for x in -7..=7 {
                    for z in -7..=7 {
                        if x * x + z * z <= 64 {
                            event.positions.push(BlockPos::new(p.x + x, p.y, p.z + z));
                        }
                    }
                }
            }
        }
    }
    struct Register {
        calls: Arc<Mutex<Vec<u8>>>,
        invalid: Arc<AtomicBool>,
        reenter: bool,
    }
    #[mod_api::__subscriptions]
    impl Hooks for Register {
        fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
            let key = r.block_key("minecraft:stone");
            r.block_part(
                key,
                ExtraPositions {
                    calls: self.calls.clone(),
                    invalid: self.invalid.clone(),
                    reenter: self.reenter,
                },
            );
        }
    }

    #[test]
    #[ignore = "daily"]
    fn crack_positions_are_scoped_stored_bounded_and_cleared_for_both_sources() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let invalid = Arc::new(AtomicBool::new(false));
        let mods = vec![ModEntry::new(
            "cracks",
            Box::new(Register {
                calls: calls.clone(),
                invalid: invalid.clone(),
                reenter: false,
            }),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_client_game(|game| {
                let _server = logged_client(game);
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                let stone = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap();
                let dirt = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:dirt")
                    .unwrap();
                let pos = [4, 64, 4];
                let four = [[4, 64, 4], [4, 64, 5], [5, 64, 4], [5, 64, 5]];
                for p in four {
                    game.world.set(p[0], p[1], p[2], stone).unwrap();
                }
                game.client.settings.improved_transparency = false;
                let camera = glam::DVec3::new(4.5, 65.0, 4.5);
                game.destroy_block_progress(10, pos, 2);
                let rendered = game.cracks(camera);
                assert_eq!(rendered.len(), 4);
                let sprite = &game.client.gui.atlas.sprites["block/destroy_stage_2"];
                assert!(rendered.iter().all(|(_, uv)| *uv == [sprite.u0, sprite.v0]));
                let mut expected = four.map(|p| (p, 2)).to_vec();
                expected.sort_unstable();
                let mut drawn = game.crack_stages();
                drawn.sort_unstable();
                assert_eq!(drawn, expected);
                for _ in 0..3 {
                    assert_eq!(game.crack_stages().len(), 4);
                }
                game.destroy_block_progress(10, pos, 2);
                assert_eq!(
                    *calls.lock().unwrap(),
                    [2],
                    "frames and unchanged stages never redispatch"
                );
                game.destroy_block_progress(10, pos, 7);
                assert!(game.crack_stages().iter().all(|(_, stage)| *stage == 7));
                let rendered = game.cracks(camera);
                assert_eq!(rendered.len(), 4);
                let sprite = &game.client.gui.atlas.sprites["block/destroy_stage_7"];
                assert!(rendered.iter().all(|(_, uv)| *uv == [sprite.u0, sprite.v0]));
                assert_eq!(*calls.lock().unwrap(), [2, 7]);
                for clear in [-1, 10] {
                    game.destroy_block_progress(10, pos, clear);
                    assert!(game.crack_stages().is_empty());
                }
                let mining = |progress| {
                    Some(Mining {
                        pos,
                        progress,
                        per_tick: 0.0,
                        ticks: 1,
                    })
                };
                game.set_client_mining(mining(0.35));
                assert_eq!(
                    game.crack_stages().len(),
                    4,
                    "local prediction includes the extras"
                );
                assert_eq!(game.cracks(camera).len(), 4);
                assert!(game.crack_stages().iter().all(|(_, stage)| *stage == 3));
                game.set_client_mining(mining(0.39));
                assert_eq!(*calls.lock().unwrap(), [2, 7, 3]);
                game.set_client_mining(mining(0.45));
                assert!(game.crack_stages().iter().all(|(_, stage)| *stage == 4));
                game.set_client_mining(None);
                assert!(game.client.mining_crack.is_none());
                assert!(game.crack_stages().is_empty());
                game.world.set(9, 64, 4, dirt).unwrap();
                game.destroy_block_progress(11, [9, 64, 4], 1);
                assert_eq!(game.crack_stages(), [([9, 64, 4], 1)]);
                assert_eq!(
                    *calls.lock().unwrap(),
                    [2, 7, 3, 4],
                    "an unsubscribed state is not dispatched"
                );
                game.destroy_block_progress(11, [9, 64, 4], -1);
                invalid.store(true, Ordering::Relaxed);
                game.destroy_block_progress(10, pos, 5);
                let drawn = game.crack_stages();
                assert_eq!(drawn.len(), 65, "base plus at most 64 display extras");
                assert_eq!(
                    drawn
                        .iter()
                        .map(|c| c.0)
                        .collect::<std::collections::HashSet<_>>()
                        .len(),
                    65
                );
                assert!(
                    drawn.contains(&([12, 64, 4], 5)),
                    "eight-block boundary is included"
                );
                assert!(!drawn.iter().any(|(p, _)| {
                    [[13, 64, 4], [12, 72, 4], [i32::MIN, i32::MAX, i32::MIN]].contains(p)
                }));
                game.world.game_time += 100;
                let calls_before = calls.lock().unwrap().len();
                game.destroy_block_progress(10, pos, 5);
                assert_eq!(calls.lock().unwrap().len(), calls_before);
                game.world.game_time += 400;
                game.expire_client_cracks();
                assert_eq!(game.client.destroying.len(), 1, "400 ticks is still live");
                game.world.game_time += 1;
                game.expire_client_cracks();
                assert!(
                    game.client.destroying.is_empty(),
                    "expiry drops the stored extras too"
                );
                game.destroy_block_progress(10, pos, 0);
                game.set_client_mining(mining(0.25));
                game.reset_client_world(0);
                assert!(game.client.destroying.is_empty());
                assert!(game.mining.is_none());
                assert!(game.client.mining_crack.is_none());
                assert!(game.crack_stages().is_empty());
            })
        });
    }

    #[test]
    #[ignore = "daily"]
    fn reentrant_same_stage_client_crack_update_dispatches_once() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![ModEntry::new(
            "reentrant-cracks",
            Box::new(Register {
                calls: calls.clone(),
                invalid: Arc::new(AtomicBool::new(false)),
                reenter: true,
            }),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_client_game(|game| {
                let _server = logged_client(game);
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                let stone = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap();
                let pos = [4, 64, 4];
                game.world.set(pos[0], pos[1], pos[2], stone).unwrap();
                game.destroy_block_progress(-1, pos, 3);
                assert_eq!(*calls.lock().unwrap(), [3]);
                let crack = &game.client.destroying[&-1];
                assert_eq!((crack.pos, crack.stage), (pos, 3));
                assert_eq!(crack.positions, [[5, 64, 4], [4, 64, 5], [5, 64, 5]]);
            })
        });
    }

    #[test]
    #[ignore = "daily"]
    fn client_cracks_clear_expire_and_do_not_cross_worlds() {
        super::super::native_entity_refs::tests::with_client_game(|game| {
            game.destroy_block_progress(-10, [1, 64, 1], 4);
            game.destroy_block_progress(-11, [2, 64, 1], 9);
            assert_eq!(game.client.destroying.len(), 2);
            game.destroy_block_progress(-10, [1, 64, 1], -1);
            assert!(!game.client.destroying.contains_key(&-10));
            assert!(game.client.destroying.contains_key(&-11));
            game.world.game_time += 401;
            game.tick().unwrap();
            assert_eq!(
                game.ticks, 0,
                "a client without a remote must not tick a server level"
            );
            assert!(game.client.destroying.is_empty());
            game.destroy_block_progress(-11, [2, 64, 1], 3);
            game.reset_client_world(0);
            assert!(game.client.destroying.is_empty());
        });
    }
}