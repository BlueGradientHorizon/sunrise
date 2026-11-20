//! Decode the ASCII and two-byte forms of Latin-1 directly into UTF-16 storage.

/// The input is valid UTF-8 already: a byte >= C4 can only be the lead of a
/// non-Latin-1 character. Within Latin-1, each C2/C3 lead accounts for one extra byte.
pub(super) fn decode(text: &str) -> Option<Vec<u16>> {
    let bytes = text.as_bytes();
    #[cfg(target_arch = "x86_64")]
    if bytes.len() <= 64 {
        // Recognizing Latin-1 while decoding it saves a pass. The byte length
        // bounds the stack space needed before the one exact allocation.
        let mut scratch = [0; 64];
        let (prev, bytes, tail) = sse2::decode_prefix::<true>(bytes, &mut scratch)?;
        let n = 64 - tail.len() + decode_tail::<true>(prev, bytes, tail)?;
        return Some(scratch[..n].to_vec());
    }
    #[cfg(target_arch = "x86_64")]
    let len = sse2::len(bytes)?;
    #[cfg(not(target_arch = "x86_64"))]
    let len = bytes.iter().try_fold(bytes.len(), |len, &b| {
        (b < 0xC4).then_some(len - usize::from(b >= 0xC2))
    })?;

    let mut units = vec![0; len];
    #[cfg(target_arch = "x86_64")]
    let (prev, bytes, tail) = sse2::decode_prefix::<false>(bytes, &mut units)?;
    #[cfg(not(target_arch = "x86_64"))]
    let (prev, tail) = (0, &mut units[..]);
    decode_tail::<false>(prev, bytes, tail)?;
    Some(units)
}

#[inline]
fn decode_tail<const CHECK: bool>(mut prev: u8, bytes: &[u8], tail: &mut [u16]) -> Option<usize> {
    let mut out = 0;
    for &b in bytes {
        if CHECK && b >= 0xC4 {
            return None;
        }
        // On a continuation byte bit 7 is already set; bit 0 of its lead
        // supplies bit 6. A lead always has a continuation to overwrite it.
        tail[out] = u16::from(b | ((prev & (b >> 7) & 1) << 6));
        out += usize::from(b < 0xC2);
        prev = b;
    }
    Some(out)
}

#[cfg(target_arch = "x86_64")]
mod sse2 {
    use safe_arch::*;

    pub(super) fn len(bytes: &[u8]) -> Option<usize> {
        let (blocks, tail) = bytes.as_chunks::<16>();
        let mut leads = 0;
        let sign = set_splat_i8_m128i(i8::MIN);
        let max_latin1_byte = set_splat_i8_m128i(0x43); // C3 with the sign bit flipped
        let max_continuation = set_splat_i8_m128i(0x3F); // BF with the sign bit flipped
        for block in blocks {
            let b = bitxor_m128i((*block).into(), sign);
            if move_mask_i8_m128i(cmp_gt_mask_i8_m128i(b, max_latin1_byte)) != 0 {
                return None;
            }
            leads +=
                move_mask_i8_m128i(cmp_gt_mask_i8_m128i(b, max_continuation)).count_ones() as usize;
        }
        tail.iter().try_fold(bytes.len() - leads, |len, &b| {
            (b < 0xC4).then_some(len - usize::from(b >= 0xC2))
        })
    }

    /// Which bytes to shift right by 1, 2 and 4 positions, in that order, to
    /// remove the bytes selected by the mask. Each kept byte moves by exactly
    /// the number of removed bytes before it.
    const COMPACT: [[u64; 3]; 256] = {
        let mut table = [[0; 3]; 256];
        let mut mask = 0usize;
        while mask < table.len() {
            let mut removed = 0;
            let mut i = 0;
            while i < 8 {
                if mask & (1 << i) != 0 {
                    removed += 1;
                } else {
                    let mut pos = i;
                    let mut step = 0;
                    while step < 3 {
                        if removed & (1 << step) != 0 {
                            table[mask][step] |= 0xFF << (8 * pos);
                            pos -= 1 << step;
                        }
                        step += 1;
                    }
                }
                i += 1;
            }
            mask += 1;
        }
        table
    };

    pub(super) fn decode_prefix<'a, 'b, const CHECK: bool>(
        mut bytes: &'a [u8],
        mut out: &'b mut [u16],
    ) -> Option<(u8, &'a [u8], &'b mut [u16])> {
        let mut prev = 0;
        let high_bits = set_splat_i8_m128i(0xC0u8 as i8);
        let bit6 = set_splat_i8_m128i(0x40);
        let zero = zeroed_m128i();
        // Store eight lanes at a time, overlapping the padding with the next
        // output except at the end of the allocation.
        while let Some((block, rest)) = bytes.split_first_chunk::<16>() {
            let b: m128i = (*block).into();
            if CHECK
                && move_mask_i8_m128i(cmp_gt_mask_i8_m128i(
                    bitxor_m128i(b, set_splat_i8_m128i(i8::MIN)),
                    set_splat_i8_m128i(0x43),
                )) != 0
            {
                return None;
            }
            let lead = cmp_eq_mask_i8_m128i(bitand_m128i(b, high_bits), high_bits);
            let preceding = bitor_m128i(
                byte_shl_imm_u128_m128i::<1>(b),
                m128i::from([u64::from(prev), 0]),
            );
            // A continuation's bit 6 comes from the lead's bit 0. Mask that
            // contribution by the current byte's high bit so ASCII stays as-is.
            let extra = bitand_m128i(
                shl_imm_u16_m128i::<6>(preceding),
                bitand_m128i(shr_imm_u16_m128i::<1>(b), bit6),
            );
            let normalized = bitandnot_m128i(lead, bitor_m128i(b, extra));
            let [lo, hi]: [u64; 2] = normalized.into();
            let mask = move_mask_i8_m128i(lead) as u16;
            for (mut group, mask) in [(lo, mask as u8), (hi, (mask >> 8) as u8)] {
                let [m1, m2, m4] = COMPACT[usize::from(mask)];
                let moving = group & m1;
                group = (group ^ moving) | moving >> 8;
                let moving = group & m2;
                group = (group ^ moving) | moving >> 16;
                let moving = group & m4;
                group = (group ^ moving) | moving >> 32;
                let padded: [u16; 8] = unpack_low_i8_m128i(m128i::from([group, 0]), zero).into();
                let written = 8 - mask.count_ones() as usize;
                if let Some(full) = out.first_chunk_mut::<8>() {
                    *full = padded;
                } else {
                    out[..written].copy_from_slice(&padded[..written]);
                }
                out = &mut out[written..];
            }
            prev = block[15];
            bytes = rest;
        }
        Some((prev, bytes, out))
    }
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn compaction_preserves_mixed_latin1_across_block_boundaries() {
        // All ASCII/two-byte patterns of eight units, with the pattern starting
        // at each byte position within a block. The suffix crosses into the heap-
        // sized path and includes the extremes of both lead bytes.
        let long = "\0\u{bf}\u{c0}".repeat(20);
        for pattern in 0..256 {
            for prefix in 0..16 {
                let mut text = "a".repeat(prefix);
                for i in 0..8 {
                    text.push(if pattern & (1 << i) == 0 {
                        '\u{7f}'
                    } else if i % 2 == 0 {
                        '\u{80}'
                    } else {
                        '\u{ff}'
                    });
                }
                for suffix in ["", long.as_str()] {
                    let input = format!("{text}{suffix}");
                    let expected: Vec<_> = input.encode_utf16().collect();
                    let units = decode(&input).unwrap();
                    assert_eq!(units, expected, "{input:?}");
                    assert_eq!(units.capacity(), expected.len());
                }
            }
        }
        for prefix in 0..80 {
            let wrong = format!("{}\u{100}", "a".repeat(prefix));
            assert_eq!(decode(&wrong), None, "prefix {prefix}");
        }
    }
}
