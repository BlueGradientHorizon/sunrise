//! Connection-group sync: per-position `u64` connection groups published by mods
//! (`LevelRef::set_connection_group`), sent by a Sunrise server to the Sunrise clients that hold
//! the chunk.
//!
//! A chunk snapshot precedes its column packet. A block-attached delta precedes its single or
//! section block update; receivers apply the metadata only with the packet it describes.
//! Independent updates apply to a block already published to the client. A vanilla client
//! never offers the channel and receives nothing.
//!
//! The channel is an engine declaration in the negotiated payload table: only a peer that offers
//! it at [`VERSION`] is sent anything.
use crate::{
    Error, Result,
    buf::{Reader, Writer},
};

/// The custom payload channel, clientbound only.
pub const CHANNEL: &str = "sunrise:connection_groups";
/// The channel's exact declaration version in the negotiated table.
pub const VERSION: &str = "1";
/// Maximum entries carried in a single message (fits well under the 1 MiB clientbound cap).
pub const MAX_ENTRIES: usize = 16_384;

/// A single entry in a chunk-scoped connection-group snapshot:
/// `(local_x 0..16, world_y, local_z 0..16, group)`.
///
/// Local `x` and `z` are stored as packed 4-bit nibbles (`(local_x << 4) | local_z`) so every
/// `ChunkEntry` guarantees `local_x < 16 && local_z < 16` by construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkEntry {
    xz: u8,
    y: i32,
    group: u64,
}

impl ChunkEntry {
    /// Construct an entry, refusing `local_x >= 16` or `local_z >= 16`.
    pub fn new(local_x: u8, y: i32, local_z: u8, group: u64) -> Result<Self> {
        if local_x >= 16 || local_z >= 16 {
            return Err(Error::new(format!(
                "connection groups: local coord ({local_x}, {local_z}) out of 0..16"
            )));
        }
        Ok(Self {
            xz: (local_x << 4) | local_z,
            y,
            group,
        })
    }

    /// Construct an entry from already-packed 4-bit nibbles `(local_x << 4) | local_z`.
    #[inline]
    pub const fn from_packed(xz: u8, y: i32, group: u64) -> Self {
        Self { xz, y, group }
    }

    /// Chunk-local X coordinate (`0..16`).
    #[inline]
    pub const fn local_x(self) -> u8 {
        self.xz >> 4
    }

    /// World Y coordinate.
    #[inline]
    pub const fn y(self) -> i32 {
        self.y
    }

    /// Chunk-local Z coordinate (`0..16`).
    #[inline]
    pub const fn local_z(self) -> u8 {
        self.xz & 15
    }

    /// Mod-published connection group identifier.
    #[inline]
    pub const fn group(self) -> u64 {
        self.group
    }
}

/// An incremental group update at a world block position: `Some(g)` sets, `None` clears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateEntry {
    pub pos: [i32; 3],
    pub group: Option<u64>,
}

/// What the server tells a client about connection groups.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// Begin (`replace: true`) or continue a snapshot for the next column packet of `(cx, cz)`.
    Chunk {
        cx: i32,
        cz: i32,
        replace: bool,
        entries: Vec<ChunkEntry>,
    },
    /// Independent updates or clears, for blocks whose state is already published.
    Update { entries: Vec<UpdateEntry> },
    /// Metadata for the next `BlockUpdate` or `SectionBlocksUpdate` in this level.
    Blocks { entries: Vec<UpdateEntry> },
}

const KIND_CHUNK: u8 = 0;
const KIND_UPDATE: u8 = 1;
const KIND_BLOCKS: u8 = 2;

/// Encode one message. Refuses lists longer than [`MAX_ENTRIES`].
pub fn encode(message: &Message) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    match message {
        Message::Chunk {
            cx,
            cz,
            replace,
            entries,
        } => {
            if entries.len() > MAX_ENTRIES {
                return Err(Error::new(format!(
                    "connection groups: {} chunk entries exceeds max {MAX_ENTRIES}",
                    entries.len()
                )));
            }
            w.u8(KIND_CHUNK);
            w.i32(*cx);
            w.i32(*cz);
            w.bool(*replace);
            w.varint(entries.len() as i32);
            for e in entries {
                w.u8(e.xz);
                w.varint(e.y);
                w.i64(e.group as i64);
            }
        }
        Message::Update { entries } | Message::Blocks { entries } => {
            if entries.len() > MAX_ENTRIES {
                return Err(Error::new(format!(
                    "connection groups: {} update entries exceeds max {MAX_ENTRIES}",
                    entries.len()
                )));
            }
            w.u8(if matches!(message, Message::Blocks { .. }) {
                KIND_BLOCKS
            } else {
                KIND_UPDATE
            });
            w.varint(entries.len() as i32);
            for e in entries {
                w.i32(e.pos[0]);
                w.varint(e.pos[1]);
                w.i32(e.pos[2]);
                match e.group {
                    Some(g) => {
                        w.bool(true);
                        w.i64(g as i64);
                    }
                    None => w.bool(false),
                }
            }
        }
    }
    Ok(w.data)
}

/// Decode one message, rejecting unknown tags, oversized counts, and trailing bytes.
pub fn decode(data: &[u8]) -> Result<Message> {
    decode_with_budget(data, MAX_ENTRIES)
}

/// Decode with the connection's remaining capacity for retained groups. Clears require no
/// capacity. Refuse at the count boundary for snapshots and before retaining a set in a delta.
/// The caller owns the connection's budget and charges the admitted groups to it.
pub fn decode_with_budget(data: &[u8], mut available: usize) -> Result<Message> {
    let mut r = Reader::new(data);
    let kind = r.u8()?;
    let msg = match kind {
        KIND_CHUNK => {
            let cx = r.i32()?;
            let cz = r.i32()?;
            let replace = r.bool()?;
            let count = r.count(MAX_ENTRIES.min(available))?;
            let mut entries = Vec::with_capacity(count.min(256));
            for _ in 0..count {
                let xz = r.u8()?;
                let y = r.varint()?;
                let group = r.i64()? as u64;
                entries.push(ChunkEntry::from_packed(xz, y, group));
            }
            Message::Chunk {
                cx,
                cz,
                replace,
                entries,
            }
        }
        KIND_UPDATE | KIND_BLOCKS => {
            let count = r.count(MAX_ENTRIES)?;
            let mut entries = Vec::with_capacity(count.min(256));
            for _ in 0..count {
                let x = r.i32()?;
                let y = r.varint()?;
                let z = r.i32()?;
                let group = if r.bool()? {
                    available = available
                        .checked_sub(1)
                        .ok_or_else(|| Error::new("connection groups: receive budget exhausted"))?;
                    Some(r.i64()? as u64)
                } else {
                    None
                };
                entries.push(UpdateEntry {
                    pos: [x, y, z],
                    group,
                });
            }
            if kind == KIND_BLOCKS {
                Message::Blocks { entries }
            } else {
                Message::Update { entries }
            }
        }
        other => {
            return Err(Error::new(format!(
                "connection groups: unknown message kind {other}"
            )));
        }
    };
    if r.remaining() != 0 {
        return Err(Error::new(format!(
            "connection groups: {} trailing bytes",
            r.remaining()
        )));
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_and_update_round_trip() {
        let chunk = Message::Chunk {
            cx: -3,
            cz: 7,
            replace: true,
            entries: vec![
                ChunkEntry::new(0, -64, 15, 0).unwrap(),
                ChunkEntry::new(15, 319, 0, u64::MAX).unwrap(),
            ],
        };
        assert_eq!(decode(&encode(&chunk).unwrap()).unwrap(), chunk);
        let decoded = decode(&encode(&chunk).unwrap()).unwrap();
        let Message::Chunk { entries, .. } = decoded else {
            panic!("expected chunk message");
        };
        assert_eq!(
            (
                entries[0].local_x(),
                entries[0].y(),
                entries[0].local_z(),
                entries[0].group()
            ),
            (0, -64, 15, 0)
        );
        assert_eq!(
            (
                entries[1].local_x(),
                entries[1].y(),
                entries[1].local_z(),
                entries[1].group()
            ),
            (15, 319, 0, u64::MAX)
        );

        let update = Message::Update {
            entries: vec![
                UpdateEntry {
                    pos: [-100, 64, 200],
                    group: Some(0),
                },
                UpdateEntry {
                    pos: [-100, 65, 200],
                    group: None,
                },
            ],
        };
        assert_eq!(decode(&encode(&update).unwrap()).unwrap(), update);
    }

    #[test]
    fn rejects_trailing_bytes_unknown_kind_and_oversized_counts() {
        let mut bytes = encode(&Message::Update { entries: vec![] }).unwrap();
        bytes.push(0);
        assert!(decode(&bytes).is_err());

        assert!(decode(&[99]).is_err());
        assert!(decode(&[]).is_err());

        let mut oversized = Writer::new();
        oversized.u8(KIND_UPDATE);
        oversized.varint((MAX_ENTRIES + 1) as i32);
        assert!(decode(&oversized.data).is_err());

        assert!(ChunkEntry::new(16, 0, 0, 1).is_err());
        assert!(ChunkEntry::new(0, 0, 16, 1).is_err());
    }
}
