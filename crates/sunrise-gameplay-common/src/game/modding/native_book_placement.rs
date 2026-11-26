//! Placement transport state belongs to the actual connection and open menu.
use std::collections::{BTreeMap, HashMap};
use sunrise_sim::recipe_book::{ModBookIndex, placement_sync as sync};

// no wire: live connection authority and validated engine presentation caches.
#[derive(Default)]
pub(in crate::game) struct State {
    pub client: sync::Client,
    pub servers: HashMap<u32, Server>,
    pub view: Option<View>,
    pub deadline: Option<std::time::Instant>,
}
// no wire: source ordinals actually published to this connection.
pub struct Server {
    pub(in crate::game) peer: sync::Authority,
    pub(in crate::game) displays: BTreeMap<String, u32>,
}
impl Server {
    pub fn new(epoch: u64) -> Self {
        Self {
            peer: sync::Authority::new(epoch),
            displays: BTreeMap::new(),
        }
    }

    pub(in crate::game) fn clear(&mut self) {
        self.peer.clear();
        self.displays.clear();
    }
}
// no wire: an installed, validated catalogue, not an untrusted transport body.
pub struct View {
    pub(in crate::game) identity: sync::CatalogIdentity,
    pub(in crate::game) index: ModBookIndex,
}
