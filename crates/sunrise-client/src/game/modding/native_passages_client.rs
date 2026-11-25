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

impl Game<'_, crate::game::client_side::ClientState> {
    /// The window forgot chunk `(x, z)`: the passages of that chunk go with it.
    pub(in crate::game::client_side) fn forget_chunk_passages(&mut self, x: i32, z: i32) {
        if !self.passages.held.is_empty() {
            self.passages.held.retain(|_, p| p.chunk() != [x, z]);
        }
    }

    /// A `sunrise:passage_sync` payload the server sent the window.
    pub(in crate::game) fn remote_passage_sync(&mut self, data: &[u8]) -> Vec<(u64, HeldRefusal)> {
        let message = match sunrise_protocol::passage_sync::decode(data) {
            Ok(message) => message,
            Err(e) => {
                log::debug!("Sunrise: a bad passage sync: {e}");
                return Vec::new();
            }
        };
        let mut refused = Vec::new();
        match message {
            Message::Add(forms) => {
                for f in forms {
                    if let Err(why) = self.hold_passage(&f) {
                        log::debug!("Sunrise: passage {} not kept: {why:?}", f.id);
                        refused.push((f.id, why));
                    }
                }
            }
            Message::Remove(ids) => {
                for id in ids {
                    self.passages.held.remove(&PassageId::from_raw(id));
                }
            }
            Message::Refused { id, reason } => self.remote_passage_refused(id, reason),
        }
        refused
    }

    pub(in crate::game::client_side) fn hold_passage(
        &mut self,
        f: &PassageForm,
    ) -> Result<(), HeldRefusal> {
        let p = passage(f);
        check(&p).map_err(HeldRefusal::Bad)?;
        let [x, z] = p.chunk();
        if !self.world.chunks.contains_key(&(x, z)) {
            return Err(HeldRefusal::ChunkNotHeld);
        }
        let held = &mut self.passages.held;
        let id = PassageId::from_raw(f.id);
        if !held.contains_key(&id) && held.len() >= MAX_PER_LEVEL as usize {
            return Err(HeldRefusal::Full);
        }
        held.insert(id, p);
        Ok(())
    }
}
