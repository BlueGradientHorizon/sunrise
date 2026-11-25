#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fluid_interactions::declared;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fluid_interactions::fluid_tag;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::fluid_interactions::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_items::items::Items;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_mod_package::content::{
    FluidInteraction,
    fluid_interaction::{BlockMatch, Neighbour},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::{
    fluid::FluidKind,
    level::{
        Behaviours,
        data::BlockSet,
        fluid_interactions::{
            BlockMatch as Resolved, FluidInteraction as Rule, FluidInteractions, Neighbour as Walk,
        },
    },
};

impl Game<'_, crate::game::server_side::ServerState> {
    /// Install the open world's fluid interactions, replacing the last load's: vanilla's two
    /// lava rules for each fluid of the world's `minecraft:lava` tag (water read from its
    /// `minecraft:water` tag; a data pack may edit both), then native mods' declarations, then
    /// content mods' files in load order and file name. Tags are read again on each call. An
    /// id that names no fluid or block refuses the world by name.
    pub(in crate::game) fn install_fluid_interactions(&self) -> Result<(), String> {
        let items = self
            .side.world_tables
            .0
            .as_ref()
            .map_or_else(|| self.items(), |t| &t.items);
        self.install_fluid_interactions_from(items)
    }
}
