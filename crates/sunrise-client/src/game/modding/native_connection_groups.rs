//! Per-position connected-texture groups (#3843): client-side receive handler for
//! `sunrise:connection_groups` (`Chunk` snapshots and incremental `Update` deltas).
use super::Game;
use sunrise_protocol::connection_groups::{self, Message};

/// Apply a decoded `sunrise:connection_groups` message to a client world (the player's active
/// world or a viewed level's world), marking affected sections and cross-border neighbours for
/// remesh.
pub(in crate::game) fn apply_connection_groups(
    world: &mut sunrise_client::world::World,
    message: Message,
) {
    match message {
        Message::Chunk {
            cx,
            cz,
            replace,
            entries,
        } => {
            world.apply_chunk_connection_groups(cx, cz, replace, entries);
        }
        Message::Update { entries } => {
            for entry in entries {
                let [x, y, z] = entry.pos;
                let _ = world.set_connection_group(x, y, z, entry.group, false);
            }
        }
    }
}

impl Game<'_> {
    /// Apply a `sunrise:connection_groups` play clientbound payload from the server to the
    /// client's current world and mark affected sections (and cross-border neighbours) for
    /// remesh.
    pub(super) fn remote_connection_groups(&mut self, data: &[u8]) {
        if self.remote.is_none() {
            return;
        }
        let message = match connection_groups::decode(data) {
            Ok(message) => message,
            Err(e) => {
                log::debug!("Sunrise: bad connection_groups payload: {e}");
                return;
            }
        };
        apply_connection_groups(&mut self.world, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{
        Dim,
        execution_role::ExecutionRole,
        native_entity_refs::tests::with_game,
        viewed_worlds::tests::{
            client as viewed_client, column as viewed_column, open as viewed_open,
            run as viewed_run,
        },
    };
    use mod_api::{
        BlockEdit, BlockRun, BlockUpdateFlags, Host, HostTransaction, LevelReadError, LevelRef,
        LevelWriteError, ServerHost, WorldEpoch,
    };
    use std::sync::Arc;
    use sunrise_client::{
        chunk::{Chunk, MIN_Y},
        ticket_levels::{Sources, ViewedTicket},
    };
    use sunrise_protocol::{
        connection::Pipe,
        connection_groups::{ChunkEntry, UpdateEntry},
        packets::{custom_payload::CustomPayload, play::Clientbound as C},
    };

    fn state(game: &Game<'_>, name: &str) -> mod_api::BlockState {
        let host: &dyn Host = game;
        host.block_state_id(name).unwrap()
    }

    #[test]
    #[ignore = "daily"]
    fn connection_group_api_errors_state_preservation_and_block_replacement() {
        with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.native_lifecycle.world_open_ready().unwrap();
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            game.world.connection_group_log = Some(Vec::new());

            let level = game.level("minecraft:overworld").unwrap();
            let log_y = state(game, "minecraft:oak_log[axis=y]");
            let log_x = state(game, "minecraft:oak_log[axis=x]");
            let stone = state(game, "minecraft:stone");
            let glass = state(game, "minecraft:glass");
            let copper_chest = state(game, "minecraft:copper_chest");
            let exposed_copper_chest = state(game, "minecraft:exposed_copper_chest");

            assert_eq!(
                level.set_block(game, [2, 64, 3], log_y, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [2, 64, 3]), Ok(None));

            // Group 0 is a valid value; repeating it reports Ok(false) without extra log entries.
            assert_eq!(
                level.set_connection_group(game, [2, 64, 3], Some(0)),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [2, 64, 3]), Ok(Some(0)));
            assert_eq!(
                level.set_connection_group(game, [2, 64, 3], Some(0)),
                Ok(false)
            );
            assert_eq!(
                game.world.connection_group_log.as_deref(),
                Some(&[(2, 64, 3)][..])
            );

            // Unloaded chunk, out-of-height, and carrier-fenced positions return Ok(false).
            assert_eq!(
                level.set_connection_group(game, [32, 64, 0], Some(1)),
                Ok(false)
            );
            assert_eq!(
                level.set_connection_group(game, [2, 9999, 3], Some(1)),
                Ok(false)
            );
            game.world.carrier_fences.lock_cell([2, 64, 3]);
            assert_eq!(
                level.set_connection_group(game, [2, 64, 3], Some(9)),
                Ok(false)
            );
            assert_eq!(level.connection_group(game, [2, 64, 3]), Ok(Some(0)));
            game.world.carrier_fences.unlock_cell([2, 64, 3]);
            assert_eq!(level.connection_group(game, [32, 64, 0]), Ok(None));
            assert_eq!(level.connection_group(game, [2, 9999, 3]), Ok(None));

            // Stale handle refuses on both read and write.
            let stale = LevelRef::new(WorldEpoch::new(0), level.dimension());
            assert_eq!(
                stale.connection_group(game, [2, 64, 3]),
                Err(LevelReadError::Stale)
            );
            assert_eq!(
                stale.set_connection_group(game, [2, 64, 3], Some(1)),
                Err(LevelWriteError::Stale)
            );

            // Same-block state change preserves the connection group.
            game.world.connection_group_log.as_mut().unwrap().clear();
            assert_eq!(
                level.set_block(game, [2, 64, 3], log_x, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [2, 64, 3]), Ok(Some(0)));
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());

            // Replacing with a different block clears storage automatically without logging an
            // explicit connection-group entry (clients derive the group when the block is sent).
            assert_eq!(
                level.set_block(game, [2, 64, 3], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [2, 64, 3]), Ok(None));
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());

            // Weathering a copper chest preserves the connection group (same rule as block entities).
            assert_eq!(
                level.set_block(game, [2, 64, 3], copper_chest, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [2, 64, 3], Some(7)),
                Ok(true)
            );
            game.world.connection_group_log.as_mut().unwrap().clear();
            assert_eq!(
                level.set_block(
                    game,
                    [2, 64, 3],
                    exposed_copper_chest,
                    BlockUpdateFlags::NONE
                ),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [2, 64, 3]), Ok(Some(7)));
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());

            // Bulk block edits follow the same keeps_connection_group rule: same-block state
            // changes and copper-chest weathering preserve groups; replacing with another block
            // clears them.
            assert_eq!(
                level.set_block(game, [5, 64, 5], log_y, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [5, 64, 5], Some(1)),
                Ok(true)
            );
            assert_eq!(
                level.set_block(game, [6, 64, 5], copper_chest, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [6, 64, 5], Some(2)),
                Ok(true)
            );
            assert_eq!(
                level.set_block(game, [7, 64, 5], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [7, 64, 5], Some(3)),
                Ok(true)
            );
            let edit = BlockEdit {
                palette: vec![log_x, exposed_copper_chest, glass],
                filters: Vec::new(),
                runs: vec![
                    BlockRun {
                        x: 5,
                        z: 5,
                        y: 64,
                        height: 1,
                        surface: None,
                        block: 0,
                        only: None,
                    },
                    BlockRun {
                        x: 6,
                        z: 5,
                        y: 64,
                        height: 1,
                        surface: None,
                        block: 1,
                        only: None,
                    },
                    BlockRun {
                        x: 7,
                        z: 5,
                        y: 64,
                        height: 1,
                        surface: None,
                        block: 2,
                        only: None,
                    },
                ],
            };
            assert_eq!(game.apply_block_edit(&edit), Ok(3));
            assert_eq!(level.connection_group(game, [5, 64, 5]), Ok(Some(1)));
            assert_eq!(level.connection_group(game, [6, 64, 5]), Ok(Some(2)));
            assert_eq!(level.connection_group(game, [7, 64, 5]), Ok(None));

            // Parked level refuses writes with Unavailable while reads still succeed.
            assert_eq!(
                level.set_connection_group(game, [2, 64, 3], Some(19)),
                Ok(true)
            );
            assert!(game.enter_command_level(Dim::Nether).is_some());
            assert_eq!(level.connection_group(game, [2, 64, 3]), Ok(Some(19)));
            assert_eq!(
                level.set_connection_group(game, [2, 64, 3], Some(20)),
                Err(LevelWriteError::Unavailable)
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn connection_group_transactions_abort_silently_and_commit_with_block_writes() {
        with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.native_lifecycle.world_open_ready().unwrap();
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            game.world.block_log = Some(Vec::new());
            game.world.connection_group_log = Some(Vec::new());
            let non_ticking = Sources {
                viewed: vec![ViewedTicket {
                    center: (0, 0),
                    radius: 0,
                    ticking: false,
                }],
                ..Sources::default()
            };
            game.tickets.update(Some(non_ticking.clone()));
            assert!(!game.tickets.levels().block_ticking(0, 0));

            let level = game.level("minecraft:overworld").unwrap();
            let stone = state(game, "minecraft:stone");
            let glass = state(game, "minecraft:glass");
            let log_y = state(game, "minecraft:oak_log[axis=y]");
            let log_x = state(game, "minecraft:oak_log[axis=x]");
            let torch = state(game, "minecraft:torch");
            let air = state(game, "minecraft:air");
            assert_eq!(
                level.set_block(game, [4, 64, 4], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [4, 64, 4], Some(11)),
                Ok(true)
            );
            game.world.dirty_sections.clear();
            game.world.block_log.as_mut().unwrap().clear();
            game.world.connection_group_log.as_mut().unwrap().clear();

            // Aborted transaction sees tentative write inside, restores silently on abort.
            let abort_seal = game.transaction_open();
            assert_eq!(
                level.set_connection_group(game, [4, 64, 4], Some(22)),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [4, 64, 4]), Ok(Some(22)));
            game.transaction_close(&abort_seal, false);
            assert_eq!(level.connection_group(game, [4, 64, 4]), Ok(Some(11)));
            assert!(game.world.dirty_sections.is_empty());
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());

            // Aborted transaction replacing a block restores the cleared group silently.
            let abort_replace = game.transaction_open();
            assert_eq!(
                level.set_block(game, [4, 64, 4], glass, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [4, 64, 4]), Ok(None));
            game.transaction_close(&abort_replace, false);
            assert_eq!(level.connection_group(game, [4, 64, 4]), Ok(Some(11)));
            assert!(game.world.dirty_sections.is_empty());
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());

            // Committed transaction replacing a block without setting a new group clears storage
            // automatically without logging an explicit connection-group entry.
            let commit_clear = game.transaction_open();
            assert_eq!(
                level.set_block(game, [4, 64, 4], glass, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [4, 64, 4]), Ok(None));
            game.transaction_close(&commit_clear, true);
            assert_eq!(level.connection_group(game, [4, 64, 4]), Ok(None));
            assert!(game.world.block_log.as_ref().unwrap().is_empty());
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());

            // Committed transaction replacing a block and setting a new group keeps the new group
            // and logs only the explicit write.
            assert_eq!(
                level.set_connection_group(game, [4, 64, 4], Some(11)),
                Ok(true)
            );
            game.world.dirty_sections.clear();
            game.world.block_log.as_mut().unwrap().clear();
            game.world.connection_group_log.as_mut().unwrap().clear();
            let commit_seal = game.transaction_open();
            assert_eq!(
                level.set_block(game, [4, 64, 4], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [4, 64, 4]), Ok(None));
            assert_eq!(
                level.set_connection_group(game, [4, 64, 4], Some(33)),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [4, 64, 4]), Ok(Some(33)));
            let sy = (64 - MIN_Y) >> 4;
            game.transaction_close(&commit_seal, true);
            assert_eq!(level.connection_group(game, [4, 64, 4]), Ok(Some(33)));
            assert!(game.world.dirty_sections.contains(&(0, sy, 0)));
            assert_eq!(
                game.world.connection_group_log.as_deref(),
                Some(&[(4, 64, 4)][..])
            );

            // Held non-ticking column round-trip (`oak_log[y] -> stone -> oak_log[x]`): clears
            // storage without logging to either `block_log` or `connection_group_log`.
            assert_eq!(
                level.set_block(game, [5, 64, 5], log_y, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [5, 64, 5], Some(7)),
                Ok(true)
            );
            game.world.block_log.as_mut().unwrap().clear();
            game.world.connection_group_log.as_mut().unwrap().clear();
            let round_trip = game.transaction_open();
            assert_eq!(
                level.set_block(game, [5, 64, 5], stone, BlockUpdateFlags::CLIENTS),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [5, 64, 5]), Ok(None));
            assert_eq!(
                level.set_block(game, [5, 64, 5], log_x, BlockUpdateFlags::CLIENTS),
                Ok(true)
            );
            game.transaction_close(&round_trip, true);
            assert_eq!(level.connection_group(game, [5, 64, 5]), Ok(None));
            assert!(game.world.block_log.as_ref().unwrap().is_empty());
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());

            // Held non-ticking column with explicit write sandwiched between two replacing writes
            // (`oak_log[y] -> stone -> set_connection_group(99) -> oak_log[x]`): second replace
            // clears storage and drops the superseded explicit log entry without leaking `None`.
            assert_eq!(
                level.set_block(game, [7, 64, 7], log_y, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [7, 64, 7], Some(7)),
                Ok(true)
            );
            game.world.block_log.as_mut().unwrap().clear();
            game.world.connection_group_log.as_mut().unwrap().clear();
            let sandwich = game.transaction_open();
            assert_eq!(
                level.set_block(game, [7, 64, 7], stone, BlockUpdateFlags::CLIENTS),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [7, 64, 7], Some(99)),
                Ok(true)
            );
            assert_eq!(
                level.set_block(game, [7, 64, 7], log_x, BlockUpdateFlags::CLIENTS),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [7, 64, 7]), Ok(None));
            game.transaction_close(&sandwich, true);
            assert_eq!(level.connection_group(game, [7, 64, 7]), Ok(None));
            assert!(game.world.block_log.as_ref().unwrap().is_empty());
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());

            // Held non-ticking column `now == was` round-trip (`oak_log[y] -> stone -> oak_log[y]`):
            // clears storage without leaking a synthetic connection-group log entry.
            assert_eq!(
                level.set_block(game, [7, 65, 7], log_y, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [7, 65, 7], Some(13)),
                Ok(true)
            );
            game.world.block_log.as_mut().unwrap().clear();
            game.world.connection_group_log.as_mut().unwrap().clear();
            let same_state = game.transaction_open();
            assert_eq!(
                level.set_block(game, [7, 65, 7], stone, BlockUpdateFlags::CLIENTS),
                Ok(true)
            );
            assert_eq!(
                level.set_block(game, [7, 65, 7], log_y, BlockUpdateFlags::CLIENTS),
                Ok(true)
            );
            game.transaction_close(&same_state, true);
            assert_eq!(level.connection_group(game, [7, 65, 7]), Ok(None));
            assert!(game.world.block_log.as_ref().unwrap().is_empty());
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());

            // The same notified round-trip in a ticking column queues a real block update.
            // The broadcast derives the clear from the same cell as the queued block.
            game.tickets.update(Some(Sources {
                forced: vec![(0, 0)],
                ..Sources::default()
            }));
            assert!(game.tickets.levels().block_ticking(0, 0));
            assert_eq!(
                level.set_connection_group(game, [7, 65, 7], Some(14)),
                Ok(true)
            );
            game.world.connection_group_log.as_mut().unwrap().clear();
            let same_state = game.transaction_open();
            assert_eq!(
                level.set_block(game, [7, 65, 7], stone, BlockUpdateFlags::CLIENTS),
                Ok(true)
            );
            assert_eq!(
                level.set_block(game, [7, 65, 7], log_y, BlockUpdateFlags::CLIENTS),
                Ok(true)
            );
            game.transaction_close(&same_state, true);
            assert_eq!(level.connection_group(game, [7, 65, 7]), Ok(None));
            assert_eq!(game.world.block_log.as_deref(), Some(&[(7, 65, 7)][..]));
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());
            game.tickets.update(Some(non_ticking));

            // Cross-position cascade on held non-ticking column: `set_connection_group(torch)`
            // followed by breaking its support pops the torch, clears its group, and drops the
            // superseded explicit log entry.
            assert_eq!(
                level.set_block(game, [6, 64, 6], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_block(game, [6, 65, 6], torch, BlockUpdateFlags::NONE),
                Ok(true)
            );
            game.world.block_log.as_mut().unwrap().clear();
            game.world.connection_group_log.as_mut().unwrap().clear();
            let cascade = game.transaction_open();
            assert_eq!(
                level.set_connection_group(game, [6, 65, 6], Some(42)),
                Ok(true)
            );
            assert_eq!(
                level.set_block(
                    game,
                    [6, 64, 6],
                    air,
                    BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
                ),
                Ok(true)
            );
            game.transaction_close(&cascade, true);
            assert_eq!(game.world.block(6, 65, 6), game.world.air);
            assert_eq!(level.connection_group(game, [6, 65, 6]), Ok(None));
            assert!(game.world.block_log.as_ref().unwrap().is_empty());
            assert!(game.world.connection_group_log.as_ref().unwrap().is_empty());

            // Bulk `ServerHost::edit_blocks` inside `HostTransaction::transaction`: permanent
            // replacement reconciles the open transaction's restore baseline and drops earlier
            // deferred group writes so neither commit nor abort resurrects a group on the new
            // block; writes after the bulk replacement still commit/abort against `None`.
            let single_replace = |pos: [i32; 3], to: mod_api::BlockState| BlockEdit {
                palette: vec![to],
                filters: Vec::new(),
                runs: vec![BlockRun {
                    x: pos[0],
                    z: pos[2],
                    y: pos[1],
                    height: 1,
                    surface: None,
                    block: 0,
                    only: None,
                }],
            };

            // Commit after set_connection_group + bulk replace.
            assert_eq!(
                level.set_block(game, [8, 64, 8], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [8, 64, 8], Some(1)),
                Ok(true)
            );
            let edit_commit = single_replace([8, 64, 8], glass);
            {
                let host: &mut dyn ServerHost = game;
                let res: Result<(), ()> = host.transaction(|host, _tx| {
                    assert_eq!(
                        level.set_connection_group(host, [8, 64, 8], Some(2)),
                        Ok(true)
                    );
                    assert_eq!(host.edit_blocks("minecraft:overworld", &edit_commit), Ok(1));
                    assert_eq!(level.connection_group(host, [8, 64, 8]), Ok(None));
                    Ok(())
                });
                assert_eq!(res, Ok(()));
            }
            assert_eq!(level.block_state(game, [8, 64, 8]), Some(glass));
            assert_eq!(level.connection_group(game, [8, 64, 8]), Ok(None));

            // Abort after set_connection_group(Some) + bulk replace.
            assert_eq!(
                level.set_block(game, [9, 64, 9], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [9, 64, 9], Some(1)),
                Ok(true)
            );
            let edit_abort = single_replace([9, 64, 9], glass);
            {
                let host: &mut dyn ServerHost = game;
                let res: Result<(), ()> = host.transaction(|host, _tx| {
                    assert_eq!(
                        level.set_connection_group(host, [9, 64, 9], Some(2)),
                        Ok(true)
                    );
                    assert_eq!(host.edit_blocks("minecraft:overworld", &edit_abort), Ok(1));
                    Err(())
                });
                assert_eq!(res, Err(()));
            }
            assert_eq!(level.block_state(game, [9, 64, 9]), Some(glass));
            assert_eq!(level.connection_group(game, [9, 64, 9]), Ok(None));

            // Abort after tentative set_connection_group(None) + bulk replace.
            assert_eq!(
                level.set_block(game, [10, 64, 10], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [10, 64, 10], Some(1)),
                Ok(true)
            );
            let edit_tentative_none = single_replace([10, 64, 10], glass);
            {
                let host: &mut dyn ServerHost = game;
                let res: Result<(), ()> = host.transaction(|host, _tx| {
                    assert_eq!(
                        level.set_connection_group(host, [10, 64, 10], None),
                        Ok(true)
                    );
                    assert_eq!(
                        host.edit_blocks("minecraft:overworld", &edit_tentative_none),
                        Ok(1)
                    );
                    Err(())
                });
                assert_eq!(res, Err(()));
            }
            assert_eq!(level.block_state(game, [10, 64, 10]), Some(glass));
            assert_eq!(level.connection_group(game, [10, 64, 10]), Ok(None));

            // Writing a fresh group AFTER bulk replace in the same transaction commits or aborts
            // cleanly against the reconciled `None` baseline.
            assert_eq!(
                level.set_block(game, [11, 64, 11], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [11, 64, 11], Some(1)),
                Ok(true)
            );
            let edit_after_commit = single_replace([11, 64, 11], glass);
            {
                let host: &mut dyn ServerHost = game;
                let res: Result<(), ()> = host.transaction(|host, _tx| {
                    assert_eq!(
                        level.set_connection_group(host, [11, 64, 11], Some(2)),
                        Ok(true)
                    );
                    assert_eq!(
                        host.edit_blocks("minecraft:overworld", &edit_after_commit),
                        Ok(1)
                    );
                    assert_eq!(
                        level.set_connection_group(host, [11, 64, 11], Some(3)),
                        Ok(true)
                    );
                    Ok(())
                });
                assert_eq!(res, Ok(()));
            }
            assert_eq!(level.block_state(game, [11, 64, 11]), Some(glass));
            assert_eq!(level.connection_group(game, [11, 64, 11]), Ok(Some(3)));

            assert_eq!(
                level.set_block(game, [12, 64, 12], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [12, 64, 12], Some(1)),
                Ok(true)
            );
            let edit_after_abort = single_replace([12, 64, 12], glass);
            {
                let host: &mut dyn ServerHost = game;
                let res: Result<(), ()> = host.transaction(|host, _tx| {
                    assert_eq!(
                        level.set_connection_group(host, [12, 64, 12], Some(2)),
                        Ok(true)
                    );
                    assert_eq!(
                        host.edit_blocks("minecraft:overworld", &edit_after_abort),
                        Ok(1)
                    );
                    assert_eq!(
                        level.set_connection_group(host, [12, 64, 12], Some(3)),
                        Ok(true)
                    );
                    Err(())
                });
                assert_eq!(res, Err(()));
            }
            assert_eq!(level.block_state(game, [12, 64, 12]), Some(glass));
            assert_eq!(level.connection_group(game, [12, 64, 12]), Ok(None));

            // A nested bulk round-trip still replaces the block twice. At edit time the
            // inner transaction has hidden the group, but neither abort may revive it.
            let pos = [13, 64, 13];
            assert_eq!(
                level.set_block(game, pos, stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(level.set_connection_group(game, pos, Some(1)), Ok(true));
            let mut round_trip = single_replace(pos, glass);
            round_trip.palette.push(stone);
            round_trip.runs.push(BlockRun {
                block: 1,
                ..round_trip.runs[0]
            });
            let outer = game.transaction_open();
            assert_eq!(level.set_connection_group(game, pos, Some(2)), Ok(true));
            let inner = game.transaction_open();
            assert_eq!(level.set_connection_group(game, pos, None), Ok(true));
            let host: &mut dyn ServerHost = game;
            assert_eq!(host.edit_blocks("minecraft:overworld", &round_trip), Ok(2));
            game.transaction_close(&inner, false);
            assert_eq!(level.connection_group(game, pos), Ok(None));
            game.transaction_close(&outer, false);
            assert_eq!(level.block_state(game, pos), Some(stone));
            assert_eq!(level.connection_group(game, pos), Ok(None));

            // Aborting the first ever tentative write must not opt the column into group sync.
            let untouched = [17, 64, 1];
            game.world
                .chunks
                .insert((1, 0), Arc::new(Chunk::empty(1, 0)));
            let abort = game.transaction_open();
            assert_eq!(
                level.set_connection_group(game, untouched, Some(3)),
                Ok(true)
            );
            game.transaction_close(&abort, false);
            assert_eq!(level.connection_group(game, untouched), Ok(None));
            assert!(!game.world.chunks[&(1, 0)].has_held_connection_groups);

            // A command can temporarily put another dimension in place while a transaction
            // stays open. The same coordinates there must not rewrite the parked baseline.
            let pos = [14, 64, 14];
            assert_eq!(
                level.set_block(game, pos, stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(level.set_connection_group(game, pos, Some(10)), Ok(true));
            let outer = game.transaction_open();
            assert_eq!(level.set_connection_group(game, pos, Some(11)), Ok(true));
            assert_eq!(game.enter_command_level(Dim::Nether), Some(Dim::Overworld));
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let nether = game.level("minecraft:the_nether").unwrap();
            assert_eq!(nether.set_connection_group(game, pos, Some(20)), Ok(true));
            let host: &mut dyn ServerHost = game;
            assert_eq!(
                host.edit_blocks("minecraft:the_nether", &single_replace(pos, glass)),
                Ok(1)
            );
            assert_eq!(nether.connection_group(game, pos), Ok(None));
            assert_eq!(nether.set_connection_group(game, pos, Some(21)), Ok(true));
            assert_eq!(game.enter_command_level(Dim::Overworld), Some(Dim::Nether));
            game.transaction_close(&outer, false);
            assert_eq!(level.connection_group(game, pos), Ok(Some(10)));
            assert_eq!(nether.connection_group(game, pos), Ok(Some(21)));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn client_applies_connection_group_payloads_and_refuses_client_writes() {
        with_game(|game| {
            game.options.execution_role = ExecutionRole::Client;
            game.options.dedicated = None;
            let (client, _server) = Pipe::pair();
            game.connect_local(client, "ConnGroupsClient").unwrap();
            game.remote.as_mut().unwrap().logged_in = true;
            game.native_lifecycle.world_open_ready().unwrap();
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            game.world
                .chunks
                .insert((1, 0), Arc::new(Chunk::empty(1, 0)));
            game.world.dirty_sections.clear();

            let level = game.level("minecraft:overworld").unwrap();
            assert_eq!(
                level.set_connection_group(game, [15, 64, 2], Some(5)),
                Err(LevelWriteError::Unavailable)
            );

            // Chunk snapshot installs groups and marks section + cross-chunk border for remesh.
            let chunk_payload = connection_groups::encode(&Message::Chunk {
                cx: 0,
                cz: 0,
                replace: true,
                entries: vec![ChunkEntry::new(15, 64, 2, 55).unwrap()],
            })
            .unwrap();
            let sy = (64 - MIN_Y) >> 4;
            game.remote_connection_groups(&chunk_payload);
            assert_eq!(level.connection_group(game, [15, 64, 2]), Ok(Some(55)));
            assert!(game.world.dirty_sections.contains(&(0, sy, 0)));
            assert!(game.world.dirty_sections.contains(&(1, sy, 0)));

            // Incremental update updates and clears entries cleanly.
            game.world.dirty_sections.clear();
            let update_payload = connection_groups::encode(&Message::Update {
                entries: vec![
                    UpdateEntry {
                        pos: [15, 64, 2],
                        group: None,
                    },
                    UpdateEntry {
                        pos: [16, 64, 2],
                        group: Some(88),
                    },
                ],
            })
            .unwrap();
            game.remote_connection_groups(&update_payload);
            assert_eq!(level.connection_group(game, [15, 64, 2]), Ok(None));
            assert_eq!(level.connection_group(game, [16, 64, 2]), Ok(Some(88)));
            assert!(game.world.dirty_sections.contains(&(0, sy, 0)));
            assert!(game.world.dirty_sections.contains(&(1, sy, 0)));
        });

        // Viewed level: wrapped `sunrise:connection_groups` payloads in `sunrise:levels` runs
        // populate and update the viewed world, mark its sections dirty, survive chunk resend
        // until an authoritative empty snapshot clears them, and stay isolated from own world.
        let sy = (64 - MIN_Y) >> 4;
        let mut resources = None;
        let mut vgame = viewed_client(&mut resources);
        vgame
            .handle_packet(viewed_open(1, "minecraft:the_nether"))
            .unwrap();
        vgame
            .handle_packet(viewed_run(1, &[viewed_column(0, 0)]))
            .unwrap();
        let viewed = vgame.level("minecraft:the_nether").unwrap();
        let own = vgame.level("minecraft:overworld").unwrap();
        vgame
            .remote
            .as_mut()
            .unwrap()
            .viewed
            .get_mut(1)
            .unwrap()
            .world
            .dirty_sections
            .clear();

        let cg_pkt = |msg: &Message| {
            C::CustomPayload(CustomPayload {
                channel: connection_groups::CHANNEL.into(),
                data: connection_groups::encode(msg).unwrap(),
            })
        };
        vgame
            .handle_packet(viewed_run(
                1,
                &[cg_pkt(&Message::Chunk {
                    cx: 0,
                    cz: 0,
                    replace: true,
                    entries: vec![ChunkEntry::new(15, 64, 2, 55).unwrap()],
                })],
            ))
            .unwrap();
        assert_eq!(viewed.connection_group(&vgame, [15, 64, 2]), Ok(Some(55)));
        assert_eq!(own.connection_group(&vgame, [15, 64, 2]), Ok(None));
        assert!(
            vgame
                .remote
                .as_ref()
                .unwrap()
                .viewed
                .get(1)
                .unwrap()
                .world
                .dirty_sections
                .contains(&(0, sy, 0))
        );

        // Bulk chunk resend on a viewed world preserves groups until the authoritative empty
        // snapshot clears and dirties the section.
        vgame
            .handle_packet(viewed_run(1, &[viewed_column(0, 0)]))
            .unwrap();
        assert_eq!(viewed.connection_group(&vgame, [15, 64, 2]), Ok(Some(55)));
        vgame
            .remote
            .as_mut()
            .unwrap()
            .viewed
            .get_mut(1)
            .unwrap()
            .world
            .dirty_sections
            .clear();
        vgame
            .handle_packet(viewed_run(
                1,
                &[cg_pkt(&Message::Chunk {
                    cx: 0,
                    cz: 0,
                    replace: true,
                    entries: Vec::new(),
                })],
            ))
            .unwrap();
        assert_eq!(viewed.connection_group(&vgame, [15, 64, 2]), Ok(None));
        assert!(
            vgame
                .remote
                .as_ref()
                .unwrap()
                .viewed
                .get(1)
                .unwrap()
                .world
                .dirty_sections
                .contains(&(0, sy, 0))
        );
    }
}
