use serde_json::json;

#[test]
fn candidate_registry_folder_collision_assigns_entry_to_minecraft_registry() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("command-registries.json"),
        br#"{"by_registry":{"example:spell":{"ids":["example:original"]},"minecraft:example/spell":{"ids":["minecraft:original"]}}}"#).unwrap();
    let view = super::candidate_registries(
        root.path(),
        &[("example/spell/test:added".into(), vec![b"{}".to_vec()])],
        &sunrise_world::conditions::LoadConditions::new([]),
    );
    assert_eq!(
        view["example:spell"]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["example:original"]
    );
    assert_eq!(
        view["minecraft:example/spell"]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["minecraft:original", "test:added"]
    );
    let conditions = sunrise_world::conditions::LoadConditions::new([]).with_registries(
        view.iter()
            .map(|(registry, ids)| (registry.as_str(), ids.iter().map(String::as_str))),
    );
    assert!(!conditions.allows(&json!({"neoforge:conditions":[{"type":"neoforge:registered","registry":"example:spell","value":"test:added"}]})));
    assert!(conditions.allows(&json!({"neoforge:conditions":[{"type":"neoforge:registered","registry":"minecraft:example/spell","value":"test:added"}]})));
    assert_eq!(conditions.check_errors(), Ok(()));
}

struct Registry;
#[mod_api::__subscriptions]
impl mod_api::Hooks for Registry {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        r.declare_datapack_registry(
            "spell",
            mod_api::DatapackRegistrySync::Required,
            vec![mod_api::DatapackField::required(
                "power",
                mod_api::DatapackFieldKind::Int { min: 0, max: 100 },
            )],
        )
        .unwrap();
    }
}
fn write(path: &std::path::Path, bytes: impl AsRef<[u8]>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}
#[test]
#[ignore = "daily"]
fn reload_conditions_read_candidate_registries_and_bound_tags() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    let pack = root.join("datapacks/base");
    write(
        &pack.join("pack.mcmeta"),
        br#"{"pack":{"description":"fixture","min_format":94,"max_format":94}}"#,
    );
    write(
        &root.join("datapacks.json"),
        br#"{"enabled":["vanilla","file/base"],"disabled":[]}"#,
    );
    for (name, condition) in [
        (
            "registered",
            json!({"type":"neoforge:registered","registry":"reload_conditions:spell","value":"reload_conditions:fire"}),
        ),
        (
            "empty",
            json!({"type":"neoforge:tag_empty","registry":"minecraft:item","tag":"reload_conditions:flag"}),
        ),
        (
            "absent",
            json!({"type":"neoforge:not","value":{"type":"neoforge:registered","registry":"reload_conditions:absent","value":"reload_conditions:fire"}}),
        ),
    ] {
        write(&pack.join(format!("data/reload_conditions/recipe/{name}.json")), serde_json::to_vec(&json!({
            "type":"minecraft:crafting_shapeless","ingredients":["minecraft:flint"],"result":{"id":"minecraft:stick"},"neoforge:conditions":[condition]
        })).unwrap());
    }
    mod_platform::testing::with_mods(
        vec![mod_api::ModEntry::new(
            "reload_conditions",
            Box::new(Registry),
        )]
        .into(),
        || {
            crate::game::native_entity_refs::tests::with_game(|game| {
                let mut request = crate::game::world_loading::WorldRequest::new(
                    "conditions",
                    Some("test"),
                    Some(1),
                    true,
                    None,
                    std::sync::Arc::new(sunrise_client::gamerules::RuleRegistry::vanilla()),
                )
                .unwrap();
                request.save_root = root.to_owned();
                game.load_world(request).unwrap();
                let answers = |game: &crate::game::Game<'_>| {
                    ["registered", "empty", "absent"].map(|name| {
                        game.recipes()
                            .recipe(&format!("reload_conditions:{name}"))
                            .is_some()
                    })
                };
                assert_eq!(answers(game), [false, true, true]);
                write(
                    &pack.join("data/reload_conditions/reload_conditions/spell/fire.json"),
                    br#"{"power":7}"#,
                );
                write(
                    &pack.join("data/reload_conditions/tags/item/flag.json"),
                    br#"{"values":["minecraft:flint"]}"#,
                );
                assert!(game.run_command("reload"));
                assert_eq!(
                    game.level(game.dim.id())
                        .unwrap()
                        .datapack_entries(game, "reload_conditions:spell"),
                    Some(vec!["reload_conditions:fire".into()])
                );
                assert_eq!(answers(game), [true, false, true]);
                std::fs::remove_file(
                    pack.join("data/reload_conditions/reload_conditions/spell/fire.json"),
                )
                .unwrap();
                std::fs::remove_file(pack.join("data/reload_conditions/tags/item/flag.json"))
                    .unwrap();
                assert!(game.run_command("reload"));
                assert_eq!(answers(game), [false, true, true]);
                assert_eq!(
                    game.mods
                        .load_condition_candidate
                        .as_ref()
                        .unwrap()
                        .check_errors(),
                    Ok(())
                );
            });
        },
    );
}
