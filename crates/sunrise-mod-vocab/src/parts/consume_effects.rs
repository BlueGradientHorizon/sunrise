//! Registration of the persistent and stream codecs for a custom consume effect.
use crate::{Compound, Identifier, callback_owner::CallbackOwner, wire};
use std::sync::Arc;
use sunrise_protocol::item::consume_effects;

// no wire: this is a process-local codec registration; only its values have a Wire form.
/// A consume-effect type's save codec and its typed `Wire` stream form.
pub struct ConsumeEffectType {
    wire_schema: u64,
    save_to_stream: Arc<SaveToStream>,
    stream_to_save: Arc<StreamToSave>,
}

type SaveToStream = dyn Fn(&Compound) -> Result<Vec<u8>, String> + Send + Sync;
type StreamToSave = dyn Fn(&[u8], bool) -> Result<(usize, Option<Compound>), String> + Send + Sync;

impl ConsumeEffectType {
    /// Construct a type from its persistent map codec. Implement the stream
    /// form by deriving [`Wire`](crate::Wire) on `T`. The game handles the
    /// dispatch key `type`; neither function reads or writes that field.
    /// Both functions run on whichever thread encodes or decodes the stack.
    /// Return an error for invalid data rather than panicking.
    pub fn new<T: wire::Wire + 'static>(
        load: fn(&Compound) -> Result<T, String>,
        save: fn(&T) -> Result<Compound, String>,
    ) -> Self {
        Self {
            wire_schema: T::describe(&mut wire::WireSchema::default()),
            save_to_stream: Arc::new(move |fields| {
                let value = load(fields)?;
                let mut out = wire::Writer::new();
                out.put(&value);
                if let Some(error) = out.error() {
                    return Err(error.to_string());
                }
                Ok(out.into_bytes())
            }),
            stream_to_save: Arc::new(move |bytes, project| {
                let mut reader = wire::Reader::new(bytes);
                let value = reader.take::<T>().map_err(|error| error.to_string())?;
                let saved = project.then(|| save(&value)).transpose()?;
                Ok((bytes.len() - reader.rest().len(), saved))
            }),
        }
    }
}

/// Consume-effect type registration.
pub trait ConsumeEffectsRegistrar {
    /// Register a custom kind during `Hooks::register`. The registry and stream
    /// schema are synchronized before play. Both peers install the same type;
    /// a duplicate ID or a package not installed on both sides is refused.
    fn register_consume_effect_type(
        &mut self,
        id: Identifier,
        ty: ConsumeEffectType,
    ) -> Result<(), String>;
}

// no wire: engine-owned collection entry with process-local callbacks.
/// An attributed consume-effect registration, not exposed to mods.
#[doc(hidden)]
pub struct RegisteredConsumeEffect {
    /// Mod that supplied the codecs.
    pub owner: &'static str,
    /// Canonical dispatch key.
    pub id: Identifier,
    /// Registered codecs.
    pub ty: ConsumeEffectType,
}

/// Reserve the built-in registry entries before installing mod entries.
#[doc(hidden)]
pub fn is_builtin_consume_effect(id: &str) -> bool {
    consume_effects::BUILTINS.contains(&id)
}

/// Freeze the registry before stacks or sessions are opened.
#[doc(hidden)]
pub fn freeze_consume_effect_types() -> Result<(), String> {
    let Some(parts) = super::installed() else {
        return Ok(());
    };
    if parts.consume_effects.is_empty() {
        return Ok(());
    }
    let mut types = Vec::with_capacity(parts.consume_effects.len());
    for effect in &parts.consume_effects {
        let owner = Arc::new(CallbackOwner::new(effect.owner).ok_or_else(|| {
            format!(
                "mod {} has no consume effect registration owner",
                effect.owner
            )
        })?);
        let reader_owner = Arc::clone(&owner);
        let save_to_stream = Arc::clone(&effect.ty.save_to_stream);
        let stream_to_save = Arc::clone(&effect.ty.stream_to_save);
        types.push(consume_effects::Type {
            name: effect.id.to_string(),
            wire_schema: effect.ty.wire_schema,
            from_stream: Box::new(move |bytes, project| {
                reader_owner.call("consume effect codec", "consume_effect_from_stream", || {
                    stream_to_save(bytes, project)
                })?
            }),
            to_stream: Box::new(move |data| {
                owner.call("consume effect codec", "consume_effect_to_stream", || {
                    save_to_stream(data)
                })?
            }),
        });
    }
    consume_effects::install(types).map_err(|error| error.to_string())
}
