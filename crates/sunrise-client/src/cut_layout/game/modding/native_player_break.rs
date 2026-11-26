#![cfg(test)]
use mod_api::PlayerRef;
#[cfg(test)]
mod tests {
    use super::super::native_entity_refs::tests::with_game;
    use super::*;
    use sunrise_command::args::GameMode;
    use sunrise_core::geometry::Vec3;
    use sunrise_items::items::ItemStack;
    use sunrise_sim::motion::PlayerMotion;

    /// A server's game with its own player in survival standing at the
    /// origin chunk, holding `held`: their handle.
    fn with_player(
        game: &mut sunrise_gameplay::game::ServerGame<'_>,
        held: ItemStack,
    ) -> PlayerRef {
        // The shared fixture does not open a server or register a ticking chunk.
        game.options.dedicated = Some(Vec::new());
        let _ = game
            .block_ticks
            .add_empty_chunk(sunrise_core::position::ChunkPos::new(0, 0));
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(4.5, 64.0, 4.5)).unwrap());
        game.server.lan_acting_uuid = Some(123);
        // The host acts, whose entity the hooks name as 0 while no guest acts.
        game.server.lan_acting_entity = Some(0);
        game.server.lan_acting_name = Some("Breaker".into());
        game.server.lan_acting_permission = Some(0);
        game.set_mode(GameMode::Survival);
        game.common.inventory[game.common.selected] = Some(held);
        game.native_acting_player().unwrap()
    }
    fn place(game: &mut sunrise_gameplay::game::ServerGame<'_>, pos: [i32; 3], block: &str) {
        let state = game.pack.assets.registry.default_state(block).unwrap();
        game.world.set(pos[0], pos[1], pos[2], state).unwrap();
    }
    fn block(game: &sunrise_gameplay::game::ServerGame<'_>, pos: [i32; 3]) -> String {
        let state = game.world.block(pos[0], pos[1], pos[2]);
        game.res.facts().get(state).name.to_string()
    }
    fn carried(game: &sunrise_gameplay::game::ServerGame<'_>, item: &str) -> u32 {
        let held = game.inventory.iter().flatten();
        held.filter(|s| &*s.item == item).map(|s| s.count).sum()
    }
    fn popped(game: &sunrise_gameplay::game::ServerGame<'_>, item: &str) -> bool {
        game.dropped.iter().any(|e| &*e.stack.item == item)
    }

    /// Stone broken with a diamond pickaxe drops cobblestone, wears the
    /// pickaxe by one and counts as mined (`Block.playerDestroy`,
    /// `ItemStack.mineBlock`), as a player's own break does.
    #[test]
    #[ignore = "daily"]
    fn a_mod_breaks_stone_as_the_player_with_their_pickaxe() {
        with_game(|game| {
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            let player = with_player(game, ItemStack::new("minecraft:diamond_pickaxe", 1));
            let pos = [6, 64, 4];
            place(game, pos, "minecraft:stone");
            assert!(game.player_destroy_block(player, pos, false));
            assert_eq!(block(game, pos), "minecraft:air");
            assert!(popped(game, "minecraft:cobblestone"));
            assert_eq!(game.held().unwrap().damage, 1);
            assert_eq!(game.stats.get("mined", "minecraft:stone"), 1);
        });
    }

    /// Only the selected slot harvests a block that requires a tool. Iron doors do not
    /// require one in 26.3, so both hand arrangements harvest their lower-half drop.
    #[test]
    #[ignore = "daily"]
    fn harvesting_uses_the_selected_slot_and_iron_doors_need_no_tool() {
        with_game(|game| {
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            let pickaxe = ItemStack::new("minecraft:iron_pickaxe", 1);
            let player = with_player(game, pickaxe.clone());
            let iron = [8, 64, 4];
            place(game, iron, "minecraft:iron_block");
            assert!(game.player_destroy_block(player, iron, false));
            assert!(
                popped(game, "minecraft:iron_block"),
                "selected pickaxe harvests"
            );
            game.dropped.clear();
            game.common.inventory[game.common.selected] = None;
            game.offhand = Some(pickaxe.clone());
            game.hand = sunrise_sim::first_person_sim::Hand::Off;
            place(game, iron, "minecraft:iron_block");
            assert!(game.player_destroy_block(player, iron, false));
            assert!(
                !popped(game, "minecraft:iron_block"),
                "offhand cannot harvest"
            );
            game.common.inventory[game.common.selected] = Some(pickaxe.clone());
            game.offhand = None;
            game.hand = sunrise_sim::first_person_sim::Hand::Main;
            let (lower, upper) = ([6, 64, 4], [6, 65, 4]);
            let door =
                |game: &mut sunrise_gameplay::game::ServerGame<'_>, half: &str, pos: [i32; 3]| {
                    let state = game
                        .pack
                        .assets
                        .registry
                        .resolve(&sunrise_world::chunk::BlockState {
                            name: "minecraft:iron_door".into(),
                            properties: [
                                ("facing", "north"),
                                ("half", half),
                                ("hinge", "left"),
                                ("open", "false"),
                                ("powered", "false"),
                            ]
                            .into_iter()
                            .map(|(k, v)| (k.into(), v.into()))
                            .collect(),
                        })
                        .unwrap();
                    game.world.set(pos[0], pos[1], pos[2], state).unwrap();
                };
            door(game, "lower", lower);
            door(game, "upper", upper);
            assert!(game.player_destroy_block(player, upper, false));
            assert_eq!(block(game, upper), "minecraft:air");
            // Vanilla takes the lower half out in the same call, by the neighbour update.
            game.server_process_level_events().unwrap();
            assert_eq!(
                block(game, lower),
                "minecraft:air",
                "main hand: correct tool"
            );
            assert!(popped(game, "minecraft:iron_door"));

            game.dropped.clear();
            game.common.inventory[game.common.selected] = None;
            game.offhand = Some(pickaxe);
            game.hand = sunrise_sim::first_person_sim::Hand::Off;
            door(game, "lower", lower);
            door(game, "upper", upper);
            assert_eq!(block(game, lower), "minecraft:iron_door");
            assert!(game.player_destroy_block(player, upper, false));
            assert_eq!(block(game, upper), "minecraft:air");
            assert_eq!(
                block(game, lower),
                "minecraft:air",
                "empty main hand: door needs no tool"
            );
            assert!(popped(game, "minecraft:iron_door"));
        });
    }

    /// Into the inventory: silk touch keeps the stone whole, and a diamond
    /// ore gives its diamond and still its experience (3 to 7 points).
    #[test]
    #[ignore = "daily"]
    fn drops_go_into_the_inventory_rolled_for_the_tools_enchantments() {
        with_game(|game| {
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            let mut silk = ItemStack::new("minecraft:diamond_pickaxe", 1);
            silk.enchant("minecraft:silk_touch", 1);
            let player = with_player(game, silk);
            let (stone, ore) = ([6, 64, 4], [7, 64, 4]);
            place(game, stone, "minecraft:stone");
            assert!(game.player_destroy_block(player, stone, true));
            assert_eq!(carried(game, "minecraft:stone"), 1);
            assert!(!popped(game, "minecraft:stone"));

            game.common.inventory[game.common.selected] =
                Some(ItemStack::new("minecraft:diamond_pickaxe", 1));
            place(game, ore, "minecraft:diamond_ore");
            assert!(game.player_destroy_block(player, ore, true));
            assert_eq!(carried(game, "minecraft:diamond"), 1);
            assert!(!game.orbs.is_empty(), "the ore's experience");
        });
    }

    /// The player's own checks hold: an adventure player whose tool has no
    /// `can_break`, a survival player at bedrock, air, and someone who isn't
    /// playing break nothing.
    #[test]
    #[ignore = "daily"]
    fn the_players_checks_hold() {
        with_game(|game| {
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            let player = with_player(game, ItemStack::new("minecraft:diamond_pickaxe", 1));
            let (stone, bedrock, air) = ([6, 64, 4], [7, 64, 4], [8, 64, 4]);
            place(game, stone, "minecraft:stone");
            place(game, bedrock, "minecraft:bedrock");
            assert!(!game.player_destroy_block(player, bedrock, false));
            assert_eq!(block(game, bedrock), "minecraft:bedrock");
            assert!(!game.player_destroy_block(player, air, false));
            let epoch = game.native_lifecycle.epoch().unwrap();
            let stranger =
                mod_api::__private::game::player_ref(mod_api::EntityRef::new(epoch, 21), 124);
            assert!(!game.player_destroy_block(stranger, stone, false));
            game.set_mode(GameMode::Adventure);
            assert!(!game.player_destroy_block(player, stone, false));
            assert_eq!(block(game, stone), "minecraft:stone");
            assert_eq!(game.held().unwrap().damage, 0, "nothing wore the pickaxe");
        });
    }

    /// A vein miner as the issue's contract builds one: the player's break of the vein's
    /// origin starts it, the mod breaks every block of the vein as the player (the origin
    /// too, then cancels the outer break), takes each break's drops and experience in
    /// `block_drops`, and sets them down at the origin merged within stack sizes.
    struct VeinMiner {
        vein: Vec<[i32; 3]>,
        busy: std::sync::atomic::AtomicBool,
        kept: std::sync::Mutex<(Vec<mod_api::ItemStack>, u32)>,
    }

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for VeinMiner {
        fn break_block(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::BreakBlockEvent<'_>,
        ) -> mod_api::Flow {
            use std::sync::atomic::Ordering;
            let origin = self.vein[0];
            if event.pos != origin || self.busy.swap(true, Ordering::Relaxed) {
                return mod_api::Flow::Continue;
            }
            let player = host.players().acting_player().expect("the breaker acts");
            for &pos in &self.vein {
                player.destroy_block(host, pos, false);
            }
            self.busy.store(false, Ordering::Relaxed);
            let (stacks, experience) = std::mem::take(&mut *self.kept.lock().unwrap());
            let mut merged: Vec<mod_api::ItemStack> = Vec::new();
            for stack in stacks {
                let max = host.max_stack_size(stack.item()).unwrap_or(64);
                let mut rest = stack.count();
                for into in merged.iter_mut().filter(|m| m.is_stackable_with(&stack)) {
                    let room = max.saturating_sub(into.count()).min(rest);
                    into.grow(room).unwrap();
                    rest -= room;
                }
                if rest > 0 {
                    let mut left = stack.clone();
                    left.set_count(rest).unwrap();
                    merged.push(left);
                }
            }
            let level = player.level(host).expect("the player's level");
            for stack in merged {
                level.pop_resource(host, origin, stack).unwrap();
            }
            let [x, y, z] = origin.map(|v| f64::from(v) + 0.5);
            level.award_experience(host, x, y, z, experience);
            mod_api::Flow::Cancel
        }
        fn block_drops(
            &self,
            _: &dyn mod_api::LevelHost,
            event: &mut mod_api::BlockDropsEvent<'_>,
        ) -> mod_api::Flow {
            let pos = [event.pos.x, event.pos.y, event.pos.z];
            if self.busy.load(std::sync::atomic::Ordering::Relaxed)
                && event.breaker == Some(123)
                && self.vein.contains(&pos)
            {
                let mut kept = self.kept.lock().unwrap();
                event.drops.retain(|drop| {
                    if let Some(stack) = drop.stack().and_then(|stack| stack.project()) {
                        kept.0.push(stack);
                        false
                    } else {
                        true
                    }
                });
                kept.1 += std::mem::take(&mut event.experience);
            }
            mod_api::Flow::Continue
        }
    }

    /// The issue's acceptance (#2953): a vein miner breaks N blocks as the player and gathers
    /// their drops, merged within stack sizes, and their experience at the origin, with no
    /// drop or orb anywhere else. The pickaxe wears once per block, as the player's own breaks.
    #[test]
    #[ignore = "daily"]
    fn a_vein_miner_sets_a_veins_drops_and_experience_down_at_the_origin() {
        // Five diamond ores in a row, then sixty-six stone: more cobblestone than one stack.
        let mut vein: Vec<[i32; 3]> = (0..5).map(|i| [2 + i, 64, 2]).collect();
        vein.extend((0..66).map(|i| [i % 11 + 2, 65 + i / 11, 2]));
        let ores = 5;
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "vein",
            Box::new(VeinMiner {
                vein: vein.clone(),
                busy: false.into(),
                kept: Default::default(),
            }),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                game.world.chunks.insert(
                    (0, 0),
                    std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
                );
                with_player(game, ItemStack::new("minecraft:diamond_pickaxe", 1));
                for (i, &pos) in vein.iter().enumerate() {
                    let block = if i < ores {
                        "minecraft:diamond_ore"
                    } else {
                        "minecraft:stone"
                    };
                    place(game, pos, block);
                }
                let origin = vein[0];
                // The player's own break of the origin: the mod broke it, and the vein.
                assert!(!game.player_break_block(origin).unwrap());
                assert!(vein.iter().all(|&pos| block(game, pos) == "minecraft:air"));
                assert_eq!(game.held().unwrap().damage, vein.len() as u32);
                assert_eq!(game.stats.get("mined", "minecraft:stone"), 66);

                let near = |at: [f64; 3], by: f64| {
                    (0..3).all(|i| (at[i] - (f64::from(origin[i]) + 0.5)).abs() <= by)
                };
                let mut stacks: Vec<(String, u32)> = Vec::new();
                for item in game.dropped.iter() {
                    let at = [item.position.x, item.position.y, item.position.z];
                    assert!(near(at, 0.5), "every drop pops at the origin: {at:?}");
                    stacks.push((item.stack.item.to_string(), item.stack.count));
                }
                stacks.sort();
                assert_eq!(
                    stacks,
                    [
                        ("minecraft:cobblestone".to_owned(), 2),
                        ("minecraft:cobblestone".to_owned(), 64),
                        ("minecraft:diamond".to_owned(), ores as u32),
                    ]
                );
                // Each diamond ore gives 3 to 7 points, all of them awarded at the origin.
                let points: u32 = game.orbs.iter().map(|o| o.points()).sum();
                assert!(
                    (3 * ores as u32..=7 * ores as u32).contains(&points),
                    "{points}"
                );
                assert!(game.orbs.iter().all(|o| {
                    let p = o.position;
                    near([p.x, p.y, p.z], 0.01)
                }));
            });
        });
    }

    /// A listener that breaks the block it hears about as the player, with no guard: the block
    /// is still there, so each break hears the next. The engine refuses the break past its limit
    /// instead of overflowing the stack, and the listener ran once per allowed level.
    struct Unguarded;

    /// How many times `Unguarded` heard a break.
    static UNGUARDED_HEARD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Unguarded {
        fn break_block(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::BreakBlockEvent<'_>,
        ) -> mod_api::Flow {
            UNGUARDED_HEARD.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let player = host.players().acting_player().expect("the breaker acts");
            player.destroy_block(host, event.pos, false);
            mod_api::Flow::Cancel
        }
    }

    #[test]
    #[ignore = "daily"]
    fn an_unguarded_listener_is_cut_off_at_the_nesting_limit() {
        let mods: mod_api::Mods =
            vec![mod_api::ModEntry::new("unguarded", Box::new(Unguarded))].into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                game.world.chunks.insert(
                    (0, 0),
                    std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
                );
                let player = with_player(game, ItemStack::new("minecraft:diamond_pickaxe", 1));
                let pos = [6, 64, 4];
                place(game, pos, "minecraft:stone");
                assert!(!game.player_destroy_block(player, pos, false));
                assert_eq!(
                    UNGUARDED_HEARD.load(std::sync::atomic::Ordering::Relaxed),
                    8
                );
                assert_eq!(block(game, pos), "minecraft:stone");
            });
        });
    }
}
use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
