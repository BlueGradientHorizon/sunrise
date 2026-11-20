//! The Rust side of tools/string_hash_kit: times every copy of `java_string_hash` (UTF-16 units) and
//! `java_string_hash_str` (`&str`) over the same strings as `mods/sunrise/src/test/java/dev/sunriselab/parity/StringHashBench.java`, and counts heap
//! allocations in the timed loops. run.py writes this crate's `copies` module beside this file.
//!
//! ```text
//! bench MIX LEN          one JSON line per entry and copy: median ns per call over 400 batches
//! check MIX LEN          the wrapping sum of the strings' hashes, which the JVM prints too
//! bench-create MIX LEN   the same for UTF-8 text made into UTF-16 units (a new Vec) and hashed
//! check-create MIX LEN
//! bench-lookup MIX LEN   ns per `keyed::compound_get` of 64 keys of LEN units, hash known
//! check-lookup MIX LEN
//! ```
mod copies;

#[cfg(not(feature = "game"))]
use std::alloc::System;
use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

struct Counting;
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
/// What the counting wrapper forwards to: the system's allocator, or with `--allocator game` the game's.
#[cfg(feature = "game")]
static INNER: sunrise_threads::GameAllocator = sunrise_threads::GAME_ALLOCATOR;
#[cfg(not(feature = "game"))]
static INNER: System = System;
// SAFETY: forwards the pointer and layout contracts to the inner allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { INNER.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { INNER.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { INNER.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { INNER.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static GLOBAL: Counting = Counting;

/// The strings of one row, as StringHashBench makes them: `count` strings of `len` units.
fn strings(mix: &str, len: usize) -> Vec<Vec<u16>> {
    let count = (32_768 / len.max(1)).clamp(64, 4096);
    let mut state: u64 = 1;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as u32
    };
    (0..count)
        .map(|_| {
            let mut s = Vec::with_capacity(len);
            while s.len() < len {
                let r = next();
                match mix {
                    "ascii" => s.push(0x20 + (r % 95) as u16),
                    "latin1" => s.push(0x20 + (r % 224) as u16),
                    "bmp" => s.push(0x100 + (r % (0xD800 - 0x100)) as u16),
                    "surrogates" if s.len() + 1 < len => {
                        s.push(0xD800 + (r % 0x400) as u16);
                        s.push(0xDC00 + ((r >> 10) % 0x400) as u16);
                    }
                    "surrogates" => s.push(0x100 + (r % (0xD800 - 0x100)) as u16),
                    _ => panic!("unknown mix {mix}"),
                }
            }
            s
        })
        .collect()
}

fn median_ns(mut f: impl FnMut() -> u32, calls: usize) -> (f64, u64) {
    for _ in 0..50 {
        std::hint::black_box(f());
    }
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    let mut t = [0f64; 400];
    for slot in &mut t {
        let start = Instant::now();
        std::hint::black_box(f());
        *slot = start.elapsed().as_nanos() as f64 / calls as f64;
    }
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - before;
    t.sort_by(f64::total_cmp);
    (t[200], allocations)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mode, mix, len) = (&args[0], &args[1], args[2].parse::<usize>().unwrap());
    let units = strings(mix, len);
    let slices: Vec<&[u16]> = units.iter().map(Vec::as_slice).collect();
    let owned: Vec<String> = units
        .iter()
        .map(|s| String::from_utf16(s).unwrap())
        .collect();
    let texts: Vec<&str> = owned.iter().map(String::as_str).collect();
    if mode == "check-create" {
        let create = copies::CREATE[0].1;
        println!("{}", create(&texts).wrapping_sub(1) as i32);
        return;
    }
    if mode == "bench-create" {
        for (copy, f) in copies::CREATE {
            let (ns, allocations) = median_ns(|| f(&texts), texts.len());
            println!(
                r#"{{"entry":"create","copy":{copy},"median_ns":{ns:.4},"allocations":{allocations}}}"#
            );
        }
        // The same with no key allocation per string: one scratch buffer per batch, reused.
        for (copy, f) in copies::DECODE {
            let (ns, allocations) = median_ns(|| f(&texts), texts.len());
            println!(
                r#"{{"entry":"decode","copy":{copy},"median_ns":{ns:.4},"allocations":{allocations}}}"#
            );
        }
        return;
    }
    if mode == "check-lookup" || mode == "bench-lookup" {
        lookup(mode == "bench-lookup", &slices);
        return;
    }
    if mode == "check" {
        // Copy 0's loops start their sums from 1.
        let sums = [copies::UTF16[0].1, copies::MAIN_UTF16[0].1]
            .map(|f| f(&slices).wrapping_sub(1) as i32);
        let sums_str =
            [copies::STR[0].1, copies::MAIN_STR[0].1].map(|f| f(&texts).wrapping_sub(1) as i32);
        println!("{} {} {} {}", sums[0], sums_str[0], sums[1], sums_str[1]);
        return;
    }
    for (copy, f) in copies::UTF16 {
        let (ns, allocations) = median_ns(|| f(&slices), slices.len());
        println!(
            r#"{{"entry":"utf16","copy":{copy},"median_ns":{ns:.4},"allocations":{allocations}}}"#
        );
    }
    for (copy, f) in copies::STR {
        let (ns, allocations) = median_ns(|| f(&texts), texts.len());
        println!(
            r#"{{"entry":"str","copy":{copy},"median_ns":{ns:.4},"allocations":{allocations}}}"#
        );
    }
    // Main's hash, which allocates nothing either, for the "vs main" ratios.
    for (copy, f) in copies::MAIN_UTF16 {
        let (ns, allocations) = median_ns(|| f(&slices), slices.len());
        println!(
            r#"{{"entry":"main_utf16","copy":{copy},"median_ns":{ns:.4},"allocations":{allocations}}}"#
        );
    }
    for (copy, f) in copies::MAIN_STR {
        let (ns, allocations) = median_ns(|| f(&texts), texts.len());
        println!(
            r#"{{"entry":"main_str","copy":{copy},"median_ns":{ns:.4},"allocations":{allocations}}}"#
        );
    }
    std::hint::black_box(copies::FILLERS);
}

/// 64 keys (the row's first 64 strings) in a `Compound`, each looked up with its hash known, as a
/// literal key is (`nbt_key!`): 16 passes over the keys per timed batch.
fn lookup(timed: bool, strings: &[&[u16]]) {
    use sunrise_core::{
        hashed_key::HashedKey,
        nbt::{Compound, JavaString, Tag, keyed},
    };
    let keys = &strings[..64];
    let mut compound = Compound::new();
    for (i, key) in keys.iter().enumerate() {
        compound.insert(JavaString(key.to_vec()), Tag::Int(i as i32));
    }
    let hashed: Vec<_> = keys
        .iter()
        .map(|key| HashedKey::<&[u16]>::new(key))
        .collect();
    let run = || {
        let mut sum = 0i32;
        for _ in 0..16 {
            for key in &hashed {
                if let Some(Tag::Int(v)) = keyed::compound_get(&compound, std::hint::black_box(key))
                {
                    sum = sum.wrapping_add(*v);
                }
            }
        }
        sum as u32
    };
    if !timed {
        println!("{}", run() as i32);
        return;
    }
    let (ns, allocations) = median_ns(run, 64 * 16);
    println!(r#"{{"entry":"lookup","copy":0,"median_ns":{ns:.4},"allocations":{allocations}}}"#);
}
