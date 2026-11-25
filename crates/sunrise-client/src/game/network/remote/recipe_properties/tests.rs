#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::{
    lan_actions::GuestCommand,
    native_entity_refs::tests::with_client_game,
    native_guest_turn_tests::with_guests,
    stations::{Kind, Station},
};
#[allow(unused_imports)]
use sunrise_client::lan::{Out, PlayerSender};
#[allow(unused_imports)]
use sunrise_mod_package::content::RECIPE_PRIORITIES_PATH;
#[allow(unused_imports)]
use sunrise_protocol::{
    buf::Reader,
    packets::recipes::{HolderSet, SlotDisplay, UpdateRecipes},
};

pub(in crate::game) fn cut_pack(root: &std::path::Path) -> impl Fn(&str, &str) {
    let pack = root.join("datapacks/cuts");
    let write = move |rel: &str, json: &str| {
        let path = pack.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, json).unwrap();
    };
    write(
        "pack.mcmeta",
        r#"{"pack":{"description":"fixture","min_format":94,"max_format":94}}"#,
    );
    std::fs::write(
        root.join("datapacks.json"),
        r#"{"enabled":["vanilla","file/cuts"],"disabled":[]}"#,
    )
    .unwrap();
    write
}

pub(in crate::game) fn place_stonecutter(server: &mut crate::game::ServerGame<'_>) -> Station {
    server
        .world
        .chunks
        .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
    let state = server
        .pack
        .assets
        .registry
        .default_state("minecraft:stonecutter")
        .unwrap();
    server.world.set(3, 64, 4, state).unwrap();
    Station::new(Kind::Stonecutter, [3, 64, 4])
}
