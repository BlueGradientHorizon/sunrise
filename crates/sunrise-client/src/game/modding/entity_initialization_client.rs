#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity_initialization::EntityInitialization;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity_initialization::TransferredEntityState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::entity_initialization::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::Holder;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::{HashMap, HashSet};

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn client_bind_constructed_entity_attachments(&mut self, _id: u32) {}

    pub(in crate::game::client_side) fn forget_remote_entity_construction(&mut self, id: i32) {
        self.forget_entity_initialization(Holder::Entity(id as u32));
        if let Some(uuid) = self
            .remote
            .as_ref()
            .and_then(|r| r.level_players.get(&id))
            .copied()
        {
            self.forget_entity_initialization(Holder::Player(uuid));
        }
    }
}
