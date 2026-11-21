//! Bound item tint sources, item model node types, and range/select item model property
//! providers registered at client startup (`register_item_tint_sources`, `register_item_models`,
//! `register_range_select_item_model_property`, `register_select_item_model_property`).
//!
//! Each table follows NeoForge's `ExtraCodecs.LateBoundIdMapper`: any valid `<namespace>:<path>`
//! identifier (including `minecraft:`) is accepted, a later registration for the same id replaces
//! the earlier one (last-writer-wins) with a warning naming both mods, and registered decoders run
//! ahead of built-in decoders so vanilla overrides take effect.
use super::SpecialModelFactories;
use mod_api::{
    __private::game::{
        CallbackOwner, ItemModelRegistration, ItemTintSourceRegistration,
        RangeSelectPropertyRegistration, SelectPropertyRegistration,
    },
    Answered, Identifier, ItemModelDecoder, ItemModelFn, ItemTintSourceDecoder, ItemTintSourceFn,
    RangeSelectPropertyDecoder, RangeSelectPropertyFn, SelectPropertyDecoder, SelectPropertyFn,
};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, OnceLock},
};
use sunrise_assets::item_model::{
    RegisteredItemModel, RegisteredRangeProperty, RegisteredSelectProperty, RegisteredTintSource,
};
use sunrise_core::nbt::Tag;

/// One installed decode-time factory paired with the mod owner that registered it.
pub struct ItemProviderEntry<D: ?Sized> {
    owner: Arc<CallbackOwner>,
    decode: Arc<D>,
}

pub type ItemTintFactories = BTreeMap<String, ItemProviderEntry<ItemTintSourceDecoder>>;
pub type ItemModelFactories = BTreeMap<String, ItemProviderEntry<ItemModelDecoder>>;
pub type RangePropertyFactories = BTreeMap<String, ItemProviderEntry<RangeSelectPropertyDecoder>>;
pub type SelectPropertyFactories = BTreeMap<String, ItemProviderEntry<SelectPropertyDecoder>>;

/// Bound item tint source stored inside [`RegisteredTintSource`].
pub struct BoundTintSource {
    pub kind: Box<str>,
    pub owner: Arc<CallbackOwner>,
    pub eval: Arc<ItemTintSourceFn>,
}

/// Bound item model node stored inside [`RegisteredItemModel`].
pub struct BoundItemModel {
    pub kind: Box<str>,
    pub owner: Arc<CallbackOwner>,
    pub eval: Arc<ItemModelFn>,
}

/// Bound numeric `range_dispatch` property stored inside [`RegisteredRangeProperty`].
pub struct BoundRangeProperty {
    pub kind: Box<str>,
    pub owner: Arc<CallbackOwner>,
    pub eval: Arc<RangeSelectPropertyFn>,
}

/// Bound categorical `select` property stored inside [`RegisteredSelectProperty`].
pub struct BoundSelectProperty {
    pub kind: Box<str>,
    pub owner: Arc<CallbackOwner>,
    pub eval: Arc<SelectPropertyFn>,
}

/// Decode-time provider tables passed into [`super::ItemRenderer::decode_definition`].
#[derive(Clone, Copy, Default)]
pub struct ItemProviderTables<'a> {
    pub special: Option<&'a SpecialModelFactories>,
    pub tint: Option<&'a ItemTintFactories>,
    pub item_model: Option<&'a ItemModelFactories>,
    pub range_property: Option<&'a RangePropertyFactories>,
    pub select_property: Option<&'a SelectPropertyFactories>,
}

static TINT_FACTORIES: OnceLock<Arc<ItemTintFactories>> = OnceLock::new();
static ITEM_MODEL_FACTORIES: OnceLock<Arc<ItemModelFactories>> = OnceLock::new();
static RANGE_PROPERTY_FACTORIES: OnceLock<Arc<RangePropertyFactories>> = OnceLock::new();
static SELECT_PROPERTY_FACTORIES: OnceLock<Arc<SelectPropertyFactories>> = OnceLock::new();

#[cfg(test)]
#[derive(Default)]
pub(super) struct ProvidersFixtureState {
    pub(super) tint: Option<Arc<ItemTintFactories>>,
    pub(super) item_model: Option<Arc<ItemModelFactories>>,
    pub(super) range_property: Option<Arc<RangePropertyFactories>>,
    pub(super) select_property: Option<Arc<SelectPropertyFactories>>,
}

#[cfg(test)]
static PROVIDERS_FIXTURE: std::sync::Mutex<Option<ProvidersFixtureState>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
static FIXTURE_CHANGED: std::sync::Condvar = std::sync::Condvar::new();

#[cfg(test)]
pub(super) struct ItemProvidersFixture;

#[cfg(test)]
impl ItemProvidersFixture {
    pub(super) fn install(state: ProvidersFixtureState) -> Self {
        let mut fixture = PROVIDERS_FIXTURE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while fixture.is_some() {
            fixture = FIXTURE_CHANGED
                .wait(fixture)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *fixture = Some(state);
        Self
    }

    pub(super) fn tint(sources: Vec<Answered<ItemTintSourceRegistration>>) -> (Self, Vec<String>) {
        let (factories, kept) = bind_item_tint_factories(sources);
        (
            Self::install(ProvidersFixtureState {
                tint: Some(Arc::new(factories)),
                ..Default::default()
            }),
            kept,
        )
    }

    pub(super) fn item_models(models: Vec<Answered<ItemModelRegistration>>) -> (Self, Vec<String>) {
        let (factories, kept) = bind_item_model_factories(models);
        (
            Self::install(ProvidersFixtureState {
                item_model: Some(Arc::new(factories)),
                ..Default::default()
            }),
            kept,
        )
    }
}

#[cfg(test)]
impl Drop for ItemProvidersFixture {
    fn drop(&mut self) {
        let mut fixture = PROVIDERS_FIXTURE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *fixture = None;
        FIXTURE_CHANGED.notify_all();
    }
}

#[derive(Default)]
pub(super) struct ActiveItemProviders {
    tint: Option<Arc<ItemTintFactories>>,
    item_model: Option<Arc<ItemModelFactories>>,
    range_property: Option<Arc<RangePropertyFactories>>,
    select_property: Option<Arc<SelectPropertyFactories>>,
}

impl ActiveItemProviders {
    pub(super) fn current() -> Self {
        #[cfg(test)]
        if let Some(fixture) = PROVIDERS_FIXTURE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            return Self {
                tint: fixture.tint.clone(),
                item_model: fixture.item_model.clone(),
                range_property: fixture.range_property.clone(),
                select_property: fixture.select_property.clone(),
            };
        }
        Self {
            tint: TINT_FACTORIES.get().cloned(),
            item_model: ITEM_MODEL_FACTORIES.get().cloned(),
            range_property: RANGE_PROPERTY_FACTORIES.get().cloned(),
            select_property: SELECT_PROPERTY_FACTORIES.get().cloned(),
        }
    }

    pub(super) fn tables<'a>(
        &'a self,
        special: Option<&'a SpecialModelFactories>,
    ) -> ItemProviderTables<'a> {
        ItemProviderTables {
            special,
            tint: self.tint.as_deref(),
            item_model: self.item_model.as_deref(),
            range_property: self.range_property.as_deref(),
            select_property: self.select_property.as_deref(),
        }
    }
}

fn is_builtin_tint_source(kind: &str) -> bool {
    matches!(
        kind,
        "minecraft:constant"
            | "minecraft:custom_model_data"
            | "minecraft:dye"
            | "minecraft:grass"
            | "minecraft:firework"
            | "minecraft:potion"
            | "minecraft:team"
    )
}

fn is_builtin_item_model(kind: &str) -> bool {
    matches!(
        kind,
        "minecraft:empty"
            | "minecraft:model"
            | "minecraft:composite"
            | "minecraft:condition"
            | "minecraft:select"
            | "minecraft:range_dispatch"
            | "minecraft:special"
            | "minecraft:bundle/selected_item"
            | "neoforge:fluid_container"
    )
}

fn is_builtin_range_property(kind: &str) -> bool {
    matches!(
        kind,
        "minecraft:custom_model_data"
            | "minecraft:bundle/fullness"
            | "minecraft:damage"
            | "minecraft:cooldown"
            | "minecraft:time"
            | "minecraft:compass"
            | "minecraft:crossbow/pull"
            | "minecraft:use_cycle"
            | "minecraft:use_duration"
            | "minecraft:count"
    )
}

fn is_builtin_select_property(kind: &str) -> bool {
    matches!(
        kind,
        "minecraft:custom_model_data"
            | "minecraft:main_hand"
            | "minecraft:charge_type"
            | "minecraft:trim_material"
            | "minecraft:block_state"
            | "minecraft:display_context"
            | "minecraft:local_time"
            | "minecraft:context_entity_type"
            | "minecraft:context_dimension"
            | "minecraft:component"
    )
}

fn bind_factories<D: ?Sized>(
    label: &'static str,
    is_builtin: fn(&str) -> bool,
    entries: impl IntoIterator<Item = (&'static str, Identifier, Arc<D>)>,
) -> (BTreeMap<String, ItemProviderEntry<D>>, Vec<String>) {
    let mut bound: BTreeMap<String, ItemProviderEntry<D>> = BTreeMap::new();
    let mut kept = Vec::new();
    let mut seen = BTreeSet::new();
    for (mod_id, id, decode) in entries {
        if id.namespace().is_empty() || id.path().is_empty() {
            log::warn!("Mod {mod_id}: {label} {id:?} must have a nonempty namespace and path");
            continue;
        }
        let Some(owner) = CallbackOwner::new(mod_id) else {
            log::warn!("{label} {id} has no installed owner {mod_id}");
            continue;
        };
        let key = id.to_string();
        let entry = ItemProviderEntry {
            owner: Arc::new(owner),
            decode,
        };
        if let Some(earlier) = bound.insert(key.clone(), entry) {
            log::warn!(
                "Sunrise: {label} {key} registered by {} is replaced by {mod_id}'s registration",
                earlier.owner.id()
            );
        } else if is_builtin(&key) {
            log::warn!(
                "Sunrise: {label} {key} registered by the game is replaced by {mod_id}'s registration"
            );
        }
        if seen.insert(key.clone()) {
            kept.push(key);
        }
    }
    (bound, kept)
}

/// Bind a batch of attributed item tint source registrations without sealing process globals.
pub fn bind_item_tint_factories(
    sources: Vec<Answered<ItemTintSourceRegistration>>,
) -> (ItemTintFactories, Vec<String>) {
    bind_factories(
        "item tint source",
        is_builtin_tint_source,
        sources
            .into_iter()
            .map(|entry| (entry.mod_id, entry.answer.id, entry.answer.decode)),
    )
}

/// Bind a batch of attributed item model registrations without sealing process globals.
pub fn bind_item_model_factories(
    models: Vec<Answered<ItemModelRegistration>>,
) -> (ItemModelFactories, Vec<String>) {
    bind_factories(
        "item model type",
        is_builtin_item_model,
        models
            .into_iter()
            .map(|entry| (entry.mod_id, entry.answer.id, entry.answer.decode)),
    )
}

/// Bind a batch of attributed numeric `range_dispatch` property registrations without sealing
/// process globals.
pub fn bind_range_property_factories(
    properties: Vec<Answered<RangeSelectPropertyRegistration>>,
) -> (RangePropertyFactories, Vec<String>) {
    bind_factories(
        "item model range property",
        is_builtin_range_property,
        properties
            .into_iter()
            .map(|entry| (entry.mod_id, entry.answer.id, entry.answer.decode)),
    )
}

/// Bind a batch of attributed categorical `select` property registrations without sealing process
/// globals.
pub fn bind_select_property_factories(
    properties: Vec<Answered<SelectPropertyRegistration>>,
) -> (SelectPropertyFactories, Vec<String>) {
    bind_factories(
        "item model select property",
        is_builtin_select_property,
        properties
            .into_iter()
            .map(|entry| (entry.mod_id, entry.answer.id, entry.answer.decode)),
    )
}

/// Install the bound item tint source decoders once at client startup, before resources load.
pub fn install_item_tint_factories(
    sources: Vec<Answered<ItemTintSourceRegistration>>,
) -> Vec<String> {
    let (bound, kept) = bind_item_tint_factories(sources);
    if TINT_FACTORIES.set(Arc::new(bound)).is_err() {
        log::warn!("item tint sources were registered already; these are left out");
        return Vec::new();
    }
    kept
}

/// Install the bound item model node decoders once at client startup, before resources load.
pub fn install_item_model_factories(models: Vec<Answered<ItemModelRegistration>>) -> Vec<String> {
    let (bound, kept) = bind_item_model_factories(models);
    if ITEM_MODEL_FACTORIES.set(Arc::new(bound)).is_err() {
        log::warn!("item model types were registered already; these are left out");
        return Vec::new();
    }
    kept
}

/// Install the bound numeric `range_dispatch` property decoders once at client startup, before
/// resources load.
pub fn install_range_property_factories(
    properties: Vec<Answered<RangeSelectPropertyRegistration>>,
) -> Vec<String> {
    let (bound, kept) = bind_range_property_factories(properties);
    if RANGE_PROPERTY_FACTORIES.set(Arc::new(bound)).is_err() {
        log::warn!("item model range properties were registered already; these are left out");
        return Vec::new();
    }
    kept
}

/// Install the bound categorical `select` property decoders once at client startup, before
/// resources load.
pub fn install_select_property_factories(
    properties: Vec<Answered<SelectPropertyRegistration>>,
) -> Vec<String> {
    let (bound, kept) = bind_select_property_factories(properties);
    if SELECT_PROPERTY_FACTORIES.set(Arc::new(bound)).is_err() {
        log::warn!("item model select properties were registered already; these are left out");
        return Vec::new();
    }
    kept
}

fn decode_entry<D, F>(
    factories: Option<&BTreeMap<String, ItemProviderEntry<D>>>,
    kind: &str,
    fields: &Map<String, Value>,
    callback_kind: &'static str,
) -> Result<Option<(Arc<CallbackOwner>, Arc<F>)>, String>
where
    D: Fn(&Tag) -> Result<Arc<F>, String> + ?Sized,
    F: ?Sized,
{
    let Some(entry) = factories.and_then(|table| table.get(kind)) else {
        return Ok(None);
    };
    let config = sunrise_core::json_nbt::json_to_compound(fields)
        .map(Tag::Compound)
        .map_err(|error| {
            format!("{callback_kind} {kind}: its JSON doesn't convert to NBT: {error}")
        })?;
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::Registration);
    match entry
        .owner
        .call(callback_kind, "decode", || (entry.decode)(&config))
    {
        Ok(Ok(eval)) => Ok(Some((Arc::clone(&entry.owner), eval))),
        Ok(Err(reason)) => Err(format!(
            "mod {} {callback_kind} {kind} refused it: {reason}",
            entry.owner.id()
        )),
        Err(error) => Err(format!("{callback_kind} {kind}: {error}")),
    }
}

/// Decode a registered tint source entry into a bound [`RegisteredTintSource`].
pub fn decode_tint_source(
    factories: Option<&ItemTintFactories>,
    kind: &str,
    fields: &Map<String, Value>,
) -> Result<Option<RegisteredTintSource>, String> {
    decode_entry(factories, kind, fields, "item tint source").map(|bound| {
        bound.map(|(owner, eval)| {
            RegisteredTintSource(Arc::new(BoundTintSource {
                kind: kind.into(),
                owner,
                eval,
            }))
        })
    })
}

/// Decode a registered item model node into a bound [`RegisteredItemModel`].
pub fn decode_item_model(
    factories: Option<&ItemModelFactories>,
    kind: &str,
    fields: &Map<String, Value>,
) -> Result<Option<RegisteredItemModel>, String> {
    decode_entry(factories, kind, fields, "item model").map(|bound| {
        bound.map(|(owner, eval)| {
            RegisteredItemModel(Arc::new(BoundItemModel {
                kind: kind.into(),
                owner,
                eval,
            }))
        })
    })
}

/// Decode a registered numeric `range_dispatch` property into a bound [`RegisteredRangeProperty`].
pub fn decode_range_property(
    factories: Option<&RangePropertyFactories>,
    kind: &str,
    fields: &Map<String, Value>,
) -> Result<Option<RegisteredRangeProperty>, String> {
    decode_entry(factories, kind, fields, "item model range property").map(|bound| {
        bound.map(|(owner, eval)| {
            RegisteredRangeProperty(Arc::new(BoundRangeProperty {
                kind: kind.into(),
                owner,
                eval,
            }))
        })
    })
}

/// Decode a registered categorical `select` property into a bound [`RegisteredSelectProperty`].
pub fn decode_select_property(
    factories: Option<&SelectPropertyFactories>,
    kind: &str,
    fields: &Map<String, Value>,
) -> Result<Option<RegisteredSelectProperty>, String> {
    decode_entry(factories, kind, fields, "item model select property").map(|bound| {
        bound.map(|(owner, eval)| {
            RegisteredSelectProperty(Arc::new(BoundSelectProperty {
                kind: kind.into(),
                owner,
                eval,
            }))
        })
    })
}

#[cfg(test)]
pub(super) fn decode_definition_without_specials(
    text: &str,
    providers: &ItemProviderTables<'_>,
) -> Result<sunrise_assets::item_model::ClientItem, sunrise_assets::DecodeError> {
    use sunrise_assets::item_model::{
        ClientItem, ItemModelDecoders, ItemModelLoader, RangePropertyLoader, SelectPropertyLoader,
        TintSourceLoader,
    };
    let tint =
        |kind: &str, fields: &Map<String, Value>| decode_tint_source(providers.tint, kind, fields);
    let item_model = |kind: &str, fields: &Map<String, Value>| {
        decode_item_model(providers.item_model, kind, fields)
    };
    let range_property = |kind: &str, fields: &Map<String, Value>| {
        decode_range_property(providers.range_property, kind, fields)
    };
    let select_property = |kind: &str, fields: &Map<String, Value>| {
        decode_select_property(providers.select_property, kind, fields)
    };
    let tint_ref: &TintSourceLoader<'_> = &tint;
    let item_model_ref: &ItemModelLoader<'_> = &item_model;
    let range_property_ref: &RangePropertyLoader<'_> = &range_property;
    let select_property_ref: &SelectPropertyLoader<'_> = &select_property;
    let decoders = ItemModelDecoders {
        special: None,
        tint: providers.tint.is_some().then_some(tint_ref),
        item_model: providers.item_model.is_some().then_some(item_model_ref),
        range_property: providers
            .range_property
            .is_some()
            .then_some(range_property_ref),
        select_property: providers
            .select_property
            .is_some()
            .then_some(select_property_ref),
    };
    ClientItem::from_json_with_decoders(text, &decoders)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{
        __private::{
            game::{RegistrationContextNew, take_callback_panic_notices},
            parts_installation::item_key,
        },
        ClientHost, Hooks, ItemStack, ItemTintInput, ModEntry, RegisterItemTintSourcesEvent,
        RegistrationContext,
        testing::FakeHost,
    };
    use sunrise_assets::item_model::{ItemModelNode, TintSource};

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).unwrap()
    }

    #[test]
    fn tint_registration_accepts_minecraft_overrides_builtins_and_last_writer_wins() {
        struct First;
        #[mod_api::__subscriptions]
        impl Hooks for First {
            fn register_item_tint_sources(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterItemTintSourcesEvent<'_>,
            ) {
                event.register(id("minecraft:constant"), |_| Ok(|_, _| 0xFF11_1111));
                event.register(id("minecraft:custom_tint"), |_| Ok(|_, _| 0xFF22_2222));
            }
        }
        struct Second;
        #[mod_api::__subscriptions]
        impl Hooks for Second {
            fn register_item_tint_sources(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterItemTintSourcesEvent<'_>,
            ) {
                event.register(id("minecraft:constant"), |_| Ok(|_, _| 0xFF33_3333));
            }
        }

        let mods: mod_api::Mods = vec![
            ModEntry::new("first", Box::new(First)),
            ModEntry::new("second", Box::new(Second)),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let mut sources = Vec::new();
            mod_api::dispatch_register_item_tint_sources(&RegistrationContext::new(), &mut sources);
            let (factories, kept) = bind_item_tint_factories(sources);
            assert_eq!(kept, ["minecraft:constant", "minecraft:custom_tint"]);

            let providers = ItemProviderTables {
                tint: Some(&factories),
                ..Default::default()
            };
            let item = decode_definition_without_specials(
                r#"{"model":{"type":"minecraft:model","model":"minecraft:item/stick","tints":[
                    {"type":"minecraft:constant","value":0},
                    {"type":"minecraft:custom_tint"}
                ]}}"#,
                &providers,
            )
            .unwrap();
            let ItemModelNode::Model(leaf) = item.model else {
                panic!("expected model leaf");
            };
            let stack = ItemStack::new(item_key(0), 1).unwrap();
            let input = ItemTintInput {
                stack: &stack,
                bundle_selected: -1,
            };
            let host = FakeHost::new();
            let colors: Vec<u32> = leaf
                .tints
                .iter()
                .map(|tint| {
                    let TintSource::Registered(registered) = tint else {
                        panic!("expected bound tint source");
                    };
                    let bound = registered.0.downcast_ref::<BoundTintSource>().unwrap();
                    (bound.eval)(&host, &input)
                })
                .collect();
            assert_eq!(colors, [0xFF33_3333, 0xFF22_2222]);
        });
    }

    #[test]
    fn decode_error_does_not_latch_and_decode_panic_poisons_only_its_owner() {
        struct Refusing;
        #[mod_api::__subscriptions]
        impl Hooks for Refusing {
            fn register_item_tint_sources(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterItemTintSourcesEvent<'_>,
            ) {
                event.register(id("provtest:strict"), |config| match config.get("color") {
                    Some(Tag::Int(color)) => {
                        let color = *color as u32;
                        Ok(move |_: &dyn ClientHost, _: &ItemTintInput<'_>| color)
                    }
                    _ => Err("missing integer color".into()),
                });
            }
        }
        struct Panicking;
        #[mod_api::__subscriptions]
        impl Hooks for Panicking {
            fn register_item_tint_sources(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterItemTintSourcesEvent<'_>,
            ) {
                event.register(
                    id("provtest:boom"),
                    |_| -> Result<fn(&dyn ClientHost, &ItemTintInput<'_>) -> u32, String> {
                        panic!("decoder boom")
                    },
                );
            }
        }

        let mods: mod_api::Mods = vec![
            ModEntry::new("refusing", Box::new(Refusing)),
            ModEntry::new("panicking", Box::new(Panicking)),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let _ = take_callback_panic_notices();
            let mut sources = Vec::new();
            mod_api::dispatch_register_item_tint_sources(&RegistrationContext::new(), &mut sources);
            let (factories, _) = bind_item_tint_factories(sources);
            let providers = ItemProviderTables {
                tint: Some(&factories),
                ..Default::default()
            };

            // A decode refusal rejects that item definition without latching or triggering a panic notice.
            assert!(
                decode_definition_without_specials(
                    r#"{"model":{"type":"minecraft:model","model":"minecraft:item/stick","tints":[{"type":"provtest:strict"}]}}"#,
                    &providers,
                )
                .is_err()
            );
            assert!(take_callback_panic_notices().is_empty());

            // A nested null fails JSON->NBT validation without latching.
            assert!(
                decode_definition_without_specials(
                    r#"{"model":{"type":"minecraft:model","model":"minecraft:item/stick","tints":[{"type":"provtest:strict","extra":null}]}}"#,
                    &providers,
                )
                .is_err()
            );

            // A valid definition still decodes cleanly afterward.
            assert!(
                decode_definition_without_specials(
                    r#"{"model":{"type":"minecraft:model","model":"minecraft:item/stick","tints":[{"type":"provtest:strict","color":123}]}}"#,
                    &providers,
                )
                .is_ok()
            );

            // A panic in a decoder poisons that mod's CallbackOwner and queues a panic notice.
            assert!(
                decode_definition_without_specials(
                    r#"{"model":{"type":"minecraft:model","model":"minecraft:item/stick","tints":[{"type":"provtest:boom"}]}}"#,
                    &providers,
                )
                .is_err()
            );
            assert_eq!(
                take_callback_panic_notices(),
                [("panicking", "item tint source")]
            );
            // Subsequent calls stay disabled without panicking again, and other mods still decode.
            assert!(
                decode_definition_without_specials(
                    r#"{"model":{"type":"minecraft:model","model":"minecraft:item/stick","tints":[{"type":"provtest:boom"}]}}"#,
                    &providers,
                )
                .is_err()
            );
            assert!(take_callback_panic_notices().is_empty());
            assert!(
                decode_definition_without_specials(
                    r#"{"model":{"type":"minecraft:model","model":"minecraft:item/stick","tints":[{"type":"provtest:strict","color":456}]}}"#,
                    &providers,
                )
                .is_ok()
            );
        });
    }
}
