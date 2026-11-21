//! Registration of the persistent and stream codecs for a custom consume effect.
use crate::{Compound, Identifier, callback_owner::CallbackOwner, wire};
use std::sync::Arc;

// no wire: this is a process-local codec registration; only its values have a Wire form.
/// A consume-effect type's save codec and its typed `Wire` stream form.
pub struct ConsumeEffectType {
    install: Box<InstallCodec>,
}

type InstallCodec = dyn Fn(&'static str) -> Result<ConsumeEffectCodecs, String> + Send + Sync;
type Load = dyn Fn(&Compound) -> Result<(Compound, Vec<u8>), String> + Send + Sync;
type Read = dyn Fn(&[u8], bool) -> Result<(usize, Option<Compound>), String> + Send + Sync;

// no wire: type-erased engine adapter for the map and Wire codec callbacks.
#[doc(hidden)]
pub struct ConsumeEffectCodecs {
    pub schema: u64,
    pub load: Box<Load>,
    pub read: Box<Read>,
}

impl ConsumeEffectType {
    /// Construct a type from its persistent map codec. Implement the stream
    /// form by deriving [`Wire`](crate::Wire) on `T`. The game handles the
    /// dispatch key `type`; neither function reads or writes that field.
    /// `load` validates saved data before the stack is admitted; `save` writes
    /// the canonical fields of that value, including when received over Wire.
    /// Neither callback depends on the live gameplay listener being enabled.
    /// Both can run on load and network threads. Return an error for invalid
    /// data rather than panicking.
    pub fn new<T: wire::Wire + 'static>(
        load: fn(&Compound) -> Result<T, String>,
        save: fn(&T) -> Result<Compound, String>,
    ) -> Self {
        let wire_schema = T::describe(&mut wire::WireSchema::default());
        Self {
            install: Box::new(move |mod_id| {
                let owner = || {
                    CallbackOwner::new(mod_id)
                        .ok_or_else(|| format!("unknown consume effect codec owner {mod_id}"))
                };
                let loader = owner()?;
                let saver = Arc::new(owner()?);
                let stream = owner()?;
                let save_value = move |value: &T| {
                    let result = saver
                        .call_registered("consume effect save codec", "consume_effect_save", || {
                            save(value)
                        })
                        .and_then(|value| value);
                    if let Err(why) = &result {
                        log::warn!("mod {mod_id}: consume effect save refused: {why}");
                    }
                    result
                };
                let save_for_load = save_value.clone();
                Ok(ConsumeEffectCodecs {
                    schema: wire_schema,
                    load: Box::new(move |fields| {
                        let result = loader
                            .call_registered(
                                "consume effect load codec",
                                "consume_effect_load",
                                || {
                                    let value = load(fields)?;
                                    let mut out = wire::Writer::new();
                                    out.put(&value);
                                    if let Some(error) = out.error() {
                                        return Err(error.to_string());
                                    }
                                    Ok((value, out.into_bytes()))
                                },
                            )
                            .and_then(|value| value);
                        if let Err(why) = &result {
                            log::warn!("mod {mod_id}: consume effect load refused: {why}");
                        }
                        let (value, bytes) = result?;
                        Ok((save_for_load(&value)?, bytes))
                    }),
                    read: Box::new(move |bytes, project| {
                        let (size, value) = stream.call_registered(
                            "consume effect Wire codec",
                            "consume_effect_wire",
                            || {
                                let mut reader = wire::Reader::new(bytes);
                                let value =
                                    reader.take::<T>().map_err(|error| error.to_string())?;
                                Ok::<_, String>((bytes.len() - reader.rest().len(), value))
                            },
                        )??;
                        let saved = if project {
                            save_value(&value).ok()
                        } else {
                            None
                        };
                        Ok((size, saved))
                    }),
                })
            }),
        }
    }
}

/// Consume-effect type registration.
pub trait ConsumeEffectsRegistrar {
    /// Register a custom kind during `Hooks::register`. The registry and stream
    /// schema are synchronized before play. Both peers install the same type;
    /// the identifier must be in an owned, non-reserved package namespace.
    /// Duplicate IDs and server-only packages are refused.
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

/// Take the registered engine-side codecs after mod registration has finished.
#[doc(hidden)]
pub fn consume_effect_codecs() -> Result<Vec<(String, ConsumeEffectCodecs)>, String> {
    let Some(parts) = super::installed() else {
        return Ok(Vec::new());
    };
    parts
        .consume_effects
        .iter()
        .map(|effect| Ok((effect.id.to_string(), (effect.ty.install)(effect.owner)?)))
        .collect()
}
