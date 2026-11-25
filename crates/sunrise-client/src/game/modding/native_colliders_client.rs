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

impl Game<'_, crate::game::client_side::ClientState> {
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

    /// Where the collider entity `id` stands in the client's view.
    pub(in crate::game) fn client_collider_origin(&self, id: u32) -> Option<Vec3> {
        let epoch = self.native_lifecycle.epoch()?;
        let at = self.native_client_tracked(EntityRef::new(epoch, id))?.1;
        Some(Vec3::new(at[0], at[1], at[2]))
    }

    pub(in crate::game::client_side) fn client_vehicle_stack(&self, id: u32) -> Vec<u32> {
        let Some(remote) = &self.remote else {
            return vec![id];
        };
        stack_in_graph(&remote.passenger_vehicles, id)
    }

    /// The frame the collider of entity `id` has now on the client.
    pub(in crate::game) fn client_collider_now_frame(
        &self,
        id: u32,
    ) -> Option<sunrise_core::geometry::oriented::Frame> {
        let state = self.remote.as_ref()?.colliders.get(&id)?;
        Some(state.frame(self.client_collider_origin(id)?))
    }

    pub(in crate::game::client_side) fn client_each_collider(
        &self,
        mut f: impl FnMut(u32, &ColliderState) -> bool,
    ) -> bool {
        if let Some(remote) = &self.remote {
            for (&id, state) in &remote.colliders {
                if !f(id, state) {
                    return false;
                }
            }
        }
        true
    }

    pub(in crate::game::client_side) fn client_is_alone(&self, id: u32) -> bool {
        self.remote
            .as_ref()
            .is_none_or(|r| r.passenger_vehicle(id).is_none() && !r.has_passengers(id))
    }

    pub(in crate::game::client_side) fn client_stack_is(&self, id: u32, kept: &[u32]) -> bool {
        if kept == [id] {
            self.client_is_alone(id)
        } else {
            let mut now = self.client_vehicle_stack(id);
            let mut kept = kept.to_vec();
            now.sort_unstable();
            kept.sort_unstable();
            now == kept
        }
    }

    /// The colliders of the client's current level, where they stand now.
    pub(in crate::game) fn client_placed_moving_colliders(
        &mut self,
    ) -> sunrise_client_render::moving_colliders::Prepared {
        use sunrise_client_render::moving_colliders::{Placed, Prepared};
        let level = self.dim;
        let mut any = false;
        self.client_each_collider(|_, _| {
            any = true;
            false
        });
        if !any {
            self.collider_placements.0.remove(&level);
            return Prepared::empty();
        }
        if let Some(kept) = self.collider_placements.0.get(&level) {
            let mut at = 0;
            let same = self.client_each_collider(|id, state| {
                let key = kept.keys.get(at);
                at += 1;
                let Some(origin) = self.client_collider_origin(id) else {
                    return false;
                };
                key.is_some_and(|key| {
                    key.id == id
                        && key.serial == state.serial()
                        && key.origin == origin_bits(origin)
                        && self.client_stack_is(id, &key.stack)
                })
            });
            if same && at == kept.keys.len() {
                return kept.prepared.clone();
            }
        }
        let mut keys = Vec::new();
        let mut placed = Vec::new();
        self.client_each_collider(|id, state| {
            if let Some(origin) = self.client_collider_origin(id) {
                let stack = self.client_vehicle_stack(id);
                keys.push(PlacementKey {
                    id,
                    serial: state.serial(),
                    origin: origin_bits(origin),
                    stack: stack.clone(),
                });
                placed.push(Placed {
                    id,
                    collider: state.oriented(origin),
                    exempt: stack,
                });
            }
            true
        });
        let prepared = Prepared::new(placed);
        self.collider_placements.0.insert(
            level,
            Placement {
                keys,
                prepared: prepared.clone(),
            },
        );
        prepared
    }

    /// The client level in place for moving-collider scoping.
    pub(in crate::game) fn client_collider_level(&self) -> Option<mod_api::LevelRef> {
        self.native_client_level()
    }

    /// Runs `f` with the client's moving colliders and collision parts scoped (no history).
    pub(in crate::game) fn client_in_this_levels_colliders<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        use sunrise_client_render::moving_colliders::{HistoryScope, Scope};

        let level = self.client_collider_level();
        if level.is_some_and(Scope::covers) {
            return f(self);
        }
        let placed = self.client_placed_moving_colliders();
        let _parts = self.collision_parts();
        let _history = HistoryScope::replace(None);
        let _scope = Scope::enter_level(level, placed);
        f(self)
    }
}
