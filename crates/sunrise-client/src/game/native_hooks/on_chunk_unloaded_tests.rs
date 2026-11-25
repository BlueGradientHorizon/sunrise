//! `on_chunk_unloaded` through a real game (#3145): a chunk with two mod block entities and a
//! vanilla chest leaves the world when its `/forceload` ticket goes, through the game's own
//! streaming, save and unload; the chunk comes back from its saved data; then the world closes.
#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::{run_fresh_cases, with_game_at};
#[allow(unused_imports)]
use crate::game::world_loading::WorldRequest;
#[allow(unused_imports)]
use mod_api::{
    BlockEntityPart, BlockPart, BlocksEvent, Hooks, HostSide, OnChunkUnloadedEvent, Registrar,
    RegistrationContext,
};
#[allow(unused_imports)]
use std::path::Path;
#[allow(unused_imports)]
use std::sync::Mutex;
#[allow(unused_imports)]
use std::time::{Duration, Instant};
#[allow(unused_imports)]
use sunrise_client::containers::BlockEntity;
#[allow(unused_imports)]
use sunrise_core::nbt::{Compound, Tag};

pub(in crate::game) const POT: &str = "unload_probe:pot";
/// Chunk (100, 100): far from the spawn, so only its ticket keeps it.
pub(in crate::game) const CHUNK: (i32, i32) = (100, 100);
pub(in crate::game) const POTS: [[i32; 3]; 2] = [[1601, 100, 1602], [1610, 120, 1613]];
pub(in crate::game) const CHEST: [i32; 3] = [1605, 100, 1605];

/// What each call saw: the side, the position its handle names, its entity's id, and
/// whether the merge the part tried inside it was refused.
pub(in crate::game) static CALLS: Mutex<Vec<(&'static str, [i32; 3], String, bool)>> =
    Mutex::new(Vec::new());

pub(in crate::game) struct Pot;
#[mod_api::sunrise_part]
impl BlockPart for Pot {}
#[mod_api::sunrise_part]
impl BlockEntityPart for Pot {
    fn on_chunk_unloaded(&self, host: HostSide<'_>, event: &mut OnChunkUnloadedEvent<'_>) {
        let HostSide::Server(host) = host else {
            CALLS
                .lock()
                .unwrap()
                .push(("client", event.pos, event.entity.id().to_owned(), true));
            return;
        };
        let mut written = Compound::new();
        written.insert("written_on_unload".into(), Tag::Byte(1));
        let refused = event.block_entity.merge_data(host, written) != Ok(true);
        CALLS
            .lock()
            .unwrap()
            .push(("server", event.pos, event.entity.id().to_owned(), refused));
    }
}

pub(in crate::game) struct Probe;
#[mod_api::__subscriptions]
impl Hooks for Probe {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let kind = r.block_entity_type("pot").blocks(&[POT]).build();
        r.block_entity_part(&kind, Pot).unwrap();
    }
    fn blocks(&self, _: &RegistrationContext, event: &mut BlocksEvent<'_>) {
        event
            .register(mod_api::Block::new(POT, "minecraft:stone"))
            .unwrap();
    }
}

pub(in crate::game) fn request(world: &Path, create: bool) -> WorldRequest {
    let rules = std::sync::Arc::new(sunrise_client::gamerules::RuleRegistry::vanilla());
    let options = sunrise_client::title::WorldOptions {
        world_type: sunrise_client::save::WorldPreset::Flat,
        rules: sunrise_client::gamerules::GameRules::with_registry(rules.clone(), &[]),
        ..Default::default()
    };
    let mut request = WorldRequest::new(
        "unload",
        Some("unload"),
        Some(1),
        true,
        create.then_some(&options),
        rules,
    )
    .unwrap();
    request.save_root = world.to_owned();
    request
}

/// The prepared resources with the installed mods' content.
pub(in crate::game) fn prepared_root() -> std::path::PathBuf {
    let base = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
    crate::game::mods::content_root(&base, &Default::default())
        .unwrap()
        .prepared()
        .to_path_buf()
}

/// Runs the game's streaming until chunk `CHUNK` is resident (`true`) or gone, and
/// nothing is left on its way (a ticket removed mid-generation is another matter).
pub(in crate::game) fn pump_until(game: &mut crate::game::ServerGame<'_>, resident: bool) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while game.world.chunks.contains_key(&CHUNK) != resident || game.streaming.busy() {
        assert!(Instant::now() < deadline, "chunk {CHUNK:?} did not change");
        game.streaming.wait(Duration::from_millis(5));
        game.side_update_world();
    }
}

/// A client's copies: a column the server forgets tells its mod block entities, while
/// the client level going (a respawn, a dimension change, leaving) tells none.
#[test]
#[ignore = "daily"]
fn a_client_tells_a_forgotten_column_but_not_its_level_going() {
    use crate::game::execution_role::ExecutionRole;
    use sunrise_client::{chunk::Chunk, containers::ModBlockEntity};
    use sunrise_protocol::connection::Pipe;
    if run_fresh_cases(
        "game::native_hooks::on_chunk_unloaded_tests::a_client_tells_a_forgotten_column_but_not_its_level_going",
        "SUNRISE_ON_CHUNK_UNLOADED_CLIENT_CHILD",
        &["game"],
    ) {
        return;
    }
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new("unload_probe", Box::new(Probe))].into();
    mod_platform::__private::install(mods.clone());
    crate::game::api_values::install_parts(&mods).unwrap();
    with_game_at(prepared_root(), |game| {
        game.options.execution_role = ExecutionRole::Client;
        game.options.dedicated = None;
        let (client, _server) = Pipe::pair();
        game.connect_local(client, "UnloadProbe").unwrap();
        game.remote.as_mut().unwrap().logged_in = true;
        let other = [1620, 64, 1600];
        for (x, z) in [CHUNK, (101, 100)] {
            game.streaming
                .insert_remote(&mut game.world, Chunk::empty(x, z));
        }
        for [x, y, z] in POTS.into_iter().chain([other]) {
            let pot = ModBlockEntity::new(POT.into(), Default::default());
            game.world
                .block_entities
                .insert((x, y, z), BlockEntity::Mod(Box::new(pot)));
        }
        let [x, y, z] = CHEST;
        game.world
            .block_entities
            .insert((x, y, z), BlockEntity::Chest(vec![]));

        game.forget_remote_columns(&mut [CHUNK].into_iter().collect(), true);
        let mut calls = std::mem::take(&mut *CALLS.lock().unwrap());
        calls.sort();
        assert_eq!(
            calls,
            POTS.map(|pos| ("client", pos, POT.to_owned(), true))
                .to_vec()
        );

        game.forget_all_remote_columns();
        assert!(game.world.block_entities.is_empty());
        assert!(CALLS.lock().unwrap().is_empty(), "told when the level went");
    });
}
