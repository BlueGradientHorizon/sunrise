use super::*;
use crate::game::resident_state::ResidentEntities;

impl Default for ResidentEntities {
    fn default() -> Self {
        Self {
            table: Box::new(RefCell::new(
                sunrise_client::join_seq::residency::table::EntityTable::new(Default::default()),
            )),
            local_player: Default::default(),
            knots: Default::default(),
            general: Default::default(),
        }
    }
}
