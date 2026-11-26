

use sunrise_core::geometry::Vec3;
#[cfg(test)]
pub(crate) mod destroyed_contents_tests {
    use super::*;
    use sunrise_gameplay::game::game_tests::tests::server_game;
    use sunrise_core::random::RandomSource;
    use sunrise_items::items::ItemStack;

    /// The motion of each spilled stack, in the order the spill made them.
    fn spilled_motions(game: &sunrise_gameplay::game::ServerGame<'_>) -> Vec<[f64; 3]> {
        game.dropped
            .iter()
            .map(|item| [item.velocity.x, item.velocity.y, item.velocity.z])
            .collect()
    }

    /// `ItemUtils.onContainerDestroyed` makes each stack with
    /// `new ItemEntity(level, x, y, z, stack)`, whose motion is 0.2 up and
    /// ±0.1 sideways from the *item's* random (ItemEntity.java:61-66), and
    /// draws nothing from the level's. Every destroyed container funnels here:
    /// lava, fire and cactus in the item's environment tick, an explosion, and
    /// the damage command.
    #[test]
    #[ignore = "daily"]
    fn spilled_stacks_rise_from_their_own_randoms() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        let at = Vec3::new(8.5, 70.0, 8.5);
        let stacks = ["minecraft:stone", "minecraft:dirt", "minecraft:glass"]
            .into_iter()
            .map(|id| (ItemStack::new(id, 1), at))
            .collect();
        let mut level = game.world.random.clone();
        game.spawn_destroyed_contents(stacks);
        let motions = spilled_motions(&game);
        assert_eq!(motions.len(), 3);
        for motion in &motions {
            assert_eq!(motion[1], 0.2, "the four-argument constructor rises 0.2");
            assert!((-0.1..0.1).contains(&motion[0]), "{motion:?}");
            assert!((-0.1..0.1).contains(&motion[2]), "{motion:?}");
        }
        // Three stacks, three different sideways motions: each drew its own.
        assert_ne!(motions[0], motions[1]);
        assert_ne!(motions[1], motions[2]);
        assert_ne!(motions[0], motions[2]);
        // The spill leaves the level's random alone.
        assert_eq!(game.world.random.next_double(), level.next_double());
        assert_eq!(game.world.random.next_double(), level.next_double());
    }

    /// The damage command is one of those callers: killing a shulker box item
    /// spills its contents with the same upward motion.
    #[test]
    #[ignore = "daily"]
    fn a_damaged_shulker_box_spills_its_stacks_upward() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        game.build_registries();
        game.set_allow_commands(true);
        game.world.chunks.insert(
            (0, 0),
            std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
        );
        let feet = Vec3::new(8.5, 70.0, 8.5);
        game.player = Some(sunrise_sim::motion::PlayerMotion::new(game.motion, feet).unwrap());
        game.update_chunk_tickets();
        let boxed = sunrise_items::item_data::parse_stack(
            r#"{id:"minecraft:shulker_box",count:1,components:{"minecraft:container":[
                {slot:0,item:{id:"minecraft:stone",count:2}},
                {slot:4,item:{id:"minecraft:dirt",count:1}},
                {slot:8,item:{id:"minecraft:glass",count:3}}]}}"#,
        )
        .unwrap()
        .unwrap();
        game.spawn_item(boxed, [8.5, 70.0, 8.5], [0.0; 3], 0);
        assert_eq!(game.dropped.len(), 1);
        assert!(game.run_command("damage @e[type=minecraft:item,limit=1] 100 minecraft:generic"));
        let mut spilled: Vec<_> = game
            .dropped
            .iter()
            .map(|item| (item.stack.item.to_string(), item.stack.count))
            .collect();
        spilled.sort();
        assert_eq!(
            spilled,
            [
                ("minecraft:dirt".to_owned(), 1),
                ("minecraft:glass".to_owned(), 3),
                ("minecraft:stone".to_owned(), 2)
            ]
        );
        for item in &game.dropped {
            assert_eq!(item.velocity.y, 0.2, "{item:?}");
        }
    }
}

#[cfg(test)]
pub(crate) mod escape_tests {
    use sunrise_gameplay::game::game_tests::tests::server_game;
    use sunrise_core::random::RandomSource;
    use sunrise_items::items::ItemStack;

    /// `Entity.moveTowardsClosestSpace` uses the item's own random, never the
    /// level's random. Seeded Java values pin vanilla's float arithmetic (#13043).
    #[test]
    #[ignore = "daily"]
    fn an_item_escapes_a_block_at_its_own_randoms_speed() {
        let mut game_resources = None;
        let mut game = server_game(&mut game_resources);
        let feet = sunrise_core::geometry::Vec3::new(8.5, 70.0, 8.5);
        game.player = Some(sunrise_sim::motion::PlayerMotion::new(game.motion, feet).unwrap());
        super::native_entity_refs::tests::prepare_entity_ticking_neighborhood(
            &mut game,
            sunrise_core::position::ChunkPos::new(0, 0),
        );
        game.update_chunk_tickets();
        assert!(game.entity_ticking_area()(8, 8));
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        game.world.set(8, 64, 8, stone).unwrap();
        let stack = ItemStack::new("minecraft:stone", 2);
        game.spawn_item(stack, [8.5, 64.4, 8.5], [0.0, 0.0, 0.0], 10);
        let item = game.dropped.last_mut().unwrap();
        item.random = sunrise_core::random::LegacyRandom::new(12345);
        let mut level = game.world.random.clone();
        game.side.ticked_entities_through = u32::MAX;
        game.tick_items();
        let item = game.dropped.last().unwrap();
        // Up is the nearest open side. Independent Java Random(12345):
        // nextFloat()*0.2F+0.1F, widened before multiplying by widened 0.98F.
        assert_eq!(item.velocity.y, f64::from_bits(0x3fc5_9ef4_570f_5500));
        assert_eq!(item.random.clone().next_float().to_bits(), 0x3f03_61b3);
        assert_eq!(game.world.random.next_float(), level.next_float());
    }
}

#[cfg(test)]
#[path = "tests/thrown_items_tests.rs"]
pub(crate) mod thrown_items_tests;
use sunrise_gameplay_common::game::chat_common::ChatCommonGameExt1 as _;
