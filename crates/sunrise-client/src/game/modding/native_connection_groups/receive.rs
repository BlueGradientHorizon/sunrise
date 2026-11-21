//! A group's block or column is the publication boundary, not the metadata packet preceding it.
use sunrise_client::{
    chunk::{
        Chunk,
        group_budget::{GroupBudget, GroupReservation},
    },
    fast_hash::FxHashMap,
    world::World,
};
use sunrise_protocol::connection_groups::{self, ChunkEntry, Message, UpdateEntry};

type Pos = [i32; 3];
type Column = (i32, i32);

pub(in crate::game) struct ReceivedGroup {
    group: Option<u64>,
    reservation: Option<GroupReservation>,
}

impl ReceivedGroup {
    pub(in crate::game) fn apply(self, world: &mut World, pos: Pos) {
        world.apply_received_group(pos, self.group, self.reservation);
    }
}

pub(in crate::game) struct GroupUpdates {
    entries: FxHashMap<Pos, Option<u64>>,
    reservation: GroupReservation,
}

impl GroupUpdates {
    fn new(entries: Vec<UpdateEntry>, budget: &GroupBudget) -> Self {
        let reservation =
            budget.reserve(entries.iter().filter(|entry| entry.group.is_some()).count());
        Self {
            entries: entries
                .into_iter()
                .map(|entry| (entry.pos, entry.group))
                .collect(),
            reservation,
        }
    }

    pub(in crate::game) fn take(&mut self, pos: Pos) -> Option<ReceivedGroup> {
        let group = self.entries.remove(&pos)?;
        Some(ReceivedGroup {
            group,
            reservation: group.map(|_| self.reservation.take_one()),
        })
    }

    pub(in crate::game) fn into_updates(self) -> impl Iterator<Item = (Pos, ReceivedGroup)> {
        let Self {
            entries,
            mut reservation,
        } = self;
        entries.into_iter().map(move |(pos, group)| {
            (
                pos,
                ReceivedGroup {
                    group,
                    reservation: group.map(|_| reservation.take_one()),
                },
            )
        })
    }
}

struct Snapshot {
    key: Column,
    entries: Vec<ChunkEntry>,
    reservation: GroupReservation,
}

enum Prefix {
    Column(Snapshot),
    Blocks(GroupUpdates),
}

/// A stream has at most one pending record. A viewed world owns its own stream, but every
/// stream of the connection charges the same budget. No state becomes visible here.
#[derive(Default)]
pub(in crate::game) struct GroupIngress(Option<Prefix>);

impl GroupIngress {
    pub(in crate::game) fn discard(&mut self) {
        self.0 = None;
    }

    pub(in crate::game) fn receive(
        &mut self,
        data: &[u8],
        budget: &GroupBudget,
    ) -> sunrise_protocol::Result<Option<GroupUpdates>> {
        let message = connection_groups::decode_with_budget(data, budget.available())?;
        match message {
            Message::Chunk {
                cx,
                cz,
                replace,
                entries,
            } => {
                let key = (cx, cz);
                let reservation = budget.reserve(entries.len());
                if replace {
                    self.0 = Some(Prefix::Column(Snapshot {
                        key,
                        entries,
                        reservation,
                    }));
                } else if let Some(Prefix::Column(snapshot)) = &mut self.0
                    && snapshot.key == key
                {
                    snapshot.entries.extend(entries);
                    snapshot.reservation.extend(reservation);
                } else {
                    return Err(sunrise_protocol::Error::new(
                        "connection groups: unexpected column continuation",
                    ));
                }
            }
            Message::Blocks { entries } => {
                self.0 = Some(Prefix::Blocks(GroupUpdates::new(entries, budget)));
            }
            Message::Update { entries } => {
                self.discard();
                return Ok(Some(GroupUpdates::new(entries, budget)));
            }
        }
        Ok(None)
    }

    pub(in crate::game) fn install_column(&mut self, key: Column, chunk: &mut Chunk) {
        if let Some(Prefix::Column(snapshot)) = self.0.take()
            && snapshot.key == key
        {
            chunk.install_group_snapshot(snapshot.entries, snapshot.reservation);
        }
    }

    pub(in crate::game) fn take_blocks(&mut self) -> Option<GroupUpdates> {
        match self.0.take() {
            Some(Prefix::Blocks(updates)) => Some(updates),
            _ => None,
        }
    }
}
