use crate::game::{Dim, Game, execution_role::ExecutionRole, native_entity_refs::tests::with_game};
use std::sync::Arc;
use sunrise_assets::resource_source::{Resource, ResourceSource};
use sunrise_client::environment::{Environment, PackModifiers, Sample};
use sunrise_protocol::{
    connection::Pipe,
    packets::play::{self, Clientbound as C},
};

/// The prepared resources' dimension type `name`, as a data pack writes it.
fn prepared_type(name: &str) -> serde_json::Value {
    let resources = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
    let path = format!("data/minecraft/dimension_type/{name}.json");
    serde_json::from_slice(&std::fs::read(resources.join(path)).unwrap()).unwrap()
}

fn spawn(dimension_type: i32, dimension: &str) -> play::SpawnInfo {
    play::SpawnInfo {
        dimension_type,
        dimension: dimension.into(),
        seed: 0,
        game_type: 1,
        previous_game_type: None,
        debug: false,
        flat: true,
        last_death: None,
        portal_cooldown: 0,
        sea_level: 63,
    }
}

/// The frame's sample at the camera: the dimension's value, as no biome sets the height.
fn drawn(game: &Game<'_>) -> sunrise_client::environment::Sky {
    game.view_environment().sample(&Sample::default())
}

/// The game's own Overworld, Nether and End stacks with a pack whose Overworld modifier adds a
/// post-process layer raising the cloud height by 10 over the value the layers before it gave.
fn pack_stacks(game: &Game<'_>) -> Arc<[Environment; 3]> {
    let pack = ResourceSource::indexed(
        [(
            "assets/minecraft/polytone/dimension_modifiers/overworld.json".to_owned(),
            Resource::bytes(
                br#"{"attributes_modifiers": {"post_process": {"visual/cloud_height": "v + 10"}}}"#
                    .to_vec(),
            ),
        )]
        .into_iter()
        .collect(),
    );
    let modifiers = PackModifiers::read(&pack);
    assert!(!modifiers.is_empty());
    let load = |d| Environment::load_dimension_with(&game.pack.data_source, d, &modifiers).unwrap();
    Arc::new([load("overworld"), load("the_nether"), load("the_end")])
}

/// A joined level's environment attributes start from the dimension type its server synced, as
/// `EnvironmentAttributeSystem.addDimensionLayer` takes `level.dimensionType().attributes()`, and
/// the packs' Polytone modifiers apply over them: a data pack's Overworld cloud height reaches
/// the frame on a client whose own resources say 192.33.
#[test]
#[ignore = "daily"]
fn a_joined_overworlds_synced_cloud_height_reaches_the_frame() {
    with_game(|game| {
        game.options.execution_role = ExecutionRole::Client;
        let (client, _server) = Pipe::pair();
        game.connect_local(client, "SyncedEnvironmentTest").unwrap();
        let mut overworld = prepared_type("overworld");
        overworld["attributes"]["minecraft:visual/cloud_height"] = serde_json::json!(250.0);
        let mut registries = (*game.remote.as_ref().unwrap().registries).clone();
        registries.registries.insert(
            "minecraft:dimension_type".into(),
            vec![
                (
                    "minecraft:overworld".into(),
                    Some(sunrise_client::scoreboard::json_to_tag(&overworld)),
                ),
                (
                    "minecraft:the_nether".into(),
                    Some(sunrise_client::scoreboard::json_to_tag(&prepared_type(
                        "the_nether",
                    ))),
                ),
                (
                    "test:high_sky".into(),
                    Some(sunrise_client::scoreboard::json_to_tag(&overworld)),
                ),
            ],
        );
        game.set_registries(Arc::new(registries));
        game.handle_packet(C::Login(play::Login {
            player_id: 7,
            hardcore: false,
            levels: vec!["minecraft:overworld".into(), "minecraft:the_nether".into()],
            max_players: 1,
            chunk_radius: 2,
            simulation_distance: 2,
            reduced_debug_info: false,
            show_death_screen: true,
            limited_crafting: false,
            spawn: spawn(0, "minecraft:overworld"),
            online_mode: false,
            enforces_secure_chat: false,
        }))
        .unwrap();
        // The client's own Overworld says 192.33; the server's type says 250.
        assert_eq!(
            game.res
                .environment()
                .sample(&Sample::default())
                .cloud_height,
            192.33
        );
        assert_eq!(drawn(game).cloud_height, 250.0);
        assert_eq!(
            game.dim_environment()
                .sample(&Sample::default())
                .cloud_height,
            250.0
        );
        // A parked Nether keeps the client's Nether (sky light level 4), not the joined
        // Overworld's type.
        assert_eq!(game.sky_darken_in(&game.world, Dim::Nether), 11);

        // A pack's modifier applies over the server's type: 250 + 10.
        game.pack.environments = Some(pack_stacks(game));
        let respawn = |game: &mut Game<'_>, spawn| {
            game.handle_packet(C::Respawn(play::Respawn { spawn, keep: 0 }))
                .unwrap();
        };
        respawn(game, spawn(0, "minecraft:overworld"));
        assert_eq!(drawn(game).cloud_height, 260.0);
        // Gameplay keeps the type without the packs.
        assert_eq!(
            game.dim_environment()
                .sample(&Sample::default())
                .cloud_height,
            250.0
        );
        // The packs have no stack of a mod's type: an Overworld of that type draws the type as
        // the server sent it, not the packs' Overworld.
        respawn(game, spawn(2, "minecraft:overworld"));
        assert_eq!(drawn(game).cloud_height, 250.0);
        respawn(game, spawn(0, "minecraft:overworld"));
        // When the packs change, the frame's environment is built again from the type.
        game.pack.environments = None;
        game.build_level_environments();
        assert_eq!(drawn(game).cloud_height, 250.0);

        // The Nether's synced type starts from the client's Nether, not its Overworld: its sky
        // colour is the Nether's, and the Overworld's would differ.
        respawn(game, spawn(1, "minecraft:the_nether"));
        let nether = game.res.nether_environment().sample(&Sample::default());
        let overworld_sky = game.res.environment().sample(&Sample::default()).sky_color;
        assert_ne!(nether.sky_color, overworld_sky);
        assert_eq!(drawn(game).sky_color, nether.sky_color);
        assert_eq!(game.sky_darken_in(&game.world, Dim::Nether), 11);
    });
}
