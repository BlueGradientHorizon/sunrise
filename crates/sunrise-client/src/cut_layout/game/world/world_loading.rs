

#[cfg(test)]
pub(crate) mod tests {

use super::*;

#[test]
    #[ignore = "daily"]
    fn recovered_world_uses_the_pack_flat_generator_before_its_first_save() {
        use sunrise_gameplay_common::game::execution_role::ExecutionRole::DedicatedServer;
        use std::fs;
        use sunrise_sim::save;

        let scratch = tempfile::tempdir().unwrap();
        let world = scratch.path();
        Level::default().write_without_player(world).unwrap();
        fs::rename(world.join("level.dat"), world.join("level.dat_old")).unwrap();
        let settings_path = world.join("data/minecraft/world_gen_settings.dat");
        fs::write(&settings_path, b"unreadable generator settings").unwrap();
        let pack = world.join("datapacks/recovery");
        let dimensions = pack.join("data/minecraft/dimension");
        fs::create_dir_all(&dimensions).unwrap();
        fs::write(
            pack.join("pack.mcmeta"),
            r#"{"pack":{"description":"Recovery test","min_format":[121,0],"max_format":[121,0]}}"#,
        )
        .unwrap();
        fs::write(
            world.join("datapacks.json"),
            r#"{"enabled":["vanilla","file/recovery"],"disabled":[]}"#,
        )
        .unwrap();
        fs::write(
            dimensions.join("overworld.json"),
            r#"{"type":"minecraft:overworld","generator":{"type":"minecraft:flat","settings":{
                "biome":"minecraft:plains","features":false,"lakes":false,"structure_overrides":[],
                "layers":[{"height":1,"block":"minecraft:bedrock"},{"height":2,"block":"minecraft:diamond_block"}]}}}"#,
        ).unwrap();

        let resources = std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap().into();
        sunrise_gameplay::game::native_entity_refs::tests::with_game_role(
            resources,
            DedicatedServer,
            |game| {
                let mut request = WorldRequest::new(
                    "recovery",
                    None,
                    Some(54_321),
                    false,
                    None,
                    Level::default().rule_registry,
                )
                .unwrap();
                request.save_root = world.to_owned();
                let mut prepared = game.prepare_world_and_rules(request).unwrap();
                assert_eq!(
                    fs::read(&settings_path).unwrap(),
                    b"unreadable generator settings"
                );
                let y = prepared.world.shape.min_y + 1;
                assert!(
                    prepared
                        .streaming
                        .set_forced_loaded(&mut prepared.world, 32, 32, true)
                );
                let chunk = &prepared.world.chunks[&(32, 32)];
                assert_eq!(
                    Some(chunk.get(0, y, 0)),
                    game.pack
                        .assets
                        .registry
                        .default_state("minecraft:diamond_block")
                        .map(|id| id.0)
                );

                // The spawn search sees the pack's flat layers before the first save.
                game.level = prepared.level.clone();
                game.side.level_shapes = prepared.shapes;
                assert_eq!(
                    game.spawn_origin().unwrap(),
                    ((0, 0), prepared.world.shape.min_y + 3)
                );

                prepared.level.write_without_player(world).unwrap();
                assert_eq!(
                    save::world_gen_settings(world).as_ref(),
                    prepared
                        .level
                        .generator
                        .as_ref()
                        .map(|generator| generator.settings()),
                );
            },
        );
    }
#[test]
    #[ignore = "daily"]
    fn both_server_roles_record_regions_before_world_generation() {
        mod_platform::__private::initialize().unwrap();
        use super::super::native_entity_refs::tests::with_game_role;
        use sunrise_gameplay_common::game::execution_role::ExecutionRole::{DedicatedServer, IntegratedServer};
        struct Regions;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Regions {
            fn register(
                &self,
                registrar: &mut dyn mod_api::Registrar,
                _: &mod_api::RegistrationContext,
            ) {
                registrar
                    .declare_biome_region(
                        "ice",
                        mod_api::BiomeRegion::new(
                            "minecraft:overworld".into(),
                            3,
                            vec![mod_api::BiomePoint {
                                biome: "minecraft:ice_spikes".into(),
                                temperature: (-2., 2.),
                                humidity: (-2., 2.),
                                continentalness: (-2., 2.),
                                erosion: (-2., 2.),
                                depth: (-2., 2.),
                                weirdness: (-2., 2.),
                                offset: 0.,
                            }],
                        ),
                    )
                    .unwrap();
            }
        }
        let root = PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        for role in [IntegratedServer, DedicatedServer] {
            let scratch = tempfile::tempdir().unwrap();
            let world = scratch.path().join("world");
            mod_api::testing::with_dispatch(
                vec![mod_api::ModEntry::new("region_fixture", Box::new(Regions))].into(),
                || {
                    with_game_role(root.clone(), role, |game| {
                        let request = WorldRequest {
                            owner: None,
                            chunk_reports: sunrise_sim::chunk_io::PROCESS_REPORTS.clone(),
                            save_root: world.clone(),
                            folder: "world".into(),
                            name: Some("Region fixture".into()),
                            seed: Some(12345),
                            creative: true,
                            create: None,
                            rule_registry: Level::default().rule_registry,
                        };
                        game.load_world(request).unwrap();
                        let layout =
                            sunrise_sim::save::biome_regions::layout(&world, "minecraft:overworld")
                                .unwrap()
                                .unwrap();
                        assert_eq!(layout.regions.len(), 1, "{role:?}");
                        assert_eq!(layout.regions[0].id, "region_fixture:ice");
                        assert_eq!(layout.regions[0].weight, 3);
                        assert_eq!(layout.regions[0].points[0].biome, "minecraft:ice_spikes");
                    });
                },
            );
        }
    }
}


#[cfg(test)]
mod unused_feature_startup_tests {
    use super::*;
    use {sunrise_gameplay_common::game::growth_features, sunrise_gameplay_common::game::pack_discovery::data_repository, sunrise_gameplay_common::game::world_preload::worldgen_overrides_of};

    #[test]
    #[ignore = "daily"]
    fn actual_world_prepare_rejects_unused_invalid_feature() {
        let resources = PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(resources.clone()),
            Vec::new(),
            None,
            &resources,
            &settings,
            Instant::now(),
            false,
            execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = resources;
        options.execution_role = execution_role::ExecutionRole::IntegratedServer;
        for (case, height) in [("valid", 0), ("invalid", -1)] {
            let root = std::env::current_dir().unwrap().join(case);
            let pack = root.join("datapacks/h259");
            std::fs::create_dir_all(pack.join("data/h259/worldgen/feature")).unwrap();
            std::fs::write(pack.join("pack.mcmeta"), r#"{"pack":{"description":"H259 unused feature","min_format":[121,0],"max_format":[121,0]}}"#).unwrap();
            let json = format!(
                r#"{{"type":"minecraft:fill_layer","height":{height},"state":"minecraft:stone"}}"#
            );
            std::fs::write(
                pack.join("data/h259/worldgen/feature/unused_probe.json"),
                &json,
            )
            .unwrap();
            let mut level = Level {
                seed: 259,
                data_packs: Some((vec!["vanilla".into(), "file/h259".into()], vec![])),
                ..Level::default()
            };
            level.write(&root, &Default::default()).unwrap();
            sunrise_sim::save::write_world_gen_settings(
                &root,
                259,
                sunrise_sim::save::WorldGen {
                    preset: sunrise_sim::save::WorldPreset::Flat,
                    structures: false,
                    bonus_chest: false,
                },
            )
            .unwrap();
            let (overrides, _) = worldgen_overrides_of(
                &root,
                &options.resources,
                loaded.resources.biomes(),
                &level.enabled_features,
                &loaded.pack.additional_data,
            );
            assert_eq!(
                overrides
                    .entries("features")
                    .find(|(id, _)| *id == "h259:unused_probe")
                    .unwrap()
                    .1,
                json
            );
            assert!(overrides.entries("placed_features").next().is_none());
            assert_eq!(
                sunrise_sim::save::WorldGen::read(&root).unwrap().preset,
                sunrise_sim::save::WorldPreset::Flat
            );
            let request = WorldRequest {
                save_root: root,
                folder: case.into(),
                name: None,
                seed: Some(259),
                creative: false,
                create: None,
                owner: None,
                chunk_reports: sunrise_sim::chunk_io::PROCESS_REPORTS.clone(),
                rule_registry: Level::default().rule_registry,
            };
            let result = request.prepare(
                &loaded.resources,
                &loaded.pack,
                &options,
                &settings.sim_settings(),
                &sunrise_world::datafix::progress::UpgradeProgress::new(),
            );
            if height == 0 {
                assert!(result.is_ok(), "valid unused registry entry must prepare");
                drop(result);
            } else {
                let error = match result {
                    Ok(_) => panic!(
                        "unused invalid h259:unused_probe was accepted by actual world preparation"
                    ),
                    Err(error) => error.to_string(),
                };
                assert!(error.contains("h259:unused_probe"), "{error}");
            }
        }
    }
}

#[cfg(test)]
mod unused_feature_preservation_tests {
    use super::*;
    use {sunrise_gameplay_common::game::growth_features, sunrise_gameplay_common::game::pack_discovery::data_repository, sunrise_gameplay_common::game::world_preload::worldgen_overrides_of};

    #[test]
    #[ignore = "daily"]
    fn startup_leaf_preservation_other_id_and_unrelated_feature() {
        let resources = PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(resources.clone()),
            Vec::new(),
            None,
            &resources,
            &settings,
            Instant::now(),
            false,
            execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = resources;
        options.execution_role = execution_role::ExecutionRole::IntegratedServer;
        let root = std::env::current_dir().unwrap().join("preservation");
        let pack = root.join("datapacks/h259");
        let entries = pack.join("data/controls/worldgen/feature");
        std::fs::create_dir_all(&entries).unwrap();
        std::fs::write(pack.join("pack.mcmeta"), r#"{"pack":{"description":"H259 preservation controls","min_format":[121,0],"max_format":[121,0]}}"#).unwrap();
        let leaf = r#"{"type":"minecraft:fill_layer","height":1,"state":"minecraft:stone"}"#;
        let unrelated = r#"{"type":"minecraft:no_op"}"#;
        std::fs::write(entries.join("another_id.json"), leaf).unwrap();
        std::fs::write(entries.join("unrelated.json"), unrelated).unwrap();
        let mut level = Level {
            seed: 259,
            data_packs: Some((vec!["vanilla".into(), "file/h259".into()], vec![])),
            ..Level::default()
        };
        level.write(&root, &Default::default()).unwrap();
        sunrise_sim::save::write_world_gen_settings(
            &root,
            259,
            sunrise_sim::save::WorldGen {
                preset: sunrise_sim::save::WorldPreset::Flat,
                structures: false,
                bonus_chest: false,
            },
        )
        .unwrap();
        let (mut overrides, _) = worldgen_overrides_of(
            &root,
            &options.resources,
            loaded.resources.biomes(),
            &level.enabled_features,
            &loaded.pack.additional_data,
        );
        assert_eq!(
            overrides.entries("features").collect::<Vec<_>>(),
            vec![
                ("controls:another_id", leaf),
                ("controls:unrelated", unrelated),
            ]
        );
        // Labeled native preservation controls, not additional Java runtime coverage.
        overrides
            .validate_fill_layers(loaded.pack.assets.registry)
            .unwrap();
        // The existing winning-entry map still owns precedence, not the validation phase.
        overrides.set(
            "features",
            "controls:another_id",
            r#"{"type":"minecraft:fill_layer","height":-1,"state":"minecraft:stone"}"#.into(),
        );
        let error = overrides
            .validate_fill_layers(loaded.pack.assets.registry)
            .unwrap_err();
        assert!(error.to_string().contains("controls:another_id"));
        overrides.set("features", "controls:another_id", leaf.into());
        overrides
            .validate_fill_layers(loaded.pack.assets.registry)
            .unwrap();
        let request = WorldRequest {
            save_root: root,
            folder: "preservation".into(),
            name: None,
            seed: Some(259),
            creative: false,
            create: None,
            owner: None,
            chunk_reports: sunrise_sim::chunk_io::PROCESS_REPORTS.clone(),
            rule_registry: Level::default().rule_registry,
        };
        assert!(
            request
                .prepare(
                    &loaded.resources,
                    &loaded.pack,
                    &options,
                    &settings.sim_settings(),
                    &sunrise_world::datafix::progress::UpgradeProgress::new(),
                )
                .is_ok(),
            "preservation: other-id leaf and unrelated no_op must still prepare"
        );
    }
}