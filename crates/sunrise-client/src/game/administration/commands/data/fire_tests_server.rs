#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{DataRef, Target};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::data::fire_tests::Active;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::data::fire_tests::WitnessRow;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::data::fire_tests::parse_witness_26_3;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::data::fire_tests::read_witness;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::data::fire_tests::run_fire_changes_on_eyes_fangs_spawners_clouds_and_bullets;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::data::fire_tests::summon_target_at;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::data::fire_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::{
    bind_local_player, prepare_entity_ticking_neighborhood, with_game,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::HashSet;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::motion::PlayerMotion;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::position::ChunkPos;

pub(in crate::game) fn target_exists(game: &crate::game::ServerGame<'_>, target: Target) -> bool {
    match target {
        Target::Other(id) => game.side.others.0.iter().any(|o| o.id == id),
        Target::Fangs(id) => game.fangs.list.iter().any(|f| f.id == id),
        Target::Spawner(id) => game.ominous_spawners.iter().any(|s| s.id == id),
        Target::Cloud(id) => game.breath_clouds.iter().any(|c| c.id == id),
        Target::Projectile(id) => game.creatures.projectiles.iter().any(|p| p.id == id),
        _ => false,
    }
}

pub(in crate::game) fn tick_fire_entities(game: &mut crate::game::ServerGame<'_>) {
    game.side.ticked_entities_through = u32::MAX;
    game.tick_summoned_eyes();
    game.tick_fangs();
    game.tick_ominous_item_spawners();
    game.tick_potion_clouds();
    game.tick_projectiles_now().unwrap();
}
