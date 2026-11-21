//! Local consume-effect codecs, keyed by name independently of any wire ID assignment.
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};
use sunrise_core::nbt::{Compound, Tag};

/// An admitted effect: the codec value's canonical saved map and its typed Wire form.
#[derive(Clone, Debug, PartialEq)]
pub struct Admitted {
    pub name: Arc<str>,
    pub saved: Arc<Tag>,
    pub wire: Arc<[u8]>,
}

type Load = dyn Fn(&Compound) -> Result<(Compound, Vec<u8>), String> + Send + Sync;
type Read = dyn Fn(&[u8], bool) -> Result<(usize, Option<Compound>), String> + Send + Sync;

/// The type-erased adapters for a mod's map and Wire codecs.
pub struct Type {
    pub schema: u64,
    pub load: Box<Load>,
    pub read: Box<Read>,
}

static TYPES: OnceLock<BTreeMap<Arc<str>, Type>> = OnceLock::new();

pub fn install(types: BTreeMap<Arc<str>, Type>) -> Result<(), String> {
    TYPES
        .set(types)
        .map_err(|_| "consume effect types already installed".into())
}

pub fn entries() -> impl Iterator<Item = (&'static Arc<str>, &'static Type)> {
    TYPES.get().into_iter().flatten()
}

pub fn by_name(name: &str) -> Option<(&'static Arc<str>, &'static Type)> {
    TYPES.get()?.get_key_value(name)
}

impl Admitted {
    pub fn new(name: &Arc<str>, mut fields: Compound, wire: Arc<[u8]>) -> Self {
        fields.insert("type".into(), Tag::String(name.as_ref().into()));
        Self {
            name: name.clone(),
            saved: Arc::new(Tag::Compound(fields)),
            wire,
        }
    }
}

/// Decode the registered map codec at persistent admission, not at packet send.
/// `None` means the key is not registered, not that its registered codec refused the value.
pub fn admit(name: &str, fields: &Compound) -> Result<Option<Admitted>, String> {
    let Some((name, ty)) = by_name(name) else {
        return Ok(None);
    };
    let mut fields = fields.clone();
    fields.remove(&"type".into());
    let (saved, wire) = (ty.load)(&fields)?;
    Ok(Some(Admitted::new(name, saved, wire.into())))
}
