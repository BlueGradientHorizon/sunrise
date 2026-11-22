//! The world's datafix chain applied to one saved compound, without a chunk or level.
use super::{CURRENT, Ty, dynamic, identifier, key, schema, tables, update};
use sunrise_core::nbt::{Compound, JavaString, Tag};

/// The deepest nesting accepted: a list or compound at depth `MAX_DEPTH` (the root compound is at 0) is refused.
/// The depth cap of every NBT read the game makes (`nbt::Limits`, vanilla's `NbtAccounter` default of 512), counted
/// on the encoded form: the compound the NBT writer puts around an element of a mixed list is a level of its own,
/// so such an element sits two levels below its list. There is no size cap: vanilla puts no quota on saved data
/// and never limits fixing or writing it.
pub const MAX_DEPTH: usize = 512;

/// `DataFixers.addFixers` starts with schema V99. World loaders can infer a missing version;
/// a caller supplying a standalone compound needs to select an actual source schema.
const FIRST_SCHEMA: i32 = 99;

/// A vanilla reference type whose root is a compound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompoundType {
    /// `References.ITEM_STACK`.
    ItemStack,
    /// `References.BLOCK_STATE`.
    BlockState,
    /// `References.BLOCK_ENTITY`.
    BlockEntity,
    /// `References.ENTITY`.
    Entity,
    /// `References.ENTITY_TREE`.
    EntityTree,
}

/// Why a value was not upgraded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpgradeError {
    /// The data version is below zero.
    NegativeVersion(i32),
    /// The data version is newer than the game's ([`CURRENT`]): the fixes only go forward.
    FutureVersion(i32),
    /// No source schema for that version.
    UnsupportedVersion(i32),
    /// The compound has no readable discriminator for the selected type, or the chain could not read it.
    MalformedCompound,
    /// The prepared tables used by the legacy fixers could not be loaded.
    LegacyTablesUnavailable,
    /// The input has a list or compound [`MAX_DEPTH`] levels deep or more, as encoded: refused before any fix.
    TooDeep,
    /// The fixes made the value deeper (1.9's `Riding` to `Passengers` doubles a chain's depth) and the result has a
    /// list or compound [`MAX_DEPTH`] levels deep or more, as encoded: no NBT read, vanilla's included, could take
    /// it back.
    UpgradedTooDeep,
}

impl std::fmt::Display for UpgradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NegativeVersion(v) => write!(f, "data version {v} is negative"),
            Self::FutureVersion(v) => {
                write!(f, "data version {v} is newer than the game's ({CURRENT})")
            }
            Self::UnsupportedVersion(v) => write!(f, "no data fixer schema for version {v}"),
            Self::MalformedCompound => f.write_str("malformed saved compound"),
            Self::LegacyTablesUnavailable => {
                f.write_str("could not load the legacy datafix tables")
            }
            Self::TooDeep => write!(f, "the data nests {MAX_DEPTH} levels deep or more"),
            Self::UpgradedTooDeep => {
                write!(f, "the upgraded data nests {MAX_DEPTH} levels deep or more")
            }
        }
    }
}

impl std::error::Error for UpgradeError {}

/// The metadata-only mod Host operation, shared by game and chunk worker hosts.
#[doc(hidden)]
pub fn upgrade_mod_nbt(
    kind: mod_api::NbtDataType,
    data_version: i32,
    nbt: sunrise_core::nbt::Compound,
) -> Result<sunrise_core::nbt::Compound, mod_api::NbtUpgradeError> {
    use mod_api::{NbtDataType, NbtUpgradeError};

    let kind = match kind {
        NbtDataType::ItemStack => CompoundType::ItemStack,
        NbtDataType::BlockState => CompoundType::BlockState,
        NbtDataType::BlockEntity => CompoundType::BlockEntity,
        NbtDataType::Entity => CompoundType::Entity,
        NbtDataType::EntityTree => CompoundType::EntityTree,
    };
    upgrade_compound(kind, data_version, nbt).map_err(|error| match error {
        UpgradeError::NegativeVersion(v) | UpgradeError::UnsupportedVersion(v) => {
            NbtUpgradeError::UnsupportedVersion(v)
        }
        UpgradeError::FutureVersion(v) => NbtUpgradeError::FutureVersion(v),
        UpgradeError::MalformedCompound => NbtUpgradeError::MalformedCompound,
        UpgradeError::LegacyTablesUnavailable => NbtUpgradeError::LegacyTablesUnavailable,
        UpgradeError::TooDeep => NbtUpgradeError::TooDeep,
        UpgradeError::UpgradedTooDeep => NbtUpgradeError::UpgradedTooDeep,
    })
}

/// `DataFixer.update` at one vanilla root reference. No world context is added or used.
pub fn upgrade_compound(
    kind: CompoundType,
    data_version: i32,
    tag: Compound,
) -> Result<Compound, UpgradeError> {
    if data_version < FIRST_SCHEMA {
        dismantle(Tag::Compound(tag));
        return Err(UpgradeError::UnsupportedVersion(data_version));
    }
    let tag = checked(data_version, tag)?;
    let root = match kind {
        CompoundType::ItemStack => Ty::ItemStack,
        CompoundType::BlockState => Ty::BlockState,
        CompoundType::BlockEntity => Ty::BlockEntity,
        CompoundType::Entity => Ty::Entity,
        CompoundType::EntityTree => Ty::EntityTree,
    };
    let discriminator = if root == Ty::BlockState { "Name" } else { "id" };
    let field = tag.get(&JavaString::from(discriminator));
    let name = match field {
        Some(Tag::String(name)) if !name.0.is_empty() => Some(name),
        // V99's item name is a number or a string. `ItemIdFix` removes the numeric form in V102.
        Some(id)
            if root == Ty::ItemStack && data_version < 102 && dynamic::as_number(id).is_some() =>
        {
            None
        }
        _ => return Err(UpgradeError::MalformedCompound),
    };
    let tagged_choice = matches!(root, Ty::Entity | Ty::EntityTree | Ty::BlockEntity);
    // The entity choices do not read mod namespaces. Item names are not a tagged choice:
    // the item fixers keep the id but still update shared stack fields.
    if data_version == CURRENT
        || tagged_choice
            && name.is_some_and(|name| {
                identifier(&name.to_string_lossy()).is_some_and(|(ns, _)| ns != "minecraft")
            })
    {
        return Ok(tag);
    }
    let from = key(data_version as u32, 0);
    let value = Tag::Compound(tag);
    let known = tables::standalone(|| {
        if root == Ty::BlockEntity {
            schema::block_entity_known(&value, from)
        } else if matches!(root, Ty::Entity | Ty::EntityTree) && from < schema::flat::END {
            schema::flat::entity_tree_known(&value, from)
        } else {
            true
        }
    })?;
    if !known {
        return into_compound(value);
    }
    if tagged_choice {
        // `Type.readAndWrite` encodes into the source codec's decode remainder. The
        // tagged-choice roots leave the original compound as that prefix; `NbtOps` lays
        // the fixed keys over it. Nested records, item stacks and block states do not
        // have this root prefix.
        let mut prefix = into_compound(value.clone())?;
        for (key, tag) in upgraded(root, value, data_version)? {
            prefix.insert(key, tag);
        }
        Ok(prefix)
    } else {
        upgraded(root, value, data_version)
    }
}

/// One block entity's NBT saved at `data_version`, upgraded as the chunk loader upgrades the block entities of a
/// chunk of that version: each block entity type's fixes, up to [`CURRENT`]. Data of the current version comes back
/// as it is, and so does a block entity whose `id` that version doesn't know, as a chunk keeps it. Fixes of the
/// whole chunk (the blocks around it, the removal of bed block entities) have no chunk to act on and don't run. A
/// tag the fixes can't read comes back as it was saved. Refused before any fix runs: a negative or future version,
/// and a value with a list or compound [`MAX_DEPTH`] deep ([`UpgradeError::TooDeep`]); after the fixes, a result
/// that deep ([`UpgradeError::UpgradedTooDeep`]).
pub fn upgrade_block_entity(data_version: i32, tag: Compound) -> Result<Compound, UpgradeError> {
    let tag = checked(data_version, tag)?;
    let id = Tag::compound([(
        "id",
        tag.get(&JavaString::from("id"))
            .cloned()
            .unwrap_or(Tag::End),
    )]);
    // Before 1.13 the known types come from the fixers' tables too: a failure there keeps the data, as a fix's would.
    let known =
        std::panic::catch_unwind(|| schema::block_entity_known(&id, key(data_version as u32, 0)));
    if known.unwrap_or(false) {
        upgraded_or_original(Ty::BlockEntity, tag, data_version)
    } else {
        Ok(tag)
    }
}

/// One entity's NBT saved at `data_version`, upgraded as the entity chunk loader upgrades the entities of a chunk of
/// that version: each entity type's fixes, its passengers' included, up to [`CURRENT`]. Data of the current version
/// comes back as it is; a tag the fixes can't read comes back as it was saved. Refused as
/// [`upgrade_block_entity`] refuses.
pub fn upgrade_entity(data_version: i32, tag: Compound) -> Result<Compound, UpgradeError> {
    let tag = checked(data_version, tag)?;
    upgraded_or_original(Ty::EntityTree, tag, data_version)
}

/// `tag` when its version can be upgraded from and it is within [`MAX_DEPTH`]. A refused value is taken apart
/// without recursion, as a value too deep could not be dropped otherwise.
fn checked(data_version: i32, tag: Compound) -> Result<Compound, UpgradeError> {
    let refusal = if data_version < 0 {
        Some(UpgradeError::NegativeVersion(data_version))
    } else if data_version > CURRENT {
        Some(UpgradeError::FutureVersion(data_version))
    } else {
        (!within_depth(&tag)).then_some(UpgradeError::TooDeep)
    };
    match refusal {
        None => Ok(tag),
        Some(error) => {
            dismantle(Tag::Compound(tag));
            Err(error)
        }
    }
}

/// The permissive standalone readers preserve unreadable data, as chunk reads do.
fn upgraded_or_original(
    root: Ty,
    tag: Compound,
    data_version: i32,
) -> Result<Compound, UpgradeError> {
    if data_version == CURRENT {
        return Ok(tag);
    }
    match upgraded(root, Tag::Compound(tag.clone()), data_version) {
        Err(UpgradeError::MalformedCompound) => Ok(tag),
        result => result,
    }
}

/// Run the shared chain. Legacy fixes can panic on malformed data; never return a partially fixed value.
fn upgraded(root: Ty, mut value: Tag, data_version: i32) -> Result<Compound, UpgradeError> {
    let run = std::panic::AssertUnwindSafe(|| update(root, &mut value, data_version, CURRENT));
    if let Err(error) = tables::standalone(run) {
        dismantle(value);
        return Err(error);
    }
    let upgraded = into_compound(value)?;
    if within_depth(&upgraded) {
        Ok(upgraded)
    } else {
        dismantle(Tag::Compound(upgraded));
        Err(UpgradeError::UpgradedTooDeep)
    }
}

fn into_compound(value: Tag) -> Result<Compound, UpgradeError> {
    match value {
        Tag::Compound(tag) => Ok(tag),
        _ => {
            dismantle(value);
            Err(UpgradeError::MalformedCompound)
        }
    }
}

/// Whether no list or compound of `tag` sits [`MAX_DEPTH`] levels deep or more as the NBT writer encodes it, walked
/// without recursion. A list written as a list of compounds (all compounds, or mixed types) puts each element that
/// is no compound, or is a compound that looks like a wrapper (one entry with an empty name), in a compound of its
/// own: that wrapper is a level, as it is on disk and on the wire, and the element sits below it.
fn within_depth(tag: &Compound) -> bool {
    let mut stack: Vec<(&Tag, usize)> = tag.values().map(|value| (value, 1)).collect();
    while let Some((value, depth)) = stack.pop() {
        match value {
            Tag::List(items) => {
                if depth >= MAX_DEPTH {
                    return false;
                }
                let first = items.first().map_or(0, Tag::id);
                let compounds = first == 10 || items.iter().any(|item| item.id() != first);
                for item in items {
                    let wrapper = matches!(item, Tag::Compound(c)
                        if c.len() == 1 && c.contains_key(&JavaString::default()));
                    if compounds && (item.id() != 10 || wrapper) {
                        // The wrapper at `depth + 1`, the element inside it at `depth + 2`.
                        if depth + 1 >= MAX_DEPTH {
                            return false;
                        }
                        stack.push((item, depth + 2));
                    } else {
                        stack.push((item, depth + 1));
                    }
                }
            }
            Tag::Compound(entries) => {
                if depth >= MAX_DEPTH {
                    return false;
                }
                stack.extend(entries.values().map(|item| (item, depth + 1)));
            }
            _ => {}
        }
    }
    true
}

/// Drops `tag` without recursion: every nested list and compound is emptied into a work list first.
fn dismantle(tag: Tag) {
    let mut pending = vec![tag];
    while let Some(value) = pending.pop() {
        match value {
            Tag::List(items) => pending.extend(items),
            Tag::Compound(entries) => pending.extend(entries.into_values()),
            _ => {}
        }
    }
}
