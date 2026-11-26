use super::{Arg, Context, Game, R, Target, err, items::Owner};
use crate::game::commands::terrain_cmds::holder;
use crate::game::commands::terrain_cmds::loot_location_block;
use crate::game::commands::terrain_cmds::num;
use sunrise_command::CommandError;
#[allow(unused_imports)]
use sunrise_core::{position::ChunkPos, random::RandomSource};
use sunrise_game_data::text::Text;
use sunrise_items::items::ItemStack;
use sunrise_sim::scoreboard_cmd::Tracker;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `LootCommand.dropBlockLoot`: copy the actual block entity's components
    /// into the mine result without removing the source block or its contents.
    pub(in crate::game::server_side) fn cmd_loot(&mut self, id: &str, ctx: &Context) -> R<i32> {
        let keep_empty = ctx.literals.iter().any(|literal| literal == "replace");
        // The drops.
        let (drops, table): (Vec<ItemStack>, Option<String>) = match id {
            "loot.loot" | "loot.fish" => {
                let Some(Arg::Resource(table)) = ctx.get("loot_table").cloned() else {
                    return Ok(0);
                };
                // An inline table (ResourceOrIdArgument) keeps its random sequence.
                if table.starts_with('{') {
                    let (tag, _) = sunrise_core::nbt::parse_snbt(&table)
                        .map_err(|e| err("argument.nbt.invalid", &[&e]))?;
                    let json = sunrise_game_data::scoreboard::tag_to_json(&tag);
                    sunrise_world::loot::validate_loot_table(&json)
                        .map_err(|e| err("argument.nbt.invalid", &[&e]))?;
                    // LootCommand's omitted fishing tool is EMPTY, not mainhand.
                    let held = if id == "loot.fish"
                        && (ctx.get("tool").is_some() || ctx.has("mainhand") || ctx.has("offhand"))
                    {
                        self.tool_stack(ctx)?
                    } else {
                        None
                    };
                    let drops = match json["random_sequence"].as_str() {
                        Some(sequence) => {
                            let random = self.side.rand_sequences.get(sequence, self.level.seed);
                            server_world_items!(self).inline_loot_table_for_command(
                                &json,
                                held.as_ref(),
                                random,
                                keep_empty,
                            )
                        }
                        None => server_world_items!(self).inline_loot_table_for_command(
                            &json,
                            held.as_ref(),
                            &mut self.world.random,
                            keep_empty,
                        ),
                    };
                    (drops, None)
                } else {
                    if !self.items().has_loot_table(&table) {
                        return Err(err("argument.resource_or_id.failed_to_parse", &[&table]));
                    }
                    let held = if id == "loot.fish" {
                        self.tool_stack(ctx)?
                    } else {
                        None
                    };
                    let drops = match server_world_items!(self).loot_sequence(&table) {
                        Some(seq) => {
                            let random = self.side.rand_sequences.get(&seq, self.level.seed);
                            server_world_items!(self).loot_table_for_command(
                                &table,
                                false,
                                held.as_ref(),
                                random,
                                keep_empty,
                            )
                        }
                        None => server_world_items!(self).loot_table_for_command(
                            &table,
                            false,
                            held.as_ref(),
                            &mut self.world.random,
                            keep_empty,
                        ),
                    };
                    (drops, None)
                }
            }
            "loot.kill" => {
                let targets = self.entities(ctx, "target", false)?;
                let Some(t) = targets.first().copied() else {
                    return Ok(0);
                };
                let ty = self.target_type(t);
                // `EntityType.getDefaultLootTable`, in the type's namespace.
                let (namespace, path) = ty.split_once(':').unwrap_or(("minecraft", ty.as_str()));
                let table = format!("{namespace}:entities/{path}");
                if t == Target::Player || !world_items!(self).has_loot_table(&table) {
                    return Err(err(
                        "commands.drop.no_loot_table.entity",
                        &[&self.target_name(t).plain()],
                    ));
                }
                // LootCommand.dropKillLoot: the source's entity is the killer.
                // A connected player is the last damage player, and its own
                // stack counts; it is in place only to read that stack.
                let guest = match self.side.command_source.map(|s| s.entity) {
                    Some(Some(Target::Guest(conn))) => self.for_player(Target::Guest(conn), |g| {
                        g.slot_get(&super::items::Slot::Inv(g.selected))
                    }),
                    _ => None,
                };
                let (by_player, held) = match guest {
                    Some(held) => (true, held),
                    None => (
                        matches!(
                            self.side.command_source.map(|s| s.entity),
                            None | Some(Some(Target::Player))
                        ),
                        self.slot_get(&super::items::Slot::Inv(self.selected)),
                    ),
                };
                let drops = self.roll_kill_loot(crate::game::loot_rolls::KillRoll {
                    kind: &ty,
                    table: &table,
                    target: t,
                    killer: self
                        .side
                        .command_source
                        .map(|s| s.entity)
                        .unwrap_or(Some(Target::Player)),
                    by_player,
                    held: held.as_ref(),
                    keep_empty,
                });
                (drops, Some(table))
            }
            _ => {
                // loot mine
                let p = self.block_pos_arg(ctx, "pos")?;
                let state = self.world.block(p[0], p[1], p[2]);
                let name = self.data_block_name(p);
                let Some(table) =
                    world_items!(self).block_loot_table(self.pack.assets.registry, state)
                else {
                    return Err(err("commands.drop.no_loot_table.block", &[&name]));
                };
                let tool = self.tool_stack(ctx)?;
                let mut pending = crate::game::spill::Pending::default();
                let key = (p[0], p[1], p[2]);
                if !self.world.block_entities.contains_key(&key)
                    && let Some(saved) = self.block_entity_nbt(p)
                {
                    pending.record_saved(p, &saved);
                }
                // LootCommand: the source's entity is `this`, if it has one.
                let entity = match self.side.command_source.map(|s| s.entity) {
                    None | Some(Some(Target::Player)) => Some(self.native_clicker().1 as u32),
                    Some(Some(Target::Guest(conn))) => self
                        .player_entity_id_by_connection(Some(conn))
                        .map(|id| id as u32),
                    Some(Some(target)) => crate::game::native_entities::target_id(target),
                    Some(None) => None,
                };
                // The hook may borrow the live level mutably. Keep a read-only
                // view of the location for this roll rather than borrow Game.
                let mut chunks = sunrise_level::chunk_map::ChunkMap::default();
                for (&key, chunk) in self.world.chunks.iter() {
                    chunks.insert(key, chunk.clone());
                }
                let items = self
                    .server_items
                    .clone()
                    .unwrap_or_else(|| std::sync::Arc::new(world_items!(self).clone()));
                let registry = self.pack.assets.registry;
                let location = |condition: &serde_json::Value| {
                    loot_location_block(condition, p, &chunks, registry, &items)
                };
                let roll = sunrise_items::items::BlockLootRoll {
                    held: tool.as_ref(),
                    keep_empty,
                    location: Some(&location),
                    breaker: entity.is_some(),
                    ..Default::default()
                };
                let source = pending.source(p);
                let drops = sunrise_world::loot::copy::with_block_source(source.as_deref(), || {
                    self.roll_block_loot(p, state, entity, roll)
                });
                (drops, Some(table))
            }
        };
        let drops: Vec<ItemStack> = drops
            .into_iter()
            .filter(|d| keep_empty || d.count > 0)
            .collect();
        // Where they go.
        let mut used: Vec<ItemStack> = Vec::new();
        let literals = &ctx.literals;
        if literals.iter().any(|l| l == "give") {
            for d in &drops {
                for &t in &self.entities(ctx, "players", true)? {
                    // LootCommand.playerGive gives to every target player: a
                    // guest gets it with their state in place, as with /give.
                    let added = self.for_player(t, |g| {
                        let complete = g.creative;
                        if let Some(rest) = g.add_stack_to_inventory(d.clone()) {
                            let p = g.source_position();
                            g.spawn_item(rest, [p[0], p[1] + 1.32, p[2]], [0.0; 3], 0);
                            complete
                        } else {
                            true
                        }
                    });
                    if added == Some(true) {
                        used.push(d.clone());
                    }
                }
            }
        } else if literals.iter().any(|l| l == "spawn") {
            let at = self
                .position_arg(ctx, "targetPos")
                .unwrap_or_else(|| self.source_position());
            for d in &drops {
                let r = &mut self.world.random;
                let v = [
                    r.next_double() * 0.2 - 0.1,
                    0.2,
                    r.next_double() * 0.2 - 0.1,
                ];
                self.spawn_item(d.clone(), at, v, 10);
                used.push(d.clone());
            }
        } else if literals.iter().any(|l| l == "insert") {
            let p = self.block_pos_arg(ctx, "targetPos")?;
            let owner = Owner::Block(p);
            let slots = self.slots_named(&owner, "container.*");
            if slots.is_empty() {
                return Err(err(
                    "commands.item.target.not_a_container",
                    &[&p[0], &p[1], &p[2]],
                ));
            }
            for d in &drops {
                let mut left = d.clone();
                let mut placed = false;
                for s in &slots {
                    if !self.slot_accepts(s, Some(&left)) {
                        continue;
                    }
                    match self.slot_get(s) {
                        None => {
                            self.slot_set(s, Some(left.clone()));
                            placed = true;
                            left.count = 0;
                            break;
                        }
                        Some(mut cur)
                            if cur.item == left.item
                                && cur.damage == left.damage
                                && cur.extra == left.extra =>
                        {
                            let max = self.items().get(&cur.item).map_or(64, |i| i.max_stack);
                            let take = left.count.min(max.saturating_sub(cur.count));
                            if take > 0 {
                                cur.count += take;
                                left.count -= take;
                                self.slot_set(s, Some(cur));
                                placed = true;
                            }
                            if left.count == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if placed {
                    used.push(d.clone());
                }
            }
        } else {
            // replace entity|block <slot> [count]
            let block = literals.iter().any(|l| l == "block");
            let owners: Vec<Owner> = if block {
                vec![Owner::Block(self.block_pos_arg(ctx, "targetPos")?)]
            } else {
                self.entities(ctx, "entities", false)?
                    .into_iter()
                    .map(Owner::Entity)
                    .collect()
            };
            let Some(Arg::Str(slot_name)) = ctx.get("slot").cloned() else {
                return Ok(0);
            };
            let count = match ctx.get("count") {
                Some(Arg::Int(c)) => *c as usize,
                _ => drops.len(),
            };
            for o in owners {
                // `prefix.N` continues as `prefix.N+1`, ...
                let (prefix, start) = match slot_name.rsplit_once('.') {
                    Some((p, n)) if n.parse::<usize>().is_ok() => {
                        (Some(p.to_owned()), n.parse::<usize>().unwrap_or(0))
                    }
                    _ => (None, 0),
                };
                let first = self.slots_named(&o, &slot_name);
                if first.is_empty() {
                    if let Owner::Block(p) = &o {
                        return Err(err(
                            "commands.item.target.not_a_container",
                            &[&p[0], &p[1], &p[2]],
                        ));
                    }
                    return Err(err("commands.item.target.no_such_slot", &[&slot_name]));
                }
                for i in 0..count {
                    let slot = match &prefix {
                        Some(p) => self
                            .slots_named(&o, &format!("{p}.{}", start + i))
                            .first()
                            .cloned(),
                        None => first.first().cloned().filter(|_| i == 0),
                    };
                    let Some(slot) = slot else { continue };
                    let item = drops.get(i).filter(|drop| drop.count > 0).cloned();
                    if self.slot_accepts(&slot, item.as_ref())
                        && self.slot_set_by_command(&slot, item.clone())
                    {
                        used.push(item.unwrap_or(ItemStack {
                            item: "minecraft:air".into(),
                            count: 0,
                            damage: 0,
                            extra: None,
                        }));
                    }
                }
            }
        }
        let mut tracker = Tracker::default();
        for u in &used {
            // LootCommand's feedback names each drop by `getDisplayName`.
            let name = self.stack_display_name(u);
            // CommandResponseTracker.track(drop): one per stack, so the
            // command returns how many stacks it used, not how many items.
            tracker.track(&holder(name), 1);
        }
        let counts: Vec<u32> = used.iter().map(|u| u.count).collect();
        let (message, total) = tracker.message(
            false,
            None,
            |h, _| {
                let n = counts.iter().copied().find(|c| *c > 0).unwrap_or(0);
                match &table {
                    Some(t) => Text::translate(
                        "commands.drop.success.single_with_table",
                        &[num(n), h.text.clone(), Text::literal(t.clone())],
                    ),
                    None => {
                        Text::translate("commands.drop.success.single", &[num(n), h.text.clone()])
                    }
                }
            },
            |c, _| match &table {
                Some(t) => Text::translate(
                    "commands.drop.success.multiple_with_table",
                    &[num(c), Text::literal(t.clone())],
                ),
                None => Text::translate("commands.drop.success.multiple", &[num(c)]),
            },
        )?;
        self.feedback(message);
        Ok(total)
    }
    pub(in crate::game::server_side) fn tool_stack(
        &mut self,
        ctx: &Context,
    ) -> R<Option<ItemStack>> {
        if let Some(Arg::Item(p)) = ctx.get("tool") {
            let (damage, extra) = match p
                .components
                .as_deref()
                .map(sunrise_items::item_data::command_components)
            {
                Some(Ok(tag)) => sunrise_items::item_data::decode_components(Some(&tag)),
                Some(Err(e)) => return Err(CommandError::new(Text::literal(e))),
                None => (0, None),
            };
            return Ok(Some(ItemStack {
                item: p.id.as_str().into(),
                count: 1,
                damage,
                extra,
            }));
        }
        let entity = self
            .side
            .command_source
            .and_then(|s| s.entity)
            .unwrap_or(Target::Player);
        let slot = if ctx.has("offhand") {
            "weapon.offhand"
        } else {
            "weapon.mainhand"
        };
        Ok(self
            .slots_named(&Owner::Entity(entity), slot)
            .first()
            .and_then(|s| self.slot_get(s)))
    }
}

#[cfg(test)]
pub(in crate::game) mod loot_mine_component_tests {
    use super::*;
    use crate::game::commands::terrain_cmds::loot_mine_component_tests::slot;
    use crate::game::{Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_core::nbt::Tag;
    use sunrise_level::chunk::Chunk;

    #[test]
    #[ignore = "daily"]
    fn loot_mine_uses_the_actual_stand_or_server_executor_presence() {
        let root = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources required"),
        );
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        assert!(game.run_command("fill 0 199 0 15 199 15 stone"));

        game.side.command_source = Some(crate::game::commands::execute::Source {
            silent: false,
            ..game.server_source()
        });
        assert!(game.source_entity().is_none());
        assert!(game.run_command("execute unless entity @s"));
        assert!(game.run_command("setblock 12 200 4 chest strict"));
        assert!(game.run_command("setblock 2 200 4 snow[layers=1] strict"));
        assert!(game.run_command("setblock 4 200 4 dirt strict"));
        assert!(game.run_command("item replace block 12 200 4 container.26 with diamond 7"));
        assert!(game.run_command("item replace block 12 200 4 container.2 with diamond_shovel"));
        assert_eq!(&*slot(&game, 2).item, "minecraft:diamond_shovel");
        assert!(game.run_command("execute if items block 12 200 4 container.2 #minecraft:shovels"));
        assert!(game.run_command(
            "summon armor_stand 8.5 200 8.5 {UUID:[I;60,0,0,1],Health:20f,NoGravity:1b}"
        ));
        let stand = "0000003c-0000-0000-0000-000000000001";
        assert!(game.run_command(&format!("execute as {stand} if entity @s[type=armor_stand,nbt={{UUID:[I;60,0,0,1],Health:20f,NoGravity:1b,Pos:[8.5d,200d,8.5d]}}]")));
        let id = game
            .armor_stands
            .iter()
            .find(|s| game.persist.uuids.get(&s.id) == Some(&[60, 0, 0, 1]))
            .unwrap()
            .id;
        let before_stand = game.command_target_nbt(Target::Stand(id)).unwrap();
        assert_eq!(
            before_stand.get("UUID"),
            Some(&Tag::IntArray(vec![60, 0, 0, 1]))
        );
        let clock = game.world.game_time;
        // Unconditional dirt proves both sources really execute the same path.
        for (as_stand, x, expected) in [
            (false, 4, "minecraft:dirt"),
            (true, 4, "minecraft:dirt"),
            (false, 2, "minecraft:emerald"),
            (true, 2, "minecraft:snowball"),
        ] {
            assert!(game.source_entity().is_none());
            assert!(game.run_command("item replace block 12 200 4 container.0 with emerald 4"));
            let before = slot(&game, 0);
            let state = game.world.block(x, 200, 4);
            let prefix = if as_stand {
                format!("execute as {stand} run ")
            } else {
                String::new()
            };
            assert!(game.run_command(&format!(
                "{prefix}loot replace block 12 200 4 container.0 mine {x} 200 4 diamond_shovel"
            )));
            let actual = slot(&game, 0);
            assert_eq!(
                &*actual.item, expected,
                "executor stand={as_stand}, block x={x}"
            );
            if !as_stand && x == 2 {
                assert_eq!(actual, before);
            } else {
                assert_eq!(actual.count, 1);
            }
            assert_eq!(
                (&*slot(&game, 26).item, slot(&game, 26).count),
                ("minecraft:diamond", 7)
            );
            assert_eq!(&*slot(&game, 2).item, "minecraft:diamond_shovel");
            assert_eq!(game.world.block(x, 200, 4), state);
            assert_eq!(
                game.command_target_nbt(Target::Stand(id)).unwrap(),
                before_stand
            );
            assert_eq!(game.world.game_time, clock);
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod loot_mine_location_tests {
    use super::*;
    use crate::game::commands::terrain_cmds::loot_mine_location_tests::TABLE;
    use std::sync::Arc;
    use sunrise_level::{chunk::Chunk, containers::BlockEntity};

    #[test]
    #[ignore = "daily"]
    fn loot_mine_location_reads_actual_offset_block_state() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.game_dir = Some(
                std::path::PathBuf::from(std::env::var_os("SUNRISE_HOME").unwrap())
                    .join("h079-game"),
            );
            game.build_registries();
            game.set_allow_commands(true);
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            game.block_ticks
                .add_empty_chunk(ChunkPos::new(0, 0))
                .unwrap();

            game.world
                .chunks
                .insert((-1, -1), Arc::new(Chunk::empty(-1, -1)));
            game.world
                .chunks
                .insert((-1, 0), Arc::new(Chunk::empty(-1, 0)));
            game.world
                .chunks
                .insert((0, -1), Arc::new(Chunk::empty(0, -1)));
            for (x, z) in [(-1, -1), (-1, 0), (0, -1)] {
                game.block_ticks
                    .add_empty_chunk(ChunkPos::new(x, z))
                    .unwrap();
            }
            game.side.world_root =
                std::path::PathBuf::from(std::env::var_os("SUNRISE_HOME").unwrap())
                    .join("h079-world");
            let pack = game.side.world_root.join("datapacks/h079");
            std::fs::create_dir_all(pack.join("data/minecraft/loot_table/blocks")).unwrap();
            std::fs::create_dir_all(pack.join("data/h079/function")).unwrap();
            std::fs::write(
                pack.join("pack.mcmeta"),
                r#"{"pack":{"description":"H079 fixture"}}"#,
            )
            .unwrap();
            std::fs::write(
                pack.join("data/minecraft/loot_table/blocks/stone.json"),
                TABLE,
            )
            .unwrap();
            std::fs::write(
                pack.join("data/h079/function/admit.mcfunction"),
                "scoreboard players set pack h079 42\n",
            )
            .unwrap();
            std::fs::write(
                game.side.world_root.join("datapacks.json"),
                r#"{"enabled":["vanilla","file/h079"],"disabled":[]}"#,
            )
            .unwrap();
            let stack = game.reload_datapacks(false, &[]).unwrap();
            game.side.world_tables.0 = game
                .stage_world_tables(&stack, false, &game.options.resources.clone())
                .unwrap()
                .tables;
            assert!(game.side.world_tables.0.is_some());
            game.ensure_datapacks();
            assert!(game.run_command("scoreboard objectives add h079 dummy"));
            game.run_command("function h079:admit");
            assert!(
                game.run_command("execute if score pack h079 matches 42"),
                "real pack function admission"
            );
            assert!(game.run_command("forceload add -16 -16 15 15"));
            assert!(game.run_command("fill -16 199 -16 15 199 15 stone"));
            let clock = game.world.game_time;
            let mut differences = Vec::new();
            for (label, x, z, counterpart, expected) in [
                ("correct", 4, 4, Some("oak_log[axis=x]"), 1),
                ("wrong-state", 8, 4, Some("oak_log[axis=z]"), 0),
                ("absent", 12, 4, None, 0),
                ("negative-center", -4, -4, Some("oak_log[axis=x]"), 1),
            ] {
                assert!(game.run_command(&format!("setblock {x} 200 {z} stone strict")));
                if let Some(other) = counterpart {
                    assert!(
                        game.run_command(&format!("setblock {} 200 {z} {other} strict", x + 1))
                    );
                }
                assert!(game.run_command(&format!(
                    "execute if block {} 200 {z} {}",
                    x + 1,
                    counterpart.unwrap_or("air")
                )));
                let before = (game.world.block(x, 200, z), game.world.block(x + 1, 200, z));
                assert!(game.run_command(&format!("setblock {x} 200 10 chest strict")));
                let Some(BlockEntity::Chest(initial)) =
                    game.world.block_entities.get(&(x, 200, 10))
                else {
                    panic!("real output chest")
                };
                assert!(initial.iter().all(Option::is_none));
                let prefix = if x < 0 {
                    "execute positioned -8.25 200.125 -8.75 run "
                } else {
                    ""
                };
                assert!(game.run_command(&format!(
                    "{prefix}loot insert {x} 200 10 mine {x} 200 {z} stick"
                )));
                let Some(BlockEntity::Chest(output)) = game.world.block_entities.get(&(x, 200, 10))
                else {
                    panic!("actual output")
                };
                assert!(
                    output
                        .iter()
                        .flatten()
                        .any(|s| s.item.as_ref() == "minecraft:emerald" && s.count == 2),
                    "pack override and unconditional roll are real"
                );
                let diamonds: u32 = output
                    .iter()
                    .flatten()
                    .filter(|s| s.item.as_ref() == "minecraft:diamond")
                    .map(|s| s.count)
                    .sum();
                if diamonds != expected {
                    differences.push((label, diamonds, expected));
                }
                assert_eq!(
                    (game.world.block(x, 200, z), game.world.block(x + 1, 200, z)),
                    before
                );
            }
            assert_eq!(game.world.game_time, clock);
            assert!(
                differences.is_empty(),
                "actual offset block predicate differences: {differences:?}"
            );
        });
    }
}

#[cfg(test)]
#[path = "loot_slot_tests_server.rs"]
pub(in crate::game) mod loot_slot_tests;
