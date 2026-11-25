#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_book_placement::Server;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_book_placement::State;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_book_placement::View;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_book_placement::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::{BTreeMap, HashMap};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::recipe_book::{
    ModBookIndex, placement_sync as sync, settings_codec,
    settings_sync::{CHANNEL, Message},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::{
    configuration::CustomPayload, play::Clientbound as C,
};

impl Server {
    pub(in crate::game) fn new(epoch: u64) -> Self {
        Self {
            peer: sync::Authority::new(epoch),
            displays: BTreeMap::new(),
        }
    }
}
