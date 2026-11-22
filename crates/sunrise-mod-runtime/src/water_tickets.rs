//! Session-only, chunk-indexed watered areas. Mod switches eagerly release attributed boxes.
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static LEVELS: Mutex<Vec<Weak<Shared>>> = Mutex::new(Vec::new());

/// Maximum columns indexed by one ticket, bounding a synchronous mod call's memory and work.
pub const MAX_CHUNKS: u64 = 65_536;

/// Opaque, non-reused identity for one watered-area ticket, issued only by [`WaterTickets::add`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WaterTicketId(u64);

struct Ticket {
    owner: Option<&'static str>,
    min: [i32; 3],
    max: [i32; 3],
}
#[derive(Default)]
struct Index {
    tickets: HashMap<WaterTicketId, Ticket>,
    chunks: HashMap<[i32; 2], Vec<WaterTicketId>>,
}
struct Shared {
    nonempty: AtomicBool,
    index: Mutex<Index>,
}

/// A level's transient watered areas. Empty levels allocate nothing and queries only test the
/// empty-index guard; neither queries nor farmland ticks call mod code.
#[derive(Default)]
pub struct WaterTickets {
    shared: Option<Arc<Shared>>,
}

impl WaterTickets {
    /// Add a half-open integer box (`min <= pos < max`) and issue its opaque [`WaterTicketId`].
    /// Empty/reversed boxes and boxes spanning more than [`MAX_CHUNKS`] columns are refused.
    /// `owner` is evaluated under the lock shared with owner-disable cleanup to capture optional
    /// cleanup attribution (`Some(id)` for an enabled installed mod, `None` otherwise); it never
    /// denies admission.
    pub fn add(
        &mut self,
        min: [i32; 3],
        max: [i32; 3],
        owner: impl FnOnce() -> Option<&'static str>,
    ) -> Option<WaterTicketId> {
        if (0..3).any(|axis| min[axis] >= max[axis]) {
            return None;
        }
        let from = [min[0] >> 4, min[2] >> 4];
        let to = [(max[0] - 1) >> 4, (max[2] - 1) >> 4];
        let columns = (i64::from(to[0]) - i64::from(from[0]) + 1) as u64
            * (i64::from(to[1]) - i64::from(from[1]) + 1) as u64;
        if columns > MAX_CHUNKS {
            return None;
        }
        // Serialize optional cleanup attribution with owner-disable cleanup so a concurrent
        // disable cannot leave an attributed ticket behind without removing it.
        let mut levels = LEVELS.lock().unwrap();
        let owner = owner();
        let id = NEXT_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .ok()
            .map(WaterTicketId)?;
        let shared = self.shared.get_or_insert_with(|| {
            let shared = Arc::new(Shared {
                nonempty: AtomicBool::new(false),
                index: Mutex::new(Index::default()),
            });
            levels.retain(|level| level.strong_count() != 0);
            levels.push(Arc::downgrade(&shared));
            shared
        });
        let mut index = shared.index.lock().unwrap();
        index.tickets.insert(id, Ticket { owner, min, max });
        for x in from[0]..=to[0] {
            for z in from[1]..=to[1] {
                index.chunks.entry([x, z]).or_default().push(id);
            }
        }
        shared.nonempty.store(true, Ordering::Release);
        Some(id)
    }

    /// Remove the ticket with this issued identity; overlapping boxes remain independent.
    pub fn remove(&mut self, id: WaterTicketId) -> bool {
        let Some(shared) = &self.shared else {
            return false;
        };
        let mut index = shared.index.lock().unwrap();
        let Some(ticket) = index.tickets.remove(&id) else {
            return false;
        };
        for x in (ticket.min[0] >> 4)..=((ticket.max[0] - 1) >> 4) {
            for z in (ticket.min[2] >> 4)..=((ticket.max[2] - 1) >> 4) {
                if let Some(ids) = index.chunks.get_mut(&[x, z]) {
                    ids.retain(|other| *other != id);
                    if ids.is_empty() {
                        index.chunks.remove(&[x, z]);
                    }
                }
            }
        }
        shared
            .nonempty
            .store(!index.tickets.is_empty(), Ordering::Release);
        true
    }

    /// Test only the position's indexed column, without allocating or invoking a mod callback.
    pub fn contains(&self, pos: [i32; 3]) -> bool {
        let Some(shared) = &self.shared else {
            return false;
        };
        if !shared.nonempty.load(Ordering::Acquire) {
            return false;
        }
        let index = shared.index.lock().unwrap();
        index
            .chunks
            .get(&[pos[0] >> 4, pos[2] >> 4])
            .is_some_and(|ids| {
                ids.iter().any(|id| {
                    let ticket = &index.tickets[id];
                    (0..3).all(|axis| ticket.min[axis] <= pos[axis] && pos[axis] < ticket.max[axis])
                })
            })
    }

    /// Closing a level invalidates all its handles, even if the same level object is reused.
    pub fn clear(&mut self) {
        if let Some(shared) = self.shared.take() {
            shared.nonempty.store(false, Ordering::Release);
        }
    }
}

/// Called at a mod's disable boundary, not on a tick. Weak registrations do not keep worlds alive.
/// Releases only tickets attributed to `owner`; anonymous and other mods' tickets stay intact.
pub fn remove_owner(owner: &str) {
    LEVELS.lock().unwrap().retain(|level| {
        let Some(shared) = level.upgrade() else {
            return false;
        };
        let mut index = shared.index.lock().unwrap();
        let Index { tickets, chunks } = &mut *index;
        tickets.retain(|_, ticket| ticket.owner != Some(owner));
        chunks.retain(|_, ids| {
            ids.retain(|id| tickets.contains_key(id));
            !ids.is_empty()
        });
        shared
            .nonempty
            .store(!tickets.is_empty(), Ordering::Release);
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_open_boxes_cross_negative_chunk_edges_and_remove_independently() {
        let mut level = WaterTickets::default();
        assert!(!level.contains([0, 64, 0]));
        let a = level
            .add([-17, 64, -1], [18, 65, 2], || Some("irrigator"))
            .unwrap();
        for pos in [[-17, 64, -1], [-16, 64, 0], [0, 64, 0], [17, 64, 1]] {
            assert!(level.contains(pos), "{pos:?}");
        }
        for pos in [
            [-18, 64, 0],
            [18, 64, 0],
            [0, 63, 0],
            [0, 65, 0],
            [0, 64, -2],
            [0, 64, 2],
        ] {
            assert!(!level.contains(pos), "{pos:?}");
        }
        let anon = level.add([0, 64, 0], [5, 65, 5], || None).unwrap();
        let b = level
            .add([2, 64, 2], [6, 65, 6], || Some("neighbor"))
            .unwrap();
        assert!(level.remove(a));
        assert!(!level.remove(a));
        assert!(!level.contains([-17, 64, 0]));
        assert!(level.contains([0, 64, 0]));
        assert!(level.contains([5, 64, 5]));
        // Unrelated owner cleanup leaves anonymous and separately attributed overlapping rows.
        remove_owner("irrigator");
        assert!(level.contains([0, 64, 0]));
        assert!(level.contains([5, 64, 5]));
        assert!(level.remove(anon));
        assert!(!level.remove(anon));
        assert!(!level.contains([0, 64, 0]));
        assert!(level.contains([2, 64, 2]));
        assert!(level.remove(b));
        assert!(!level.remove(b));
        assert!(!level.contains([2, 64, 2]));
    }

    #[test]
    fn owner_disable_releases_all_levels_without_resurrecting_old_handles() {
        let mut overworld = WaterTickets::default();
        let mut nether = WaterTickets::default();
        let old = overworld
            .add([0, 64, 0], [5, 65, 5], || Some("water_test_owner"))
            .unwrap();
        let anon = overworld.add([1, 64, 1], [2, 65, 2], || None).unwrap();
        assert!(!nether.contains([0, 64, 0]));
        let nether_owned = nether
            .add([0, 64, 0], [5, 65, 5], || Some("water_test_owner"))
            .unwrap();
        let nether_other = nether
            .add([3, 64, 3], [4, 65, 4], || Some("water_test_other"))
            .unwrap();
        remove_owner("water_test_owner");
        assert!(!overworld.contains([0, 64, 0]));
        assert!(overworld.contains([1, 64, 1]));
        assert!(!nether.contains([0, 64, 0]));
        assert!(nether.contains([3, 64, 3]));
        assert!(!overworld.remove(old));
        assert!(!nether.remove(nether_owned));
        // A disabled-state observation admits an unattributed ticket without reviving old keys.
        let unattributed = overworld.add([4, 64, 4], [5, 65, 5], || None).unwrap();
        assert_ne!(old, unattributed);
        assert!(overworld.contains([4, 64, 4]));
        assert!(!overworld.contains([0, 64, 0]));
        assert!(!overworld.remove(old));
        let new = overworld
            .add([0, 64, 0], [5, 65, 5], || Some("water_test_owner"))
            .unwrap();
        assert_ne!(old, new);
        assert!(!overworld.remove(old));
        assert!(overworld.contains([0, 64, 0]));
        assert!(overworld.remove(anon));
        assert!(overworld.remove(unattributed));
        overworld.clear();
        assert!(!overworld.contains([0, 64, 0]));
        assert!(!overworld.remove(new));
        assert!(nether.remove(nether_other));
        assert!(!nether.contains([3, 64, 3]));
        let reopened = WaterTickets::default();
        assert!(!reopened.contains([0, 64, 0]));
    }

    #[test]
    fn a_box_of_exactly_the_column_limit_is_accepted_and_one_column_over_is_refused() {
        // 256 x 256 chunk columns, 4096 x 4096 blocks: exactly MAX_CHUNKS.
        let mut level = WaterTickets::default();
        assert_eq!(MAX_CHUNKS, 256 * 256);
        let id = level.add([0, 64, 0], [4096, 65, 4096], || Some("limit"));
        assert!(id.is_some());
        assert!(level.contains([4095, 64, 4095]));
        assert!(!level.contains([4096, 64, 4095]));
        // One block further east is a 257th column of chunks: 257 x 256 columns.
        let mut over = WaterTickets::default();
        assert!(
            over.add([0, 64, 0], [4097, 65, 4096], || Some("limit"))
                .is_none()
        );
        assert!(over.shared.is_none());
    }

    #[test]
    fn invalid_boxes_do_not_create_an_index() {
        let mut level = WaterTickets::default();
        assert!(
            level
                .add([0, 64, 0], [0, 65, 5], || Some("invalid"))
                .is_none()
        );
        assert!(
            level
                .add([5, 64, 0], [0, 65, 5], || Some("invalid"))
                .is_none()
        );
        assert!(
            level
                .add([i32::MIN, 0, i32::MIN], [i32::MAX, 1, i32::MAX], || Some(
                    "invalid"
                ),)
                .is_none()
        );
        assert!(level.shared.is_none());
    }
}
