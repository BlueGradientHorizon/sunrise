//! Client-only mod-content asset hooks (fluid visuals and client pack merging).
use crate::{Result, mod_content::ContentMod};
use std::path::{Path, PathBuf};

/// Remembers how the mods' fluids look (`data/<id>/sunrise/fluid/`) and publishes client models.
pub fn set_fluid_looks(mods: &[ContentMod]) {
    crate::mod_content::set_fluid_looks(mods);
    crate::mod_content::with_fluid_looks(|looks| {
        crate::fluid_models::set_world_looks(looks.values());
    });
}

/// Open passive layers for a client load, merging mod asset layers through `packs::resources`.
pub fn resolved_resources(
    base: &Path,
    prepared: PathBuf,
    jar_store: &Path,
    mods: &[ContentMod],
    cache: PathBuf,
) -> Result<sunrise_assets::resource_source::ResourceRoot> {
    crate::mod_content::resolved_resources_with(
        base,
        prepared,
        jar_store,
        mods,
        cache,
        |assets_base, layer_sources| {
            let layers: Vec<_> = layer_sources
                .into_iter()
                .map(|(id, source)| {
                    crate::packs::Pack::mod_layer(
                        id,
                        sunrise_level::pack_meta::Source::Opened(source),
                    )
                })
                .collect();
            crate::packs::resources(assets_base, &layers)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::resolved_resources;
    use crate::{
        assets::Assets,
        blocks::BlockFacts,
        mod_content::{
            ContentMod, biome_modifiers_over_packs,
            test_fixtures::{
                biome_base, build_fixture, folder_mod, manifest, prepared, relative_base,
                resource_root_fixture, small_base, zinc_mod,
            },
        },
    };
    use std::{collections::BTreeMap, fs, path::Path};
    use sunrise_mod_package::{content::Content, manifest::JarRef};

    /// A Java jar's assets are a pack layer under the mod's own files, in mod
    /// order under the later mods. Passive assets stay in the jar, and the files
    /// every pack stacks (atlases here) merge across vanilla, the jars and the
    /// mods in that same order.
    #[test]
    fn a_mods_jars_layer_under_its_files_in_mod_order() {
        use std::io::Write as _;
        let scratch = std::env::temp_dir().join(format!("sunrise-mod-jars-{}", std::process::id()));
        let _ = fs::remove_dir_all(&scratch);
        let base = scratch.join("base");
        small_base(&base);
        // `build` reads the base's registry tables even for a plan with no
        // blocks: a minimal facts pair satisfies it without the full export.
        fs::write(
            base.join("facts.json"),
            r#"{"states": [["minecraft:stone"]], "defaults": {"minecraft:stone": 0}}"#,
        )
        .unwrap();
        fs::write(
            base.join("gameplay-facts.json"),
            r#"{"states": [{"item": "minecraft:stone"}]}"#,
        )
        .unwrap();
        let vanilla_path = "assets/minecraft/textures/block/vanilla_path.png";
        fs::create_dir_all(base.join(vanilla_path).parent().unwrap()).unwrap();
        fs::write(base.join(vanilla_path), b"vanilla").unwrap();
        fs::create_dir_all(base.join("assets/minecraft/atlases")).unwrap();
        fs::write(
            base.join("assets/minecraft/atlases/blocks.json"),
            r#"{"sources": [{"type": "directory", "source": "block", "prefix": "block/"}]}"#,
        )
        .unwrap();
        let atlas = |source: &str| {
            format!(
                "{{\"sources\": [{{\"type\": \"directory\", \"source\": \"{source}\", \"prefix\": \"\"}}]}}"
            )
        };
        // A jar in the store as the launcher names it: `<sha512>.jar`, with
        // entries in any namespace (a jar is not fenced as a mod's files are).
        let jar = |name: &str, entries: &[(&str, String)]| {
            let sha512 = format!("{}{}", "1".repeat(127), name.chars().last().unwrap());
            let file = scratch.join(format!("jars/{sha512}.jar"));
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            let mut zip = zip::ZipWriter::new(fs::File::create(&file).unwrap());
            let options = zip::write::SimpleFileOptions::default();
            for (entry, text) in entries
                .iter()
                .chain(std::iter::once(&("Example.class", "dummy".to_owned())))
            {
                zip.start_file(*entry, options).unwrap();
                zip.write_all(text.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
            JarRef::new(name.into(), "1.0".into(), sha512).unwrap()
        };
        let ported = |dir: &str, id: &str, files: &[(&str, String)], jars: Vec<JarRef>| {
            let mut m = folder_mod(
                &scratch.join(dir),
                &files
                    .iter()
                    .map(|(name, text)| (*name, text.as_str()))
                    .chain(std::iter::once((
                        "sunrise-mod.toml",
                        manifest(id, "").as_str(),
                    )))
                    .collect::<Vec<_>>(),
            );
            m.jars = jars;
            m
        };
        fn own<'a>(path: &'a str, text: &str) -> (&'a str, String) {
            (path, text.to_owned())
        }
        let atlas_file = |source: &str| ("assets/minecraft/atlases/blocks.json", atlas(source));
        let jar_a = jar(
            "porteda",
            &[
                own("assets/alpha/own_and_jar.png", "jar-a"),
                own("assets/alpha/own_only.png", "jar-a"),
                own(vanilla_path, "jar-a-mc"),
                atlas_file("jar_a"),
            ],
        );
        let jar_b = jar(
            "portedb",
            &[
                own("assets/alpha/own_and_jar.png", "jar-b"),
                own("assets/beta/only_jar.png", "jar-b-only"),
                atlas_file("jar_b"),
            ],
        );
        let alpha = ported(
            "alpha",
            "alpha",
            &[
                own("assets/alpha/own_and_jar.png", "mod-a"),
                own("assets/alpha/own_only.png", "mod-a"),
                atlas_file("mod_a"),
            ],
            vec![jar_a.clone()],
        );
        let beta = ported(
            "beta",
            "beta",
            &[own("assets/beta/own.png", "mod-b"), atlas_file("mod_b")],
            vec![jar_b.clone()],
        );
        let root = scratch.join("root");
        let mods = [alpha, beta];
        let warnings = build_fixture(&base, &scratch.join("jars"), &mods, &root).unwrap();
        let resources = resolved_resources(
            &base,
            root.clone(),
            &scratch.join("jars"),
            &mods,
            scratch.join("cache"),
        )
        .unwrap();
        assert!(!root.join("assets/alpha").exists());
        assert!(!root.join("assets/beta").exists());
        let mut listed = warnings.clone();
        listed.sort();
        assert_eq!(
            listed,
            [
                "mod alpha: jar porteda also provides assets/alpha/own_and_jar.png; the mod's own file wins",
                "mod alpha: jar porteda also provides assets/alpha/own_only.png; the mod's own file wins",
            ]
        );
        // A mod's own files win over its jars; a later mod's jar wins over an
        // earlier mod's own file; a jar wins over vanilla.
        assert_eq!(
            resources
                .source()
                .read("assets/alpha/own_only.png")
                .unwrap(),
            b"mod-a"
        );
        assert_eq!(
            resources
                .source()
                .read("assets/alpha/own_and_jar.png")
                .unwrap(),
            b"jar-b"
        );
        assert_eq!(resources.source().read(vanilla_path).unwrap(), b"jar-a-mc");
        assert_eq!(
            resources.source().read("assets/beta/only_jar.png").unwrap(),
            b"jar-b-only"
        );
        // The stacked files merge across vanilla, the jars and the mods in the
        // same order, and no root Java bytecode leaves the jar.
        let sources: Vec<_> = serde_json::from_slice::<serde_json::Value>(
            &resources
                .source()
                .read("assets/minecraft/atlases/blocks.json")
                .unwrap(),
        )
        .unwrap()["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["source"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(sources, ["block", "jar_a", "mod_a", "jar_b", "mod_b"]);
        assert!(!root.join("Example.class").exists());
        // A mod without its own files still layers its jar, under an earlier
        // mod's own files.
        let root = scratch.join("root2");
        let mods = [
            ported(
                "alpha2",
                "alpha",
                &[own("assets/alpha/own.png", "mod-a")],
                vec![jar_a],
            ),
            ported("beta-empty", "beta", &[], vec![jar_b]),
        ];
        build_fixture(&base, &scratch.join("jars"), &mods, &root).unwrap();
        let resources = resolved_resources(
            &base,
            root.clone(),
            &scratch.join("jars"),
            &mods,
            scratch.join("cache"),
        )
        .unwrap();
        assert!(resources.source().contains("assets/alpha/own.png"));
        assert!(!root.join("assets/alpha").exists());
        assert_eq!(
            resources.source().read("assets/beta/only_jar.png").unwrap(),
            b"jar-b-only"
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    /// A mod's `assets/minecraft/atlases/blocks.json` adds its sources after vanilla's.
    #[test]
    fn a_content_root_appends_a_mods_atlas_sources() {
        let Some(base) = prepared() else {
            return;
        };
        let scratch =
            std::env::temp_dir().join(format!("sunrise-mod-atlas-{}", std::process::id()));
        let _ = fs::remove_dir_all(&scratch);
        let m = folder_mod(
            &scratch.join("gauge"),
            &[
                ("sunrise-mod.toml", &manifest("gauge", "")),
                (
                    "assets/minecraft/atlases/blocks.json",
                    r#"{"sources": [{"type": "minecraft:directory", "source": "gauge_liquid", "prefix": "gauge_liquid/"}]}"#,
                ),
            ],
        );
        let root = scratch.join("root");
        build_fixture(&base, Path::new(""), std::slice::from_ref(&m), &root).unwrap();
        let resources = resolved_resources(
            &base,
            root.clone(),
            Path::new(""),
            std::slice::from_ref(&m),
            scratch.join("cache"),
        )
        .unwrap();
        let sources =
            |source: &sunrise_assets::resource_source::ResourceSource| -> Vec<serde_json::Value> {
                let text = source.read("assets/minecraft/atlases/blocks.json").unwrap();
                let doc: serde_json::Value = serde_json::from_slice(&text).unwrap();
                doc["sources"].as_array().unwrap().clone()
            };
        let (vanilla, built) = (
            sources(&sunrise_assets::resource_source::ResourceSource::directory(
                base.clone(),
            )),
            sources(resources.source()),
        );
        assert_eq!(built.len(), vanilla.len() + 1);
        assert_eq!(built[..vanilla.len()], vanilla[..]);
        assert_eq!(built[vanilla.len()]["source"], "gauge_liquid");
        let _ = fs::remove_dir_all(&scratch);
    }

    /// A mod's creative tab comes after vanilla's, on page 1 in the first top
    /// slot, with only the items the game has; its new items join the search tab
    /// under their English names.
    #[test]
    #[ignore = "daily"]
    fn creative_tabs_come_on_their_own_page() {
        let Some(base) = prepared() else {
            return;
        };
        let scratch = std::env::temp_dir().join(format!("sunrise-mod-tabs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&scratch);
        let dir = scratch.join("gems");
        for (name, text) in [
            (
                "sunrise-mod.toml",
                "id = \"gems\"\nname = \"Gems\"\nversion = \"0.1.0\"\napi = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\n",
            ),
            (
                "data/gems/sunrise/item/ruby.json",
                r#"{"template": "minecraft:diamond"}"#,
            ),
            (
                "data/gems/sunrise/block/ruby_block.json",
                r#"{"template": "minecraft:diamond_block"}"#,
            ),
            (
                "data/gems/sunrise/creative_tab/gems.json",
                r#"{"title": "Gems", "icon": "gems:ruby", "items": ["gems:ruby", "minecraft:diamond", "gems:missing"]}"#,
            ),
            (
                "assets/gems/lang/en_us.json",
                r#"{"item.gems.ruby": "Ruby"}"#,
            ),
        ] {
            let path = dir.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let package = sunrise_mod_package::Package::read(&dir).unwrap();
        let m = ContentMod {
            id: "gems".into(),
            content: Content::read(&dir, &package.manifest).unwrap(),
            jars: Vec::new(),
        };
        let relative = relative_base(&base);
        let root = resource_root_fixture(
            &relative,
            Path::new(""),
            std::slice::from_ref(&m),
            &scratch.join("cache"),
        )
        .unwrap();
        let mut creative = crate::creative::Creative::load(&root).unwrap();
        let lists = creative.catalog().build(
            sunrise_items::creative_catalog::Parameters {
                operator: false,
                features: &["minecraft:vanilla".into()],
                holders: &Default::default(),
                items: &crate::items::Items::empty(),
            },
            None,
            |_, _, _| {},
        );
        creative.publish(
            lists,
            Default::default(),
            Default::default(),
            &crate::item_names::Names::default(),
        );
        assert_eq!(creative.pages(), 2);
        let i = creative
            .tabs
            .iter()
            .position(|t| t.id == "gems:gems")
            .unwrap();
        let tab = &creative.tabs[i];
        assert_eq!(
            (tab.page, tab.top, tab.column, tab.title.as_str()),
            (1, true, 0, "Gems")
        );
        let items: Vec<&str> = tab.items.iter().map(|s| &*s.item).collect();
        assert_eq!(items, ["gems:ruby", "minecraft:diamond"]);
        assert!(
            creative
                .search_items()
                .iter()
                .any(|s| &*s.item == "gems:ruby")
        );
        // A block's item is in search once.
        let blocks = creative
            .search_items()
            .iter()
            .filter(|s| &*s.item == "gems:ruby_block");
        assert_eq!(blocks.count(), 1);
        // Search, on every page, stays selected over a page turn.
        let search = creative.tabs.iter().position(|t| t.search).unwrap();
        creative.select(search);
        creative.set_page(1);
        assert_eq!((creative.selected_index(), creative.page), (search, 1));
        creative.select(0);
        // Page 1 shows the mod's tab and the right-hand ones, not vanilla's.
        creative.set_page(1);
        assert_eq!(creative.selected_index(), i);
        let shown: Vec<&str> = creative
            .tabs
            .iter()
            .filter(|t| creative.on_page(t))
            .map(|t| t.id.as_str())
            .collect();
        assert!(
            shown.contains(&"gems:gems") && shown.contains(&"minecraft:search"),
            "{shown:?}"
        );
        assert!(!shown.contains(&"minecraft:building_blocks"), "{shown:?}");
        // Selecting a vanilla tab goes back to its page.
        creative.select(0);
        assert_eq!(creative.page, 0);
        fs::remove_dir_all(&scratch).unwrap();
    }

    /// A sign's four forms come from its template, whatever the blocks are named,
    /// its wood from its declaration, and its item places the wall form it names
    /// and is that form's item too. A `road_sign` of stone is no sign.
    #[test]
    #[ignore = "daily"]
    fn signs_take_their_form_wood_and_wall_block_from_their_declaration() {
        use sunrise_world::level::{Behaviours, ItemKind};

        let Some(base) = prepared() else {
            return;
        };
        let scratch =
            std::env::temp_dir().join(format!("sunrise-mod-signs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&scratch);
        let m = folder_mod(
            &scratch.join("signs"),
            &[
                ("sunrise-mod.toml", &manifest("signs", "")),
                (
                    "data/signs/sunrise/block/notice.json",
                    r#"{"template": "minecraft:oak_sign", "wood_type": "signs:elm", "wall_block": "signs:board"}"#,
                ),
                (
                    "data/signs/sunrise/block/board.json",
                    r#"{"template": "minecraft:oak_wall_sign", "wood_type": "signs:elm", "item": false}"#,
                ),
                (
                    "data/signs/sunrise/block/lantern.json",
                    r#"{"template": "minecraft:spruce_hanging_sign", "wall_block": "signs:bracket"}"#,
                ),
                (
                    "data/signs/sunrise/block/bracket.json",
                    r#"{"template": "minecraft:spruce_wall_hanging_sign", "wood_type": "cherry", "item": false}"#,
                ),
                (
                    "data/signs/sunrise/block/road_sign.json",
                    r#"{"template": "minecraft:stone"}"#,
                ),
            ],
        );
        let root = resource_root_fixture(
            &base,
            Path::new(""),
            std::slice::from_ref(&m),
            &scratch.join("cache"),
        )
        .unwrap();
        let assets = Assets::load(&root).unwrap();
        let registry = assets.registry;
        let facts = BlockFacts::load(&root, registry).unwrap();
        let behaviours = Behaviours::load(registry, &root).unwrap();
        let block = |name: &str| facts.get(registry.default_state(name).unwrap());
        let form = |hanging, wall| Some(crate::blocks::SignForm { hanging, wall });
        assert_eq!(block("signs:notice").sign_form(), form(false, false));
        assert_eq!(block("signs:board").sign_form(), form(false, true));
        assert_eq!(block("signs:lantern").sign_form(), form(true, false));
        assert_eq!(block("signs:bracket").sign_form(), form(true, true));
        assert_eq!(block("signs:road_sign").sign_form(), None);
        assert_eq!(
            block("minecraft:oak_wall_sign").sign_form(),
            form(false, true)
        );
        // The declared wood, or the template's.
        assert_eq!(facts.wood_type("signs:notice"), Some("signs:elm"));
        assert_eq!(facts.wood_type("signs:bracket"), Some("cherry"));
        assert_eq!(facts.wood_type("signs:lantern"), Some("spruce"));
        assert_eq!(
            facts.wood_type("minecraft:dark_oak_wall_sign"),
            Some("dark_oak")
        );
        assert_eq!(facts.wood_type("signs:road_sign"), None);
        // Each item places its sign and the wall form it names, which it is the item of.
        let id = |name: &str| behaviours.table.block_by_name(name);
        let notice = behaviours.item_binding("signs:notice").unwrap();
        assert_eq!(notice.kind, ItemKind::StandingAndWall);
        assert_eq!(
            (Some(notice.block), notice.wall_block),
            (id("signs:notice"), id("signs:board"))
        );
        let lantern = behaviours.item_binding("signs:lantern").unwrap();
        assert_eq!(lantern.kind, ItemKind::HangingSign);
        assert_eq!(
            (Some(lantern.block), lantern.wall_block),
            (id("signs:lantern"), id("signs:bracket"))
        );
        assert_eq!(&*block("signs:board").item, "signs:notice");
        assert_eq!(&*block("signs:bracket").item, "signs:lantern");
        let defaults = sunrise_world::item_defaults::read(&root).unwrap();
        let mut renderer = crate::block_entity_render::BlockEntityRenderer::load(
            &sunrise_assets::resource_source::ResourceRoot::directory(root.to_path_buf()),
            registry,
            &defaults,
        )
        .unwrap();
        renderer.add_mod_signs(&facts, registry);
        for name in [
            "signs:notice",
            "signs:board",
            "signs:lantern",
            "signs:bracket",
        ] {
            assert!(
                renderer.renders(registry.default_state(name).unwrap().0),
                "{name}"
            );
        }
        assert!(!renderer.renders(registry.default_state("signs:road_sign").unwrap().0));
        // Vanilla's pair stays as it was.
        let oak = behaviours.item_binding("minecraft:oak_sign").unwrap();
        assert_eq!(oak.wall_block, id("minecraft:oak_wall_sign"));
        let _ = fs::remove_dir_all(&scratch);
    }

    /// `sunrise:modify_biome` writes ambient particles, ambient sounds and music as the vanilla
    /// attribute values (the shapes of `warped_forest.json`), each replacing the biome's value.
    #[test]
    fn modify_biome_sets_ambient_particles_sounds_and_music() {
        let base = biome_base("biome-modifiers-modify-effects");
        let scratch = base.parent().unwrap().to_owned();
        fs::write(
            base.join("registries.json"),
            r#"{"minecraft:item":{"entries":{"minecraft:test":{"protocol_id":0}}},"minecraft:block":{"entries":{"minecraft:test":{"protocol_id":0}}},"minecraft:sound_event":{"entries":{"minecraft:ambient.warped_forest.loop":{"protocol_id":0},"minecraft:ambient.warped_forest.additions":{"protocol_id":1},"minecraft:ambient.warped_forest.mood":{"protocol_id":2}}}}"#,
        )
        .unwrap();
        let m = zinc_mod(
            &scratch.join("zinc"),
            &[(
                "dunes",
                r#"{"type":"sunrise:modify_biome","biomes":"minecraft:plains","attributes":{
                    "minecraft:visual/ambient_particles":{"argument":[{"particle":{"type":"minecraft:ash"},"probability":0.25}]},
                    "minecraft:audio/ambient_sounds":{"loop":"minecraft:ambient.warped_forest.loop",
                        "additions":{"sound":"minecraft:ambient.warped_forest.additions","tick_chance":1.5},
                        "mood":{"sound":"minecraft:ambient.warped_forest.mood","tick_delay":6000,"block_search_extent":8,"offset":2.0}},
                    "minecraft:audio/background_music":{"default":{"sound":{"sound_id":"zinc:music.dunes"},"min_delay":25000,"max_delay":24000}},
                    "minecraft:audio/music_volume":0.5}}"#,
            )],
        );
        let out = scratch.join("out");
        let warnings = build_fixture(&base, &base.join("jars"), &[m], &out).unwrap();
        assert!(
            warnings.iter().all(|w| !w.contains("skipped")),
            "{warnings:?}"
        );
        let after: BTreeMap<String, String> =
            serde_json::from_slice(&fs::read(out.join("worldgen/biomes.json")).unwrap()).unwrap();
        let plains: serde_json::Value = serde_json::from_str(&after["minecraft:plains"]).unwrap();
        let attributes = &plains["attributes"];
        assert_eq!(
            attributes["minecraft:visual/ambient_particles"],
            serde_json::json!([{"particle": {"type": "minecraft:ash"}, "probability": 0.25}])
        );
        assert_eq!(
            attributes["minecraft:audio/ambient_sounds"],
            serde_json::json!({
                "loop": "minecraft:ambient.warped_forest.loop",
                "additions": {"sound": "minecraft:ambient.warped_forest.additions", "tick_chance": 1.5},
                "mood": {"sound": "minecraft:ambient.warped_forest.mood", "tick_delay": 6000,
                    "block_search_extent": 8, "offset": 2.0},
            })
        );
        assert_eq!(
            attributes["minecraft:audio/background_music"],
            serde_json::json!({"default": {"sound": {"sound_id": "zinc:music.dunes"}, "min_delay": 25000, "max_delay": 24000}})
        );
        assert_eq!(attributes["minecraft:audio/music_volume"], 0.5);

        // The modified biome flows through playback selection with its custom sound and volume.
        let builtin = std::sync::Arc::new(
            sunrise_protocol::registry::Builtin::load(&base.join("registries.json")).unwrap(),
        );
        let music_tables = crate::music::MusicTables::load(&out, builtin.clone());
        let (bg, vol) = music_tables.at("minecraft:overworld", "minecraft:plains", Some(&plains));
        let selected = crate::music::Situation {
            screen_music: None,
            in_world: true,
            in_end: false,
            boss_music: false,
            background: &bg,
            probe_volume: vol,
            creative: false,
            underwater: false,
        };
        assert_eq!(
            selected.music().map(|m| m.sound),
            Some("zinc:music.dunes".to_owned())
        );
        assert_eq!(selected.volume(), 0.5);

        // Typed ModifyBiome declared in code emits inline sound events that load and play identically.
        let typed = mod_api::BiomeModifier {
            biomes: vec!["minecraft:desert".into()],
            operation: mod_api::BiomeModifierOperation::ModifyBiome {
                temperature: None,
                downfall: None,
                has_precipitation: None,
                temperature_modifier: None,
                sky_color: None,
                fog_color: None,
                water_color: None,
                water_fog_color: None,
                foliage_color: None,
                dry_foliage_color: None,
                grass_color: None,
                ambient_particles: None,
                ambient_loop: None,
                ambient_additions: None,
                ambient_mood: None,
                background_music: Some(mod_api::BiomeMusic {
                    sound: "zinc:music.oasis".into(),
                    min_delay: 100,
                    max_delay: 200,
                }),
                music_volume: Some(0.25),
            },
        };
        let declared = [["zinc".to_owned(), "declared:oasis".to_owned(), typed.json()]];
        let packs = sunrise_world::worldgen::overrides::WorldgenOverrides::default();
        let overridden = biome_modifiers_over_packs(&out, &packs, &declared)
            .unwrap()
            .expect("typed modifier produces biome overrides");
        let desert_text = overridden
            .entries("biomes")
            .find_map(|(id, text)| (id == "minecraft:desert").then_some(text))
            .expect("desert override present");
        let desert: serde_json::Value = serde_json::from_str(desert_text).unwrap();
        let (desert_bg, desert_vol) =
            music_tables.at("minecraft:overworld", "minecraft:desert", Some(&desert));
        assert_eq!(
            desert_bg.default.map(|m| m.sound),
            Some("zinc:music.oasis".to_owned())
        );
        assert_eq!(desert_vol, 0.25);

        fs::remove_dir_all(scratch).unwrap();
    }
}
