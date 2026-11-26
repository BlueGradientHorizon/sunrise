

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> Identifier {
        Identifier::parse(value).unwrap()
    }

    #[test]
    #[ignore = "daily"]
    fn server_resource_uses_the_higher_of_two_selected_world_packs() {
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            let scratch = tempfile::tempdir().unwrap();
            game.side.world_root = scratch.path().to_owned();
            for (name, bytes) in [("low", "low"), ("high", "high")] {
                let pack = game.side.world_root.join("datapacks").join(name);
                std::fs::create_dir_all(pack.join("data/demo/value")).unwrap();
                std::fs::write(
                    pack.join("pack.mcmeta"),
                    r#"{"pack":{"description":"test","pack_format":121,"min_format":[121,0],"max_format":[121,0]}}"#,
                )
                .unwrap();
                std::fs::write(pack.join("data/demo/value/order.json"), bytes).unwrap();
            }
            game.build_registries();
            game.set_allow_commands(true);
            assert!(game.run_command("reload"));
            assert!(game.run_command("datapack disable \"file/high\""));
            assert!(game.run_command("datapack disable \"file/low\""));
            assert!(game.run_command("datapack enable \"file/low\" first"));
            assert!(game.run_command("datapack enable \"file/high\" last"));
            assert_eq!(
                game.resource(PackType::ServerData, &id("demo:value/order.json"))
                    .as_deref(),
                Some(b"high".as_slice())
            );
            assert!(game.run_command("datapack disable \"file/low\""));
            assert!(game.run_command("datapack enable \"file/low\" last"));
            assert_eq!(
                game.resource(PackType::ServerData, &id("demo:value/order.json"))
                    .as_deref(),
                Some(b"low".as_slice())
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn server_data_reads_vanilla_files_and_refuses_missing_ones() {
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            let bytes = game
                .resource(
                    PackType::ServerData,
                    &id("minecraft:enchantment/sharpness.json"),
                )
                .expect("the game's own data pack holds sharpness");
            assert!(bytes.starts_with(b"{"));
            assert!(
                game.resource(
                    PackType::ServerData,
                    &id("minecraft:enchantment/not_a_real_enchantment.json")
                )
                .is_none()
            );
            // A server has no selected client asset stack to read.
            assert!(
                game.resource(
                    PackType::ClientResources,
                    &id("minecraft:models/block/stone.json")
                )
                .is_none()
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn server_data_reads_mod_content_and_lets_it_override_the_game() {
        use std::collections::BTreeMap;
        use sunrise_assets::resource_source::{Resource, ResourceSource};
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            let vanilla = game
                .resource(
                    PackType::ServerData,
                    &id("minecraft:enchantment/sharpness.json"),
                )
                .expect("the game's own data pack holds sharpness");
            game.pack.data_changes = ResourceSource::indexed(BTreeMap::from([
                (
                    "data/demo/machines/furnace.json".to_owned(),
                    Resource::bytes(b"{\"mod\":true}".to_vec()),
                ),
                (
                    "data/minecraft/enchantment/sharpness.json".to_owned(),
                    Resource::bytes(b"{\"pack\":true}".to_vec()),
                ),
            ]));
            // A mod's own data file resolves through its content layer.
            assert_eq!(
                game.resource(PackType::ServerData, &id("demo:machines/furnace.json"))
                    .as_deref(),
                Some(&b"{\"mod\":true}"[..])
            );
            // A later layer in the stack overrides the game's own data.
            assert_eq!(
                game.resource(
                    PackType::ServerData,
                    &id("minecraft:enchantment/sharpness.json")
                )
                .as_deref(),
                Some(&b"{\"pack\":true}"[..])
            );
            assert_ne!(vanilla, b"{\"pack\":true}");
        });
    }

    /// `getResourceStack` gives each copy lowest first, and `listResources` the names under a
    /// folder of every namespace with the suffix, path first, without metadata files.
    #[test]
    #[ignore = "daily"]
    fn resource_stacks_and_listings_cover_every_pack_and_namespace() {
        use std::collections::BTreeMap;
        use sunrise_assets::resource_source::{Resource, ResourceSource};
        let name = "game::server_side::native_resources::tests::resource_stacks_and_listings_cover_every_pack_and_namespace";
        let flag = "SUNRISE_RESOURCE_STACK_CASE";
        if sunrise_gameplay::game::native_entity_refs::tests::run_fresh_cases(
            name,
            flag,
            &["server", "client"],
        ) {
            return;
        }
        let files = |top: &str| -> BTreeMap<String, Resource> {
            [
                ("demo/machines/a.json", "a"),
                ("demo/machines/a.json.mcmeta", "meta"),
                ("demo/machines/sub/b.json", "b"),
                ("demo/machines/e.txt", "e"),
                ("demo/other/d.json", "d"),
                ("other/machines/c.json", "c"),
            ]
            .into_iter()
            .map(|(path, bytes)| (format!("{top}/{path}"), Resource::bytes(bytes.into())))
            .collect()
        };
        let listed = [
            id("demo:machines/a.json"),
            id("other:machines/c.json"),
            id("demo:machines/sub/b.json"),
        ];
        match std::env::var(flag).unwrap().as_str() {
            "server" => sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
                let sharpness = id("minecraft:enchantment/sharpness.json");
                let vanilla = game.resource(PackType::ServerData, &sharpness).unwrap();
                let mut changes = files("data");
                changes.insert(
                    "data/minecraft/enchantment/sharpness.json".to_owned(),
                    Resource::bytes(b"{\"pack\":true}".to_vec()),
                );
                game.pack.data_changes = ResourceSource::indexed(changes);
                assert_eq!(
                    game.resource_stack(PackType::ServerData, &sharpness),
                    [vanilla, b"{\"pack\":true}".to_vec()]
                );
                assert_eq!(
                    game.resource_stack(PackType::ServerData, &id("demo:machines/none.json")),
                    Vec::<Vec<u8>>::new()
                );
                assert_eq!(
                    game.list_resources(PackType::ServerData, "machines", ".json"),
                    listed
                );
                assert!(
                    game.list_resources(PackType::ServerData, "../machines", "")
                        .is_empty()
                );
                // A server has no client resource stack.
                assert!(
                    game.resource_stack(PackType::ClientResources, &listed[0])
                        .is_empty()
                );
                assert!(
                    game.list_resources(PackType::ClientResources, "", "")
                        .is_empty()
                );
            }),
            "client" => sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
                let mut index = files("assets");
                let model = "assets/demo/models/gem.json".to_owned();
                let copies = vec![
                    Resource::bytes(b"low".to_vec()),
                    Resource::bytes(b"high".to_vec()),
                ];
                index.insert(model.clone(), copies[1].clone());
                game.pack.source =
                    ResourceSource::indexed_with_layers(index, BTreeMap::from([(model, copies)]));
                assert_eq!(
                    game.resource_stack(PackType::ClientResources, &id("demo:models/gem.json")),
                    [b"low".to_vec(), b"high".to_vec()]
                );
                // A top copy above the limit is left out of the stack, which then ends with the
                // lower pack's copy, while `resource` reads nothing.
                let big = "assets/demo/models/big.json".to_owned();
                let oversized = usize::try_from(mod_api::MAX_RESOURCE_BYTES + 1).unwrap();
                let copies = vec![
                    Resource::bytes(b"low".to_vec()),
                    Resource::bytes(vec![b' '; oversized]),
                ];
                let mut index = files("assets");
                index.insert(big.clone(), copies[1].clone());
                game.pack.source =
                    ResourceSource::indexed_with_layers(index, BTreeMap::from([(big, copies)]));
                let big = id("demo:models/big.json");
                assert_eq!(
                    game.resource_stack(PackType::ClientResources, &big),
                    [b"low".to_vec()]
                );
                assert_eq!(game.resource(PackType::ClientResources, &big), None);
                assert_eq!(
                    game.list_resources(PackType::ClientResources, "machines", ".json"),
                    listed
                );
            }),
            case => unreachable!("{case}"),
        }
    }

    #[test]
    #[ignore = "daily"]
    fn client_assets_read_the_winning_pack_file() {
        sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
            let bytes = game
                .resource(
                    PackType::ClientResources,
                    &id("minecraft:models/block/stone.json"),
                )
                .expect("the vanilla asset pack holds stone's model");
            assert!(bytes.starts_with(b"{"));
            assert!(
                game.resource(
                    PackType::ClientResources,
                    &id("minecraft:models/block/not_a_real_model.json")
                )
                .is_none()
            );
        });
    }
}
use sunrise_gameplay_common::game::chat_common::ChatCommonGameExt1 as _;
