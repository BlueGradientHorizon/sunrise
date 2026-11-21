//! Compact output of the network NBT decoder, not another wire codec.
use super::{
    ArrayKind, Compound, DecodeOutput, Error, JavaString, Limits, Tag, network_reader,
    read_network_with_reader, unwrap_list_element,
};

/// Decoded NBT roots in insertion order, staged without materializing maps or list slots.
/// Only the network decoder can add a non-string root; materialization cannot fail.
#[derive(Default)]
pub struct NetworkTape {
    parts: Vec<Part>,
    text: Vec<u16>,
    arrays: Vec<ArraySpan>,
    longs: Vec<i64>,
    origin: usize,
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
    Array,
    Text(u32),
    List(u32),
    Compound,
    EndCompound,
    EmptyCompounds(u32),
}
const _: () = assert!(size_of::<Part>() <= 8);

struct ArraySpan {
    start: usize,
    len: usize,
    kind: ArrayKind,
}

impl NetworkTape {
    /// Add one nameless root at `offset` in the shared backing buffer, returning
    /// its type ID and the consumed byte count. Discard the tape on failure.
    pub fn read(
        &mut self,
        bytes: &[u8],
        offset: usize,
        limits: Limits,
    ) -> Result<(u8, usize), Error> {
        self.origin = offset;
        read_network_with_reader(&mut network_reader(&bytes[offset..], limits), self)
    }

    /// Convert the already checked fixed-width array bodies in place to native words.
    /// Applying a second time restores the exact original packet for observation.
    pub fn swap_array_endianness(&self, bytes: &mut [u8]) {
        if cfg!(target_endian = "little") {
            for span in &self.arrays {
                let width = span.kind.width();
                if width != 1 {
                    for word in bytes[span.start..span.start + span.len].chunks_exact_mut(width) {
                        word.reverse();
                    }
                }
            }
        }
    }

    /// Store a string already decoded by an enclosing protocol.
    pub fn push_string(&mut self, value: JavaString) {
        self.parts.push(Part::Text(
            u32::try_from(value.0.len()).expect("bounded network string"),
        ));
        self.text.extend(value.0);
    }

    /// Release any spare capacity before retaining the tape in a queue.
    pub fn shrink_to_fit(&mut self) {
        self.parts.shrink_to_fit();
        self.text.shrink_to_fit();
        self.arrays.shrink_to_fit();
        self.longs.shrink_to_fit();
    }

    /// Bytes of the owned staging allocations.
    pub fn owned_heap_bytes(&self) -> usize {
        self.parts.capacity() * size_of::<Part>()
            + self.text.capacity() * size_of::<u16>()
            + self.arrays.capacity() * size_of::<ArraySpan>()
            + self.longs.capacity() * size_of::<i64>()
    }

    /// Materialize roots from the tape and the shared backing buffer's native words.
    pub fn tags<'a>(&'a self, native_arrays: &'a [u8]) -> impl Iterator<Item = Tag> + 'a {
        let mut materialize = Materialize {
            parts: &self.parts,
            text: &self.text,
            arrays: self.arrays.iter(),
            native_arrays,
            longs: &self.longs,
            empty_compounds: 0,
        };
        std::iter::from_fn(move || {
            (!materialize.parts.is_empty() || materialize.empty_compounds != 0)
                .then(|| materialize.tag())
        })
    }
}

impl DecodeOutput for NetworkTape {
    type Value = u8;
    type List = ();
    type Map = usize;
    type Key = ();

    fn scalar(&mut self, tag: Tag) -> u8 {
        let id = tag.id();
        let part = match tag {
            Tag::End => Part::End,
            Tag::Byte(v) => Part::Byte(v),
            Tag::Short(v) => Part::Short(v),
            Tag::Int(v) => Part::Int(v),
            Tag::Long(v) => {
                self.longs.push(v);
                Part::Long
            }
            Tag::Float(v) => Part::Float(v),
            Tag::Double(v) => {
                self.longs.push(v as i64);
                Part::Double
            }
            Tag::String(v) => {
                self.push_string(v);
                return id;
            }
            _ => unreachable!("the network decoder writes collections directly"),
        };
        self.parts.push(part);
        id
    }
    fn array(&mut self, kind: ArrayKind, offset: usize, bytes: &[u8]) -> u8 {
        self.arrays.push(ArraySpan {
            start: self.origin + offset,
            len: bytes.len(),
            kind,
        });
        self.parts.push(Part::Array);
        match kind {
            ArrayKind::Byte => 7,
            ArrayKind::Int => 11,
            ArrayKind::Long => 12,
        }
    }
    fn list(&mut self, len: usize) {
        self.parts.push(Part::List(len as u32));
    }
    fn push(&mut self, _: &mut (), _: u8) {}
    fn end_list(&mut self, _: ()) -> u8 {
        9
    }
    fn compound(&mut self) -> Self::Map {
        let index = self.parts.len();
        self.parts.push(Part::Compound);
        index
    }
    fn contains(&self, _: &Self::Map, _: &JavaString) -> bool {
        unreachable!("strict duplicate checking belongs to bounded disk reads")
    }
    fn key(&mut self, key: JavaString) {
        self.push_string(key);
    }
    fn insert(&mut self, _: &mut usize, _: (), _: u8) -> bool {
        false // Network tapes do not use the disk heap quota.
    }
    fn end_compound(&mut self, index: usize) -> u8 {
        if self.parts.len() != index + 1 {
            self.parts.push(Part::EndCompound);
        } else {
            self.parts.pop();
            // An empty compound can take one wire byte in a homogeneous list.
            // A run occupies one tape slot, not one slot per decoded node.
            if let Some(Part::EmptyCompounds(count)) = self.parts.last_mut()
                && *count != u32::MAX
            {
                *count += 1;
            } else {
                self.parts.push(Part::EmptyCompounds(1));
            }
        }
        10
    }
}

struct Materialize<'a> {
    parts: &'a [Part],
    text: &'a [u16],
    arrays: std::slice::Iter<'a, ArraySpan>,
    native_arrays: &'a [u8],
    longs: &'a [i64],
    empty_compounds: u32,
}

impl Materialize<'_> {
    fn tag(&mut self) -> Tag {
        if self.empty_compounds != 0 {
            self.empty_compounds -= 1;
            return Tag::Compound(Compound::new());
        }
        match take(&mut self.parts, 1)[0] {
            Part::End => Tag::End,
            Part::Byte(v) => Tag::Byte(v),
            Part::Short(v) => Tag::Short(v),
            Part::Int(v) => Tag::Int(v),
            Part::Long => Tag::Long(take(&mut self.longs, 1)[0]),
            Part::Float(v) => Tag::Float(v),
            Part::Double => Tag::Double(take(&mut self.longs, 1)[0] as u64),
            Part::Array => {
                let span = self.arrays.next().expect("one span per staged array");
                let bytes = &self.native_arrays[span.start..span.start + span.len];
                span.kind.tag(bytes, true)
            }
            Part::Text(len) => Tag::String(JavaString(take(&mut self.text, len).to_vec())),
            Part::List(len) => {
                Tag::List((0..len).map(|_| unwrap_list_element(self.tag())).collect())
            }
            Part::Compound => {
                let mut compound = Compound::new();
                while !matches!(self.parts[0], Part::EndCompound) {
                    let Tag::String(key) = self.tag() else {
                        unreachable!("each field has a decoded key")
                    };
                    compound.insert(key, self.tag());
                }
                self.parts = &self.parts[1..];
                Tag::Compound(compound)
            }
            Part::EndCompound => unreachable!("only a compound reads its terminator"),
            Part::EmptyCompounds(count) => {
                self.empty_compounds = count - 1;
                Tag::Compound(Compound::new())
            }
        }
    }
}

fn take<'a, T>(remaining: &mut &'a [T], len: u32) -> &'a [T] {
    let (read, rest) = remaining.split_at(len as usize);
    *remaining = rest;
    read
}
