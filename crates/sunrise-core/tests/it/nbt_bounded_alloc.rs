//! A bounded entry's list is refused from its header, before the reader reserves memory for
//! the count it declares. This is its own test binary because it counts the allocations of
//! the whole process.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use sunrise_core::{
    hashed_key::{HashedKey, InsertKey},
    nbt::{Bounded, JavaString, Limits, Tag, read_named_bounded, read_network, write_network},
    nbt_key,
};

/// The system allocator, recording the largest single request.
struct Largest;
static LARGEST: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static ALLOCS: Cell<Option<usize>> = const { Cell::new(None) };
}

// SAFETY: every call forwards to `System` with the same arguments; only a counter is added.
unsafe impl GlobalAlloc for Largest {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        let _ = ALLOCS.try_with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
        // SAFETY: the caller's contract for `alloc` is passed on unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System.alloc` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Largest = Largest;

#[test]
fn a_bounded_list_past_its_budget_reserves_nothing_for_its_count() {
    const ELEMENTS: usize = 1_000_000;
    let budget = Bounded {
        parents: &[&["p"]],
        key: "b",
        max_bytes: 512 * 1024,
        max_depth: 2,
    };
    // {p:{b:[1_000_000 bytes]}}: the file holds every element, the budget does not.
    let mut raw = vec![10, 0, 0, 10, 0, 1, b'p', 9, 0, 1, b'b', 1];
    raw.extend((ELEMENTS as i32).to_be_bytes());
    raw.resize(raw.len() + ELEMENTS, 0);
    raw.extend([0, 0]);
    LARGEST.store(0, Ordering::Relaxed);
    let (named, used, rejected) = read_named_bounded(&raw, Limits::saved(), budget).unwrap();
    let largest = LARGEST.load(Ordering::Relaxed);
    assert_eq!((used, rejected), (raw.len(), 1));
    assert_eq!(named.tag.get("p").and_then(|p| p.get("b")), None);
    // A `Vec<Tag>` for the declared count would be tens of megabytes.
    assert!(largest < ELEMENTS, "largest allocation {largest} bytes");
}

#[test]
fn short_java_strings_need_no_allocation_for_their_units() {
    const SHORT: &[&str] = &["", "a", "éé", "xyz", "𐐷𐐷", "🦀xyz", "abcdef", "ééééééé"];
    for (len, &text) in SHORT.iter().enumerate() {
        let expected: Vec<u16> = text.encode_utf16().collect();
        assert_eq!(expected.len(), len);
        let source = expected.clone();
        let encoded =
            write_network(&Tag::String(JavaString::from(text)), Limits::default()).unwrap();
        ALLOCS.with(|count| count.set(Some(0)));
        let plain = JavaString::from(black_box(text));
        let hashed = HashedKey::<JavaString>::from_text(black_box(text));
        let from_vec = JavaString::from(black_box(source)).clone();
        let (decoded, _) = read_network(black_box(&encoded), Limits::default()).unwrap();
        let allocations = ALLOCS.with(|count| count.replace(None).unwrap());
        assert_eq!(allocations, 0, "{text:?}");
        assert_eq!(plain.units(), expected);
        assert_eq!(hashed.text(), &plain);
        assert_eq!(from_vec, plain);
        assert_eq!(decoded, Tag::String(plain));
    }

    // This literal is too long for inline storage, but still needs no allocation.
    ALLOCS.with(|count| count.set(Some(0)));
    let literal = black_box(nbt_key!("long-literal-key")).into_key();
    let allocations = ALLOCS.with(|count| count.replace(None).unwrap());
    assert_eq!(allocations, 0);
    assert_eq!(literal.text(), &JavaString::from("long-literal-key"));
}

#[test]
fn a_hashed_heap_string_reserves_once() {
    for text in [
        "abcdefgh",
        "éééééééé",
        "ééééééééééé",
        "本語日本語日本語",
        "🦀🦀🦀🦀🦀🦀",
    ] {
        let expected: Vec<_> = text.encode_utf16().collect();
        assert!(expected.len() > 7);
        ALLOCS.with(|count| count.set(Some(0)));
        let hashed = HashedKey::<JavaString>::from_text(black_box(text));
        let allocations = ALLOCS.with(|count| count.replace(None).unwrap());
        assert_eq!(allocations, 1, "{text:?}");
        assert_eq!(hashed.text().units(), expected);
    }
}
