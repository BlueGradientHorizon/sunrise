//! `ConsumeEffect.Type` dispatch through the connection's synchronized registry.
use crate::{Error, Result, buf::Reader, packets::configuration::RegistryEntry};
use sunrise_core::nbt::Tag;
use sunrise_world::component_value::consume_effects::{Admitted, by_name, entries};

pub const REGISTRY: &str = "minecraft:consume_effect_type";
const BUILTINS: [&str; 5] = [
    "minecraft:apply_effects",
    "minecraft:remove_effects",
    "minecraft:clear_all_effects",
    "minecraft:teleport_randomly",
    "minecraft:play_sound",
];

pub(crate) fn builtin_names() -> &'static [&'static str] {
    &BUILTINS
}

pub fn builtin_id(name: &str) -> Option<i32> {
    BUILTINS
        .iter()
        .position(|&key| key == name)
        .map(|i| i as i32)
}

pub fn builtin_name(id: i32) -> Option<&'static str> {
    BUILTINS.get(usize::try_from(id).ok()?).copied()
}

pub fn has_custom_types() -> bool {
    entries().next().is_some()
}

/// Registry rows in the server's protocol ID order. Custom rows carry a Wire schema.
pub fn configuration() -> impl Iterator<Item = (&'static str, u64)> {
    BUILTINS
        .into_iter()
        .map(|name| (name, 0))
        .chain(entries().map(|(name, ty)| (name.as_ref(), ty.schema)))
}

pub(crate) fn resolve_configuration(entries: &[RegistryEntry]) -> Result<Vec<Kind>> {
    let mut seen = std::collections::HashSet::new();
    let mut kinds = Vec::with_capacity(entries.len());
    for entry in entries {
        let name = entry.id.as_str();
        let (kind, expected) = if let Some(id) = builtin_id(name) {
            (Kind::Builtin(id), 0)
        } else if let Some((name, ty)) = by_name(name) {
            (Kind::Native(name, ty), ty.schema)
        } else {
            return Err(Error::new(format!("unknown consume effect {name}")));
        };
        if !seen.insert(name)
            || !matches!(&entry.data, Some(Tag::Long(schema)) if *schema as u64 == expected)
        {
            return Err(Error::new(format!(
                "duplicate or incompatible consume effect {name}"
            )));
        }
        kinds.push(kind);
    }
    if BUILTINS.iter().any(|name| !seen.contains(name)) {
        return Err(Error::new("missing built-in consume effect type"));
    }
    Ok(kinds)
}

/// Stream discriminant after resolving the connection's ID to its registered type.
#[derive(Clone, Copy)]
pub enum Kind {
    Builtin(i32),
    Native(
        &'static std::sync::Arc<str>,
        &'static sunrise_world::component_value::consume_effects::Type,
    ),
}

impl std::fmt::Debug for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin(id) => f.debug_tuple("Builtin").field(id).finish(),
            Self::Native(name, _) => f.debug_tuple("Native").field(name).finish(),
        }
    }
}

pub fn read_kind(reader: &mut Reader<'_>) -> Result<Kind> {
    let id = reader.varint()?;
    reader
        .consume_effect_kind(id)
        .ok_or_else(|| Error::new(format!("unknown consume effect type {id}")))
}

pub(super) fn skip_native(
    ty: &sunrise_world::component_value::consume_effects::Type,
    reader: &mut Reader<'_>,
) -> Result<()> {
    let (size, _) = (ty.read)(reader.unread(), false).map_err(Error::new)?;
    reader.bytes(size)?;
    Ok(())
}

/// Read one structural value and project it onto the persistent map. A mod whose
/// save callback fails loses only that effect; its Wire framing is still usable.
pub fn read_native(
    name: &std::sync::Arc<str>,
    ty: &sunrise_world::component_value::consume_effects::Type,
    reader: &mut Reader<'_>,
) -> Result<Option<Admitted>> {
    let (size, saved) = (ty.read)(reader.unread(), true).map_err(Error::new)?;
    let bytes = reader.bytes(size)?;
    Ok(saved.map(|saved| Admitted::new(name, saved, bytes.into())))
}
