//! Java's `String.hashCode`: `s[0]*31^(n-1) + ... + s[n-1]` over the string's UTF-16 units, in
//! wrapping `int` arithmetic. Bit-exact on every input, including lone surrogates; nothing allocates.
//!
//! On x86-64, strings of sixteen units or more are folded eight units at a time with the SSE2
//! operations every x86-64 CPU has, through `safe_arch`'s safe wrappers, so the crate stays free of
//! `unsafe`. Each unit is weighted by its power of 31 straight from a table (a power of 31 modulo
//! 2^32, since wrapping multiplication and addition are a ring), so the units of a block are
//! independent sums and no multiplication waits for another: the same value as `h = 31 * h + c`,
//! summed in another order. Elsewhere, and below sixteen units, it is the plain loop. This module
//! depends on nothing else in the crate, so that `tools/string_hash_kit` can time byte-identical
//! copies of it.

use simd::{WIDE_FROM, fold};

mod units;
pub(crate) use units::{INLINE_CAPACITY, Units};

/// `String.hashCode` of a string's UTF-16 units.
#[inline]
pub fn java_string_hash(units: &[u16]) -> i32 {
    if units.is_empty() {
        return 0;
    }
    fold(0, units) as i32
}

/// The UTF-16 units of `text` and their `String.hashCode`, made together. Short strings
/// stay inline; longer ASCII text (each byte a unit) uses a vector widen and a vector hash.
pub(crate) fn utf16_with_hash(text: &str) -> (Units, i32) {
    let bytes = text.as_bytes();
    if bytes.len() <= INLINE_CAPACITY && bytes.is_ascii() {
        let mut units = [0; INLINE_CAPACITY];
        let mut hash = 0i32;
        for (out, &byte) in units.iter_mut().zip(bytes) {
            *out = u16::from(byte);
            hash = hash.wrapping_mul(31).wrapping_add(i32::from(byte));
        }
        return (Units::inline(units, bytes.len()), hash);
    }
    if bytes.is_ascii() {
        let units = bytes.iter().map(|&b| u16::from(b)).collect();
        return (units, java_string_hash_ascii(bytes));
    }
    // One allocation of exactly the units the text encodes to: one per byte that starts a character,
    // and a second for each four-byte character (a surrogate pair). A larger guess would keep the
    // spare capacity in every key and can move the block up an allocator size class.
    let len = bytes
        .iter()
        .map(|&b| usize::from(b & 0xC0 != 0x80) + usize::from(b >= 0xF0))
        .sum();
    if len <= INLINE_CAPACITY {
        let mut units = [0; INLINE_CAPACITY];
        let mut hash = 0i32;
        for (out, unit) in units.iter_mut().zip(text.encode_utf16()) {
            *out = unit;
            hash = hash.wrapping_mul(31).wrapping_add(i32::from(unit));
        }
        return (Units::inline(units, len), hash);
    }
    let mut units;
    if bytes.iter().all(|&b| b < 0xC4) {
        // Only ASCII and the two-byte forms of U+0080..U+00FF (C2 or C3, then a continuation): each
        // non-continuation byte is one unit, worked out from it and the byte after it, so that no
        // step waits for the one before. The unit is written even for a continuation byte, and the
        // next unit overwrites it, so a text that ends in a continuation byte writes one past the
        // last unit, which the truncate drops.
        units = vec![0u16; len + 1];
        let mut out = 0;
        for (i, &b) in bytes.iter().enumerate() {
            let next = bytes.get(i + 1).copied().unwrap_or(0);
            units[out] = if b >= 0xC0 {
                u16::from(b & 3) << 6 | u16::from(next & 0x3F)
            } else {
                u16::from(b)
            };
            out += usize::from(b & 0xC0 != 0x80);
        }
        units.truncate(out);
    } else {
        units = Vec::with_capacity(len);
        units.extend(text.encode_utf16());
    }
    let hash = java_string_hash(&units);
    (Units::Heap(units), hash)
}

/// `String.hashCode` of the Java string holding `text`'s UTF-16 units (`text.encode_utf16()`),
/// without collecting them.
#[inline]
pub fn java_string_hash_str(text: &str) -> i32 {
    // For ASCII text each byte is one UTF-16 unit. Short text is folded byte by byte until a
    // byte that is not ASCII, from which the encoded units are folded; longer text is checked
    // first, so that text that is not ASCII does not pay for a vector fold it throws away.
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return 0;
    }
    if bytes.len() < WIDE_FROM {
        let mut hash = 0u32;
        for (i, &b) in bytes.iter().enumerate() {
            if b >= 0x80 {
                return fold_encoded(hash, &text[i..]) as i32;
            }
            hash = hash.wrapping_mul(31).wrapping_add(u32::from(b));
        }
        return hash as i32;
    }
    if bytes.is_ascii() {
        return fold(0, bytes) as i32;
    }
    fold_encoded(0, text) as i32
}

/// `String.hashCode` of `text`, for a constant: a plain loop over the units its characters encode
/// to, which the compiler runs. Same value as [`java_string_hash_str`] (a test compares them).
pub const fn java_string_hash_const(text: &str) -> i32 {
    let bytes = text.as_bytes();
    let (mut hash, mut i) = (0u32, 0);
    while i < bytes.len() {
        let (code, len) = decode_const(bytes, i);
        i += len;
        if code < 0x1_0000 {
            hash = hash.wrapping_mul(31).wrapping_add(code);
        } else {
            let c = code - 0x1_0000;
            hash = hash.wrapping_mul(31).wrapping_add(0xD800 | (c >> 10));
            hash = hash.wrapping_mul(31).wrapping_add(0xDC00 | (c & 0x3FF));
        }
    }
    hash as i32
}

/// `String.hashCode` of UTF-16 units, for a constant.
pub const fn java_string_hash_units_const(units: &[u16]) -> i32 {
    let (mut hash, mut i) = (0u32, 0);
    while i < units.len() {
        hash = hash.wrapping_mul(31).wrapping_add(units[i] as u32);
        i += 1;
    }
    hash as i32
}

/// `String.hashCode` of ASCII bytes, each one UTF-16 unit. The bytes must be ASCII (a non-ASCII byte
/// is not the unit it stands for).
#[inline]
pub fn java_string_hash_ascii(bytes: &[u8]) -> i32 {
    debug_assert!(bytes.is_ascii());
    fold(0, bytes) as i32
}

/// The number of UTF-16 units `text` encodes to.
pub const fn utf16_len_const(text: &str) -> usize {
    let bytes = text.as_bytes();
    let (mut units, mut i) = (0, 0);
    while i < bytes.len() {
        let (code, len) = decode_const(bytes, i);
        i += len;
        units += if code < 0x1_0000 { 1 } else { 2 };
    }
    units
}

/// The UTF-16 units of `text`, which is `N` of them (`utf16_len_const`).
pub const fn utf16_units_const<const N: usize>(text: &str) -> [u16; N] {
    let bytes = text.as_bytes();
    let (mut units, mut at, mut i) = ([0u16; N], 0, 0);
    while i < bytes.len() {
        let (code, len) = decode_const(bytes, i);
        i += len;
        if code < 0x1_0000 {
            units[at] = code as u16;
            at += 1;
        } else {
            let c = code - 0x1_0000;
            units[at] = 0xD800 | (c >> 10) as u16;
            units[at + 1] = 0xDC00 | (c & 0x3FF) as u16;
            at += 2;
        }
    }
    units
}

/// The character at byte `i` of valid UTF-8 and its length in bytes.
const fn decode_const(bytes: &[u8], i: usize) -> (u32, usize) {
    let b0 = bytes[i] as u32;
    if b0 < 0x80 {
        return (b0, 1);
    }
    let c1 = (bytes[i + 1] & 0x3F) as u32;
    if b0 < 0xE0 {
        return (((b0 & 0x1F) << 6) | c1, 2);
    }
    let c2 = (bytes[i + 2] & 0x3F) as u32;
    if b0 < 0xF0 {
        return (((b0 & 0x0F) << 12) | (c1 << 6) | c2, 3);
    }
    let c3 = (bytes[i + 3] & 0x3F) as u32;
    (((b0 & 0x07) << 18) | (c1 << 12) | (c2 << 6) | c3, 4)
}

/// `String.hashCode` of `text` if every byte passes `allowed` (and so is one ASCII unit), else
/// `None`: validation and hashing in one pass, for short ASCII-only names.
#[inline]
pub fn java_string_hash_ascii_if(text: &str, allowed: impl Fn(u8) -> bool) -> Option<i32> {
    let mut hash = 0u32;
    for &b in text.as_bytes() {
        if !allowed(b) || b >= 0x80 {
            return None;
        }
        hash = hash.wrapping_mul(31).wrapping_add(u32::from(b));
    }
    Some(hash as i32)
}

/// `hash` continued over the UTF-16 units `text` encodes to.
#[inline]
fn fold_encoded(hash: u32, text: &str) -> u32 {
    text.encode_utf16().fold(hash, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(u32::from(unit))
    })
}

#[cfg(target_arch = "x86_64")]
mod simd {
    use safe_arch::*;

    /// 31^0 to 31^BLOCK, wrapping.
    const POWERS: [u32; BLOCK + 1] = {
        let mut p = [1u32; BLOCK + 1];
        let mut i = 1;
        while i <= BLOCK {
            p[i] = p[i - 1].wrapping_mul(31);
            i += 1;
        }
        p
    };
    /// Units folded per block, each block as long a run of vector steps as the weight tables hold.
    const BLOCK: usize = 256;
    /// Units per vector step.
    const STEP: usize = 8;

    /// Units from here on take vector steps.
    pub(super) const WIDE_FROM: usize = 2 * STEP;

    /// `hash` continued over `units`: the hash of a string whose first part hashed to `hash`.
    #[inline(always)]
    pub(super) fn fold<T: Unit>(hash: u32, units: &[T]) -> u32 {
        if units.len() < WIDE_FROM {
            // Under a vector step the plain loop is the fastest there is.
            return units
                .iter()
                .fold(hash, |h, &u| h.wrapping_mul(31).wrapping_add(u.into()));
        }
        fold_wide(hash, units)
    }

    /// Sixteen units or more: vector steps.
    #[inline(always)]
    fn fold_wide<T: Unit>(mut hash: u32, units: &[T]) -> u32 {
        let (blocks, rest) = units.as_chunks::<BLOCK>();
        for block in blocks {
            hash = hash
                .wrapping_mul(POWERS[BLOCK])
                .wrapping_add(weighted_sum(block));
        }
        let (head, body) = rest.split_at(rest.len() % STEP);
        for &u in head {
            hash = hash.wrapping_mul(31).wrapping_add(u.into());
        }
        if !body.is_empty() {
            hash = hash
                .wrapping_mul(POWERS[body.len()])
                .wrapping_add(weighted_sum(body));
        }
        hash
    }

    /// A UTF-16 unit, or an ASCII byte standing for one.
    pub(super) trait Unit: Copy + Into<u32> {
        const ZERO: Self;
        /// Two vector steps of units as 16-bit lanes.
        fn load(units: &[Self; 2 * STEP]) -> [m128i; 2];
    }
    impl Unit for u16 {
        const ZERO: u16 = 0;
        #[inline(always)]
        fn load(units: &[u16; 2 * STEP]) -> [m128i; 2] {
            let (a, b) = units.split_at(STEP);
            [
                m128i::from(<[u16; STEP]>::try_from(a).unwrap()),
                m128i::from(<[u16; STEP]>::try_from(b).unwrap()),
            ]
        }
    }
    impl Unit for u8 {
        const ZERO: u8 = 0;
        #[inline(always)]
        fn load(units: &[u8; 2 * STEP]) -> [m128i; 2] {
            let bytes = m128i::from(*units);
            let zero = zeroed_m128i();
            [
                unpack_low_i8_m128i(bytes, zero),
                unpack_high_i8_m128i(bytes, zero),
            ]
        }
    }

    /// Each unit's weight in a block, 31^(BLOCK-1-k), as the signed low half and the high half that
    /// makes up the difference: `weight == lo + (hi << 16)` modulo 2^32.
    const WEIGHT_LO: [[i16; STEP]; BLOCK / STEP] = {
        let mut w = [[0i16; STEP]; BLOCK / STEP];
        let mut k = 0;
        while k < BLOCK {
            w[k / STEP][k % STEP] = POWERS[BLOCK - 1 - k] as u16 as i16;
            k += 1;
        }
        w
    };
    const WEIGHT_HI: [[i16; STEP]; BLOCK / STEP] = {
        let mut w = [[0i16; STEP]; BLOCK / STEP];
        let mut k = 0;
        while k < BLOCK {
            let lo = WEIGHT_LO[k / STEP][k % STEP] as i32 as u32;
            w[k / STEP][k % STEP] = (POWERS[BLOCK - 1 - k].wrapping_sub(lo) >> 16) as u16 as i16;
            k += 1;
        }
        w
    };
    /// 0x8000 times the sum of the first `8 * n` low halves' weights, for each n: what biasing every
    /// unit by 0x8000 (below) takes off the low products.
    const BIAS: [u32; BLOCK / STEP + 1] = {
        let mut bias = [0u32; BLOCK / STEP + 1];
        let mut n = 1;
        while n <= BLOCK / STEP {
            let mut sum = 0u32;
            let mut k = 0;
            while k < STEP {
                sum = sum.wrapping_add(WEIGHT_LO[BLOCK / STEP - n][k] as i32 as u32);
                k += 1;
            }
            bias[n] = bias[n - 1].wrapping_add(sum.wrapping_mul(0x8000));
            n += 1;
        }
        bias
    };

    /// The units (a multiple of 8, at most a block) each weighted by 31 to the number of units after
    /// it: the weights are the last of a block's. Eight units over a multiple of 16 go in front of
    /// the rest as a step of 16 with eight zeros before them, which weigh nothing.
    ///
    /// A unit `c` times a weight is `c * lo + (c * hi << 16)` for the weight's halves. `lo` is taken as
    /// signed and `c` biased by 0x8000 (undone by `BIAS`), so that `lo` products are the signed 16-bit
    /// pairs a multiply-add (`pmaddwd`) sums; `hi` products only matter modulo 2^16, which a 16-bit
    /// multiply (`pmullw`) gives.
    #[inline(always)]
    fn weighted_sum<T: Unit>(units: &[T]) -> u32 {
        let (front, rest) = units.split_at(units.len() % (2 * STEP));
        let pairs = rest.as_chunks::<{ 2 * STEP }>().0;
        let rows = 2 * (pairs.len() + usize::from(!front.is_empty()));
        let first = BLOCK / STEP - rows;
        let bias = set_splat_i16_m128i(i16::MIN);
        // Two sets of accumulators, one for each of a step's two vectors, so that the sums are
        // not one chain.
        let (mut low, mut high) = ([zeroed_m128i(); 2], [zeroed_m128i(); 2]);
        let mut row = first;
        let mut step = |chunk: &[T; 2 * STEP], row: usize| {
            for (k, v) in T::load(chunk).into_iter().enumerate() {
                let (lo, hi) = (
                    m128i::from(WEIGHT_LO[row + k]),
                    m128i::from(WEIGHT_HI[row + k]),
                );
                low[k] = add_i32_m128i(
                    low[k],
                    mul_i16_horizontal_add_m128i(bitxor_m128i(v, bias), lo),
                );
                high[k] = add_i16_m128i(high[k], mul_i16_keep_low_m128i(v, hi));
            }
        };
        if !front.is_empty() {
            let mut padded = [T::ZERO; 2 * STEP];
            padded[STEP..].copy_from_slice(front);
            step(&padded, row);
            row += 2;
        }
        for pair in pairs {
            step(pair, row);
            row += 2;
        }
        let (low, high) = (
            add_i32_m128i(low[0], low[1]),
            add_i16_m128i(high[0], high[1]),
        );
        // Sum the 16-bit lanes of `high` in pairs, as the top halves of 32-bit lanes.
        let high =
            shl_imm_u32_m128i::<16>(mul_i16_horizontal_add_m128i(high, set_splat_i16_m128i(1)));
        let [a, b, c, d]: [u32; 4] = add_i32_m128i(low, high).into();
        a.wrapping_add(b)
            .wrapping_add(c)
            .wrapping_add(d)
            .wrapping_add(BIAS[rows])
    }
}

/// The plain loop, where there are no vector steps to take.
#[cfg(not(target_arch = "x86_64"))]
mod simd {
    /// A UTF-16 unit, or an ASCII byte standing for one.
    pub(super) trait Unit: Copy + Into<u32> {}
    impl Unit for u8 {}
    impl Unit for u16 {}

    /// Units from here on would take vector steps.
    pub(super) const WIDE_FROM: usize = 16;

    /// `hash` continued over `units`: the hash of a string whose first part hashed to `hash`.
    pub(super) fn fold<T: Unit>(hash: u32, units: &[T]) -> u32 {
        units
            .iter()
            .fold(hash, |h, &u| h.wrapping_mul(31).wrapping_add(u.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values from `String.hashCode` on JDK 25 (`tools/test_string_hash_parity.py` checks every
    /// one- and two-unit string and billions of random ones).
    #[test]
    fn utf16_with_hash_matches_encode_utf16() {
        let pieces = [
            "a",
            "\u{e9}",
            "\u{ff}",
            "\u{80}",
            "\u{100}",
            "\u{20ac}",
            "\u{1f600}",
            "Z",
            "\u{c0}",
        ];
        let mut seed = 7u32;
        for len in 0..80 {
            let mut text = String::new();
            for _ in 0..len {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                text.push_str(pieces[(seed >> 24) as usize % pieces.len()]);
            }
            let (units, hash) = utf16_with_hash(&text);
            let expected: Vec<u16> = text.encode_utf16().collect();
            assert_eq!(units.as_slice(), expected, "{text:?}");
            if let Units::Heap(units) = &units {
                // At most one spare unit (the Latin-1 path's overwrite slot).
                assert!(units.capacity() <= expected.len() + 1, "{text:?}");
            }
            assert_eq!(hash, java_string_hash(&expected));
        }
    }

    #[test]
    fn known_java_hashes() {
        assert_eq!(java_string_hash_str(""), 0);
        assert_eq!(java_string_hash_str("minecraft"), 695_073_197);
        assert_eq!(java_string_hash_str("\u{1F600}"), 1_772_899);
        assert_eq!(
            java_string_hash_str("the quick brown fox jumps over the lazy dog, twice!"),
            1_262_420_192
        );
        let mixed = "caf\u{e9} \u{4e16}\u{754c} \u{1F600} abcdefghijklmnopqrstuvwxyz0123456789 \u{ff}\u{100}";
        assert_eq!(java_string_hash_str(mixed), 1_354_549_821);
        let units: Vec<u16> = mixed.encode_utf16().collect();
        assert_eq!(java_string_hash(&units), 1_354_549_821);
        assert_eq!(java_string_hash(&[0xD800]), 55_296);
    }

    /// Every length around the vector step and the block, on the extreme units (0x8000 and 0xFFFF
    /// are where the biasing matters), against the definition `h = 31 * h + c`.
    #[test]
    fn every_length_matches_the_definition() {
        let naive = |units: &[u16]| {
            units
                .iter()
                .fold(0i32, |h, &c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
        };
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        for len in (0..=3 * 256 + 9).chain([1024, 1025, 4099]) {
            for fill in [0u16, 0x7F, 0x8000, 0xFFFF, 1] {
                let units = vec![fill; len];
                assert_eq!(java_string_hash(&units), naive(&units), "{len} x {fill:#x}");
            }
            let units: Vec<u16> = (0..len)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    (state >> 20) as u16
                })
                .collect();
            assert_eq!(java_string_hash(&units), naive(&units), "{len} random");
            let ascii: String = units
                .iter()
                .map(|&u| char::from(b' ' + (u % 95) as u8))
                .collect();
            let ascii_units: Vec<u16> = ascii.encode_utf16().collect();
            assert_eq!(
                java_string_hash_str(&ascii),
                naive(&ascii_units),
                "{len} ascii"
            );
            let text: String = ascii
                .chars()
                .enumerate()
                .map(|(i, c)| if i % 5 == 3 { '\u{e9}' } else { c })
                .collect();
            let text_units: Vec<u16> = text.encode_utf16().collect();
            assert_eq!(
                java_string_hash_str(&text),
                naive(&text_units),
                "{len} latin1 text"
            );
            let wide: String = ascii
                .chars()
                .enumerate()
                .map(|(i, c)| match i % 7 {
                    2 => '\u{4e16}',
                    5 => '\u{1F600}',
                    _ => c,
                })
                .collect();
            let wide_units: Vec<u16> = wide.encode_utf16().collect();
            assert_eq!(
                java_string_hash_str(&wide),
                naive(&wide_units),
                "{len} 3- and 4-byte text"
            );
        }
    }

    /// The constant hash, its length and its units agree with the runtime ones on ASCII, Latin-1,
    /// BMP, supplementary text and the empty string.
    #[test]
    fn constant_hash_matches_the_runtime_hash() {
        macro_rules! check {
            ($($text:literal),*) => {$({
                const LEN: usize = utf16_len_const($text);
                const UNITS: [u16; LEN] = utf16_units_const::<LEN>($text);
                const HASH: i32 = java_string_hash_const($text);
                let units: Vec<u16> = $text.encode_utf16().collect();
                assert_eq!(UNITS.as_slice(), units.as_slice(), "units of {:?}", $text);
                assert_eq!(HASH, java_string_hash(&units), "hash of {:?}", $text);
                assert_eq!(HASH, java_string_hash_str($text), "str hash of {:?}", $text);
            })*};
        }
        check!(
            "",
            "a",
            "Count",
            "minecraft:stone",
            "caf\u{e9}",
            "\u{4e16}\u{754c}",
            "\u{1F600}",
            "a\u{1F600}b\u{ff}\u{100}",
            "the quick brown fox jumps over the lazy dog, twice!"
        );
    }

    #[test]
    fn ascii_if_hashes_only_allowed_bytes() {
        assert_eq!(
            java_string_hash_ascii_if("minecraft", |b| b.is_ascii_lowercase()),
            Some(695_073_197)
        );
        assert_eq!(java_string_hash_ascii_if("a/b", |b| b != b'/'), None);
        assert_eq!(java_string_hash_ascii_if("\u{e9}", |_| true), None);
    }
}
