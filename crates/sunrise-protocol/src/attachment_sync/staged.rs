//! Compact, typed attachment values between packet decoding and holder application.
//!
//! A network NBT compound can cost far more in Java hash-map slabs than on the wire.
//! Keep the decoded fields in wire order, building the maps only after the game owns
//! the turn. The tape has at most eight retained bytes per wire byte (an empty
//! compound in a homogeneous list takes one byte); array data is stored by width.
//! Materialization never reads the wire buffer, and cannot refuse a decoded value.
use super::{MAX_NAME, Message, value_limits};
use crate::{Error, Result, buf::Reader};
use sunrise_core::nbt::{self, Compound, JavaString, Tag};
use sunrise_mod_runtime::attachments::{AttachmentSync, Holder};

/// A structurally decoded attachment message awaiting the holder's turn.
pub struct StagedMessage {
    holder: Holder,
    replace: bool,
    changes: usize,
    parts: Box<[Part]>,
    text: Box<[u16]>,
    bytes: Box<[u8]>,
    ints: Box<[i32]>,
    longs: Box<[i64]>,
}

#[derive(Clone, Copy)]
enum Part {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long,
    Float(u32),
    Double,
    Bytes(u32),
    Text(u32),
    List(u32),
    Compound(u32),
    Ints(u32),
    Longs(u32),
}
const _: () = assert!(size_of::<Part>() <= 8);

#[derive(Default)]
struct Stage {
    parts: Vec<Part>,
    text: Vec<u16>,
    bytes: Vec<u8>,
    ints: Vec<i32>,
    longs: Vec<i64>,
    string_scratch: Vec<u8>,
}

pub(super) fn stage(
    data: &[u8],
    reader: &mut Reader<'_>,
    holder: Holder,
    replace: bool,
    changes: usize,
) -> Result<StagedMessage> {
    let mut stage = Stage::default();
    let limits = value_limits();
    for _ in 0..changes {
        for _ in 0..2 {
            let name = reader.string(MAX_NAME)?;
            let start = stage.text.len();
            stage.text.extend(name.encode_utf16());
            stage
                .parts
                .push(Part::Text((stage.text.len() - start) as u32));
        }
        let start = reader.position();
        let len = reader.remaining().min(limits.max_bytes);
        let mut value = Reader::new(&data[start..start + len]);
        match value.u8()? {
            0 => stage.parts.push(Part::End),
            10 => {
                let mut nodes_left = limits.max_nodes;
                stage.tag(&mut value, 10, 0, &mut nodes_left, limits.max_depth)?;
            }
            _ => return Err(Error::new("attachment sync: value is not a compound")),
        }
        reader.bytes(value.position())?;
    }
    if reader.remaining() != 0 {
        return Err(Error::new("attachment sync: trailing bytes"));
    }
    Ok(StagedMessage {
        holder,
        replace,
        changes,
        parts: stage.parts.into_boxed_slice(),
        text: stage.text.into_boxed_slice(),
        bytes: stage.bytes.into_boxed_slice(),
        ints: stage.ints.into_boxed_slice(),
        longs: stage.longs.into_boxed_slice(),
    })
}

impl Stage {
    fn text(&mut self, reader: &mut Reader<'_>) -> Result<Part> {
        let start = reader.position();
        let len = usize::from(reader.u16()?);
        let bytes = reader.bytes(len)?;
        let text_start = self.text.len();
        if bytes.is_ascii() {
            self.text.extend(bytes.iter().copied().map(u16::from));
        } else {
            // Reuse NBT's DataInput.readUTF behavior, including overlong forms
            // and lone surrogates, by supplying the marker for a string root.
            self.string_scratch.clear();
            self.string_scratch.push(8);
            self.string_scratch.extend_from_slice(reader.since(start));
            let (Tag::String(text), _) = nbt::read_network(&self.string_scratch, value_limits())
                .map_err(|e| Error::new(format!("NBT: {e}")))?
            else {
                unreachable!("the staged string has a string type marker");
            };
            self.text.extend(text.0);
        }
        Ok(Part::Text((self.text.len() - text_start) as u32))
    }

    fn tag(
        &mut self,
        r: &mut Reader<'_>,
        id: u8,
        depth: usize,
        nodes_left: &mut usize,
        max_depth: usize,
    ) -> Result<()> {
        if *nodes_left == 0 {
            return Err(Error::new("NBT: node limit exceeded"));
        }
        *nodes_left -= 1;
        if matches!(id, 9 | 10) && depth >= max_depth {
            return Err(Error::new("NBT: depth limit exceeded"));
        }
        let part = match id {
            1 => Part::Byte(r.i8()?),
            2 => Part::Short(r.i16()?),
            3 => Part::Int(r.i32()?),
            4 => {
                self.longs.push(r.i64()?);
                Part::Long
            }
            5 => {
                let bits = r.u32()?;
                Part::Float(if f32::from_bits(bits) == 0.0 { 0 } else { bits })
            }
            6 => {
                let bits = r.i64()?;
                self.longs.push(if f64::from_bits(bits as u64) == 0.0 {
                    0
                } else {
                    bits
                });
                Part::Double
            }
            7 => {
                let len = collection_len(r, 1)?;
                self.bytes.extend_from_slice(r.bytes(len as usize)?);
                Part::Bytes(len)
            }
            8 => self.text(r)?,
            9 => {
                let id = r.u8()?;
                let width = match id {
                    1 | 10 => 1,
                    2 | 8 => 2,
                    3 | 5 | 7 | 11 | 12 => 4,
                    4 | 6 => 8,
                    9 => 5,
                    _ => 0,
                };
                let len = collection_len(r, width)?;
                if len != 0 && width == 0 {
                    return Err(Error::new("NBT: invalid list element type"));
                }
                if len as usize > *nodes_left {
                    return Err(Error::new("NBT: node limit exceeded"));
                }
                self.parts.push(Part::List(len));
                for _ in 0..len {
                    self.tag(r, id, depth + 1, nodes_left, max_depth)?;
                }
                return Ok(());
            }
            10 => {
                let start = self.parts.len();
                self.parts.push(Part::Compound(0));
                let mut len = 0;
                loop {
                    let id = r.u8()?;
                    if id == 0 {
                        break;
                    }
                    let key = self.text(r)?;
                    self.parts.push(key);
                    self.tag(r, id, depth + 1, nodes_left, max_depth)?;
                    len += 1;
                }
                self.parts[start] = Part::Compound(len);
                return Ok(());
            }
            11 => {
                let len = collection_len(r, 4)?;
                for _ in 0..len {
                    self.ints.push(r.i32()?);
                }
                Part::Ints(len)
            }
            12 => {
                let len = collection_len(r, 8)?;
                for _ in 0..len {
                    self.longs.push(r.i64()?);
                }
                Part::Longs(len)
            }
            _ => return Err(Error::new("NBT: unknown tag type")),
        };
        self.parts.push(part);
        Ok(())
    }
}

fn collection_len(r: &mut Reader<'_>, width: usize) -> Result<u32> {
    let len = u32::try_from(r.i32()?).map_err(|_| Error::new("NBT: negative collection length"))?;
    if (len as usize).saturating_mul(width) > r.remaining() {
        return Err(Error::new("NBT: collection exceeds available bytes"));
    }
    Ok(len)
}

struct Materialize<'a> {
    parts: &'a [Part],
    text: &'a [u16],
    bytes: &'a [u8],
    ints: &'a [i32],
    longs: &'a [i64],
}

impl Materialize<'_> {
    fn text(&mut self) -> JavaString {
        let Part::Text(len) = take(&mut self.parts, 1)[0] else {
            unreachable!("the tape stores a key before a compound value");
        };
        JavaString(take(&mut self.text, len).to_vec())
    }

    fn tag(&mut self) -> Tag {
        match take(&mut self.parts, 1)[0] {
            Part::End => Tag::End,
            Part::Byte(v) => Tag::Byte(v),
            Part::Short(v) => Tag::Short(v),
            Part::Int(v) => Tag::Int(v),
            Part::Long => Tag::Long(take(&mut self.longs, 1)[0]),
            Part::Float(v) => Tag::Float(v),
            Part::Double => Tag::Double(take(&mut self.longs, 1)[0] as u64),
            Part::Bytes(len) => Tag::ByteArray(take(&mut self.bytes, len).to_vec()),
            Part::Text(len) => Tag::String(JavaString(take(&mut self.text, len).to_vec())),
            Part::Ints(len) => Tag::IntArray(take(&mut self.ints, len).to_vec()),
            Part::Longs(len) => Tag::LongArray(take(&mut self.longs, len).to_vec()),
            Part::List(len) => Tag::List(
                (0..len)
                    .map(|_| {
                        let mut tag = self.tag();
                        // ListTag.addAndUnwrap checks the final map after duplicate keys.
                        if let Tag::Compound(compound) = &mut tag
                            && compound.len() == 1
                            && let Some(value) = compound.remove(&JavaString::default())
                        {
                            return value;
                        }
                        tag
                    })
                    .collect(),
            ),
            Part::Compound(len) => {
                let mut compound = Compound::new();
                for _ in 0..len {
                    let name = self.text();
                    compound.insert(name, self.tag());
                }
                Tag::Compound(compound)
            }
        }
    }
}

fn take<'a, T>(remaining: &mut &'a [T], len: u32) -> &'a [T] {
    let (read, rest) = remaining.split_at(len as usize);
    *remaining = rest;
    read
}

impl StagedMessage {
    /// Bytes of owned staging allocations, not counting the inline message.
    pub fn owned_heap_bytes(&self) -> usize {
        size_of_val(&*self.parts)
            + size_of_val(&*self.text)
            + size_of_val(&*self.bytes)
            + size_of_val(&*self.ints)
            + size_of_val(&*self.longs)
    }

    /// Build the holder's values from decoded fields in their original insertion order.
    pub fn into_message(self) -> Message {
        let mut values = Materialize {
            parts: &self.parts,
            text: &self.text,
            bytes: &self.bytes,
            ints: &self.ints,
            longs: &self.longs,
        };
        let changes = (0..self.changes)
            .map(|_| AttachmentSync {
                namespace: values.text(),
                name: values.text(),
                value: match values.tag() {
                    Tag::End => None,
                    Tag::Compound(compound) => Some(compound),
                    _ => unreachable!("attachment roots are compounds or removals"),
                },
            })
            .collect();
        Message {
            holder: self.holder,
            replace: self.replace,
            changes,
        }
    }
}
