//! Resource readers for native mods through the game's selected pack stacks.
use super::Game;
use mod_api::{Identifier, PackType, ResourceOpenError};
use std::io::Read;
use sunrise_assets::resource_source::open_winning_resource;

impl Game<'_> {
    pub(in crate::game) fn open_resource(
        &self,
        pack_type: PackType,
        identifier: &Identifier,
    ) -> Result<Option<Box<dyn Read + '_>>, ResourceOpenError> {
        if pack_type == PackType::ClientResources {
            if !self.options.execution_role.is_client() {
                return Ok(None);
            }
            // `packs::resources` already applies client asset overrides and filters.
            let name = format!("assets/{}/{}", identifier.namespace(), identifier.path());
            return open_winning_resource([&self.pack.source], &name)
                .map_err(ResourceOpenError::Io);
        }

        let name = format!("data/{}/{}", identifier.namespace(), identifier.path());
        let sources = self
            .options
            .execution_role
            .is_logical_server()
            .then(|| self.fns.pack_sources())
            .into_iter()
            .flatten()
            .chain(std::iter::once(&self.pack.data_changes))
            .chain((!self.cmds.vanilla_disabled).then_some(&self.pack.data_source));
        open_winning_resource(sources, &name).map_err(ResourceOpenError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::prelude::Host;
    use std::collections::BTreeMap;
    use sunrise_assets::resource_source::{Resource, ResourceSource};

    fn read(host: &dyn Host, pack_type: PackType, name: &str) -> Option<Vec<u8>> {
        let mut reader = host
            .open_resource(pack_type, &Identifier::parse(name).unwrap())
            .unwrap()?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        Some(bytes)
    }

    #[test]
    #[ignore = "daily"]
    fn server_resource_uses_the_higher_of_two_selected_world_packs() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let scratch = tempfile::tempdir().unwrap();
            game.world_root = scratch.path().to_owned();
            for (name, bytes) in [("low", "low"), ("high", "high")] {
                let pack = game.world_root.join("datapacks").join(name);
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
                read(game, PackType::ServerData, "demo:value/order.json").as_deref(),
                Some(b"high".as_slice())
            );
            assert!(game.run_command("datapack disable \"file/low\""));
            assert!(game.run_command("datapack enable \"file/low\" last"));
            assert_eq!(
                read(game, PackType::ServerData, "demo:value/order.json").as_deref(),
                Some(b"low".as_slice())
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn server_data_reads_vanilla_files_and_refuses_missing_ones() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let bytes = read(
                game,
                PackType::ServerData,
                "minecraft:enchantment/sharpness.json",
            )
            .expect("the game's own data pack holds sharpness");
            assert!(bytes.starts_with(b"{"));
            assert!(
                read(
                    game,
                    PackType::ServerData,
                    "minecraft:enchantment/not_a_real_enchantment.json"
                )
                .is_none()
            );
            // A server has no selected client asset stack to read.
            assert!(
                read(
                    game,
                    PackType::ClientResources,
                    "minecraft:models/block/stone.json"
                )
                .is_none()
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn server_data_reads_mod_content_and_lets_it_override_the_game() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let vanilla = read(
                game,
                PackType::ServerData,
                "minecraft:enchantment/sharpness.json",
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
                read(game, PackType::ServerData, "demo:machines/furnace.json").as_deref(),
                Some(b"{\"mod\":true}".as_slice())
            );
            // A later layer in the stack overrides the game's own data.
            assert_eq!(
                read(
                    game,
                    PackType::ServerData,
                    "minecraft:enchantment/sharpness.json"
                )
                .as_deref(),
                Some(b"{\"pack\":true}".as_slice())
            );
            assert_ne!(vanilla, b"{\"pack\":true}");
        });
    }

    #[test]
    #[ignore = "daily"]
    fn client_assets_read_the_winning_pack_file() {
        super::super::native_entity_refs::tests::with_client_game(|game| {
            let bytes = read(
                game,
                PackType::ClientResources,
                "minecraft:models/block/stone.json",
            )
            .expect("the vanilla asset pack holds stone's model");
            assert!(bytes.starts_with(b"{"));
            assert!(
                read(
                    game,
                    PackType::ClientResources,
                    "minecraft:models/block/not_a_real_model.json"
                )
                .is_none()
            );
        });
    }
}
