#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Dim, Game, creatures::Finalized};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::raids::Lvl;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::raids::PatrolCx;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::raids::Raiding;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::raids::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::{HashMap, HashSet};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::villager as mobs_villager;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    effects::{EffectInstance, OmenTick},
    lan::playerdata::ints_uuid,
    mobs::{
        Mob, Species, raider,
        spawn_rules::{self, SpawnLevel},
        spawn_selection::{Heightmap, NaturalLevel},
    },
    patrol::{GameplayAttributes, Member, PatrolLevel, PatrolSpawner},
    raid::{self, Action, BarName, RaidLevel, RaidPlayer, RaiderInfo, RaiderType, Raids, Spawn},
    random_source,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3,
    random::{LegacyRandom, LevelRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::custom_spawner::{
    section, sections_to_village,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::village_siege::{SiegeLevel, VillageSiege};

#[cfg(test)]
#[path = "tests/bar_players_client.rs"]
pub(in crate::game) mod bar_players_tests;
