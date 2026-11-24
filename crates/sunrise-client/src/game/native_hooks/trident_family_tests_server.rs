#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::api_values::TryIntoApi;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::{
    bind_local_player, with_game,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::trident_family_tests::Hit;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::trident_family_tests::Probe;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::trident_family_tests::Seen;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::trident_family_tests::step;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::trident_family_tests::throw_mob;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::trident_family_tests::throw_player;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::trident_family_tests::victim;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::trident_family_tests::weapon;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::trident_family_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    Flow, HitHost, Hooks, HurtSource, LivingDeathEvent, LivingIncomingDamageEvent, ModEntry,
    ServerHost,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    chunk::Chunk,
    items::ItemStack,
    mobs::{Mob, Who},
    motion::PlayerMotion,
    projectiles::{Projectile, ProjectileKind},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{geometry::Vec3, nbt::Tag, position::ChunkPos};

pub(in crate::game) fn fixture(run: impl FnOnce(&mut Game<'_>, &Arc<Mutex<Seen>>)) {
    crate::entry::install_client_hooks();
    let seen = Arc::new(Mutex::new(Seen::default()));
    mod_platform::testing::with_mods(
        vec![ModEntry::new(
            "trident-family",
            Box::new(Probe(seen.clone())),
        )]
        .into(),
        || {
            with_game(|game| {
                game.build_registries();
                game.load_server_loot();
                game.set_allow_commands(true);
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                game.block_ticks
                    .add_empty_chunk(ChunkPos::new(0, 0))
                    .unwrap();
                assert!(game.run_command("fill 0 79 0 15 79 15 stone"));
                game.player =
                    Some(PlayerMotion::new(game.motion, Vec3::new(4.5, 80.0, 4.5)).unwrap());
                bind_local_player(game, 11, 123);
                game.set_mode_id(0);
                game.server.next_entity = 100;
                run(game, &seen);
            })
        },
    );
}
