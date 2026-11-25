#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, Lerp, Remote};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::general::general_type_catalog;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::general::RemoteGeneral;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::general::RemoteGenerals;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::general::tick_remote_general;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::general::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use indexmap::IndexMap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::HashMap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::join_seq::JoinSeq;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::{Aabb, Vec3};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_mod_runtime::attachments::{Attachments, Holder};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::general_entity::{self, Message};

impl Game<'_> {
    /// The server removed general row `id` from the window.
    pub(in crate::game::client_side) fn forget_remote_general(r: &mut Remote, id: i32) {
        r.general.rows.shift_remove(&id);
    }
    /// A `sunrise:general_entity` payload the server sent the window.
    pub(in crate::game) fn remote_general_entity(&mut self, data: &[u8]) {
        let message = match general_entity::decode(data) {
            Ok(message) => message,
            Err(e) => return log::debug!("Sunrise: a bad general entity spawn: {e}"),
        };
        let Message::Spawn {
            id,
            uuid,
            type_key,
            position,
            rotation,
            velocity: _,
        } = message;
        // Both sides froze the same declarations (general types come from both-side
        // packages): a key this client lacks comes from a server that does not match it.
        let Some(type_slot) = general_type_catalog().slot(&type_key) else {
            if let Some(r) = self.remote.as_mut()
                && r.general.unknown.insert(type_key.clone())
            {
                log::warn!(
                    "Sunrise: the server sent general entity type {type_key}, not installed here"
                );
            }
            return;
        };
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        if r.general.rows.contains_key(&id) {
            if !std::mem::replace(&mut r.general.duplicate, true) {
                log::warn!("Sunrise: the server sent general entity {id} twice; the first stays");
            }
            return;
        }
        let render_attachments = match u32::try_from(id)
            .ok()
            .and_then(|id| r.attachments.get(&Holder::Entity(id)))
        {
            Some(existing) => mod_api::__private::game::capture_entity_render_attachments(existing),
            None => {
                mod_api::__private::game::capture_entity_render_attachments(&Attachments::default())
            }
        };
        let row = RemoteGeneral::new(type_slot, uuid, position, rotation, render_attachments);
        self.persist
            .uuids
            .insert(id as u32, sunrise_client::lan::playerdata::uuid_ints(uuid));
        // Tracked: the id-keyed move, position sync, teleport and removal handlers apply.
        // Snapshot any old emitter body before publishing this new row.
        self.spawn_entity(
            id,
            &type_key,
            position,
            [rotation[0], rotation[1], rotation[0]],
        );
        if let Some(r) = self.remote.as_mut() {
            r.general.rows.insert(id, row);
            r.lerps.remove(&id);
        }
        self.pick_section_sync_id(id as u32);
    }

    /// `ClientLevel.tickPassenger` for the general rows in an applied passenger
    /// list. Seat a nested general vehicle before its own general passengers.
    pub(in crate::game) fn position_remote_general_passengers(&mut self) {
        let Some(remote) = self.remote.as_mut() else {
            return;
        };
        if remote.general.rows.is_empty() || remote.passenger_vehicles.is_empty() {
            return;
        }
        let mut riders = std::mem::take(&mut remote.general.riders);
        riders.clear();
        riders.extend(remote.passenger_vehicles.iter().filter_map(|(&id, seat)| {
            let id = id as i32;
            if !remote.general.rows.contains_key(&id) {
                return None;
            }
            let depth = std::iter::successors(Some(seat.vehicle), |&v| remote.passenger_vehicle(v))
                .take(remote.passenger_vehicles.len() + 1)
                .count();
            Some((depth, id, seat.vehicle, seat.index))
        }));
        riders.sort_unstable();
        for &(_, id, vehicle, _) in &riders {
            if let Some(at) = self.general_passenger_position(vehicle, id as u32)
                && let Some(remote) = self.remote.as_mut()
                && let Some(row) = remote.general.rows.get_mut(&id)
            {
                row.position = [at.x, at.y, at.z];
            }
        }
        if let Some(remote) = self.remote.as_mut() {
            remote.general.riders = riders;
        }
    }
}

impl RemoteGeneral {
    pub(in crate::game) fn new(
        type_slot: usize,
        uuid: u128,
        position: [f64; 3],
        rotation: [f32; 2],
        render_attachments: mod_api::EntityRenderAttachments,
    ) -> Self {
        Self {
            join: JoinSeq::next(),
            type_slot,
            uuid,
            position,
            previous: position,
            rotation,
            previous_rotation: rotation,
            age: 0,
            render_attachments,
        }
    }
    pub(in crate::game::client_side) fn publish_attachments(&mut self, attachments: &Attachments) {
        self.render_attachments =
            mod_api::__private::game::capture_entity_render_attachments(attachments);
    }
}

impl RemoteGenerals {
    /// Publish after the complete message has been applied.
    pub fn attachments_arrived(&mut self, id: i32, attachments: &Attachments) {
        if let Some(row) = self.rows.get_mut(&id) {
            row.publish_attachments(attachments);
        }
    }
    /// Forget every row: a world change or a respawn into another dimension.
    pub fn clear(&mut self) {
        self.rows.clear();
    }
}
