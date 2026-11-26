//! Block entities removed by the block behaviour (`LevelChunk.setBlockState`
//! running `BlockEntity.preRemoveSideEffects`) and the resources a destroyed
//! block drops (`Block.dropResources`), drawn from the level random at the
//! moment vanilla draws them. The item entities they make are added to the
//! world as soon as the behaviour returns, within the same tick.
use super::{Game, GameLevel};
use crate::game::spill_common::{
    BlockDrops, BlockItemDrop, Pending, Spawn, UsedDrops, saved_entity, world_source,
};
use mod_api::EntityRef;
use sunrise_core::position::BlockPos;
use sunrise_items::items::{BlockLootRoll, ItemStack};
use sunrise_level::containers::BlockEntity;
use sunrise_world::registry::BlockStateId;

/// Lets `say` change each use's run of `spawns`, newest first so the earlier
/// ranges hold. A run's stacks keep their spots, and a stack `say` adds takes
/// the run's last spot, else the block's top.
fn retell_runs(
    spawns: &mut Vec<Spawn>,
    used: &[UsedDrops],
    mut say: impl FnMut(&UsedDrops, [f64; 3], &mut Vec<ItemStack>),
) {
    for used in used.iter().rev() {
        let run: Vec<Spawn> = spawns.drain(used.start..used.end).collect();
        let mut drops: Vec<ItemStack> = run.iter().map(|s| s.stack.clone()).collect();
        let origin = used.pos.map(|c| f64::from(c) + 0.5);
        say(used, origin, &mut drops);
        let top = [origin[0], origin[1] + 0.5, origin[2]];
        let kept: Vec<Spawn> = drops
            .into_iter()
            .enumerate()
            .map(|(i, stack)| Spawn {
                random: run.get(i).and_then(|s| s.random),
                stack,
                pos: run.get(i).or(run.last()).map_or(top, |s| s.pos),
                motion: run.get(i).and_then(|s| s.motion),
                delay: run.get(i).map_or(10, |s| s.delay),
            })
            .collect();
        spawns.splice(used.start..used.start, kept);
    }
}

/// `Block.getDrops(state, level, pos, blockEntity, entity, tool)` for the block still
/// standing: `drop_resources_now`'s roll, with what the block entity copies into the drops,
/// drawn from the same random, but nothing popped, no `block_drops` event and no
/// experience. A scratch record keeps a break in progress at `pos` as it was.
pub(in crate::game::server_side) fn block_drops_of(
    level: &mut GameLevel<'_>,
    pos: BlockPos,
    state: BlockStateId,
    tool: Option<&ItemStack>,
    entity: Option<EntityRef>,
) -> Vec<ItemStack> {
    let key = (pos.x, pos.y, pos.z);
    let at = [pos.x, pos.y, pos.z];
    let mut copied = Pending::default();
    if let Some(block_entity) = level.world.block_entities.get(&key) {
        let saved = level.encode_live_block_entity(key, block_entity, &level.facts.get(state).name);
        copied.record_from(level.world, at, saved);
    } else if let Some(tag) = saved_entity(level.world, at) {
        copied.record_saved(at, &tag);
    }
    let dynamic = copied.dynamic(at);
    let roll = sunrise_items::items::BlockLootRoll {
        held: tool,
        dynamic: &dynamic,
        breaker: entity.is_some(),
        ..Default::default()
    };
    let source = copied.source(at);
    let mut drops = level.roll_block_loot(pos, state, entity, roll, source.as_deref());
    drops.retain(|stack| &*stack.item != "minecraft:air" && stack.count > 0);
    drops
}

impl Game<'_, crate::game::server_side::ServerState> {
    /// [`GameLevel::roll_block_loot`] for a roll outside a level write:
    /// `entity` is `THIS_ENTITY`'s id. With no listener it builds no level.
    pub(in crate::game) fn roll_block_loot(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        entity: Option<u32>,
        roll: BlockLootRoll<'_>,
    ) -> Vec<ItemStack> {
        let source = self.pending.source(pos).or_else(|| {
            self.pack.assets.registry.state(state).and_then(|block| {
                world_source(&self.world, pos, &block.name, |key, entity, block| {
                    self.encode_live_block_entity(key, entity, block, world_items!(self))
                })
            })
        });
        let storage = self.score.storage.snapshot();
        sunrise_world::loot::copy::with_storage(storage, || {
            sunrise_world::loot::copy::with_block_source(source.as_deref(), || {
                if mod_api::__private::listened(mod_api::HookId::EnchantedBlockLoot) {
                    let at = BlockPos::new(pos[0], pos[1], pos[2]);
                    let entity = self.loot_entity_ref(entity);
                    return self.with_level(|_, level, _| {
                        level.roll_block_loot(at, state, entity, roll, source.as_deref())
                    });
                }
                server_world_items!(self).block_drops(
                    self.pack.assets.registry,
                    state,
                    roll,
                    &mut self.world.random,
                    Some((&mut self.side.rand_sequences, self.level.seed)),
                )
            })
        })
    }
    /// The handle of `LootContextParams.THIS_ENTITY`, by its id.
    pub(in crate::game) fn loot_entity_ref(&self, id: Option<u32>) -> Option<EntityRef> {
        let epoch = self.native_lifecycle.epoch()?;
        Some(EntityRef::new(epoch, id.filter(|id| *id != 0)?))
    }
    /// Apply the shared block-drop event's item entities and experience unchanged.
    pub(in crate::game::server_side) fn spawn_block_drops(
        &mut self,
        pos: [i32; 3],
        dropped: BlockDrops,
    ) {
        for item in dropped.items {
            self.add_item_drop(item);
        }
        if dropped.experience > 0 {
            self.spawn_orbs(pos.map(|v| f64::from(v) + 0.5), dropped.experience);
        }
    }
    /// Adds a drop's item entity, with the state a drop hook set.
    pub(in crate::game) fn add_item_drop(&mut self, drop: BlockItemDrop) {
        let random = drop.random_with_state();
        let velocity = Some(drop.velocity);
        self.add_item_entity_prepared(
            drop.stack,
            drop.position,
            velocity,
            drop.pickup_delay,
            random,
        );
    }
    pub(in crate::game) fn publish_removal_spawns(
        &mut self,
        spawns: Vec<Spawn>,
        orbs: Vec<([f64; 3], u32, i32)>,
    ) {
        let _save_lease = sunrise_sim::join_seq::visibility::PassLease::enter();
        for spawn in spawns {
            self.push_item_entity(
                spawn.stack,
                spawn.pos,
                spawn.motion,
                spawn.delay,
                spawn.random,
            );
            self.join_removal_item(self.side.next_entity, spawn.pos);
        }
        for (at, value, merge_id) in orbs {
            self.award_orb(at, value, merge_id);
        }
    }

    /// Add the item entities the block behaviour spawned and follow up on the
    /// block entities it removed.
    pub(in crate::game) fn flush_spill(&mut self) {
        if self.side.spill.0.is_empty() {
            return;
        }
        let _save_lease = sunrise_sim::join_seq::visibility::PassLease::enter();
        let mut spill = std::mem::take(&mut self.side.spill.0);
        self.finish_piston_position_changes(std::mem::take(&mut spill.piston_positions));
        let used = std::mem::take(&mut spill.used);
        let tool = self.held().cloned();
        let player = Some(self.native_acting_id());
        retell_runs(&mut spill.spawns, &used, |used, origin, drops| {
            let block = Some((used.pos, used.state));
            self.native_use_drops(
                used.table,
                origin,
                block,
                None,
                player,
                tool.as_ref(),
                drops,
            );
        });
        for s in spill.spawns {
            self.add_item_entity_prepared(s.stack, s.pos, s.motion, s.delay, s.random);
        }
        for (at, value, merge_id) in spill.orbs {
            self.award_orb(at, value, merge_id);
        }
        for (pos, entity) in spill.removed {
            let key = (pos[0], pos[1], pos[2]);
            if matches!(entity, BlockEntity::Jukebox(_)) {
                self.jukebox_removed(pos);
            }
            if self.container == Some(key) {
                self.container_opener_event(key, false);
                self.container = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::spill_common::BlockDropEntity;
    use sunrise_core::random::RandomSource;
    use sunrise_world::level::removal;

    struct RemovalMod(u8);
    struct RemovalPart(u8, mod_api::ItemKey);
    #[mod_api::sunrise_part]
    impl mod_api::BlockPart for RemovalPart {}
    #[mod_api::sunrise_part]
    impl mod_api::BlockEntityPart for RemovalPart {
        fn pre_remove_side_effects(
            &self,
            _: &mut dyn mod_api::LevelRandomHost,
            event: &mut mod_api::PreRemoveSideEffectsEvent<'_>,
        ) {
            match self.0 {
                1 => event.keep_contents = true,
                2 => {
                    let key = self.1;
                    event.spawns.push((
                        mod_api::ItemStack::new(key, 1).unwrap(),
                        [1.25, 65.75, -2.5],
                    ));
                }
                3 => event.experience.push((10, [1.25, 65.75, -2.5])),
                4 => {
                    event.drops.push(None);
                    let key = self.1;
                    event
                        .drops
                        .push(Some(mod_api::ItemStack::new(key, 1).unwrap()));
                }
                _ => {}
            }
        }
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for RemovalMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            let kind = r.block_entity_type("machine").blocks(&[]).build();
            let diamond = r.item_key("minecraft:diamond");
            r.block_entity_part(&kind, RemovalPart(self.0, diamond))
                .unwrap();
        }
    }
    fn removal_outputs(
        mode: u8,
        check: impl FnOnce(&mut GameLevel<'_>, &mut sunrise_core::random::LevelRandom),
    ) {
        use std::sync::Arc;
        use sunrise_level::containers::{ModBlockEntity, ModBlockEntityFields, ModContainer};
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "removal",
            Box::new(RemovalMod(mode)),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            crate::game::native_entity_refs::tests::with_game(|game| {
                game.options.dedicated = Some(Vec::new());
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)));
                let state = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap();
                let mut entity =
                    ModBlockEntity::new("removal:machine".into(), ModBlockEntityFields::default());
                if mode == 1 {
                    entity.items = vec![Some(ItemStack::new("minecraft:diamond", 1))];
                    game.world.block_entities.set_mod_containers(
                        [(
                            state.0,
                            Arc::new(ModContainer {
                                slots: 1,
                                ..Default::default()
                            }),
                        )]
                        .into_iter()
                        .collect(),
                    );
                }
                game.world
                    .block_entities
                    .insert((1, 64, 2), BlockEntity::Mod(Box::new(entity)));
                game.with_level(|_, level, _| {
                    let mut reference = level.world.random.clone();
                    level.remove_entity(BlockPos::new(1, 64, 2), state, true);
                    check(level, &mut reference);
                });
            });
        });
    }
    #[test]
    #[ignore = "daily"]
    fn removal_keep_contents_suppresses_slot_scatter() {
        removal_outputs(1, |level, reference| {
            assert!(level.spill.spawns.is_empty());
            assert_eq!(level.world.random.next_int(), reference.next_int());
        });
    }
    #[test]
    #[ignore = "daily"]
    fn removal_uses_the_same_random_state_for_lazy_and_padded_slots() {
        use std::sync::Arc;
        use sunrise_core::random::LegacyRandom;
        use sunrise_level::containers::{ModBlockEntity, ModBlockEntityFields, ModContainer};

        crate::game::native_entity_refs::tests::with_game(|game| {
            let state = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            game.world.block_entities.set_mod_containers(
                [(
                    state.0,
                    Arc::new(ModContainer {
                        slots: 3,
                        ..Default::default()
                    }),
                )]
                .into_iter()
                .collect(),
            );
            let mut reference = LegacyRandom::new(7);
            for _ in 0..9 {
                reference.next_double();
            }
            let pos = BlockPos::new(1, 64, 2);
            for len in [3, 0, 1] {
                game.world.random.set_seed(7);
                game.world.block_entities.insert(
                    (pos.x, pos.y, pos.z),
                    BlockEntity::Mod(Box::new(ModBlockEntity::new(
                        "removal:machine".into(),
                        ModBlockEntityFields {
                            items: vec![None; len],
                            ..Default::default()
                        },
                    ))),
                );
                game.with_level(|_, level, _| {
                    level.remove_entity(pos, state, true);
                    assert!(level.spill.spawns.is_empty());
                    assert_eq!(
                        level.world.random.state(),
                        reference.state(),
                        "stored slots: {len}"
                    );
                });
            }
        });
    }

    #[test]
    #[ignore = "daily"]
    fn removal_spawn_has_exact_position_and_default_delay() {
        removal_outputs(2, |level, _| {
            assert_eq!(level.spill.spawns.len(), 1);
            let spawn = &level.spill.spawns[0];
            assert_eq!(spawn.pos, [1.25, 65.75, -2.5]);
            assert_eq!(spawn.delay, 10);
            assert_eq!(&*spawn.stack.item, "minecraft:diamond");
            assert_eq!(spawn.stack.count, 1);
            assert!(spawn.motion.is_none());
        });
    }
    #[test]
    #[ignore = "daily"]
    fn removal_experience_awards_positioned_orbs() {
        removal_outputs(3, |level, reference| {
            assert_eq!(
                level
                    .spill
                    .orbs
                    .iter()
                    .map(|(_, value, _)| *value)
                    .collect::<Vec<_>>(),
                [7, 3]
            );
            for (at, _, merge) in &level.spill.orbs {
                assert_eq!(*at, [1.25, 65.75, -2.5]);
                assert_eq!(*merge, reference.next_int_bound(40).unwrap());
            }
            assert_eq!(level.world.random.next_int(), reference.next_int());
        });
    }
    #[test]
    #[ignore = "daily"]
    fn removal_empty_slot_draws_three_doubles_without_spawning() {
        removal_outputs(4, |level, reference| {
            // Containers.dropItemStack draws x, y, z before its isEmpty loop.
            for _ in 0..3 {
                reference.next_double();
            }
            // The filled slot follows the empty one, using its own three draws.
            let at = [
                1.125 + reference.next_double() * 0.75,
                64.0 + reference.next_double() * 0.75,
                2.125 + reference.next_double() * 0.75,
            ];
            reference.next_int_bound(21).unwrap();
            for _ in 0..6 {
                reference.next_double();
            }
            assert_eq!(level.spill.spawns.len(), 1);
            assert_eq!(level.spill.spawns[0].pos, at);
            assert_eq!(level.spill.spawns[0].stack.count, 1);
            assert_eq!(level.world.random.next_int(), reference.next_int());
        });
    }
    #[test]
    #[ignore = "daily"]
    fn removal_unanswered_part_leaves_random_untouched() {
        removal_outputs(0, |level, reference| {
            assert!(level.spill.spawns.is_empty());
            assert!(level.spill.orbs.is_empty());
            assert_eq!(level.world.random.next_int(), reference.next_int());
        });
    }

    /// Listens to `block_drops`: takes any break of dirt, adds 2 points to the rest.
    struct DropsMod;
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for DropsMod {
        fn block_drops(
            &self,
            _: &dyn mod_api::LevelHost,
            event: &mut mod_api::BlockDropsEvent<'_>,
        ) -> mod_api::Flow {
            if event.drops.iter().any(|d| {
                d.stack().is_some_and(|s| {
                    mod_api::__private::item_key_id(s.item()) == Some("minecraft:dirt")
                })
            }) {
                return mod_api::Flow::Cancel;
            }
            event.experience += 2;
            mod_api::Flow::Continue
        }
    }

    /// `Block.dropResources` builds its item entities (and samples the experience) before
    /// `BlockDropsEvent`, so the level random ends where it does without a listener; a mod that
    /// cancels gets no entities and no experience but the draws still happened; a mod that
    /// edits the amount changes only the amount.
    #[test]
    #[ignore = "daily"]
    fn block_drops_draws_before_the_event_and_cancel_empties_the_break() {
        use std::sync::Arc;
        use sunrise_core::random::LegacyRandom;
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)));
            let registry = game.pack.assets.registry;
            let dirt = registry.default_state("minecraft:dirt").unwrap();
            let ore = registry.default_state("minecraft:diamond_ore").unwrap();
            let pos = BlockPos::new(1, 64, 2);
            let break_of = |game: &mut crate::game::ServerGame<'_>, state, item, exp_to_drop| {
                game.world.random =
                    sunrise_core::random::LevelRandom::for_level(LegacyRandom::new(7));
                let dropped = game.with_level(|_, level, _| {
                    level.block_dropped(
                        pos,
                        state,
                        None,
                        None,
                        BlockDropEntity::None,
                        vec![ItemStack::new(item, 1)],
                        exp_to_drop,
                    )
                });
                (dropped, game.world.random.next_int())
            };
            let (plain_ore, ore_after) = break_of(game, ore, "minecraft:diamond", None);
            let (plain_dirt, dirt_after) = break_of(game, dirt, "minecraft:dirt", None);
            let (overridden, _) = break_of(game, ore, "minecraft:diamond", Some(17));
            assert_eq!(overridden.experience, 17);
            assert_eq!(plain_dirt.items.len(), 1);
            let mods: mod_api::Mods =
                vec![mod_api::ModEntry::new("block-drops", Box::new(DropsMod))].into();
            mod_platform::testing::with_mods(mods, || {
                let (heard, after) = break_of(game, ore, "minecraft:diamond", None);
                assert_eq!(after, ore_after);
                assert_eq!(heard.items.len(), 1);
                assert_eq!(heard.items[0].position, plain_ore.items[0].position);
                assert_eq!(heard.experience, plain_ore.experience + 2);
                assert!(!heard.canceled);
                let (overridden, _) = break_of(game, ore, "minecraft:diamond", Some(17));
                assert_eq!(overridden.experience, 19);
                let (taken, after) = break_of(game, dirt, "minecraft:dirt", None);
                assert_eq!(after, dirt_after);
                assert!(taken.canceled && taken.items.is_empty() && taken.experience == 0);
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn disabled_block_drops_draw_offsets_without_constructing_items() {
        use crate::game::native_entity_refs::tests::with_game;
        use std::sync::{Arc, Mutex};
        use sunrise_level::random_source::record_seeds;

        struct Observe(Arc<Mutex<Vec<usize>>>);
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Observe {
            fn block_drops(
                &self,
                _: &dyn mod_api::LevelHost,
                event: &mut mod_api::BlockDropsEvent<'_>,
            ) -> mod_api::Flow {
                self.0.lock().unwrap().push(event.drops.len());
                mod_api::Flow::Continue
            }
        }

        let seen = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![mod_api::ModEntry::new(
            "observe",
            Box::new(Observe(seen.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                game.options.dedicated = Some(Vec::new());
                assert!(game.ui.rules.set(
                    "block_drops",
                    sunrise_game_data::gamerules::Value::Bool(false),
                ));
                let dirt = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:dirt")
                    .unwrap();
                game.world.random.set_seed(7);
                let (dropped, seeds) = record_seeds(|| {
                    game.with_level(|_, level, _| {
                        level.block_dropped(
                            BlockPos::new(2, 64, 1),
                            dirt,
                            None,
                            None,
                            BlockDropEntity::None,
                            vec![ItemStack::new("minecraft:dirt", 1)],
                            Some(5),
                        )
                    })
                });
                assert!(dropped.items.is_empty());
                assert_eq!(dropped.experience, 0);
                assert!(seeds.complete(), "no entity RNG seed is drawn");
                // Java Random(7) after three nextDouble calls for the offsets.
                assert_eq!(game.world.random.next_int(), -441_191_359);
            });
        });
        assert_eq!(*seen.lock().unwrap(), [0]);
    }

    /// Removing drop 0 shifts drop 1 without stealing drop 0's `ItemRandom`, and a later
    /// listener's invalid stack edit or out-of-bounds edit is rejected per call without rolling
    /// back earlier listeners' valid drop mutations or experience.
    #[test]
    #[ignore = "daily"]
    fn block_drops_per_edit_rejection_and_random_preservation() {
        use mod_api::{BlockDropsEvent, Flow, Hooks, ItemDrop, ModEntry, Mods, StackWriteError};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        use sunrise_core::random::LegacyRandom;

        struct MutatingListener;
        #[mod_api::__subscriptions]
        impl Hooks for MutatingListener {
            fn block_drops(
                &self,
                _: &dyn mod_api::LevelHost,
                event: &mut BlockDropsEvent<'_>,
            ) -> Flow {
                assert_eq!(event.drops.len(), 2);
                assert!(event.drops.remove(0));
                assert_eq!(event.drops.len(), 1);
                event.drops.get_mut(0).unwrap().set_pickup_delay(25);
                let diamond = mod_api::ItemStack::new(
                    mod_api::__private::item_key("minecraft:diamond").unwrap(),
                    1,
                )
                .unwrap();
                event
                    .drops
                    .push(ItemDrop::new(
                        diamond,
                        [1.5, 65.0, 2.5],
                        [0.0, 0.2, 0.0],
                        10,
                    ))
                    .unwrap();
                event.experience = 9;
                Flow::Continue
            }
        }

        struct InvalidEditListener(Arc<AtomicBool>);
        #[mod_api::__subscriptions]
        impl Hooks for InvalidEditListener {
            fn block_drops(
                &self,
                _: &dyn mod_api::LevelHost,
                event: &mut BlockDropsEvent<'_>,
            ) -> Flow {
                assert_eq!(event.drops.len(), 2);
                let stone = mod_api::__private::item_key("minecraft:stone").unwrap();
                let invalid = mod_api::ItemStack::new(stone, 65).unwrap();
                let invalid_drop = ItemDrop::new(invalid.clone(), [1.5, 65.0, 2.5], [0.0; 3], 10);
                assert_eq!(
                    event.drops.push(invalid_drop),
                    Err(StackWriteError::CountExceeded { maximum: 64 })
                );
                assert_eq!(
                    event.drops.get_mut(0).unwrap().set_stack(invalid),
                    Err(StackWriteError::CountExceeded { maximum: 64 })
                );
                let valid = ItemDrop::new(
                    mod_api::ItemStack::new(stone, 1).unwrap(),
                    [1.5, 65.0, 2.5],
                    [0.0; 3],
                    10,
                );
                assert_eq!(event.drops.insert(99, valid), Ok(false));
                assert!(!event.drops.remove(99));
                assert_eq!(event.drops.len(), 2);
                let surviving = event.drops.get(0).unwrap();
                assert_eq!(
                    mod_api::__private::item_key_id(surviving.stack().unwrap().item()),
                    Some("minecraft:cobblestone")
                );
                assert_eq!(surviving.pickup_delay(), 25);
                self.0.store(true, Ordering::Relaxed);
                Flow::Continue
            }
        }

        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)));
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            let pos = BlockPos::new(1, 64, 2);
            let break_of = |game: &mut crate::game::ServerGame<'_>| {
                game.world.random =
                    sunrise_core::random::LevelRandom::for_level(LegacyRandom::new(7));
                let dropped = game.with_level(|_, level, _| {
                    level.block_dropped(
                        pos,
                        stone,
                        None,
                        None,
                        BlockDropEntity::None,
                        vec![
                            ItemStack::new("minecraft:stone", 1),
                            ItemStack::new("minecraft:cobblestone", 1),
                        ],
                        None,
                    )
                });
                (dropped, game.world.random.next_int())
            };

            let ((baseline, baseline_after), entity_seeds) =
                sunrise_level::random_source::record_seeds(|| break_of(game));
            assert_eq!(baseline.items.len(), 2);
            assert!(baseline.items[0].random.is_some());
            assert!(baseline.items[1].random.is_some());
            assert_ne!(baseline.items[0].random, baseline.items[1].random);

            let ran_second = Arc::new(AtomicBool::new(false));
            let mods: Mods = vec![
                ModEntry::new("first", Box::new(MutatingListener)),
                ModEntry::new("second", Box::new(InvalidEditListener(ran_second.clone()))),
            ]
            .into();
            mod_platform::testing::with_mods(mods, || {
                let (heard, after) =
                    sunrise_level::random_source::replay_seeds(Arc::clone(&entity_seeds), || {
                        break_of(game)
                    });
                assert!(entity_seeds.complete());
                assert!(ran_second.load(Ordering::Relaxed));
                assert_eq!(after, baseline_after);
                assert_eq!(heard.experience, 9);
                assert!(!heard.canceled);
                assert_eq!(heard.items.len(), 2);
                assert_eq!(&*heard.items[0].stack.item, "minecraft:cobblestone");
                assert_eq!(heard.items[0].stack.count, 1);
                assert_eq!(heard.items[0].pickup_delay, 25);
                assert_eq!(heard.items[0].position, baseline.items[1].position);
                assert_eq!(heard.items[0].velocity, baseline.items[1].velocity);
                assert_eq!(heard.items[0].random, baseline.items[1].random);
                assert_ne!(heard.items[0].random, baseline.items[0].random);
                assert_eq!(&*heard.items[1].stack.item, "minecraft:diamond");
                assert_eq!(heard.items[1].stack.count, 1);
                assert_eq!(heard.items[1].position, [1.5, 65.0, 2.5]);
                assert_eq!(heard.items[1].velocity, [0.0, 0.2, 0.0]);
                assert_eq!(heard.items[1].pickup_delay, 10);
                assert_eq!(heard.items[1].random, None);
            });
        });
    }

    /// An empty or unrepresentable optional tool (such as an unregistered item from an older
    /// save) omits only `event.tool` and still dispatches `block_drops` so listeners can edit
    /// drops and experience or cancel the break.
    #[test]
    #[ignore = "daily"]
    fn block_drops_dispatches_with_empty_or_unrepresentable_tool() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use sunrise_core::random::LegacyRandom;

        struct ToolOmitListener(Arc<AtomicUsize>);
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for ToolOmitListener {
            fn block_drops(
                &self,
                _: &dyn mod_api::LevelHost,
                event: &mut mod_api::BlockDropsEvent<'_>,
            ) -> mod_api::Flow {
                self.0.fetch_add(1, Ordering::Relaxed);
                assert!(
                    event.tool.is_none(),
                    "empty or unrepresentable tools omit only event.tool"
                );
                if event.drops.iter().any(|drop| {
                    drop.stack().is_some_and(|stack| {
                        mod_api::__private::item_key_id(stack.item()) == Some("minecraft:dirt")
                    })
                }) {
                    return mod_api::Flow::Cancel;
                }
                event.drops.get_mut(0).unwrap().set_pickup_delay(44);
                event.experience += 5;
                mod_api::Flow::Continue
            }
        }

        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)));
            let registry = game.pack.assets.registry;
            let ore = registry.default_state("minecraft:diamond_ore").unwrap();
            let dirt = registry.default_state("minecraft:dirt").unwrap();
            let pos = BlockPos::new(1, 64, 2);
            let calls = Arc::new(AtomicUsize::new(0));
            let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
                "tool-omit",
                Box::new(ToolOmitListener(calls.clone())),
            )]
            .into();
            mod_platform::testing::with_mods(mods, || {
                for tool in [
                    ItemStack::new("missingmod:ghost_pick", 1),
                    ItemStack::new("minecraft:air", 1),
                    ItemStack::new("minecraft:diamond_pickaxe", 0),
                ] {
                    game.world.random =
                        sunrise_core::random::LevelRandom::for_level(LegacyRandom::new(7));
                    let edited = game.with_level(|_, level, _| {
                        level.block_dropped(
                            pos,
                            ore,
                            None,
                            Some(&tool),
                            BlockDropEntity::None,
                            vec![ItemStack::new("minecraft:diamond", 1)],
                            Some(3),
                        )
                    });
                    assert!(!edited.canceled);
                    assert_eq!(edited.experience, 8);
                    assert_eq!(edited.items.len(), 1);
                    assert_eq!(edited.items[0].pickup_delay, 44);

                    let canceled = game.with_level(|_, level, _| {
                        level.block_dropped(
                            pos,
                            dirt,
                            None,
                            Some(&tool),
                            BlockDropEntity::None,
                            vec![ItemStack::new("minecraft:dirt", 1)],
                            Some(3),
                        )
                    });
                    assert!(canceled.canceled);
                    assert!(canceled.items.is_empty());
                    assert_eq!(canceled.experience, 0);
                }
            });
            assert_eq!(calls.load(Ordering::Relaxed), 6);
        });
    }

    /// A mod block entity's part answers its removal as vanilla's `preRemoveSideEffects`
    /// override does: found by the entity's type, not its block, reading its attachments in
    /// place, its stacks scattered after the block entity's own; skipped under flag 256, and
    /// drawing nothing when it gives nothing back.
    #[test]
    #[ignore = "daily"]
    fn a_block_entity_part_gives_back_what_it_held_when_removed() {
        use mod_api::Holder;
        use mod_platform::__private::part_tests::{HOARD, HOARD_ASKED, game_thread};
        use std::sync::Arc;
        use std::sync::atomic::Ordering;
        use sunrise_level::chunk::Chunk;
        use sunrise_level::containers::{ModBlockEntity, ModBlockEntityFields};
        crate::game::native_entity_refs::tests::with_game(|game| {
            let _mods = game_thread();
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            // Each case: the entity's type, its hoard, side effects; the gold that comes out.
            let asked = HOARD_ASKED.load(Ordering::Relaxed);
            for (id, hoard, side_effects, gold) in [
                ("doubler:hoard", 5, true, 5),
                ("doubler:hoard", 5, false, 0),
                ("doubler:hoard", 0, true, 0),
                // A different declared type's removal doesn't reach `Hoard`.
                ("doubler:press_entity", 5, true, 0),
            ] {
                let pos = BlockPos::new(1, 64, 2);
                let key = (pos.x, pos.y, pos.z);
                let entity = BlockEntity::Mod(Box::new(ModBlockEntity::new(
                    id.into(),
                    ModBlockEntityFields::default(),
                )));
                game.world.block_entities.insert(key, entity);
                let holder = Holder::BlockEntity([pos.x, pos.y, pos.z]);
                let host: &mut dyn mod_api::Host = game;
                HOARD.set(host, holder, hoard).unwrap();
                let (spawns, random) = game.with_level(|_, level, _| {
                    let mut scatter = level.world.random.clone();
                    level.remove_entity(pos, stone, side_effects);
                    // The game's own draws for what came out, from the random as it was.
                    let count = level.spill.spawns.iter().map(|s| s.stack.count).sum();
                    if count > 0 {
                        removal::drop_item_stack(&mut scatter, pos, count);
                    }
                    let spawns: Vec<_> = level
                        .spill
                        .spawns
                        .iter()
                        .map(|s| (s.stack.item.to_string(), s.stack.count))
                        .collect();
                    (spawns, scatter.next_int() == level.world.random.next_int())
                });
                let total: u32 = spawns.iter().map(|(_, count)| count).sum();
                assert!(
                    spawns
                        .iter()
                        .all(|(item, _)| item == "minecraft:gold_ingot")
                );
                assert_eq!(
                    total, gold,
                    "{id} holding {hoard}, side effects {side_effects}"
                );
                assert!(random, "{id}: the random drew only the scatter");
                assert!(game.world.block_entities.get(&key).is_none());
            }
            // Only the two `doubler:hoard` removals with side effects asked it.
            assert_eq!(HOARD_ASKED.load(Ordering::Relaxed) - asked, 2);
        });
    }

    /// LevelChunk ticks the resident row and calls its pre-removal effects before retiring it.
    /// Replacing its own block must spill both the container contents and the Part's gold.
    #[test]
    #[ignore = "daily"]
    fn a_block_entity_removing_itself_in_its_tick_runs_resident_removal() {
        use mod_api::Holder;
        use mod_platform::__private::part_tests::{HOARD, HOARD_ASKED};
        use std::sync::Arc;
        use std::sync::atomic::Ordering;
        use sunrise_level::chunk::Chunk;
        use sunrise_level::containers::{ModBlockEntity, ModBlockEntityFields, ModContainer};
        let Some(root) = crate::game::native_entity_refs::tests::declared_part_root(
            "game::server_side::spill::tests::a_block_entity_removing_itself_in_its_tick_runs_resident_removal",
        ) else {
            return;
        };
        crate::game::native_entity_refs::tests::with_game_at(root, |game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let registry = &game.pack.assets.registry;
            let hoard = registry.default_state("test:hoard").unwrap();
            let granite = registry.default_state("minecraft:granite").unwrap();
            let pos = BlockPos::new(1, 64, 2);
            let key = (pos.x, pos.y, pos.z);
            game.world.set(pos.x, pos.y, pos.z, hoard).unwrap();
            game.world.block_entities.set_mod_containers(
                [(
                    hoard.0,
                    Arc::new(ModContainer {
                        slots: 1,
                        ..Default::default()
                    }),
                )]
                .into_iter()
                .collect(),
            );
            // Data `[1]` makes the hoard's tick set its own block to granite.
            let fields = ModBlockEntityFields {
                items: vec![Some(ItemStack::new("minecraft:diamond", 3))],
                data: vec![1],
                ..Default::default()
            };
            game.world.block_entities.insert(
                key,
                BlockEntity::Mod(Box::new(ModBlockEntity::new(
                    "doubler:hoard".into(),
                    fields,
                ))),
            );
            let holder = Holder::BlockEntity([pos.x, pos.y, pos.z]);
            let host: &mut dyn mod_api::Host = game;
            HOARD.set(host, holder, 5).unwrap();
            let asked = HOARD_ASKED.load(Ordering::Relaxed);
            let dropped = game.dropped.len();
            assert!(game.tick_mod_block_entity(key));
            assert_eq!(game.world.block(pos.x, pos.y, pos.z), granite);
            assert_eq!(HOARD_ASKED.load(Ordering::Relaxed), asked + 1);
            assert!(game.world.block_entities.get(&key).is_none());
            assert!(game.world.block_entities.attachments(key).is_none());
            let out: Vec<_> = game.dropped[dropped..]
                .iter()
                .map(|d| (d.stack.item.to_string(), d.stack.count))
                .collect();
            assert_eq!(
                out,
                [
                    ("minecraft:diamond".to_owned(), 3),
                    ("minecraft:gold_ingot".to_owned(), 5),
                ]
            );
        });
    }
    use sunrise_core::nbt::Tag;
    use sunrise_items::item_data;
    use sunrise_sim::{block_entity_item, mobs::raider, save};

    #[test]
    fn a_used_run_is_retold_in_place() {
        let spawn = |item: &str, x: f64| Spawn {
            random: None,
            stack: ItemStack::new(item, 1),
            pos: [x, 0.0, 0.0],
            motion: None,
            delay: 10,
        };
        let mut spawns = vec![
            spawn("minecraft:stone", 0.0),
            spawn("minecraft:sweet_berries", 1.0),
            spawn("minecraft:sweet_berries", 2.0),
            spawn("minecraft:dirt", 3.0),
        ];
        let used = [UsedDrops {
            start: 1,
            end: 3,
            pos: [4, 64, 4],
            state: BlockStateId(0),
            table: "minecraft:harvest/sweet_berry_bush",
        }];
        retell_runs(&mut spawns, &used, |used, origin, drops| {
            assert_eq!(used.table, "minecraft:harvest/sweet_berry_bush");
            assert_eq!(origin, [4.5, 64.5, 4.5]);
            assert_eq!(drops.len(), 2);
            drops.remove(0);
            drops.push(ItemStack::new("minecraft:diamond", 1));
        });
        let items: Vec<(&str, f64)> = spawns.iter().map(|s| (&*s.stack.item, s.pos[0])).collect();
        assert_eq!(
            items,
            [
                ("minecraft:stone", 0.0),
                ("minecraft:sweet_berries", 1.0),
                ("minecraft:diamond", 2.0),
                ("minecraft:dirt", 3.0),
            ]
        );
    }

    #[test]
    fn tool_source_keeps_the_full_patch_and_distinguishes_stored_enchantments() {
        let potion = Tag::compound([("potion", Tag::String("minecraft:healing".into()))]);
        let stored = Tag::compound([("minecraft:sharpness", Tag::Int(13))]);
        let patch = Tag::compound([
            ("minecraft:potion_contents", potion.clone()),
            ("minecraft:stored_enchantments", stored.clone()),
            ("!minecraft:unbreakable", Tag::Compound(Default::default())),
        ]);
        let (damage, extra) = item_data::decode_components(Some(&patch));
        let tool = ItemStack {
            item: std::sync::Arc::from("minecraft:enchanted_book"),
            count: 1,
            damage,
            extra,
        };
        let snapshot = crate::game::creatures::to_loot(&tool);
        assert!(snapshot.enchantments.is_empty());
        let prototype = [
            (
                "minecraft:unbreakable".into(),
                Tag::Compound(Default::default()),
            ),
            (
                "minecraft:enchantments".into(),
                Tag::Compound(Default::default()),
            ),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            sunrise_world::loot::copy::stack_components(&snapshot, Some(&prototype)),
            [
                ("minecraft:potion_contents".into(), potion),
                ("minecraft:stored_enchantments".into(), stored),
                (
                    "minecraft:enchantments".into(),
                    Tag::Compound(Default::default())
                )
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn ominous_banner_survives_placement_save_and_drop() {
        let original = raider::ominous_banner();
        let Tag::Compound(components) = item_data::component_snapshot(&original).unwrap() else {
            panic!("banner components");
        };
        for block in ["minecraft:white_banner", "minecraft:white_wall_banner"] {
            let pos = (1, 64, 2);
            let fresh = BlockEntity::for_block(block).unwrap();
            let fresh = save::block_entity_encode::encode(pos, &fresh, block, &Default::default());
            let placed = block_entity_item::place(&fresh, &components, false);
            let (_, placed) = save::decode_block_entity(&placed).unwrap();
            let saved = save::block_entity_encode::encode(pos, &placed, block, &Default::default());
            let (_, loaded) = save::decode_block_entity(&saved).unwrap();
            let BlockEntity::Banner(banner) = &loaded else {
                panic!("placed banner");
            };
            assert_eq!(banner.layers.len(), item_data::OMINOUS_LAYERS.len());
            let mut pending = Pending::default();
            pending.record_saved([pos.0, pos.1, pos.2], &saved);
            let mut items = sunrise_items::items::Items::for_tests(Default::default());
            items.set_loot_tables(std::sync::Arc::new([(
                "test:banner".to_owned(),
                serde_json::json!({"type":"minecraft:empty","pools":[{"rolls":1,"entries":[{
                    "type":"minecraft:item","name":"minecraft:white_banner",
                    "modifier":{"type":"minecraft:copy_components","source":"block_entity",
                        "include":["minecraft:banner_patterns","minecraft:custom_name",
                            "minecraft:item_name","minecraft:rarity","minecraft:tooltip_display"]}
                }]}]}),
            )].into_iter().collect()));
            let source = pending.source([pos.0, pos.1, pos.2]);
            let dropped = sunrise_world::loot::copy::with_block_source(source.as_deref(), || {
                items
                    .loot_table(
                        "test:banner",
                        false,
                        None,
                        &mut sunrise_core::random::LegacyRandom::new(0),
                    )
                    .remove(0)
            });
            assert!(dropped.data().unwrap().ominous_banner);
            assert!(sunrise_sim::banner::tooltip_layers(&dropped).is_empty());
            assert_eq!(
                sunrise_client::item_tooltip::name_line(&dropped, &|_| None),
                "Ominous Banner"
            );
            assert_eq!(
                sunrise_sim::stations::rarity(&dropped, "common"),
                "uncommon"
            );
            assert_eq!(
                item_data::component_snapshot(&dropped),
                Some(Tag::Compound(components.clone()))
            );
        }
    }
}

#[cfg(test)]
mod crafter_drop_name_tests {
    use super::*;
    use sunrise_core::nbt::Tag;

    #[test]
    #[ignore = "daily"]
    fn actual_crafter_destruction_keeps_source_name_off_the_dropped_item() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            game.block_ticks
                .add_empty_chunk(BlockPos::new(0, 0, 0).chunk())
                .unwrap();
            assert!(game.run_command("forceload add 0 0"));
            assert!(game.run_command("fill 0 199 0 15 199 15 stone"));
            assert!(
                game.ui
                    .rules
                    .bool_at(sunrise_game_data::gamerules::vanilla::BLOCK_DROPS)
            );
            let clock = game.world.game_time;
            let mut unexpected = Vec::new();
            // Positive name-copy and unnamed controls precede the assigned case.
            for (x, z, kind, name) in [
                (3, 11, "chest", Some("Box")),
                (11, 11, "shulker_box", Some("Box")),
                (11, 3, "crafter", None),
                (3, 3, "crafter", Some("Rig")),
            ] {
                let extra = name.map_or(String::new(), |n| format!(",CustomName:\"{n}\""));
                assert!(game.run_command(&format!(
                "setblock {x} 200 {z} minecraft:{kind}{{Items:[{{Slot:0b,id:\"minecraft:diamond\",count:3,components:{{\"minecraft:custom_data\":{{h212:1}}}}}}]{extra}}} strict"
            )));
                let key = (x, 200, z);
                let item_id = format!("minecraft:{kind}");
                assert_eq!(
                    &*game.res.facts().get(game.world.block(x, 200, z)).name,
                    item_id
                );
                assert_eq!(
                    &*game.res.facts().get(game.world.block(x, 199, z)).name,
                    "minecraft:stone"
                );
                assert_eq!(
                    game.world.block_names.get(&key),
                    name.map(|n| Tag::String(n.into())).as_ref()
                );
                let entity = game
                    .world
                    .block_entities
                    .get(&key)
                    .expect("actual source block entity");
                assert_eq!(matches!(entity, BlockEntity::Crafter(_)), kind == "crafter");
                let diamond = entity.slots()[0].as_ref().expect("actual loaded inventory");
                assert_eq!((&*diamond.item, diamond.count), ("minecraft:diamond", 3));
                assert_eq!(
                    diamond
                        .data()
                        .and_then(|d| d.other("minecraft:custom_data"))
                        .and_then(|t| t.get("h212")),
                    Some(&Tag::Int(1))
                );
                assert!(game.world.loaded(x, z), "actual destruction-level chunk");
                assert!(game.run_command(&format!(
                "execute if data block {x} 200 {z} {{id:\"minecraft:{kind}\",x:{x},y:200,z:{z}}}"
            )));
                if let Some(n) = name {
                    assert!(game.run_command(&format!(
                        "execute if data block {x} 200 {z} {{CustomName:\"{n}\"}}"
                    )));
                } else {
                    assert!(
                        game.run_command(&format!(
                            "execute unless data block {x} 200 {z} CustomName"
                        ))
                    );
                }
                let start = game.dropped.len();
                assert!(game.run_command(&format!("setblock {x} 200 {z} air destroy")));
                game.flush_spill();
                assert!(game.res.facts().is_air(game.world.block(x, 200, z)));
                assert!(!game.world.block_entities.contains_key(&key));
                assert!(!game.world.block_names.contains_key(&key));
                let emitted = &game.dropped[start..];
                let matching: Vec<_> = emitted
                    .iter()
                    .filter(|d| *d.stack.item == item_id)
                    .collect();
                assert_eq!(matching.len(), 1, "one actual block item: {kind}");
                let drop = matching[0];
                assert_eq!(drop.stack.count, 1);
                assert_eq!((drop.age, drop.pickup_delay), (0, 10));
                let actual = drop.stack.data().and_then(|d| d.custom_name.as_ref());
                if kind == "crafter" {
                    if actual.is_some() {
                        unexpected.push((name, actual.cloned()));
                    }
                } else {
                    assert_eq!(
                        actual,
                        Some(&Tag::String("Box".into())),
                        "positive ordinary name-copy control"
                    );
                }
                if kind == "shulker_box" {
                    let contents = &drop.stack.data().unwrap().container;
                    assert_eq!(contents.len(), 1);
                    let kept = &contents[0].1;
                    assert_eq!((&*kept.item, kept.count), ("minecraft:diamond", 3));
                    assert_eq!(
                        kept.data()
                            .and_then(|d| d.other("minecraft:custom_data"))
                            .and_then(|t| t.get("h212")),
                        Some(&Tag::Int(1))
                    );
                    assert_eq!(emitted.len(), 1);
                } else {
                    let contents: Vec<_> = emitted
                        .iter()
                        .filter(|d| &*d.stack.item == "minecraft:diamond")
                        .collect();
                    assert_eq!(contents.len(), 1);
                    assert_eq!(contents[0].stack.count, 3);
                    assert_eq!(
                        contents[0]
                            .stack
                            .data()
                            .and_then(|d| d.other("minecraft:custom_data"))
                            .and_then(|t| t.get("h212")),
                        Some(&Tag::Int(1))
                    );
                    assert_eq!(emitted.len(), 2);
                }
                assert_eq!(game.world.game_time, clock, "zero observation ticks");
            }
            assert!(
                unexpected.is_empty(),
                "actual vanilla crafter drop retained source name: {unexpected:?}"
            );
        });
    }
}
