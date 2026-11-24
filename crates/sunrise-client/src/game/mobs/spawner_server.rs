#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game,
    creatures::{Finalized, SpawnReason},
    persist::SpawnTree,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::spawner::Planned;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::spawner::Pos;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::spawner::SpawnerState;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::spawner::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::HashMap;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    Result,
    containers::BlockEntity,
    mobs::{
        Mob, Who,
        spawner::{SpawnDataExt, Spawner, SpawnerAdded, SpawnerEvent, SpawnerExt, SpawnerSpawn},
        trial_spawner::{TrialSpawnerExt, TrialState},
    },
    particles::{ParticleOptions, ParticleType},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_render::block_entity_reach::Reach;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::Vec3,
    random::{LegacyRandom, LevelRandom, RandomSource},
};

impl Game<'_> {
    /// `PlayerList.isOp` for the player acting now: a server's guest by their
    /// own level, else the window's player.
    pub(in crate::game) fn acting_player_is_op(&self) -> bool {
        self.server
            .lan_acting_permission
            .unwrap_or(self.ui.registries.permission)
            > 0
    }
}
