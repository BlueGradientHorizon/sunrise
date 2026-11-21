use sunrise_core::nbt::Tag;

// no wire: borrows the live entity's saved tags from the host.
/// A borrowed view of an entity's scoreboard tags (`Entity.getTags`).
// exhaustive: its field is private
#[derive(Clone, Copy)]
pub struct EntityTags<'a>(&'a [Tag]);

impl<'a> EntityTags<'a> {
    /// Visit string tags in saved order as their exact UTF-16 code units.
    /// Non-string entries are skipped. An unpaired surrogate is not replaced.
    pub fn iter_utf16(self) -> impl Iterator<Item = &'a [u16]> {
        self.0.iter().filter_map(|tag| match tag {
            Tag::String(text) => Some(text.0.as_slice()),
            _ => None,
        })
    }

    /// Compare a Rust string's UTF-16 code units with the saved tags without
    /// allocating or replacing unpaired surrogates.
    pub fn contains(&self, tag: &str) -> bool {
        self.iter_utf16()
            .any(|units| units.iter().copied().eq(tag.encode_utf16()))
    }
}

/// Lend the game's saved tag list without exposing NBT to mods.
pub fn borrow_entity_tags(tags: &[Tag]) -> EntityTags<'_> {
    EntityTags(tags)
}
