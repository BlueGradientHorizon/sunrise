//! Resolved tag names and the read-only merged-tag service.
use crate::{BlockKey, FluidKey, Identifier, ItemKey};

/// A process-lifetime tag name resolved once through [`Registrar::tag_key`](crate::TagsRegistrar::tag_key)
/// or [`Tags::key`].
/// Membership belongs to the current authoritative data reload, not to the key.
// no wire: decoding validates the u32 slot with TagKey::from_slot on the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: the runtime alone creates name slots
pub struct TagKey(pub(crate) u32);

impl TagKey {
    pub(crate) fn from_slot(slot: u32) -> Result<Self, &'static str> {
        sunrise_mod_runtime::registry_keys::with_tag_name(slot, |id| {
            id.map(|_| Self(slot)).ok_or("unknown TagKey slot")
        })
    }
}

/// The result of reading one item tag from the current authoritative snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: the lookup can only find a tag, miss it, or be unavailable.
pub enum ItemTagMembers {
    /// The snapshot has no tag of this name.
    Missing,
    /// The loaded tag's members in vanilla's order. The list may be empty.
    Members(Vec<ItemKey>),
    /// The snapshot or a required key mapping is unavailable. No partial list is returned.
    Unavailable,
}

/// Read merged block, item and fluid tags without borrowing the game's registry structs.
/// A client reads the tags its server sent. [`Tags::items`] distinguishes absent tags
/// from loaded empty tags; the membership predicate treats both as false.
pub trait Tags {
    /// Resolve a validated name once. The name need not currently have members;
    /// retain its key across reloads. None if the runtime cannot allocate a name slot.
    fn key(&self, _id: &Identifier) -> Option<TagKey> {
        crate::world::phase::unprovided(
            "Tags::key",
            "tags_key",
            crate::world::phase::LoadAccess::Metadata,
        )
    }
    /// Whether `item` is in the current item tag. An unknown tag is false;
    /// unavailable authoritative tags or item mappings answer None.
    fn is_in_tag(&self, _item: ItemKey, _tag: TagKey) -> Option<bool> {
        crate::world::phase::unprovided(
            "Tags::is_in_tag",
            "tags_is_in_tag",
            crate::world::phase::LoadAccess::Metadata,
        )
    }
    /// Read the current item tag, including unrequested vanilla items, in vanilla's order.
    /// An absent tag is [`ItemTagMembers::Missing`], not a loaded empty list.
    /// Before the authoritative item tags are applied, or if a required key mapping
    /// cannot be resolved, the result is [`ItemTagMembers::Unavailable`]. A retained
    /// key reads the current snapshot on each call, including after reload.
    fn items(&self, _tag: TagKey) -> ItemTagMembers {
        crate::world::phase::unprovided(
            "Tags::items",
            "tags_items",
            crate::world::phase::LoadAccess::Metadata,
        )
    }
    /// Fluid members in vanilla's order. Unknown tags are empty;
    /// unavailable authoritative tags or fluid mappings answer None.
    fn fluids(&self, _tag: TagKey) -> Option<Vec<FluidKey>> {
        crate::world::phase::unprovided(
            "Tags::fluids",
            "tags_fluids",
            crate::world::phase::LoadAccess::Metadata,
        )
    }
    /// Block members in vanilla's order, including unrequested vanilla blocks.
    /// Unknown tags are empty; unavailable authoritative tags or block mappings answer None.
    fn blocks(&self, _tag: TagKey) -> Option<Vec<BlockKey>> {
        crate::world::phase::unprovided(
            "Tags::blocks",
            "tags_blocks",
            crate::world::phase::LoadAccess::Metadata,
        )
    }
}

/// Resolve a tag name at the game boundary.
pub fn tag_key(id: &Identifier) -> Option<TagKey> {
    sunrise_mod_runtime::registry_keys::tag_slot(id).map(TagKey)
}

/// Borrow a tag's canonical name within the runtime's read lock, without formatting or cloning.
pub fn with_tag_name<R>(key: TagKey, read: impl FnOnce(Option<&str>) -> R) -> R {
    sunrise_mod_runtime::registry_keys::with_tag_name(key.0, read)
}
