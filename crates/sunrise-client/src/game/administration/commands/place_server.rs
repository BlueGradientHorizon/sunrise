#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Arg, Context, Game, R, err};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::place::num;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::place::read_template_data;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::place::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::structure_blocks::{
    load_settings, template_size,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::OnceLock;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::text::Text;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
pub(in crate::game::server_side) use {
    sunrise_core::{
        nbt,
        nbt::{Limits, Tag},
    },
    sunrise_world::{
        compression::Compression,
        worldgen::structure::{Mirror, Rotation, StateTransforms},
    },
};

impl Game<'_, crate::game::server_side::ServerState> {
    /// `PlaceCommand.checkLoaded`: every chunk of the inclusive range.
    pub(in crate::game::server_side) fn place_check_loaded(
        &self,
        min: [i32; 2],
        max: [i32; 2],
    ) -> R<()> {
        let dimension = self.side.command_source.map_or(self.dim, |s| s.dimension);
        for cx in min[0]..=max[0] {
            for cz in min[1]..=max[1] {
                if dimension != self.dim || !self.world.chunks.contains_key(&(cx, cz)) {
                    return Err(err("argument.pos.unloaded", &[]));
                }
            }
        }
        Ok(())
    }
    /// A saved world template, or the winning passive/world-pack resource.
    pub(in crate::game::server_side) fn read_template_tag(&self, id: &str) -> Option<Tag> {
        use sunrise_assets::resource_source::{Resource, stays_inside};
        let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
        if !stays_inside(ns) || !stays_inside(path) {
            return None;
        }
        let saved = self
            .side
            .world_root
            .join("generated")
            .join(ns)
            .join("structures")
            .join(format!("{path}.nbt"));
        let prepared = self
            .options
            .resources
            .join("worldgen/structure")
            .join(ns)
            .join(format!("{path}.nbt"));
        let name = format!("data/{ns}/structure/{path}.nbt");
        let resource = self.side.fns.0.data_resource(&name).or_else(|| {
            self.pack
                .data_source
                .contains(&name)
                .then(|| Resource::file(self.pack.data_source.clone(), name))
        });
        read_template_data(&saved, &prepared, resource)
    }
}
