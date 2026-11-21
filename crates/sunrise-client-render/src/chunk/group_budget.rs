//! Admission for one server connection's group data, shared by its active and viewed levels.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::Relaxed},
};

/// At most this many received group entries may be retained. This also leaves room for the
/// fixed number of in-flight mesh snapshots, which share a column's logical charge.
const LIMIT: usize = 1 << 20;

#[derive(Clone)]
pub struct GroupBudget {
    used: Arc<AtomicUsize>,
    limit: usize,
}

impl Default for GroupBudget {
    fn default() -> Self {
        Self {
            used: Arc::default(),
            limit: LIMIT,
        }
    }
}

impl GroupBudget {
    pub fn available(&self) -> usize {
        self.limit - self.used.load(Relaxed)
    }

    /// A reservation admitted by the decoder. The receive thread is the only one that charges;
    /// mesh workers can only release the last reference to a charge.
    pub fn reserve(&self, entries: usize) -> GroupReservation {
        assert!(entries <= self.available());
        self.used.fetch_add(entries, Relaxed);
        GroupReservation {
            used: self.used.clone(),
            entries,
        }
    }
}

/// Capacity charged to a decoded message or a deferred block correction until adopted.
pub struct GroupReservation {
    used: Arc<AtomicUsize>,
    entries: usize,
}

impl GroupReservation {
    pub fn take_one(&mut self) -> Self {
        self.entries -= 1;
        Self {
            used: self.used.clone(),
            entries: 1,
        }
    }

    pub fn extend(&mut self, mut other: Self) {
        debug_assert!(Arc::ptr_eq(&self.used, &other.used));
        self.entries += other.entries;
        other.entries = 0;
    }

    pub(super) fn move_to(&mut self, retained: &mut Option<Arc<RetainedGroups>>, count: usize) {
        if count == 0 {
            return;
        }
        self.entries -= count;
        let retained = retained.get_or_insert_with(|| {
            Arc::new(RetainedGroups {
                used: self.used.clone(),
                entries: AtomicUsize::default(),
            })
        });
        debug_assert!(Arc::ptr_eq(&self.used, &retained.used));
        retained.entries.fetch_add(count, Relaxed);
    }
}

impl Drop for GroupReservation {
    fn drop(&mut self) {
        self.used.fetch_sub(self.entries, Relaxed);
    }
}

/// The live column's group charge. Copies held by the bounded meshing pipeline share it; a
/// column replaced by a fresh packet has a fresh charge. Dropping or forgetting a world drops
/// its column charges without a separate relocation or unload ledger.
pub(super) struct RetainedGroups {
    used: Arc<AtomicUsize>,
    entries: AtomicUsize,
}

impl RetainedGroups {
    pub(super) fn remove_one(&self) {
        self.entries.fetch_sub(1, Relaxed);
        self.used.fetch_sub(1, Relaxed);
    }
}

impl Drop for RetainedGroups {
    fn drop(&mut self) {
        self.used.fetch_sub(*self.entries.get_mut(), Relaxed);
    }
}
