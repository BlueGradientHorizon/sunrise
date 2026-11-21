//! Per-position connected-texture groups (#3843): client-side receive handler for
//! `sunrise:connection_groups` (`Chunk` snapshots and incremental `Update` deltas).
use super::Game;
use sunrise_protocol::connection_groups::{self, Message};

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
        match message {
            Message::Chunk {
                cx,
                cz,
                replace,
                entries,
            } => {
                let coords = entries
                    .into_iter()
                    .map(|e| (e.local_x, e.y, e.local_z, e.group));
                self.world
                    .apply_chunk_connection_groups(cx, cz, replace, coords);
            }
            Message::Update { entries } => {
                for entry in entries {
                    let [x, y, z] = entry.pos;
                    let _ = self.world.set_connection_group(x, y, z, entry.group, false);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Dim, execution_role::ExecutionRole, native_entity_refs::tests::with_game};
    use mod_api::{BlockUpdateFlags, Host, LevelReadError, LevelRef, LevelWriteError, WorldEpoch};
    use std::sync::Arc;
    use sunrise_client::chunk::{Chunk, MIN_Y};
    use sunrise_protocol::{
        connection::Pipe,
        connection_groups::{ChunkEntry, UpdateEntry},
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
                Some(&[((2, 64, 3), Some(0))][..])
            );

            // Unloaded chunk and out-of-height positions return typed refusals.
            assert_eq!(
                level.set_connection_group(game, [32, 64, 0], Some(1)),
                Err(LevelWriteError::NotLoaded)
            );
            assert_eq!(
                level.set_connection_group(game, [2, 9999, 3], Some(1)),
                Err(LevelWriteError::OutOfHeight)
            );
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

            // Replacing with a different block clears the group and logs None.
            assert_eq!(
                level.set_block(game, [2, 64, 3], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(level.connection_group(game, [2, 64, 3]), Ok(None));
            assert_eq!(
                game.world.connection_group_log.as_deref(),
                Some(&[((2, 64, 3), None)][..])
            );

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
            game.world.connection_group_log = Some(Vec::new());

            let level = game.level("minecraft:overworld").unwrap();
            let stone = state(game, "minecraft:stone");
            let glass = state(game, "minecraft:glass");
            assert_eq!(
                level.set_block(game, [4, 64, 4], stone, BlockUpdateFlags::NONE),
                Ok(true)
            );
            assert_eq!(
                level.set_connection_group(game, [4, 64, 4], Some(11)),
                Ok(true)
            );
            game.world.dirty_sections.clear();
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

            // Committed transaction replacing a block and setting a new group keeps the new group.
            let commit_seal = game.transaction_open();
            assert_eq!(
                level.set_block(game, [4, 64, 4], glass, BlockUpdateFlags::NONE),
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
                Some(&[((4, 64, 4), None), ((4, 64, 4), Some(33))][..])
            );
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
                entries: vec![ChunkEntry {
                    local_x: 15,
                    y: 64,
                    local_z: 2,
                    group: 55,
                }],
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
    }
}
