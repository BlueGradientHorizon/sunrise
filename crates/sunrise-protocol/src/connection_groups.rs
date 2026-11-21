//! Connection-group sync: per-position `u64` connection groups published by mods
//! (`LevelRef::set_connection_group`), sent by a Sunrise server to the Sunrise clients that hold
//! the chunk.
//!
//! When a chunk is sent to a client, any non-empty group map in that chunk follows in one
//! [`Message::Chunk`] (split into batches of at most [`MAX_ENTRIES`] if needed, with `replace`
//! on the first batch only). Incremental edits and clears follow as [`Message::Update`]. A
//! vanilla client never offers the channel and receives nothing.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkEntry {
    pub local_x: u8,
    pub y: i32,
    pub local_z: u8,
    pub group: u64,
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
    /// Snapshot for one chunk `(cx, cz)`. When `replace` is true, replaces any previously stored
    /// groups in that chunk; otherwise merges into them (for multi-part chunk snapshots).
    Chunk {
        cx: i32,
        cz: i32,
        replace: bool,
        entries: Vec<ChunkEntry>,
    },
    /// Incremental updates or clears at world block positions.
    Update { entries: Vec<UpdateEntry> },
}

const KIND_CHUNK: u8 = 0;
const KIND_UPDATE: u8 = 1;

/// Encode one message. Refuses lists longer than [`MAX_ENTRIES`] or out-of-range local coords.
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
                if e.local_x >= 16 || e.local_z >= 16 {
                    return Err(Error::new(format!(
                        "connection groups: local coord ({}, {}) out of 0..16",
                        e.local_x, e.local_z
                    )));
                }
                w.u8((e.local_x << 4) | e.local_z);
                w.varint(e.y);
                w.i64(e.group as i64);
            }
        }
        Message::Update { entries } => {
            if entries.len() > MAX_ENTRIES {
                return Err(Error::new(format!(
                    "connection groups: {} update entries exceeds max {MAX_ENTRIES}",
                    entries.len()
                )));
            }
            w.u8(KIND_UPDATE);
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
    let mut r = Reader::new(data);
    let kind = r.u8()?;
    let msg = match kind {
        KIND_CHUNK => {
            let cx = r.i32()?;
            let cz = r.i32()?;
            let replace = r.bool()?;
            let count = r.count(MAX_ENTRIES)?;
            let mut entries = Vec::with_capacity(count.min(256));
            for _ in 0..count {
                let xz = r.u8()?;
                let y = r.varint()?;
                let group = r.i64()? as u64;
                entries.push(ChunkEntry {
                    local_x: xz >> 4,
                    y,
                    local_z: xz & 15,
                    group,
                });
            }
            Message::Chunk {
                cx,
                cz,
                replace,
                entries,
            }
        }
        KIND_UPDATE => {
            let count = r.count(MAX_ENTRIES)?;
            let mut entries = Vec::with_capacity(count.min(256));
            for _ in 0..count {
                let x = r.i32()?;
                let y = r.varint()?;
                let z = r.i32()?;
                let present = r.bool()?;
                let group = if present { Some(r.i64()? as u64) } else { None };
                entries.push(UpdateEntry {
                    pos: [x, y, z],
                    group,
                });
            }
            Message::Update { entries }
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
                ChunkEntry {
                    local_x: 0,
                    y: -64,
                    local_z: 15,
                    group: 0,
                },
                ChunkEntry {
                    local_x: 15,
                    y: 319,
                    local_z: 0,
                    group: u64::MAX,
                },
            ],
        };
        assert_eq!(decode(&encode(&chunk).unwrap()).unwrap(), chunk);

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

        let bad_local = Message::Chunk {
            cx: 0,
            cz: 0,
            replace: true,
            entries: vec![ChunkEntry {
                local_x: 16,
                y: 0,
                local_z: 0,
                group: 1,
            }],
        };
        assert!(encode(&bad_local).is_err());
    }
}
