//! `ConsumeEffect.Type` dispatch for the extension entries in the synchronized registry.
use crate::{Error, Result, buf::Reader};
use std::sync::OnceLock;
use sunrise_core::nbt::{Compound, Tag};

pub const REGISTRY: &str = "minecraft:consume_effect_type";
pub const BUILTINS: [&str; 5] = [
    "minecraft:apply_effects",
    "minecraft:remove_effects",
    "minecraft:clear_all_effects",
    "minecraft:teleport_randomly",
    "minecraft:play_sound",
];

type FromStream =
    dyn Fn(&[u8], bool) -> std::result::Result<(usize, Option<Compound>), String> + Send + Sync;
type ToStream = dyn Fn(&Compound) -> std::result::Result<Vec<u8>, String> + Send + Sync;

/// The transport half of a registered type. Callers supply the existing Wire codec,
/// specialized to the mod's type; protocol never needs to know its Rust type.
pub struct Type {
    pub name: String,
    pub wire_schema: u64,
    pub from_stream: Box<FromStream>,
    pub to_stream: Box<ToStream>,
}

static TYPES: OnceLock<Vec<Type>> = OnceLock::new();

/// Freeze the native extension IDs after all registrations have completed.
pub fn install(mut types: Vec<Type>) -> Result<()> {
    types.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    TYPES
        .set(types)
        .map_err(|_| Error::new("consume effect types already installed"))
}

pub fn entries() -> impl Iterator<Item = (i32, &'static Type)> {
    TYPES
        .get()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, ty)| ((BUILTINS.len() + index) as i32, ty))
}

/// Frozen registry rows in protocol ID order. Built-in streams have the
/// pinned protocol layout; extension entries publish their Wire schema.
pub fn configuration() -> impl Iterator<Item = (&'static str, u64)> {
    BUILTINS
        .into_iter()
        .map(|name| (name, 0))
        .chain(entries().map(|(_, ty)| (ty.name.as_str(), ty.wire_schema)))
}

pub fn id(name: &str) -> Option<i32> {
    if let Some(id) = BUILTINS.iter().position(|&key| key == name) {
        return Some(id as i32);
    }
    let types = TYPES.get()?;
    let index = types
        .binary_search_by(|ty| ty.name.as_str().cmp(name))
        .ok()?;
    Some((BUILTINS.len() + index) as i32)
}

fn by_id(id: i32) -> Result<&'static Type> {
    usize::try_from(id)
        .ok()
        .and_then(|id| id.checked_sub(BUILTINS.len()))
        .and_then(|index| TYPES.get()?.get(index))
        .ok_or_else(|| Error::new(format!("unknown consume effect type {id}")))
}

pub(super) fn write_saved(kind: i32, mut data: Compound) -> Result<Vec<u8>> {
    let ty = by_id(kind)?;
    data.remove(&"type".into());
    (ty.to_stream)(&data).map_err(|why| Error::new(format!("consume effect {}: {why}", ty.name)))
}

pub(super) fn skip(kind: i32, reader: &mut Reader<'_>) -> Result<()> {
    let ty = by_id(kind)?;
    let (size, _) = (ty.from_stream)(reader.unread(), false)
        .map_err(|why| Error::new(format!("consume effect {}: {why}", ty.name)))?;
    reader.bytes(size)?;
    Ok(())
}

/// Read the registered stream form and project it onto the persistent dispatch map.
pub fn read_saved(kind: i32, reader: &mut Reader<'_>) -> Result<Tag> {
    let ty = by_id(kind)?;
    let (size, data) = (ty.from_stream)(reader.unread(), true).map_err(|why| {
        let error = Error::new(format!("consume effect {}: {why}", ty.name));
        log::warn!("Cannot project consume effect from stream: {error}");
        error
    })?;
    reader.bytes(size)?;
    let mut data = data.ok_or_else(|| Error::new("missing saved consume effect fields"))?;
    data.insert("type".into(), Tag::String(ty.name.as_str().into()));
    Ok(Tag::Compound(data))
}
