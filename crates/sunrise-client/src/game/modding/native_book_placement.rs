//! Placement transport state belongs to the actual connection and open menu.
use super::*;
use std::collections::{BTreeMap, HashMap};
use sunrise_client::recipe_book::{
    ModBookIndex, placement_sync as sync, settings_codec,
    settings_sync::{CHANNEL, Message},
};
use sunrise_protocol::packets::{configuration::CustomPayload, play::Clientbound as C};

// no wire: live connection authority and validated engine presentation caches.
#[derive(Default)]
pub(super) struct State {
    pub(super) client: sync::Client,
    pub(super) servers: HashMap<u32, Server>,
    pub(super) view: Option<View>,
    pub(super) deadline: Option<std::time::Instant>,
    /// The placement a `recipe_book_place` part is answering, while its hook runs.
    pub(super) session: Option<super::native_book_grid::Session>,
}
// no wire: source ordinals actually published to this connection.
pub(super) struct Server {
    pub(super) peer: sync::Authority,
    pub(super) displays: BTreeMap<String, u32>,
}
impl Server {
    pub(super) fn new(epoch: u64) -> Self {
        Self {
            peer: sync::Authority::new(epoch),
            displays: BTreeMap::new(),
        }
    }
    pub(super) fn clear(&mut self) {
        self.peer.clear();
        self.displays.clear();
    }
}
// no wire: an installed, validated catalogue, not an untrusted transport body.
pub(super) struct View {
    pub(super) identity: sync::CatalogIdentity,
    pub(super) index: ModBookIndex,
}

impl Game<'_> {
    pub(super) fn send_guest_mod_book_placement(&self, conn: u32, frame: sync::Frame) {
        if let Some(guest) = self.lan.as_ref().and_then(|host| host.hub.guest(conn))
            && let Some(data) = settings_codec::encode(&Message::Placement(frame))
        {
            guest.send(&C::CustomPayload(CustomPayload {
                channel: CHANNEL.into(),
                data,
            }));
        }
    }
}
