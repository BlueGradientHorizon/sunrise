//! `fluid_util::dispense_container` from a real dispenser: a container's `dispense_item` part
//! that returns it fills and empties a bucket-like container (its empty and full forms
//! different items), picks up before it places, sends the remainder into the dispenser or out
//! of its front, and dispenses an unchanged container where nothing moves.
use crate::game::{Game, dispense_fixture};
use mod_api::*;
use std::sync::Arc;
use sunrise_client::{chunk::Chunk, items::ItemStack};
use sunrise_core::geometry::Direction;
use sunrise_world::registry::BlockStateId;

/// The dispenser, facing east, and the block in front of it.
const DISPENSER: [i32; 3] = [5, 64, 5];
const FRONT: [i32; 3] = [6, 64, 5];

/// Answers the dispense of the items it is attached to with `dispense_container`.
struct Container;
#[mod_api::sunrise_part]
impl ItemPart for Container {
    fn dispense_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut DispenseItemEvent<'_>,
    ) -> Option<Dispensed> {
        fluid_util::dispense_container(host, event)
    }
}

/// Vanilla's buckets, through the game's own `ITEM_FLUIDS`, and paper made a 2000 mB tank in
/// its `custom_data`: a container that can both take more and give.
struct Containers;
#[mod_api::__subscriptions]
impl Hooks for Containers {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for item in [
            "minecraft:bucket",
            "minecraft:water_bucket",
            "minecraft:paper",
            "minecraft:glass_bottle",
            "minecraft:clay_ball",
        ] {
            let key = r.item_key(item);
            r.item_part(key, Container);
        }
        r.item_capability(
            &ITEM_FLUIDS,
            "minecraft:paper",
            |_: &mut dyn ServerHost, place: &ItemPlace| {
                let data = ComponentId::new("minecraft:custom_data").unwrap();
                Some(
                    Box::new(ItemComponentFluidTank::new(place.clone(), data, 2000))
                        as Box<dyn ResourceHandler<FluidResource>>,
                )
            },
        );
        r.item_capability(
            &ITEM_FLUIDS,
            "minecraft:glass_bottle",
            |_: &mut dyn ServerHost, place: &ItemPlace| {
                Some(Box::new(Jug(place.clone())) as Box<dyn ResourceHandler<FluidResource>>)
            },
        );
        r.item_capability(
            &ITEM_FLUIDS,
            "minecraft:clay_ball",
            |_: &mut dyn ServerHost, place: &ItemPlace| {
                Some(Box::new(MultiStepJug(place.clone()))
                    as Box<dyn ResourceHandler<FluidResource>>)
            },
        );
    }
}

/// An empty glass bottle a bucket of water turns into a potion: a mod's handler that acts on
/// every container its place shows, as one written for a single container may.
struct Jug(ItemPlace);
impl ResourceHandler<FluidResource> for Jug {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        1
    }
    fn resource(&self, _: &mut dyn ServerHost, _: usize) -> Option<FluidResource> {
        None
    }
    fn amount(&self, _: &mut dyn ServerHost, _: usize) -> u32 {
        0
    }
    fn capacity(&self, _: &mut dyn ServerHost, _: usize, _: Option<&FluidResource>) -> u32 {
        1000
    }
    fn insert_at(
        &self,
        host: &mut dyn ServerHost,
        tx: &TransactionContext,
        _: usize,
        _: &FluidResource,
        amount: u32,
    ) -> u32 {
        if amount < 1000 {
            return 0;
        }
        let potion = mod_api::__private::item_key("minecraft:potion").unwrap();
        let potion = ItemResource::of(&mod_api::ItemStack::new(potion, 1).unwrap());
        let all = self.0.amount(host);
        match self.0.exchange(host, tx, &potion, all) == all {
            true => 1000,
            false => 0,
        }
    }
    fn extract_at(
        &self,
        _: &mut dyn ServerHost,
        _: &TransactionContext,
        _: usize,
        _: &FluidResource,
        _: u32,
    ) -> u32 {
        0
    }
}

/// A custom container handler that extracts twice and attempts a multi-item insert through the
/// `OneByOne` wrapper, exercising: (1) `OneByOne` capping inserts at 1, (2) `dispense_container`
/// returning the actual final first slot when overflow is present (`count - 2`, not `count - 1`),
/// and (3) `(None, Some(changed))` when the second extract empties slot 0 after slot 1 holds the
/// inserted item.
struct MultiStepJug(ItemPlace);
impl ResourceHandler<FluidResource> for MultiStepJug {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        1
    }
    fn resource(&self, _: &mut dyn ServerHost, _: usize) -> Option<FluidResource> {
        None
    }
    fn amount(&self, _: &mut dyn ServerHost, _: usize) -> u32 {
        0
    }
    fn capacity(&self, _: &mut dyn ServerHost, _: usize, _: Option<&FluidResource>) -> u32 {
        1000
    }
    fn insert_at(
        &self,
        host: &mut dyn ServerHost,
        tx: &TransactionContext,
        _: usize,
        _: &FluidResource,
        amount: u32,
    ) -> u32 {
        if amount < 1000 {
            return 0;
        }
        let Some(current) = self.0.resource(host) else {
            return 0;
        };
        let potion = mod_api::__private::item_key("minecraft:potion").unwrap();
        let potion = ItemResource::of(&mod_api::ItemStack::new(potion, 1).unwrap());
        if self.0.extract(host, tx, &current, 1) != 1 {
            return 0;
        }
        // Through `OneByOne`, asking to insert 3 caps at 1 (into overflow while slot 0 still holds clay).
        if self.0.insert(host, tx, &potion, 3) != 1 {
            return 0;
        }
        // Extract a second unit from slot 0 (emptying it when initial count was 2, leaving 2 when 4).
        if self.0.extract(host, tx, &current, 1) != 1 {
            return 0;
        }
        1000
    }
    fn extract_at(
        &self,
        _: &mut dyn ServerHost,
        _: &TransactionContext,
        _: usize,
        _: &FluidResource,
        _: u32,
    ) -> u32 {
        0
    }
}

/// A dedicated server with a chunk at the origin, a stone floor, an empty dispenser facing east
/// and the containers' parts installed.
fn with_dispenser(test: impl FnOnce(&mut Game<'_>, LevelRef)) {
    let mods: Mods = vec![ModEntry::new("containers", Box::new(Containers))].into();
    crate::game::api_values::install_parts(&mods).unwrap();
    mod_platform::testing::with_mods(mods, || {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            game.native_lifecycle
                .begin_open(mod_api::Authority::IntegratedServer);
            let stone = state(game, "minecraft:stone");
            for x in 0..16 {
                for z in 0..16 {
                    game.world.set(x, 63, z, stone).unwrap();
                }
            }
            let level = game.level("minecraft:overworld").unwrap();
            let base = state(game, "minecraft:dispenser");
            let dispenser = game.with_props(base, &[("facing", "east")]).unwrap();
            assert_eq!(
                game.level_set_block(level, DISPENSER, dispenser, BlockUpdateFlags::NONE),
                Ok(true)
            );
            test(game, level);
        });
    });
}

fn state(game: &Game<'_>, name: &str) -> BlockStateId {
    game.pack.assets.registry.default_state(name).unwrap()
}

fn front(game: &Game<'_>) -> BlockStateId {
    game.world.block(FRONT[0], FRONT[1], FRONT[2])
}

fn set_front(game: &mut Game<'_>, name: &str) {
    let block = state(game, name);
    game.world.set(FRONT[0], FRONT[1], FRONT[2], block).unwrap();
}

/// The dispenser dispenses `stack` from its slot: what the slot holds then.
fn dispense(game: &mut Game<'_>, stack: ItemStack) -> Option<ItemStack> {
    let at = (DISPENSER[0], DISPENSER[1], DISPENSER[2]);
    game.dispense_item(at, Direction::East, stack).unwrap()
}

fn item(stack: &Option<ItemStack>) -> Option<(String, u32)> {
    stack
        .as_ref()
        .map(|stack| (stack.item.to_string(), stack.count))
}

#[test]
#[ignore = "daily"]
fn a_bucket_like_container_fills_from_a_source_and_empties_into_the_air() {
    with_dispenser(|game, _| {
        set_front(game, "minecraft:water");
        let filled = dispense(game, ItemStack::new("minecraft:bucket", 1));
        assert_eq!(item(&filled), Some(("minecraft:water_bucket".into(), 1)));
        assert_eq!(front(game), state(game, "minecraft:air"));
        let emptied = dispense(game, filled.unwrap());
        assert_eq!(item(&emptied), Some(("minecraft:bucket".into(), 1)));
        assert_eq!(front(game), state(game, "minecraft:water"));
        assert!(game.dropped.is_empty());
    });
}

#[test]
#[ignore = "daily"]
fn a_stack_of_three_in_a_full_dispenser_drops_the_new_container_out_of_the_front() {
    with_dispenser(|game, _| {
        let pos = (DISPENSER[0], DISPENSER[1], DISPENSER[2]);
        for cell in game.world.block_entities.get_mut(&pos).unwrap().slots_mut() {
            *cell = Some(ItemStack::new("minecraft:stone", 64));
        }
        set_front(game, "minecraft:water");
        dispense_fixture::start_recording();
        let left = dispense(game, ItemStack::new("minecraft:bucket", 3));
        let lines = dispense_fixture::stop_recording();
        assert_eq!(item(&left), Some(("minecraft:bucket".into(), 2)));
        assert_eq!(front(game), state(game, "minecraft:air"));
        assert_eq!(game.dropped.len(), 1);
        assert_eq!(&*game.dropped[0].stack.item, "minecraft:water_bucket");
        // The fluid's event, then the spill's click and smoke, then the dispense's own.
        let clicks = lines
            .iter()
            .filter(|line| line.starts_with("L 1000 "))
            .count();
        assert_eq!(clicks, 2, "{lines:?}");
        assert!(lines[0].ends_with("minecraft:fluid_pickup"), "{lines:?}");
    });
}

#[test]
#[ignore = "daily"]
fn a_failed_transfer_dispenses_one_unchanged_container_and_changes_nothing() {
    with_dispenser(|game, _| {
        set_front(game, "minecraft:stone");
        dispense_fixture::start_recording();
        let left = dispense(game, ItemStack::new("minecraft:bucket", 2));
        let lines = dispense_fixture::stop_recording();
        assert_eq!(item(&left), Some(("minecraft:bucket".into(), 1)));
        assert_eq!(game.dropped.len(), 1);
        assert_eq!(&*game.dropped[0].stack.item, "minecraft:bucket");
        assert_eq!(front(game), state(game, "minecraft:stone"));
        assert!(
            !lines.iter().any(|line| line.starts_with("G ")),
            "{lines:?}"
        );
    });
}

#[test]
#[ignore = "daily"]
fn a_container_that_could_give_picks_up_first_and_its_event_comes_before_the_click() {
    with_dispenser(|game, _| {
        crate::game::native_item_places::tests::open_to_lan(game);
        set_front(game, "minecraft:water");
        let half = dispense(game, ItemStack::new("minecraft:paper", 1));
        assert_eq!(front(game), state(game, "minecraft:air"));
        // Holding 1000 of 2000, it could empty into the air or take more: with a source in
        // front it takes more.
        set_front(game, "minecraft:water");
        game.creatures.lan_sounds.clear();
        dispense_fixture::start_recording();
        let full = dispense(game, half.unwrap());
        let lines = dispense_fixture::stop_recording();
        assert_eq!(front(game), state(game, "minecraft:air"));
        assert_eq!(item(&full), Some(("minecraft:paper".into(), 1)));
        let pickup = lines
            .iter()
            .position(|line| line.ends_with("minecraft:fluid_pickup"));
        let click = lines.iter().position(|line| line.starts_with("L 1000 "));
        assert!(pickup.is_some() && pickup < click, "{lines:?}");
        let sounds: Vec<_> = game
            .creatures
            .lan_sounds
            .iter()
            .map(|s| s.sound.clone())
            .collect();
        assert_eq!(sounds, ["minecraft:item.bucket.fill"]);
        // Full, it can only give: it places what it holds.
        let given = dispense(game, full.unwrap());
        assert_eq!(front(game), state(game, "minecraft:water"));
        assert_eq!(item(&given), Some(("minecraft:paper".into(), 1)));

        // Restore-before-fallback: holding 1500 of 2000 mB facing a 1000 mB water source,
        // `try_pickup_world_fluid` mutates the lent scratch slot to 2000 mB (500 mB accepted)
        // and then aborts because the full bucket did not fit. The abort must restore the lent
        // scratch slot to 1500 mB before `try_place_fluid` runs and extracts 1000 mB, leaving
        // 500 mB (not 1000 mB).
        let mut almost_full = ItemStack::new("minecraft:paper", 1);
        almost_full.data_mut().set_other(
            "minecraft:custom_data",
            Some(Tag::compound([
                ("id", Tag::String("minecraft:water".into())),
                ("amount", Tag::Int(1500)),
            ])),
        );
        game.creatures.lan_sounds.clear();
        dispense_fixture::start_recording();
        let placed = dispense(game, almost_full);
        let lines = dispense_fixture::stop_recording();
        assert_eq!(front(game), state(game, "minecraft:water"));
        assert_eq!(item(&placed), Some(("minecraft:paper".into(), 1)));
        let data = placed
            .as_ref()
            .unwrap()
            .custom_data()
            .expect("restored tank retains 500 mB after fallback placement");
        assert_eq!(
            data.get(&JavaString::from("id")),
            Some(&Tag::String("minecraft:water".into()))
        );
        assert_eq!(data.get(&JavaString::from("amount")), Some(&Tag::Int(500)));
        let place_event = lines
            .iter()
            .position(|line| line.ends_with("minecraft:fluid_place"));
        let click = lines.iter().position(|line| line.starts_with("L 1000 "));
        assert!(place_event.is_some() && place_event < click, "{lines:?}");
        assert!(
            !lines
                .iter()
                .any(|line| line.ends_with("minecraft:fluid_pickup")),
            "{lines:?}"
        );
        let sounds: Vec<_> = game
            .creatures
            .lan_sounds
            .iter()
            .map(|s| s.sound.clone())
            .collect();
        assert_eq!(sounds, ["minecraft:item.bucket.empty"]);
        assert!(game.dropped.is_empty());
        assert_eq!(game.live_scratch_count(), 0);
    });
}

#[test]
#[ignore = "daily"]
fn a_stack_of_custom_containers_gives_their_handler_one_at_a_time() {
    with_dispenser(|game, _| {
        set_front(game, "minecraft:water");
        let left = dispense(game, ItemStack::new("minecraft:glass_bottle", 3));
        // The handler turned every container it saw: one, not the stack.
        assert_eq!(item(&left), Some(("minecraft:glass_bottle".into(), 2)));
        assert_eq!(front(game), state(game, "minecraft:air"));
        let pos = (DISPENSER[0], DISPENSER[1], DISPENSER[2]);
        let held: Vec<_> = game
            .world
            .block_entities
            .get(&pos)
            .unwrap()
            .slots()
            .iter()
            .flatten()
            .map(|stack| (stack.item.to_string(), stack.count))
            .collect();
        assert_eq!(held, [("minecraft:potion".to_owned(), 1)]);
        assert!(game.dropped.is_empty());
    });
}

#[test]
#[ignore = "daily"]
fn dispense_container_preserves_first_slot_caps_one_by_one_insert_and_rolls_back_in_enclosing_tx() {
    with_dispenser(|game, _| {
        let pos = (DISPENSER[0], DISPENSER[1], DISPENSER[2]);

        // 1. Actual final first slot (`4 -> 2`, not `stack - 1 = 3`) plus `OneByOne`-capped
        //    insert of 1 potion routed into the dispenser.
        set_front(game, "minecraft:water");
        let left = dispense(game, ItemStack::new("minecraft:clay_ball", 4));
        assert_eq!(item(&left), Some(("minecraft:clay_ball".into(), 2)));
        assert_eq!(front(game), state(game, "minecraft:air"));
        let held: Vec<_> = game
            .world
            .block_entities
            .get(&pos)
            .unwrap()
            .slots()
            .iter()
            .flatten()
            .map(|stack| (stack.item.to_string(), stack.count))
            .collect();
        assert_eq!(held, [("minecraft:potion".to_owned(), 1)]);
        assert!(game.dropped.is_empty());

        // 2. `(None, Some(changed))`: slot 0 emptied after slot 1 took the changed item returns
        //    the changed item directly into the dispenser's active slot without adding a second copy.
        set_front(game, "minecraft:water");
        let replaced = dispense(game, ItemStack::new("minecraft:clay_ball", 2));
        assert_eq!(item(&replaced), Some(("minecraft:potion".into(), 1)));
        assert_eq!(front(game), state(game, "minecraft:air"));
        let held: Vec<_> = game
            .world
            .block_entities
            .get(&pos)
            .unwrap()
            .slots()
            .iter()
            .flatten()
            .map(|stack| (stack.item.to_string(), stack.count))
            .collect();
        assert_eq!(held, [("minecraft:potion".to_owned(), 1)]);

        // 3. Aborting an enclosing transaction around `fluid_util::dispense_container` rolls back
        //    both the world fluid pickup and the dispenser slot write (empty-slot remainder path).
        for cell in game.world.block_entities.get_mut(&pos).unwrap().slots_mut() {
            *cell = None;
        }
        set_front(game, "minecraft:water");
        dispense_fixture::start_recording();
        let aborted_slot = game.storage_transaction(|game, _| {
            let mut answer = None;
            let dim = game.dim.id();
            let stack = ItemStack::new("minecraft:bucket", 3);
            game.native_dispense_item(dim, DISPENSER, Direction::East, &stack, &mut answer);
            assert!(answer.is_some());
            // Tentatively visible inside the enclosing transaction.
            assert_eq!(front(game), state(game, "minecraft:air"));
            let tentative: Vec<_> = game
                .world
                .block_entities
                .get(&pos)
                .unwrap()
                .slots()
                .iter()
                .flatten()
                .map(|s| (s.item.to_string(), s.count))
                .collect();
            assert_eq!(tentative, [("minecraft:water_bucket".to_owned(), 1)]);
            Err::<(), _>(())
        });
        let lines = dispense_fixture::stop_recording();
        assert_eq!(aborted_slot, Err(()));
        assert_eq!(front(game), state(game, "minecraft:water"));
        assert!(
            game.world
                .block_entities
                .get(&pos)
                .unwrap()
                .slots()
                .iter()
                .all(Option::is_none)
        );
        assert!(game.dropped.is_empty());
        assert!(
            lines.is_empty(),
            "aborted tx must publish no events: {lines:?}"
        );

        // 4. Aborting an enclosing transaction on a full dispenser (overflow drop path) defers
        //    and discards both the dropped remainder and its click/smoke/pickup events; committing
        //    publishes them.
        for cell in game.world.block_entities.get_mut(&pos).unwrap().slots_mut() {
            *cell = Some(ItemStack::new("minecraft:stone", 64));
        }
        dispense_fixture::start_recording();
        let aborted_drop = game.storage_transaction(|game, _| {
            let mut answer = None;
            let dim = game.dim.id();
            let stack = ItemStack::new("minecraft:bucket", 3);
            game.native_dispense_item(dim, DISPENSER, Direction::East, &stack, &mut answer);
            assert!(answer.is_some());
            assert!(game.dropped.is_empty(), "drop must wait for root commit");
            Err::<(), _>(())
        });
        let lines = dispense_fixture::stop_recording();
        assert_eq!(aborted_drop, Err(()));
        assert_eq!(front(game), state(game, "minecraft:water"));
        assert!(game.dropped.is_empty());
        assert!(
            lines.is_empty(),
            "aborted overflow must publish no events: {lines:?}"
        );

        dispense_fixture::start_recording();
        let committed_drop = game.storage_transaction(|game, _| {
            let mut answer = None;
            let dim = game.dim.id();
            let stack = ItemStack::new("minecraft:bucket", 3);
            game.native_dispense_item(dim, DISPENSER, Direction::East, &stack, &mut answer);
            assert!(answer.is_some());
            assert!(game.dropped.is_empty(), "drop waits until root commit");
            Ok::<_, ()>(())
        });
        let lines = dispense_fixture::stop_recording();
        assert_eq!(committed_drop, Ok(()));
        assert_eq!(front(game), state(game, "minecraft:air"));
        assert_eq!(game.dropped.len(), 1);
        assert_eq!(&*game.dropped[0].stack.item, "minecraft:water_bucket");
        assert!(
            lines
                .iter()
                .any(|line| line.ends_with("minecraft:fluid_pickup")),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.starts_with("L 1000 ")),
            "{lines:?}"
        );
        assert_eq!(game.live_scratch_count(), 0);
    });
}
