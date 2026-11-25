#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Remote;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_format::LevelEnvironments;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_format::LevelFormat;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_format::pack_environment;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_format::same_stacks;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_format::sane_facts;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::level_format::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{Dim, remote_chunks::ChunkFormat};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{Arc, Weak};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    chunk::LevelShape, environment::Environment,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::{packets::play, registry::DimensionType};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::dimension_type::DimensionFacts;

impl crate::game::Game<'_> {
    /// The client's table of the server's levels on entering `entered`: a level past vanilla's
    /// three that the process does not know yet is added, so that [`Dim::from_id`] finds it, and
    /// the level left is dropped when the server opened it after Login (a runtime level), so that a
    /// long session through many of them keeps none. A viewed world of such a level needs no
    /// entry: it keeps its own name. Only a level this connection added leaves the table: in single
    /// player the integrated server shares the table, and a runtime level it opened is its own
    /// until it closes it (`runtime_levels.rs`).
    pub(in crate::game::client_side) fn track_remote_levels(&mut self, entered: &str) {
        if let Some(r) = self.remote.as_mut()
            && r.level != entered
            && !Dim::is_vanilla(&r.level)
            && !r.levels.contains(&r.level)
        {
            if r.added_levels.remove(&r.level) {
                Dim::remove_extra(&r.level);
            }
            r.facts_cache.remove(&r.level);
        }
        if !Dim::is_vanilla(entered)
            && !Dim::exists(entered)
            && let Some(r) = self.remote.as_mut()
        {
            Dim::add_extra(entered);
            r.added_levels.insert(entered.to_owned());
        }
    }
}
