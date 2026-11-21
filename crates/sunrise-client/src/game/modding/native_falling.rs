//! Falling blocks for native server mods (docs/modding/src/how-to/world/spawning.md,
//! "Spawning from a level"): `LevelRef::spawn_falling_block` and the canonical
//! `FallingBlockEntity` constructor shared with `LevelEvent::FallingBlock`.
use super::{FallingBlock, Game};
use sunrise_core::{geometry::Vec3, position::BlockPos};
use sunrise_world::registry::BlockStateId;

impl Game<'_> {
    /// Canonical falling-block entity construction shared by world events
    /// (`LevelEvent::FallingBlock`, matching `FallingBlockEntity.fall`) and
    /// [`Self::level_spawn_falling_block`]. Sets `start_pos` from `position`
    /// (`FallingBlockEntity` constructor's `this.setStartPos(this.blockPosition())`),
    /// keeps `velocity` exact (`Entity.setDeltaMovement`), allocates the next entity id,
    /// and appends the live row to `self.falling`.
    pub(super) fn spawn_falling_block_entity(
        &mut self,
        state: BlockStateId,
        position: Vec3,
        velocity: Vec3,
        drop_item: bool,
        damage: sunrise_client::falling_block::Damage,
    ) -> u32 {
        let id = self.take_entity_id();
        let start_pos = BlockPos::new(
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        );
        self.falling.push(FallingBlock {
            attachment_lifetime: Default::default(),
            rotation: [0.0, 0.0],
            join: Default::default(),
            id,
            state,
            start_pos,
            position,
            previous: position,
            velocity,
            age: 0,
            no_gravity: false,
            on_ground: false,
            fire_ticks: 0,
            sync_velocity: false,
            drop_item,
            fall_distance: 0.0,
            stuck: Vec3::ZERO,
            damage,
            saved: None,
            portal: Default::default(),
        });
        id
    }

    /// `LevelRef::spawn_falling_block`: spawns a `FallingBlockEntity` of `state` at `pos`
    /// (its bottom centre) with `velocity` (blocks a tick, `Entity.setDeltaMovement`) in
    /// `level` without modifying the block at `pos`.
    ///
    /// Typed errors distinguish stale/closed levels (`Stale`), client or parked levels
    /// (`Unavailable`), and unknown block states (`InvalidState`). Returns `Ok(None)` for
    /// exhausted entity ids, non-finite position or velocity, a position outside
    /// `Level.isInSpawnableBounds`, an unloaded chunk, or a join cancelled by
    /// `entity_join_level`.
    pub(super) fn level_spawn_falling_block(
        &mut self,
        level: mod_api::LevelRef,
        state: BlockStateId,
        pos: [f64; 3],
        velocity: [f64; 3],
        falling: mod_api::Falling,
    ) -> Result<Option<mod_api::EntityRef>, mod_api::LevelWriteError> {
        let _ = self.native_level_world_result(level)?;
        if self.dim.key() != level.dimension() {
            return Err(mod_api::LevelWriteError::Unavailable);
        }
        if self.pack.assets.registry.state(state).is_none() {
            return Err(mod_api::LevelWriteError::InvalidState(
                mod_api::BlockStateError::UnknownState,
            ));
        }
        if self.entity_ids_exhausted() || !pos.iter().chain(&velocity).all(|v| v.is_finite()) {
            return Ok(None);
        }
        // BlockPos.containing. `as` saturates, and a value past `i32` is out of bounds.
        let block = pos.map(|v| v.floor() as i32);
        if self.level_in_spawnable_bounds(level, block) != Some(true)
            || !self.world.loaded(block[0], block[2])
        {
            return Ok(None);
        }
        let mut damage =
            sunrise_client::falling_block::Damage::falling(&self.res.facts().get(state).name);
        if falling.hurts {
            damage.hurt_entities = true;
            damage.per_distance = 2.0;
            damage.max = 40;
        }
        let id = self.spawn_falling_block_entity(
            state,
            Vec3::new(pos[0], pos[1], pos[2]),
            Vec3::new(velocity[0], velocity[1], velocity[2]),
            falling.drop_item,
            damage,
        );
        // The join as any fresh spawn's: told now, so a mod's veto refuses
        // the call, or at the root commit inside a transaction.
        Ok(self
            .native_joins_flush(false, id)
            .then(|| mod_api::EntityRef::new(level.epoch(), id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};
    use sunrise_mod_runtime::attachments::{
        AttachmentType, Attachments, HasAttachmentLifetime, Holder, index::AttachmentIndex,
    };

    static VALUE: AttachmentType<i32> = AttachmentType {
        namespace: "test",
        name: "falling_lifetime",
        default: || 0,
        codec: None,
        copy_on_death: false,
        sync: false,
    };

    #[test]
    #[ignore = "daily"]
    fn falling_tick_preserves_its_owner_on_success_and_collision_error() {
        bound_falling_case(false, |game, _| {
            assert!(game.spawn_remote_projectile(
                "minecraft:falling_block",
                &sunrise_protocol::packets::play::AddEntity {
                    id: 72,
                    uuid: 72,
                    kind: 0,
                    position: [4.5, 70.0, 0.5],
                    velocity: [0.0; 3],
                    pitch: 0,
                    yaw: 0,
                    head_yaw: 0,
                    data: game.falling[0].state.0 as i32
                },
                (0.0, 0.0),
            ));
            let expected: Vec<_> = game.falling.iter().map(|f| (f.id, f.join)).collect();
            game.tick_one_falling(0, &[]).unwrap();
            assert_eq!(
                game.falling
                    .iter()
                    .map(|f| (f.id, f.join))
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(game.falling[0].age, 1);
            assert!(game.falling[0].position.y < 70.0);
            game.tick_one_falling(1, &[]).unwrap();
            assert_eq!(
                game.falling
                    .iter()
                    .map(|f| (f.id, f.join))
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!([game.falling[0].age, game.falling[1].age], [1, 1]);
            // A corrupt resident palette makes the real collision lookup fail.
            Arc::make_mut(game.world.chunks.get_mut(&(0, 0)).unwrap()).set(0, 69, 0, 65_535);
            let position = game.falling[0].position;
            assert!(game.tick_one_falling(0, &[]).is_err());
            assert_eq!(
                game.falling
                    .iter()
                    .map(|f| (f.id, f.join))
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(game.falling[0].position, position);
            assert_eq!(game.falling[0].age, 1);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn falling_client_tick_commits_state_without_detaching_its_owner() {
        bound_falling_case(true, |game, _| {
            game.remote_projectile_data(
                71,
                &[(
                    8,
                    10,
                    sunrise_protocol::entity_data::Value::Pos([8, 90, -4]),
                )],
            );
            assert_eq!(game.falling[0].start_pos, BlockPos::new(8, 90, -4));
            for age in 1..=3 {
                let previous = game.falling[0].position;
                game.tick_remote_projectiles(|_, at| at);
                assert_eq!(game.falling[0].age, age);
                assert_eq!(game.falling[0].previous, previous);
                assert!(game.falling[0].position.y < previous.y);
                assert!(game.falling[0].velocity.y < 0.0);
            }
        });
    }

    #[test]
    #[ignore = "daily"]
    fn falling_parked_swap_preserves_state_and_owner_until_discard() {
        bound_falling_case(false, |game, index| {
            let home = game.dim;
            game.tick_one_falling(0, &[]).unwrap();
            let position = game.falling[0].position;
            game.enter_level(super::super::Dim::Nether).unwrap();
            assert!(game.falling.is_empty());
            assert_eq!(
                index
                    .lock()
                    .unwrap()
                    .snapshot(&("test".into(), "falling_lifetime".into()))
                    .len(),
                1
            );
            game.enter_level(home).unwrap();
            assert_eq!(game.falling.len(), 1);
            assert_eq!(game.falling[0].id, 71);
            assert_eq!(game.falling[0].age, 1);
            assert_eq!(game.falling[0].position, position);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn falling_lifecycle_commands_unload_park_and_remote() {
        use super::super::{
            commands::Target,
            entity::taken_pass_tests::{LifecycleFixture, RemoteLifecycle, SlotIdentities},
        };
        let mut identities = SlotIdentities::default();
        LifecycleFixture {
            seed: |game, entries| {
                game.falling.clear();
                let state = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:sand")
                    .unwrap();
                for &(id, at) in entries {
                    assert!(game.spawn_remote_projectile(
                        "minecraft:falling_block",
                        &sunrise_protocol::packets::play::AddEntity {
                            id: id as i32,
                            uuid: u128::from(id),
                            kind: 0,
                            position: at,
                            velocity: [0.0; 3],
                            pitch: 0,
                            yaw: 0,
                            head_yaw: 0,
                            data: state.0 as i32
                        },
                        (0.0, 0.0),
                    ));
                }
            },
            ids: |game| game.falling.iter().map(|f| f.id).collect(),
            target: Target::Falling,
            unloads_with_chunk: true,
            remote: Some(RemoteLifecycle {
                prepare: |game| game.clear_remote_projectiles(),
                apply: |game, remove, _| game.remove_remote_projectile(remove),
            }),
        }
        .run_with(|game| {
            identities.observe(
                &mut game.falling,
                game.native_lifecycle.epoch().unwrap(),
                |f| f.id,
            );
        });
    }

    fn bound_falling_case(
        client: bool,
        exercise: impl FnOnce(&mut Game<'_>, &Arc<Mutex<AttachmentIndex<mod_api::EntityRef>>>),
    ) {
        let test = |game: &mut Game<'_>| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
            let state = game
                .pack
                .assets
                .registry
                .default_state("minecraft:sand")
                .unwrap();
            let at = Vec3::new(0.5, 70.0, 0.5);
            game.falling.push(FallingBlock {
                on_ground: false,
                fire_ticks: 0,
                sync_velocity: false,
                attachment_lifetime: Default::default(),
                rotation: [0.0; 2],
                join: Default::default(),
                id: 71,
                state,
                start_pos: BlockPos::new(0, 70, 0),
                position: at,
                previous: at,
                velocity: Vec3::ZERO,
                age: 0,
                no_gravity: false,
                portal: Default::default(),
                drop_item: false,
                fall_distance: 0.0,
                stuck: Vec3::ZERO,
                damage: Default::default(),
                saved: None,
            });
            let id = game.falling[0].id;
            let holder = mod_api::EntityRef::new(game.native_lifecycle.epoch().unwrap(), id);
            let index = Arc::new(Mutex::new(AttachmentIndex::default()));
            let mut values = Attachments::default();
            VALUE.set(&mut values, Holder::Entity(id), 10).unwrap();
            let original = game.falling.update(0, |f| {
                f.attachment_lifetime().bind(&mut values, &index, holder);
                f.attachment_lifetime().registration().unwrap()
            });
            let key = ("test".into(), "falling_lifetime".into());
            let before = index.lock().unwrap().snapshot(&key);
            assert_eq!(before.len(), 1);
            exercise(game, &index);
            let current = game
                .falling
                .update(0, |f| f.attachment_lifetime().registration().unwrap());
            assert!(original.same_registration(&current));
            assert_eq!(index.lock().unwrap().snapshot(&key).len(), 1);
            assert_eq!(VALUE.get(&mut values, Holder::Entity(id)), Ok(10));
            // Exercise the production clear before any fixture cleanup can mask it.
            game.clear_remote_projectiles();
            assert!(game.falling.is_empty());
            assert!(index.lock().unwrap().snapshot(&key).is_empty());
            assert_eq!(before[0].entry.read(&VALUE), Ok(&10));
        };
        if client {
            super::super::native_entity_refs::tests::with_client_game(test);
        } else {
            super::super::native_entity_refs::tests::with_game(test);
        }
    }

    #[test]
    #[ignore = "daily"]
    fn level_spawn_falling_block_preserves_state_flags_unclamped_velocity_and_bounds() {
        use crate::game::api_values::TryIntoApi;
        use mod_api::{
            BlockState, BlockStateError, Falling, LevelRef, LevelWriteError, WorldEpoch,
        };

        super::super::native_entity_refs::tests::with_game(|game| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
            let level = game.level(game.dim.id()).unwrap();
            let sand = game
                .pack
                .assets
                .registry
                .default_state("minecraft:sand")
                .unwrap();
            let anvil = game
                .pack
                .assets
                .registry
                .default_state("minecraft:anvil")
                .unwrap();
            let air = game
                .pack
                .assets
                .registry
                .default_state("minecraft:air")
                .unwrap();
            let api_sand: BlockState = sand.try_into_api().unwrap();
            let api_anvil: BlockState = anvil.try_into_api().unwrap();
            let api_air: BlockState = air.try_into_api().unwrap();
            let invalid: BlockState =
                mod_api::__private::block_state_from_ordinal(api_sand.block(), u16::MAX);

            // Typed handle boundary rejects unknown states and wrong phases without unwinding.
            assert_eq!(
                level.spawn_falling_block(
                    game,
                    invalid,
                    [1.5, 70.0, 2.5],
                    [0.0; 3],
                    Falling::default(),
                ),
                Err(LevelWriteError::InvalidState(BlockStateError::UnknownState))
            );
            {
                let _setup =
                    mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::Setup);
                assert_eq!(
                    level.spawn_falling_block(
                        game,
                        api_sand,
                        [1.5, 70.0, 2.5],
                        [0.0; 3],
                        Falling::default(),
                    ),
                    Err(LevelWriteError::Unavailable)
                );
            }

            // Live velocity is kept unclamped (past the old 3.9 clamp), and flags are preserved.
            let thrown = level
                .spawn_falling_block(
                    game,
                    api_sand,
                    [1.5, 70.0, 2.5],
                    [0.5, 9.0, -9.0],
                    Falling {
                        drop_item: false,
                        hurts: true,
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                game.entity_kind(thrown)
                    .and_then(|kind| kind.id(game))
                    .as_deref(),
                Some("minecraft:falling_block")
            );
            assert_eq!(game.entity_position(thrown), Some([1.5, 70.0, 2.5]));
            assert_eq!(game.entity_velocity(thrown), Some([0.5, 9.0, -9.0]));
            let block = game.falling.iter().find(|f| f.id == thrown.id()).unwrap();
            assert_eq!(block.state, sand);
            assert_eq!(block.start_pos, BlockPos::new(1, 70, 2));
            assert_eq!(block.velocity, Vec3::new(0.5, 9.0, -9.0));
            assert!(!block.drop_item);
            assert!(block.damage.hurt_entities);
            assert_eq!(block.damage.per_distance, 2.0);
            assert_eq!(block.damage.max, 40);

            // Anvil keeps its vanilla hurt defaults even with `hurts: false`.
            let anvil_ref = level
                .spawn_falling_block(
                    game,
                    api_anvil,
                    [2.5, 70.0, 2.5],
                    [0.0; 3],
                    Falling::default(),
                )
                .unwrap()
                .unwrap();
            let anvil_row = game
                .falling
                .iter()
                .find(|f| f.id == anvil_ref.id())
                .unwrap();
            assert!(anvil_row.drop_item);
            assert!(anvil_row.damage.hurt_entities);
            assert_eq!(anvil_row.damage.per_distance, 2.0);
            assert_eq!(anvil_row.damage.max, 40);

            // Air spawns and is discarded on its first tick (FallingBlockEntity.tick).
            let air_ref = level
                .spawn_falling_block(
                    game,
                    api_air,
                    [3.5, 70.0, 2.5],
                    [0.0; 3],
                    Falling::default(),
                )
                .unwrap()
                .unwrap();
            let air_index = game
                .falling
                .iter()
                .position(|f| f.id == air_ref.id())
                .unwrap();
            assert_eq!(game.falling[air_index].state, air);
            game.tick_one_falling(air_index, &[]).unwrap();
            assert!(!game.falling.iter().any(|f| f.id == air_ref.id()));

            // No 1,024 per-level quota.
            for i in 0..1030 {
                let spawned = game
                    .level_spawn_falling_block(
                        level,
                        sand,
                        [0.5 + f64::from((i % 16) as u8), 70.0, 0.5],
                        [0.0; 3],
                        Falling::default(),
                    )
                    .unwrap();
                assert!(spawned.is_some(), "falling block {i} should spawn");
            }
            assert!(game.falling.len() >= 1030);

            // Refusals: stale handle, non-finite coordinates/velocity, spawnable bounds, unloaded chunk.
            let count = game.falling.len();
            let stale = LevelRef::new(WorldEpoch::new(0), game.dim.key());
            assert_eq!(
                stale.spawn_falling_block(
                    game,
                    api_sand,
                    [1.5, 70.0, 1.5],
                    [0.0; 3],
                    Falling::default(),
                ),
                Err(LevelWriteError::Stale)
            );
            assert_eq!(
                game.level_spawn_falling_block(
                    level,
                    BlockStateId(u32::MAX),
                    [1.5, 70.0, 1.5],
                    [0.0; 3],
                    Falling::default(),
                ),
                Err(LevelWriteError::InvalidState(BlockStateError::UnknownState))
            );
            for (pos, vel) in [
                ([f64::NAN, 70.0, 1.5], [0.0; 3]),
                ([1.5, f64::INFINITY, 1.5], [0.0; 3]),
                ([1.5, 70.0, 1.5], [0.0, f64::NAN, 0.0]),
                ([1.5, 70.0, 1.5], [f64::INFINITY, 0.0, 0.0]),
                ([1.0e300, 70.0, 1.5], [0.0; 3]),
                ([1.5, 20_000_000.0, 1.5], [0.0; 3]),
                ([1.5, -20_000_001.0, 1.5], [0.0; 3]),
                ([1000.5, 70.0, 1.5], [0.0; 3]),
            ] {
                assert_eq!(
                    level.spawn_falling_block(game, api_sand, pos, vel, Falling::default()),
                    Ok(None),
                    "pos={pos:?} vel={vel:?}"
                );
            }
            assert_eq!(game.falling.len(), count);

            // Parked dimension refuses with Unavailable.
            let home = game.dim;
            game.enter_level(super::super::Dim::Nether).unwrap();
            assert_eq!(
                level.spawn_falling_block(
                    game,
                    api_sand,
                    [1.5, 70.0, 1.5],
                    [0.0; 3],
                    Falling::default(),
                ),
                Err(LevelWriteError::Unavailable)
            );
            game.enter_level(home).unwrap();
        });

        // Client side refuses with Unavailable.
        super::super::native_entity_refs::tests::with_client_game(|game| {
            let epoch = game.native_lifecycle.epoch().unwrap();
            let level = LevelRef::new(epoch, game.dim.key());
            let sand = game
                .pack
                .assets
                .registry
                .default_state("minecraft:sand")
                .unwrap();
            assert_eq!(
                game.level_spawn_falling_block(
                    level,
                    sand,
                    [1.5, 70.0, 1.5],
                    [0.0; 3],
                    Falling::default(),
                ),
                Err(LevelWriteError::Unavailable)
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn level_spawn_falling_block_joins_cancel_and_roll_back_in_transactions() {
        use mod_api::{
            EntityConstructingEvent, EntityJoinLevelEvent, Falling, Flow, Holder, HostSide,
            ServerHost,
        };
        use std::sync::atomic::{AtomicBool, Ordering};

        struct Listener {
            heard: Arc<Mutex<Vec<String>>>,
            cancel: Arc<AtomicBool>,
        }
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Listener {
            fn entity_constructing(&self, _: HostSide<'_>, event: &mut EntityConstructingEvent) {
                if let Holder::Entity(_) = event.holder {
                    self.heard
                        .lock()
                        .unwrap()
                        .push(format!("construct {}", event.kind));
                }
            }
            fn entity_join_level(
                &self,
                _: &mut dyn ServerHost,
                event: &mut EntityJoinLevelEvent<'_>,
            ) -> Flow {
                self.heard
                    .lock()
                    .unwrap()
                    .push(format!("join {} {}", event.kind, event.loaded_from_disk));
                if self.cancel.load(Ordering::Relaxed) {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }
        }

        let heard = Arc::new(Mutex::new(Vec::new()));
        let cancel = Arc::new(AtomicBool::new(false));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "falling-listener",
            Box::new(Listener {
                heard: heard.clone(),
                cancel: cancel.clone(),
            }),
        )]
        .into();

        super::super::native_entity_refs::tests::with_game(|game| {
            mod_platform::testing::with_mods(mods, || {
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
                let level = game.level(game.dim.id()).unwrap();
                let sand = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:sand")
                    .unwrap();

                // Normal spawn runs construct + join and keeps the falling block.
                let kept = game
                    .level_spawn_falling_block(
                        level,
                        sand,
                        [1.5, 70.0, 1.5],
                        [0.0; 3],
                        Falling::default(),
                    )
                    .unwrap()
                    .unwrap();
                assert!(game.falling.iter().any(|f| f.id == kept.id()));
                assert_eq!(
                    *heard.lock().unwrap(),
                    [
                        "construct minecraft:falling_block",
                        "join minecraft:falling_block false",
                    ]
                );
                heard.lock().unwrap().clear();

                // Cancelled join removes the falling block and returns Ok(None).
                cancel.store(true, Ordering::Relaxed);
                let count = game.falling.len();
                assert_eq!(
                    game.level_spawn_falling_block(
                        level,
                        sand,
                        [2.5, 70.0, 1.5],
                        [0.0; 3],
                        Falling::default(),
                    ),
                    Ok(None)
                );
                assert_eq!(game.falling.len(), count);
                assert_eq!(
                    *heard.lock().unwrap(),
                    [
                        "construct minecraft:falling_block",
                        "join minecraft:falling_block false",
                    ]
                );
                heard.lock().unwrap().clear();
                cancel.store(false, Ordering::Relaxed);

                // Aborted transaction removes the falling block and never announces join.
                let seal = game.transaction_open();
                let aborted = game
                    .level_spawn_falling_block(
                        level,
                        sand,
                        [3.5, 70.0, 1.5],
                        [0.0; 3],
                        Falling::default(),
                    )
                    .unwrap()
                    .unwrap();
                assert!(game.falling.iter().any(|f| f.id == aborted.id()));
                assert_eq!(
                    *heard.lock().unwrap(),
                    ["construct minecraft:falling_block"]
                );
                game.transaction_close(&seal, false);
                assert!(!game.falling.iter().any(|f| f.id == aborted.id()));
                assert_eq!(
                    *heard.lock().unwrap(),
                    ["construct minecraft:falling_block"]
                );
                heard.lock().unwrap().clear();

                // Committed transaction announces join at commit and keeps the falling block.
                let seal = game.transaction_open();
                let committed = game
                    .level_spawn_falling_block(
                        level,
                        sand,
                        [4.5, 70.0, 1.5],
                        [0.0; 3],
                        Falling::default(),
                    )
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    *heard.lock().unwrap(),
                    ["construct minecraft:falling_block"]
                );
                game.transaction_close(&seal, true);
                assert!(game.falling.iter().any(|f| f.id == committed.id()));
                assert_eq!(
                    *heard.lock().unwrap(),
                    [
                        "construct minecraft:falling_block",
                        "join minecraft:falling_block false",
                    ]
                );
            });
        });
    }
}
