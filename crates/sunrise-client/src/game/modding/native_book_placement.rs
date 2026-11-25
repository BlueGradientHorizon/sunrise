//! Placement transport state belongs to the actual connection and open menu.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, HashMap};
#[allow(unused_imports)]
use sunrise_client::recipe_book::{
    ModBookIndex, placement_sync as sync, settings_codec,
    settings_sync::{CHANNEL, Message},
};
#[allow(unused_imports)]
use sunrise_protocol::packets::{configuration::CustomPayload, play::Clientbound as C};

// no wire: live connection authority and validated engine presentation caches.
#[derive(Default)]
pub(in crate::game) struct State {
    pub(in crate::game) client: sync::Client,
    pub(in crate::game) servers: HashMap<u32, Server>,
    pub(in crate::game) view: Option<View>,
    pub(in crate::game) deadline: Option<std::time::Instant>,
    /// The placement a `recipe_book_place` part is answering, while its hook runs.
    pub(in crate::game) session: Option<super::native_book_grid::Session>,
}
// no wire: source ordinals actually published to this connection.
pub(in crate::game) struct Server {
    pub(in crate::game) peer: sync::Authority,
    pub(in crate::game) displays: BTreeMap<String, u32>,
}
impl Server {
    pub(in crate::game) fn clear(&mut self) {
        self.peer.clear();
        self.displays.clear();
    }
}
// no wire: an installed, validated catalogue, not an untrusted transport body.
pub(in crate::game) struct View {
    pub(in crate::game) identity: sync::CatalogIdentity,
    pub(in crate::game) index: ModBookIndex,
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn send_guest_mod_book_placement(&self, conn: u32, frame: sync::Frame) {
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
