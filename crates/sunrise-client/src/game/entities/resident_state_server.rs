#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::resident_state::ResidentEntities;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::resident_state::*;

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
