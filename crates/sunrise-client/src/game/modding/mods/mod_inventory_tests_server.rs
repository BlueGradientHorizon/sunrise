use super::*;
use crate::game::mods::tests::inventory::EmptyCode;
use crate::game::mods::tests::inventory::Guard;
use mod_api::ModEntry;
use mod_platform::testing::with_mods;
use std::sync::atomic::{AtomicBool, Ordering};
use sunrise_mod_package::enabled::EnabledMods;

#[test]
#[ignore = "daily"]
fn code_only_reload_publishes_one_new_inventory_after_prepare_and_preserves_old_readers() {
    let fixture = Fixture::new("inventory-reload");
    smod(&fixture.0, "machine_sample");
    let sources = ModsState {
        native: Arc::new(fixture.runtime()),
        ..Default::default()
    };
    let reject = Arc::new(AtomicBool::new(false));
    with_mods(
        vec![
            ModEntry::new("machine_sample", Box::new(EmptyCode)),
            ModEntry::new("reload_guard", Box::new(Guard(reject.clone()))),
        ]
        .into(),
        || {
            crate::game::native_entity_refs::tests::with_game(|game| {
                let original = game.options.resources.clone();
                let initial = content_overlay(&original, &sources, &fixture.0.join("jars"))
                    .unwrap()
                    .root;
                game.options.resources = initial.prepared().to_owned();
                game.pack.data_source = initial.data().clone();
                game.pack.data_changes = initial.data_changes().clone();
                game.mods = sources.clone();
                game.side.world_root = fixture.0.join("world");
                let pack = game.side.world_root.join("datapacks/inventory");
                let recipes = pack.join("data/inventorytest/recipe");
                std::fs::create_dir_all(&recipes).unwrap();
                std::fs::write(pack.join("pack.mcmeta"), serde_json::to_vec(&serde_json::json!({
                "pack": {"description":"inventory fixture", "min_format":sunrise_core::version::DATA_PACK_FORMAT, "max_format":sunrise_core::version::DATA_PACK_FORMAT}
            })).unwrap()).unwrap();
                std::fs::write(recipes.join("requires_code.json"), br#"{"type":"minecraft:crafting_shapeless","ingredients":["minecraft:stone"],"result":{"id":"minecraft:stick"},"neoforge:conditions":[{"type":"neoforge:mod_loaded","modid":"machine_sample"}]}"#).unwrap();
                let tags = pack.join("data/inventorytest/tags/item");
                std::fs::create_dir_all(&tags).unwrap();
                std::fs::write(tags.join("requires_code.json"), br#"{"neoforge:conditions":[{"type":"neoforge:mod_loaded","modid":"machine_sample"}],"values":["minecraft:diamond"]}"#).unwrap();
                for (kind, mut value) in [
                    ("loot_table", serde_json::json!({"pools":[]})),
                    (
                        "predicate",
                        serde_json::json!({"type":"minecraft:random_chance","chance":1}),
                    ),
                    (
                        "item_modifier",
                        serde_json::json!({"type":"minecraft:set_count","count":1}),
                    ),
                    (
                        "jukebox_song",
                        serde_json::json!({"sound_event":"minecraft:music_disc.cat","length_in_seconds":1,"comparator_output":1,"description":"condition test"}),
                    ),
                    (
                        "instrument",
                        serde_json::json!({"sound_event":"minecraft:item.goat_horn.sound.0","use_duration":7,"range":256,"description":"condition test"}),
                    ),
                    (
                        "enchantment",
                        serde_json::json!({
                            "description":"condition test", "supported_items":"minecraft:stick", "weight":1, "max_level":3,
                            "min_cost":{"base":1,"per_level_above_first":0}, "max_cost":{"base":1,"per_level_above_first":0},
                            "anvil_cost":1, "slots":["mainhand"]
                        }),
                    ),
                ] {
                    value["neoforge:conditions"] = serde_json::json!([{"type":"neoforge:mod_loaded","modid":"machine_sample"}]);
                    let dir = pack.join("data/inventorytest").join(kind);
                    std::fs::create_dir_all(&dir).unwrap();
                    std::fs::write(dir.join("requires_code.json"), value.to_string()).unwrap();
                    std::fs::write(
                        dir.join("unsupported.json"),
                        br#"{"neoforge:conditions":[{"type":"neoforge:never"}]}"#,
                    )
                    .unwrap();
                }
                // This fixture skips world open, which normally loads the built-in sound holders.
                let registries = crate::game::chat_common::command_export(&original, false)
                    .expect("prepared command registries");
                game.ui.registries.fill_from_exported(&registries);
                game.reload_datapacks(true, &[]).unwrap();
                let loaded = |game: &Game<'_, crate::game::server_side::ServerState>, id: &str| {
                    let items = game.items();
                    (
                        items.has_loot_table(id),
                        items.songs.contains_key(id),
                        items.instruments.contains_key(id),
                        items.enchantments.list.iter().any(|e| &*e.id == id),
                        game.side.fns.0.store.predicates.contains_key(id),
                        game.side.fns.0.store.item_modifiers.contains_key(id),
                    )
                };
                assert_eq!(
                    loaded(game, "inventorytest:requires_code"),
                    (true, true, true, true, true, true)
                );
                assert_eq!(
                    loaded(game, "inventorytest:unsupported"),
                    (false, false, false, false, false, false)
                );
                let old_songs = game.items().songs.clone();
                let read_tags = |game: &Game<'_, crate::game::server_side::ServerState>| {
                    game.items().item_tags.clone()
                };
                let read = |game: &Game<'_, crate::game::server_side::ServerState>| {
                    crate::game::lan::sync::server_sources(
                        &game.options.resources,
                        &game.res.core.item_data,
                        game.items(),
                        &game.pack_files(),
                    )
                    .unwrap()
                };
                let has_recipe = |sources: &sunrise_sim::crafting::ResolvedSources| {
                    sources
                        .recipes
                        .iter()
                        .any(|recipe| recipe.name == "inventorytest:requires_code")
                };
                let old = read(game);
                assert!(has_recipe(&old));
                let old_tags = read_tags(game);
                assert!(old_tags["inventorytest:requires_code"].contains("minecraft:diamond"));
                let old_root = game.options.resources.clone();
                let old_data = game.pack.data_source.clone();
                assert!(mod_api::__private::set_mod_enabled("machine_sample", false));
                reject.store(true, Ordering::Relaxed);
                assert!(game.reload_datapacks(true, &[]).is_err());
                assert_eq!(game.options.resources, old_root);
                assert!(game.pack.data_source.same_generation(&old_data));
                assert!(
                    read_tags(game)["inventorytest:requires_code"].contains("minecraft:diamond")
                );
                assert_eq!(
                    loaded(game, "inventorytest:requires_code"),
                    (true, true, true, true, true, true)
                );
                assert_eq!(
                    EnabledMods::read(&old_root)
                        .unwrap()
                        .server_ids()
                        .collect::<Vec<_>>(),
                    ["machine_sample"]
                );
                reject.store(false, Ordering::Relaxed);
                game.reload_datapacks(true, &[]).unwrap();
                assert_eq!(game.options.resources, original);
                assert!(!has_recipe(&read(game)));
                // Instruments and enchantments are world registries, frozen at world open: a
                // reload re-evaluates the reloadable data (loot, songs, predicates, modifiers).
                assert_eq!(
                    loaded(game, "inventorytest:requires_code"),
                    (false, false, true, true, false, false)
                );
                assert!(old_songs.contains_key("inventorytest:requires_code"));
                // Tag files ignore load conditions, including mod_loaded.
                assert!(
                    read_tags(game)["inventorytest:requires_code"].contains("minecraft:diamond")
                );
                assert!(old_tags["inventorytest:requires_code"].contains("minecraft:diamond"));
                assert!(has_recipe(&old));
                assert!(mod_api::__private::set_mod_enabled("machine_sample", true));
                game.reload_datapacks(true, &[]).unwrap();
                assert!(has_recipe(&read(game)));
                assert_eq!(
                    loaded(game, "inventorytest:requires_code"),
                    (true, true, true, true, true, true)
                );
                assert!(
                    read_tags(game)["inventorytest:requires_code"].contains("minecraft:diamond")
                );
                assert_eq!(game.options.resources, old_root);
            });
        },
    );
}
