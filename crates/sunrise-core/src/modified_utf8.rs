//! Java's modified UTF-8, as `DataOutput.writeUTF` writes it and `DataInput.readUTF` reads it: each UTF-16
//! unit on its own (a surrogate pair is two 3-byte sequences, a lone surrogate one), and U+0000 as `C0 80`.
//! NBT uses it on disk and on the wire. `tools/test_modified_utf8_parity.py` checks it against JDK 25 and
//! `tools/modified_utf8_kit/` times it against HotSpot.
//!
//! The module uses only `std`, so the speed kit can compile byte-identical copies of it.

/// `writeUTF`'s limit on the encoded bytes, the most its unsigned 16-bit length prefix holds; above it,
/// `writeUTF` throws `UTFDataFormatException` and writes nothing.
pub const MAX_LEN: usize = 65_535;

/// The bytes `unit` takes: 1 for U+0001 to U+007F, 2 for U+0000 and U+0080 to U+07FF, 3 above.
#[inline(always)]
pub const fn unit_len(unit: u16) -> usize {
    1 + (unit.wrapping_sub(1) >= 0x7f) as usize + (unit >= 0x800) as usize
}

/// Whether `unit` takes one byte (U+0001 to U+007F), written as itself.
#[inline(always)]
const fn one_byte(unit: u16) -> bool {
    unit.wrapping_sub(1) < 0x7f
}

/// The encoded byte length of `units`, without the length prefix.
#[inline]
pub fn encoded_len(units: &[u16]) -> usize {
    // Summed in 16-bit lanes a block at a time (at most 3 x 256 per block), so that it vectorizes.
    let (blocks, rest) = units.as_chunks::<256>();
    let mut len = 0;
    for block in blocks {
        len += usize::from(block.iter().map(|&u| unit_len(u) as u16).sum::<u16>());
    }
    len + rest.iter().map(|&u| unit_len(u)).sum::<usize>()
}

/// The bytes of the longest run of whole units at the start of `units` that fits in `room` bytes: where
/// writing them one at a time into `room` bytes stops.
pub fn fitting_len(units: &[u16], room: usize) -> usize {
    let mut len = 0;
    for &u in units {
        let next = len + unit_len(u);
        if next > room {
            break;
        }
        len = next;
    }
    len
}

/// Appends the encoding of `units` to `out`, without a length prefix. `len` is [`encoded_len`]`(units)`, which
/// the caller has computed for the prefix. Allocates only if `out` lacks the room.
#[inline]
pub fn encode(units: &[u16], len: usize, out: &mut Vec<u8>) {
    debug_assert_eq!(len, encoded_len(units));
    if len == units.len() {
        // Every unit is one byte (only those keep the length equal).
        out.extend(units.iter().map(|&u| u as u8));
        return;
    }
    let start = out.len();
    out.resize(start + len, 0);
    encode_into(units, &mut out[start..]);
}

/// Writes the encoding of `units` into `dst`, which is exactly [`encoded_len`]`(units)` bytes long.
fn encode_into(units: &[u16], dst: &mut [u8]) {
    let mut j = 0;
    let mut rest = units;
    // Eight units at a time while 24 bytes are left: one ASCII test for all eight, else each unit's three
    // bytes written whole and the next unit's overwriting the ones it doesn't use.
    while let Some((chunk, tail)) = rest.split_first_chunk::<8>() {
        let Some(window) = dst.get_mut(j..j + 24) else {
            break;
        };
        let window: &mut [u8; 24] = window.try_into().unwrap();
        if chunk.iter().fold(true, |all, &u| all & one_byte(u)) {
            for (d, &u) in window.iter_mut().zip(chunk) {
                *d = u as u8;
            }
            j += 8;
        } else {
            let mut p = 0;
            for &u in chunk {
                let n = unit_len(u);
                let bytes = if n == 1 {
                    [u as u8, 0, 0]
                } else if n == 2 {
                    [0xc0 | (u >> 6) as u8, 0x80 | (u & 63) as u8, 0]
                } else {
                    [
                        0xe0 | (u >> 12) as u8,
                        0x80 | ((u >> 6) & 63) as u8,
                        0x80 | (u & 63) as u8,
                    ]
                };
                window[p..p + 3].copy_from_slice(&bytes);
                p += n;
            }
            j += p;
        }
        rest = tail;
    }
    for &u in rest {
        match unit_len(u) {
            1 => {
                dst[j] = u as u8;
                j += 1;
            }
            2 => {
                dst[j..j + 2].copy_from_slice(&[0xc0 | (u >> 6) as u8, 0x80 | (u & 63) as u8]);
                j += 2;
            }
            _ => {
                dst[j..j + 3].copy_from_slice(&[
                    0xe0 | (u >> 12) as u8,
                    0x80 | ((u >> 6) & 63) as u8,
                    0x80 | (u & 63) as u8,
                ]);
                j += 3;
            }
        }
    }
    debug_assert_eq!(j, dst.len());
}

/// `readUTF`'s decoding of `bytes` (the length prefix already read), accepting what it accepts, overlong
/// forms included; `None` where it throws `UTFDataFormatException`: a byte `10xxxxxx` or `1111xxxx` where a
/// sequence starts, a continuation byte that isn't `10xxxxxx`, or a sequence cut off at the end.
#[inline]
pub fn decode(bytes: &[u8]) -> Option<Vec<u16>> {
    if bytes.is_ascii() {
        return Some(bytes.iter().map(|&b| u16::from(b)).collect());
    }
    let mut units = vec![0u16; bytes.len()];
    let count = decode_into(bytes, &mut units)?;
    units.truncate(count);
    Some(units)
}

/// The sequence size for each lead byte's top four bits: 1 for `0xxxxxxx`, 2 for `110xxxxx`, 3 for `1110xxxx`,
/// 0 (refused) for `10xxxxxx` and `1111xxxx`.
const SIZES: u64 = 0x0322_0000_1111_1111;

/// One sequence led by `b0`, with the two bytes after it (0 past the end, which no sequence accepts as a
/// continuation): its unit and size, or `None` where `readUTF` refuses it. Branch-free but for the refusal.
#[inline(always)]
fn sequence(b0: u8, b1: u8, b2: u8) -> Option<(u16, usize)> {
    let size = ((SIZES >> ((b0 >> 4) * 4)) & 15) as usize;
    let c1 = b1 & 0xc0 == 0x80;
    let c2 = b2 & 0xc0 == 0x80;
    let ok = (size != 0) & (size < 2 || c1) & (size < 3 || c2);
    ok.then(|| (unit(b0, b1, b2), size))
}

/// The top bit of each byte of a `u64`.
const HIGH: u64 = 0x8080_8080_8080_8080;

/// The unit of the sequence that `b0` leads, `b1` and `b2` following it, for a sequence already checked.
#[inline(always)]
fn unit(b0: u8, b1: u8, b2: u8) -> u16 {
    let two = (u16::from(b0 & 0x1f) << 6) | u16::from(b1 & 0x3f);
    let three = (u16::from(b0 & 0x0f) << 12) | (u16::from(b1 & 0x3f) << 6) | u16::from(b2 & 0x3f);
    let wide = std::hint::select_unpredictable(b0 < 0xe0, two, three);
    std::hint::select_unpredictable(b0 < 0x80, u16::from(b0), wide)
}

/// Decodes `bytes` into `units` (at least as long as `bytes`), returning the unit count.
///
/// Eight bytes at a time while ten are left, with each byte's kind as a bit of a mask: the bytes are modified UTF-8
/// exactly when none is `1111xxxx` and the continuation bytes (`10xxxxxx`) are exactly the one after each lead
/// byte (`110xxxxx`, `1110xxxx`) and the second after each `1110xxxx`, those of the block before included
/// (`carry`). Then one unit per byte that isn't a continuation, each independent of the others: the first four
/// unconditionally, packed in one store (past the block's last, they land where the next block's go), the rest one
/// at a time. The last bytes go one sequence at a time.
fn decode_into(bytes: &[u8], units: &mut [u16]) -> Option<usize> {
    let (mut i, mut k, mut carry) = (0, 0, 0u64);
    while i + 10 <= bytes.len() {
        let window: &[u8; 10] = bytes[i..i + 10].try_into().unwrap();
        let w = u64::from_le_bytes(window[..8].try_into().unwrap());
        let out: &mut [u16; 8] = (&mut units[k..k + 8]).try_into().unwrap();
        let high = w & HIGH;
        if high | carry == 0 {
            for (u, &b) in out.iter_mut().zip(window) {
                *u = u16::from(b);
            }
            i += 8;
            k += 8;
            continue;
        }
        // Each byte's bits 6, 5 and 4, moved to its bit 7.
        let (b6, b5, b4) = ((w << 1) & HIGH, (w << 2) & HIGH, (w << 3) & HIGH);
        let continuation = high & !b6;
        let four = high & b6 & b5 & b4;
        let lead = high & b6 & !four;
        let lead3 = lead & b5;
        if four != 0 || continuation != (lead << 8) | (lead3 << 16) | carry {
            return None;
        }
        carry = (lead >> 56) | (lead3 >> 48);
        let mut starts = !continuation & HIGH;
        // One bit per start, in bytes: their sum, in the top byte of the product (without `popcnt`, which x86-64's
        // baseline lacks).
        let count = ((starts >> 7).wrapping_mul(0x0101_0101_0101_0101) >> 56) as usize;
        // Four units in one word and one store (as separate stores, the compiler gathers them into vectors, which is
        // slower), then the rest one at a time.
        let mut packed = 0u64;
        for lane in 0..4 {
            let p = (starts.trailing_zeros() / 8) as usize & 7;
            packed |= u64::from(unit(window[p], window[p + 1], window[p + 2])) << (16 * lane);
            starts &= starts.wrapping_sub(1);
        }
        let [a, b, c, d] = [
            packed as u16,
            (packed >> 16) as u16,
            (packed >> 32) as u16,
            (packed >> 48) as u16,
        ];
        out[..4].copy_from_slice(&[a, b, c, d]);
        for u in out.iter_mut().take(count).skip(4) {
            let p = (starts.trailing_zeros() / 8) as usize & 7;
            *u = unit(window[p], window[p + 1], window[p + 2]);
            starts &= starts.wrapping_sub(1);
        }
        i += 8;
        k += count;
    }
    // The continuation bytes the last block's sequences run into: the first, or the first two.
    let carried = (carry != 0) as usize + (carry >> 15 != 0) as usize;
    if bytes[i..i + carried].iter().any(|&b| b & 0xc0 != 0x80) {
        return None;
    }
    i += carried;
    while i < bytes.len() {
        let at = |n: usize| bytes.get(i + n).copied().unwrap_or(0);
        let (unit, size) = sequence(bytes[i], at(1), at(2))?;
        units[k] = unit;
        k += 1;
        i += size;
    }
    Some(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `writeUTF`'s bytes for one unit of each width and for the edges between them (JDK 25).
    #[test]
    fn units_encode_as_write_utf_writes_them() {
        let cases: [(u16, &[u8]); 9] = [
            (0x0000, &[0xc0, 0x80]),
            (0x0001, &[0x01]),
            (0x007f, &[0x7f]),
            (0x0080, &[0xc2, 0x80]),
            (0x07ff, &[0xdf, 0xbf]),
            (0x0800, &[0xe0, 0xa0, 0x80]),
            (0xd800, &[0xed, 0xa0, 0x80]),
            (0xdfff, &[0xed, 0xbf, 0xbf]),
            (0xffff, &[0xef, 0xbf, 0xbf]),
        ];
        for (unit, bytes) in cases {
            // Alone, and among enough units to take the eight-unit path.
            for units in [vec![unit], [vec![b'a'.into(); 7], vec![unit; 9]].concat()] {
                let mut expected =
                    vec![b'a'; units.len() - units.iter().filter(|&&u| u == unit).count()];
                for _ in 0..units.len() - expected.len() {
                    expected.extend(bytes);
                }
                let mut out = vec![9];
                encode(&units, encoded_len(&units), &mut out);
                assert_eq!(out[1..], expected, "{unit:04x}");
                assert_eq!(decode(&expected), Some(units), "{unit:04x}");
            }
        }
    }

    /// `readUTF` accepts overlong forms and refuses a stray continuation byte, a `1111xxxx` lead, a bad
    /// continuation and a cut-off sequence (JDK 25).
    #[test]
    fn decoding_accepts_and_refuses_what_read_utf_does() {
        assert_eq!(decode(&[0xc0, 0x80]), Some(vec![0]));
        assert_eq!(decode(&[0xe0, 0x81, 0x81]), Some(vec![0x41]));
        assert_eq!(decode(&[0xc1, 0xbf, b'x']), Some(vec![0x7f, 0x78]));
        for bad in [
            &[0x80][..],
            &[0xf0, 0x80, 0x80, 0x80],
            &[0xc2, 0x41],
            &[0xe0, 0x80, 0xc0],
            &[b'a', 0xc2],
            &[0xe0, 0x80],
        ] {
            assert_eq!(decode(bad), None, "{bad:02x?}");
        }
    }

    #[test]
    fn the_fitting_length_stops_before_the_unit_that_overflows() {
        let units = [b'a'.into(), 0x800, 0];
        assert_eq!(fitting_len(&units, 3), 1);
        assert_eq!(fitting_len(&units, 4), 4);
        assert_eq!(fitting_len(&units, 6), 6);
    }
}
