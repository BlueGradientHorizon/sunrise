#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Dim, Game,
    commands::Target,
    remote_vehicles::{PassengerSeat, passenger_root},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::ColliderChange;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::ColliderEntry;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::EntityMaps;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::Histories;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::Placement;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::PlacementKey;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::Placements;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::SAVE_KEY;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::origin_bits;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::stack_in_graph;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_colliders::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{Collider, ColliderError, EntityRef};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::{HashMap, HashSet};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    colliders::{ColliderState, valid_rotation},
    fast_hash::FxHashMap,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3,
    nbt::{Compound, Tag},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_mod_runtime::attachments::{Attachments, Holder};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::collider_sync::{self, Message};

impl Game<'_> {
    /// A client forgets the collider of an entity the server stopped showing it.
    pub(in crate::game::client_side) fn forget_client_collider(&mut self, id: i32) {
        if let Some(remote) = self.remote.as_mut() {
            remote.colliders.remove(&(id as u32));
        }
    }
    /// A `sunrise:collider_sync` payload the server sent the window.
    pub(in crate::game) fn remote_collider_sync(&mut self, data: &[u8]) {
        let message = match collider_sync::decode(data) {
            Ok(message) => message,
            Err(e) => return log::debug!("Sunrise: a bad collider sync: {e}"),
        };
        let Some(remote) = self.remote.as_mut() else {
            return;
        };
        let entity = match message {
            Message::Boxes { entity, .. }
            | Message::Patch { entity, .. }
            | Message::Rotation { entity, .. }
            | Message::Clear { entity } => entity,
        };
        let mut held = remote.colliders.remove(&entity);
        ColliderState::apply(&mut held, &message);
        if let Some(state) = held {
            remote.colliders.insert(entity, state);
        }
    }
}
