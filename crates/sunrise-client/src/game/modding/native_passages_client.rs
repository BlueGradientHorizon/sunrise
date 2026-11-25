#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::AXIS_TOLERANCE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::Dirty;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::HORIZONTAL_LIMIT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::HeldRefusal;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::LevelPassages;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::MAX_PER_CHUNK;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::MAX_PER_LEVEL;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::MAX_SIDE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::Passages;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::VERTICAL_LIMIT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::check;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::form;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::passage;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::turn_index;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_passages::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    DimensionKey, LevelRef, Passage, PassageId, PassageRegistrationError as Refused, PassageTurn,
    Vec3,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::fast_hash::FxHashMap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::passage_sync::{Message, PassageForm};

impl Game<'_> {
    /// The window forgot chunk `(x, z)`: the passages of that chunk go with it.
    pub(in crate::game::client_side) fn forget_chunk_passages(&mut self, x: i32, z: i32) {
        if !self.passages.held.is_empty() {
            self.passages.held.retain(|_, p| p.chunk() != [x, z]);
        }
    }
}
