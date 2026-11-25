use crate::game::native_book_placement::Server;
use std::collections::BTreeMap;
use sunrise_client::recipe_book::placement_sync as sync;

impl Server {
    pub(in crate::game) fn new(epoch: u64) -> Self {
        Self {
            peer: sync::Authority::new(epoch),
            displays: BTreeMap::new(),
        }
    }
}
