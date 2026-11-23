//! A mod container's loot table and seed (`RandomizableContainer.setLootTable`,
//! `unpackLootTable`, `LootTable.fill`), rolled by its first access. The fixture installs the
//! process's block-entity type table, so the test runs in a fresh process.
use super::quiet_write_tests::{KEY, POS, SLOTS, with_crate};
use super::*;
use sunrise_core::random::LegacyRandom;

const TABLE: &str = "minecraft:chests/simple_dungeon";

fn slots(game: &Game<'_>) -> Vec<Option<(String, u32)>> {
    game.world.block_entities[&KEY]
        .slots()
        .iter()
        .map(|slot| slot.as_ref().map(|s| (s.item.to_string(), s.count)))
        .collect()
}

/// `LootTable.fill` for an empty `SLOTS`-slot container with `RandomSource.create(seed)`,
/// as `sunrise_world`'s vanilla-pinned fill gives it, in this game's item form.
fn filled(game: &Game<'_>, seed: i64) -> Vec<Option<(String, u32)>> {
    let loot = game.server_loot.get().unwrap();
    loot.fill_with(TABLE, &mut LegacyRandom::new(seed), SLOTS, None)
        .iter()
        .map(|slot| {
            slot.as_ref().map(|stack| {
                let stack = sunrise_client::structure_blocks::loot_item_with_components(stack);
                (stack.item.to_string(), stack.count)
            })
        })
        .collect()
}

#[test]
#[ignore = "daily"]
fn a_mod_containers_loot_table_rolls_once_with_fills_placement() {
    if crate::game::native_entity_refs::tests::run_fresh_cases(
        "game::native_block_entity_refs::container_loot_table_tests::a_mod_containers_loot_table_rolls_once_with_fills_placement",
        "SUNRISE_CONTAINER_LOOT_TABLE_CHILD",
        &["loot"],
    ) {
        return;
    }
    loot_case();
}

/// The table and seed wait beside the container, saved as vanilla's keys; the first look
/// into it rolls them into its slots with `LootTable.fill`'s placement, and only once.
/// Setting the same table and seed again rolls the same items into the same slots.
fn loot_case() {
    with_crate(|game, handle| {
        let empty = vec![None; SLOTS];
        assert_eq!(handle.set_loot_table(game, TABLE, 42), Ok(true));
        assert_eq!(handle.set_loot_table(game, TABLE, 42), Ok(false));
        assert_eq!(handle.set_loot_table(game, "Not An Id", 42), Ok(false));
        let saved = game
            .data_of(&crate::game::commands::data::DataRef::Block(POS))
            .unwrap();
        assert_eq!(saved.get("LootTable"), Some(&Tag::String(TABLE.into())));
        assert_eq!(saved.get("LootTableSeed"), Some(&Tag::Long(42)));
        assert_eq!(saved.get("Items"), None);
        assert_eq!(slots(game), empty, "nothing rolls before an access");

        let expected = filled(game, 42);
        assert!(
            expected.iter().any(Option::is_some),
            "the table drops items"
        );
        game.loot_inventory_item(KEY, 0);
        assert_eq!(slots(game), expected);
        let saved = game
            .data_of(&crate::game::commands::data::DataRef::Block(POS))
            .unwrap();
        assert_eq!(saved.get("LootTable"), None, "the table is consumed");
        game.loot_inventory_item(KEY, 0);
        assert_eq!(slots(game), expected, "a second access rolls nothing");

        if let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get_mut(&KEY) {
            entity.items = vec![None; SLOTS];
        }
        assert_eq!(handle.set_loot_table(game, TABLE, 42), Ok(true));
        game.loot_inventory_item(KEY, 0);
        assert_eq!(slots(game), expected, "the same seed fills the same slots");
    });
}
