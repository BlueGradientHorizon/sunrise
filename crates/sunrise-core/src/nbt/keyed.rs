//! Engine-only NBT access by keys whose Java hash is already known.
//!
//! `Compound` and `Tag` also serve the mod API, so their inherent methods and trait implementations stay
//! unchanged. Lookups borrow UTF-16 units; a literal key can use [`nbt_key!`](crate::nbt_key) to do no runtime
//! encoding or hashing.

use super::{Compound, CompoundEntry, JavaString, Tag};
use crate::{
    hashed_key::{HashedKey, InsertKey, LookupKey},
    java_hash_map::{HashedIntoIter, HashedIter, JavaHashMap},
};

/// Look up a key in a compound tag, returning `None` for any other kind of tag.
pub fn get<'a, Q: LookupKey + ?Sized>(tag: &'a Tag, key: &Q) -> Option<&'a Tag> {
    match tag {
        Tag::Compound(map) => compound_get(map, key),
        _ => None,
    }
}

/// Look up a mutable value in a compound tag, returning `None` for any other kind of tag.
pub fn get_mut<'a, Q: LookupKey + ?Sized>(tag: &'a mut Tag, key: &Q) -> Option<&'a mut Tag> {
    match tag {
        Tag::Compound(map) => compound_get_mut(map, key),
        _ => None,
    }
}

pub fn compound_get<'a, Q: LookupKey + ?Sized>(map: &'a Compound, key: &Q) -> Option<&'a Tag> {
    map.0
        .as_deref()?
        .get_hashed_by(&key.units_key(), JavaString::units)
}

pub fn compound_get_mut<'a, Q: LookupKey + ?Sized>(
    map: &'a mut Compound,
    key: &Q,
) -> Option<&'a mut Tag> {
    map.0
        .as_deref_mut()?
        .get_mut_hashed_by(&key.units_key(), JavaString::units)
}

pub fn contains_key<Q: LookupKey + ?Sized>(map: &Compound, key: &Q) -> bool {
    compound_get(map, key).is_some()
}

pub fn insert(map: &mut Compound, key: impl InsertKey, value: Tag) -> Option<Tag> {
    map.0
        .get_or_insert_with(|| Box::new(JavaHashMap::default()))
        .put_hashed(key.into_key(), value)
}

pub fn remove<Q: LookupKey + ?Sized>(map: &mut Compound, key: &Q) -> Option<Tag> {
    map.0
        .as_deref_mut()?
        .remove_hashed_by(&key.units_key(), JavaString::units)
}

pub fn entry(map: &mut Compound, key: impl InsertKey) -> CompoundEntry<'_> {
    CompoundEntry {
        map,
        key: key.into_key(),
    }
}

/// Iterate in map order, returning the hash the map stored for each key so copying need not rehash.
pub fn iter(map: &Compound) -> impl ExactSizeIterator<Item = (HashedKey<&JavaString>, &Tag)> {
    HashedCompoundIter(map.0.as_deref().map(|map| map.entries().hashed()))
}

pub fn keys(map: &Compound) -> impl ExactSizeIterator<Item = HashedKey<&JavaString>> {
    iter(map).map(|(key, _)| key)
}

/// Iterate by value in map order, returning each key with its stored hash.
pub fn into_iter(map: Compound) -> impl ExactSizeIterator<Item = (HashedKey<JavaString>, Tag)> {
    HashedCompoundIntoIter(map.0.map(|map| (*map).into_iter().hashed()))
}

/// Build a compound tag from already-hashed keys (literals or copied keys).
pub fn compound<K: InsertKey>(entries: impl IntoIterator<Item = (K, Tag)>) -> Tag {
    let mut map = Compound::new();
    for (key, value) in entries {
        insert(&mut map, key, value);
    }
    Tag::Compound(map)
}

struct HashedCompoundIter<'a>(Option<HashedIter<'a, JavaString, Tag>>);
impl<'a> Iterator for HashedCompoundIter<'a> {
    type Item = (HashedKey<&'a JavaString>, &'a Tag);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut()?.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.as_ref().map_or((0, Some(0)), Iterator::size_hint)
    }
}
impl ExactSizeIterator for HashedCompoundIter<'_> {}
struct HashedCompoundIntoIter(Option<HashedIntoIter<JavaString, Tag>>);
impl Iterator for HashedCompoundIntoIter {
    type Item = (HashedKey<JavaString>, Tag);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut()?.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.as_ref().map_or((0, Some(0)), Iterator::size_hint)
    }
}
impl ExactSizeIterator for HashedCompoundIntoIter {}
