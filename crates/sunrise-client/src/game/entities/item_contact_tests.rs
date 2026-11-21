use super::*;
use mod_api::{
    self, BlockPart, Flow, Hooks, HostSide, ItemAfterVerticalCollisionEvent, Registrar,
    RegistrationContext,
};
use std::sync::Mutex;
use sunrise_client::{chunk::Chunk, join_seq::Joined as _};

type Contact = (bool, u32, [i32; 3], [f64; 3], u32);

#[derive(Clone, Default)]
struct Probe {
    contacts: Arc<Mutex<Vec<Contact>>>,
}

#[mod_api::sunrise_part]
impl BlockPart for Probe {
    fn item_after_vertical_collision(
        &self,
        _: HostSide,
        event: &mut ItemAfterVerticalCollisionEvent<'_>,
    ) -> Flow {
        let stack = event.stack.as_ref().expect("active item stack");
        self.contacts.lock().unwrap().push((
            event.client,
            event.entity,
            event.block_pos,
            event.item_pos,
            stack.count(),
        ));
        if stack.count() == 1 {
            return Flow::Cancel;
        }
        *event.stack = event.stack.take().unwrap().shrink(2);
        Flow::Continue
    }
}

#[mod_api::__subscriptions]
impl Hooks for Probe {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.block_key("minecraft:stone");
        r.block_part(key, self.clone());
    }
}

#[test]
#[ignore = "daily"]
fn the_active_stack_survives_a_partial_contact_and_consumption_removes_the_entity() {
    crate::game::in_own_process(
        "game::item_contact_tests::the_active_stack_survives_a_partial_contact_and_consumption_removes_the_entity",
        the_active_stack_survives_a_partial_contact_and_consumption_removes_the_entity_isolated,
    );
}

fn the_active_stack_survives_a_partial_contact_and_consumption_removes_the_entity_isolated() {
    use crate::game::native_entity_refs::tests::with_game;
    let prepare = |game: &mut Game<'_>| {
        game.options.dedicated = Some(Vec::new());
        game.native_lifecycle
            .begin_open(mod_api::Authority::IntegratedServer);
        game.world = World::new(game.res.facts().air, 1);
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        game.world.set(4, 63, 4, stone).unwrap();
    };

    // The centre of the item is outside the block, while its box overlaps it.
    let landing = [3.99, 64.0, 4.5];
    let mut input = ItemStack::new("minecraft:iron_ingot", 13);
    input
        .data_mut()
        .set_other("fixture:opaque", Some(sunrise_core::nbt::Tag::Long(99)));
    let spawn = |game: &mut Game<'_>, count| {
        game.dropped.clear();
        let mut stack = input.clone();
        stack.count = count;
        game.spawn_item(stack, [landing[0], 64.1, landing[2]], [0.0, -0.3, 0.0], 20);
        game.ticked_entities_through = game.next_entity;
        game.dropped[0].clone()
    };
    let mut vanilla = None;
    with_game(|game| {
        prepare(game);
        let before = spawn(game, 13);
        game.tick_items();
        let item = game.dropped[0].clone();
        assert_eq!(item.id, before.id);
        assert_eq!(item.position, Vec3::new(landing[0], landing[1], landing[2]));
        assert_eq!(item.velocity, Vec3::ZERO);
        assert_eq!(item.stack, input);
        assert_eq!(item.age, 1);

        vanilla = Some(item);
    });
    let vanilla = vanilla.unwrap();

    let probe = Probe::default();
    let mods: mod_api::Mods =
        vec![mod_api::ModEntry::new("contact", Box::new(probe.clone()))].into();
    mod_platform::testing::with_mods(mods.clone(), || {
        crate::game::api_values::install_parts(&mods).unwrap();
        with_game(|game| {
            prepare(game);
            let old = spawn(game, 13);
            game.tick_items();
            let item = &game.dropped[0];
            assert_eq!(item.id, old.id);
            assert_eq!(item.join_seq(), old.join_seq());
            assert_eq!(item.position, vanilla.position);
            assert_eq!(item.velocity, vanilla.velocity);
            let mut expected = input.clone();
            expected.count = 11;
            assert_eq!(item.stack, expected);
            assert_eq!(
                probe.contacts.lock().unwrap().as_slice(),
                &[(false, old.id, [4, 63, 4], landing, 13)]
            );

            for count in [1, 2] {
                spawn(game, count);
                game.tick_items();
                assert!(game.dropped.is_empty(), "consumed stack of {count}");
            }
        });
    });
}

type BounceContact = (bool, i32, String, [i32; 3]);

#[derive(Clone, Default)]
struct BounceProbe {
    calls: Arc<Mutex<Vec<BounceContact>>>,
}

#[mod_api::sunrise_part]
impl BlockPart for BounceProbe {
    fn bounce_restitution(
        &self,
        level: mod_api::LevelSide<'_>,
        event: &mut mod_api::BounceRestitutionEvent<'_>,
    ) {
        let (client, host) = match level {
            mod_api::LevelSide::Client(host) => (true, host),
            mod_api::LevelSide::Server(host) => (false, host),
        };
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        assert_eq!(host.block_state(pos), Some(event.state));
        self.calls
            .lock()
            .unwrap()
            .push((client, event.entity, event.kind.into(), pos));
        event.factor = 0.67_f32;
    }
}

#[mod_api::__subscriptions]
impl Hooks for BounceProbe {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for block in [
            "minecraft:glass",
            "minecraft:white_carpet",
            "minecraft:honey_block",
        ] {
            let key = r.block_key(block);
            r.block_part(key, self.clone());
        }
    }
}

#[test]
#[ignore = "daily"]
fn item_restitution_uses_the_legacy_contact_on_both_simulating_sides() {
    crate::game::in_own_process(
        "game::item_contact_tests::item_restitution_uses_the_legacy_contact_on_both_simulating_sides",
        || {
            use crate::game::native_entity_refs::tests::{with_client_game, with_game};
            let part = BounceProbe::default();
            let mods: mod_api::Mods =
                vec![mod_api::ModEntry::new("bounce", Box::new(part.clone()))].into();
            mod_platform::testing::with_mods(mods.clone(), || {
                crate::game::api_values::install_parts(&mods).unwrap();
                crate::game::native_hooks::install_block_contact_hooks();
                let run = |game: &mut Game<'_>, client| {
                    let registry = game.pack.assets.registry;
                    let block = |name| registry.default_state(name).unwrap();
                    let glass = block("minecraft:glass");
                    let carpet = block("minecraft:white_carpet");
                    let stone = block("minecraft:stone");
                    let slime = block("minecraft:slime_block");
                    let honey = block("minecraft:honey_block");
                    let cases = [
                        // The item's centre lies outside the block; its moved footprint does not.
                        (
                            glass,
                            None,
                            None,
                            3.99,
                            64.25,
                            -0.96,
                            64.0,
                            Some(0.67_f32),
                            true,
                        ),
                        // The carpet supports the box, but getOnPosLegacy addresses the glass below it.
                        (
                            glass,
                            Some(carpet),
                            None,
                            4.5,
                            64.3125,
                            -0.96,
                            64.0625,
                            Some(0.67),
                            true,
                        ),
                        (
                            stone,
                            Some(carpet),
                            None,
                            4.5,
                            64.3125,
                            -0.96,
                            64.0625,
                            None,
                            false,
                        ),
                        (slime, None, None, 4.5, 64.25, -0.96, 64.0, Some(1.0), false),
                        (honey, None, None, 4.5, 64.25, -0.96, 63.9375, None, false),
                        (glass, None, None, 4.5, 64.03125, 0.0, 64.0, None, false),
                        // A ceiling collision addresses the carpet cell but never queries its factor.
                        (
                            stone,
                            Some(carpet),
                            Some(glass),
                            4.5,
                            64.5,
                            1.0,
                            64.75,
                            None,
                            false,
                        ),
                    ];
                    for (floor, above, ceiling, x, y, vy, landed_y, factor, calls) in cases {
                        game.world = World::new(game.res.facts().air, 1);
                        game.world
                            .chunks
                            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                        game.world.set(4, 63, 4, floor).unwrap();
                        if let Some(above) = above {
                            game.world.set(4, 64, 4, above).unwrap();
                        }
                        if let Some(ceiling) = ceiling {
                            game.world.set(4, 65, 4, ceiling).unwrap();
                        }
                        game.dropped.clear();
                        part.calls.lock().unwrap().clear();
                        game.spawn_item(
                            ItemStack::new("minecraft:iron_ingot", 1),
                            [x, y, 4.5],
                            [0.0, vy, 0.0],
                            20,
                        );
                        let mut item = game.dropped[0].clone();
                        assert_eq!(
                            game.tick_item_with_block_contacts(
                                &mut item,
                                None,
                                &[],
                                client,
                                &mut |_| {}
                            ),
                            (true, false)
                        );
                        assert_eq!(item.position.y, landed_y);
                        let expected = factor.map_or(0.0, |f| {
                            let speed = vy - 0.04;
                            let portion = (landed_y - y) / speed;
                            let drag = f64::from(0.98_f32);
                            (portion * 0.04 - speed)
                                * (1.0 + portion * (drag - 1.0))
                                * f64::from(f * 0.8_f32)
                                * drag
                        });
                        assert_eq!(item.velocity.y, expected, "floor {floor:?}, start {y}");
                        let got = part.calls.lock().unwrap();
                        if calls {
                            assert_eq!(
                                *got,
                                [(
                                    client,
                                    item.id as i32,
                                    "minecraft:item".to_owned(),
                                    [4, 63, 4]
                                )]
                            );
                        } else {
                            assert!(got.is_empty(), "unexpected query: {got:?}");
                        }
                    }
                };
                with_game(|game| run(game, false));
                with_client_game(|game| run(game, true));
            });
        },
    );
}

#[test]
#[ignore = "daily"]
fn item_support_survives_idle_then_clears_on_an_airborne_move() {
    crate::game::in_own_process(
        "game::item_contact_tests::item_support_survives_idle_then_clears_on_an_airborne_move",
        || {
            crate::game::native_entity_refs::tests::with_game(|game| {
                let air = game.res.facts().air;
                let stone = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap();
                let support = BlockPos::new(4, 63, 4);
                game.world = World::new(air, 1);
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                game.world
                    .set(support.x, support.y, support.z, stone)
                    .unwrap();
                game.spawn_item(
                    ItemStack::new("minecraft:iron_ingot", 1),
                    [4.5, 64.25, 4.5],
                    [1.0, -0.96, 0.0],
                    20,
                );
                let mut item = game.dropped[0].clone();
                let tick = |game: &mut Game<'_>, item: &mut ItemEntity| {
                    game.tick_item_with_block_contacts(item, None, &[], false, &mut |_| {})
                };
                assert_eq!(tick(game, &mut item), (true, false));
                assert_eq!(item.position, Vec3::new(5.5, 64.0, 4.5));
                // Only the old horizontal footprint had a supporting block.
                assert!(item.ground.on_ground);
                assert_eq!(item.ground.supporting_block, Some(support));

                game.world
                    .set(support.x, support.y, support.z, air)
                    .unwrap();
                item.velocity = Vec3::ZERO;
                // The next tick must not be one of the four-tick movement updates.
                item.ticks = item.id.wrapping_neg() & 3;
                assert_eq!(tick(game, &mut item), (true, false));
                assert_eq!(item.position, Vec3::new(5.5, 64.0, 4.5));
                assert_eq!(item.ground.supporting_block, Some(support));

                item.velocity = Vec3::new(0.1, 0.2, 0.0);
                assert_eq!(tick(game, &mut item), (true, false));
                assert!(item.position.y > 64.0);
                assert!(!item.ground.on_ground);
                assert_eq!(item.ground.supporting_block, None);
            });
        },
    );
}
