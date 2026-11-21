//! Bound item tint sources, item model types, and range/select item model property providers
//! registered at client startup (`register_item_tint_sources`, `register_item_models`,
//! `register_range_select_item_model_property`, `register_select_item_model_property`).
use crate::{
    ClientHost, Identifier, ItemModelContext, ItemModelLayer, ItemStack, RegisterItemModelsEvent,
    RegisterItemTintSourcesEvent, RegisterRangeSelectItemModelPropertyEvent,
    RegisterSelectItemModelPropertyEvent, Tag,
};
use std::sync::Arc;

/// Borrowed input passed to a bound item tint source callback (`ItemTintSource.calculate`).
// no wire: borrowed per-stack item tint evaluation input.
// exhaustive: the game constructs this view; mods only read it.
pub struct ItemTintInput<'a> {
    /// Owned snapshot of the stack being resolved (including nested bundle items).
    pub stack: &'a ItemStack,
    /// Runtime bundle selection index (`BundleContents.selectedItem`), or `-1` when none.
    pub bundle_selected: i32,
}

/// A bound item tint source instance produced at item definition decode time and invoked per stack.
// no wire: retained process-local item tint source callback.
pub type ItemTintSourceFn = dyn Fn(&dyn ClientHost, &ItemTintInput<'_>) -> u32 + Send + Sync;

/// A decode-time factory bound at `register_item_tint_sources` that parses a tint entry's JSON
/// object (as a [`Tag`]) once at resource load and returns the per-stack tint callback.
// no wire: retained process-local item tint source decoder callback.
pub type ItemTintSourceDecoder =
    dyn Fn(&Tag) -> Result<Arc<ItemTintSourceFn>, String> + Send + Sync;

/// A staged item tint source decoder; dispatch attributes it to the registering mod.
// no wire: retained process-local item tint source decoder registration.
#[doc(hidden)]
#[non_exhaustive]
pub struct ItemTintSourceRegistration {
    /// The tint source type identifier.
    pub id: Identifier,
    /// The decode-time factory invoked once per matching tint entry during resource load.
    pub decode: Arc<ItemTintSourceDecoder>,
}

impl RegisterItemTintSourcesEvent<'_> {
    /// Register an item tint source decoder under `id` (NeoForge's
    /// `RegisterColorHandlersEvent.ItemTintSources::register`).
    ///
    /// Any valid `<namespace>:<path>` identifier (including `minecraft:`) is accepted; a later
    /// registration for the same id replaces the earlier one (last-writer-wins, matching
    /// NeoForge's `LateBoundIdMapper`), and registering a built-in vanilla tint source id
    /// overrides that built-in when decoding item definitions.
    ///
    /// `decode` runs once per matching tint entry on each resource load and reload with the
    /// entry's JSON object converted to a [`Tag`], returning the bound per-stack ARGB colour
    /// callback (or `Err(reason)` to reject the enclosing item definition without falling back to
    /// a built-in).
    pub fn register<D, F>(&mut self, id: Identifier, decode: D)
    where
        D: Fn(&Tag) -> Result<F, String> + Send + Sync + 'static,
        F: Fn(&dyn ClientHost, &ItemTintInput<'_>) -> u32 + Send + Sync + 'static,
    {
        self.sources.push(ItemTintSourceRegistration {
            id,
            decode: Arc::new(move |config| {
                let bound: Arc<ItemTintSourceFn> = Arc::new(decode(config)?);
                Ok(bound)
            }),
        });
    }
}

/// Borrowed input passed to a bound item model callback (`ItemModel.update`).
// no wire: borrowed per-stack item model update input.
// exhaustive: the game constructs this view; mods only read it.
pub struct ItemModelUpdateInput<'a> {
    /// Owned snapshot of the stack being resolved (including nested bundle items).
    pub stack: &'a ItemStack,
    /// Runtime bundle selection index (`BundleContents.selectedItem`), or `-1` when none.
    pub bundle_selected: i32,
    /// Display context, living holder, client level, and draw seed.
    pub context: &'a ItemModelContext,
}

/// A bound item model instance produced at item definition decode time and invoked per stack to
/// push the layers to draw (`ItemModel.update`).
// no wire: retained process-local item model update callback.
pub type ItemModelFn =
    dyn Fn(&dyn ClientHost, &ItemModelUpdateInput<'_>, &mut Vec<ItemModelLayer>) + Send + Sync;

/// Decode-time builder passed to a registered item model decoder (`register_item_models`) so it
/// can decode child item model nodes from any JSON field, list, object, or model identifier
/// without losing JSON fidelity through an NBT round-trip.
// no wire: engine-owned decode-time builder for child item models.
pub trait ItemModelChildren {
    /// Decode one child item model node from the JSON field `field` on this node and bind it
    /// under `name` for [`ItemModelLayer::new`].
    fn add_field(&mut self, name: &str, field: &str) -> Result<(), String>;
    /// Decode every child item model node in the JSON array at `field` (for example vanilla
    /// `minecraft:composite`'s `"models": [...]`), returning their bound layer names in order.
    fn add_list(&mut self, field: &str) -> Result<Vec<String>, String>;
    /// Decode every child item model node in the JSON object at `field`, binding each under its
    /// JSON key and returning the keys in order.
    fn add_object(&mut self, field: &str) -> Result<Vec<String>, String>;
    /// Decode every child item model node in the optional `"models"` JSON object if present,
    /// binding each under its JSON key and returning the keys in order (empty when absent).
    fn add_named_models(&mut self) -> Result<Vec<String>, String>;
    /// Bind a plain `minecraft:model` leaf referencing `model` under `name`.
    fn add_model(&mut self, name: &str, model: Identifier) -> Result<(), String>;
}

/// A decode-time factory bound at `register_item_models` that parses an item model node's JSON
/// object (as a [`Tag`]), declares any child models on [`ItemModelChildren`], and returns the
/// per-stack update callback.
// no wire: retained process-local item model decoder callback.
pub type ItemModelDecoder =
    dyn Fn(&Tag, &mut dyn ItemModelChildren) -> Result<Arc<ItemModelFn>, String> + Send + Sync;

/// A staged item model decoder; dispatch attributes it to the registering mod.
// no wire: retained process-local item model decoder registration.
#[doc(hidden)]
#[non_exhaustive]
pub struct ItemModelRegistration {
    /// The item model node type identifier.
    pub id: Identifier,
    /// The decode-time factory invoked once per matching node during resource load.
    pub decode: Arc<ItemModelDecoder>,
}

impl RegisterItemModelsEvent<'_> {
    /// Register an item model node decoder under `id` (NeoForge's
    /// `RegisterItemModelsEvent::register`).
    ///
    /// Any valid [`Identifier`] (including `minecraft:`) is accepted; a later registration for the
    /// same id replaces the earlier one (last-writer-wins, matching NeoForge's
    /// `LateBoundIdMapper`), and registering a built-in vanilla or engine item model type id
    /// overrides that built-in when decoding item definitions.
    ///
    /// `decode` runs once per matching node on each resource load and reload with the node's JSON
    /// object converted to a [`Tag`] and an [`ItemModelChildren`] builder for decoding child item
    /// models from any schema (`models` array, `on_true`/`on_false`, `model` id, named `models`
    /// map, etc.), returning the bound per-stack layer callback (or `Err(reason)` to refuse the
    /// enclosing item definition without falling back to a built-in).
    pub fn register<D, F>(&mut self, id: Identifier, decode: D)
    where
        D: Fn(&Tag, &mut dyn ItemModelChildren) -> Result<F, String> + Send + Sync + 'static,
        F: Fn(&dyn ClientHost, &ItemModelUpdateInput<'_>, &mut Vec<ItemModelLayer>)
            + Send
            + Sync
            + 'static,
    {
        self.models.push(ItemModelRegistration {
            id,
            decode: Arc::new(move |config, children| {
                let bound: Arc<ItemModelFn> = Arc::new(decode(config, children)?);
                Ok(bound)
            }),
        });
    }
}

/// Borrowed input passed to bound range-select and select item model property callbacks
/// (`RangeSelectItemModelProperty.get`, `SelectItemModelProperty.get`).
// no wire: borrowed per-stack item model property evaluation input.
// exhaustive: the game constructs this view; mods only read it.
pub struct ItemModelPropertyInput<'a> {
    /// Owned snapshot of the stack being resolved (including nested bundle items).
    pub stack: &'a ItemStack,
    /// Runtime bundle selection index (`BundleContents.selectedItem`), or `-1` when none.
    pub bundle_selected: i32,
    /// Display context, living holder, client level, and draw seed.
    pub context: &'a ItemModelContext,
}

/// A bound numeric `range_dispatch` property instance produced at item definition decode time and
/// invoked per stack (`RangeSelectItemModelProperty.get`).
// no wire: retained process-local range select item model property callback.
pub type RangeSelectPropertyFn =
    dyn Fn(&dyn ClientHost, &ItemModelPropertyInput<'_>) -> f32 + Send + Sync;

/// A decode-time factory bound at `register_range_select_item_model_property` that parses a
/// `range_dispatch` node's JSON object (as a [`Tag`]) once at resource load and returns the
/// per-stack numeric property callback.
// no wire: retained process-local range select item model property decoder callback.
pub type RangeSelectPropertyDecoder =
    dyn Fn(&Tag) -> Result<Arc<RangeSelectPropertyFn>, String> + Send + Sync;

/// A staged numeric item model property decoder; dispatch attributes it to the registering mod.
// no wire: retained process-local range select item model property registration.
#[doc(hidden)]
#[non_exhaustive]
pub struct RangeSelectPropertyRegistration {
    /// The range-select property identifier.
    pub id: Identifier,
    /// The decode-time factory invoked once per matching `range_dispatch` node during resource load.
    pub decode: Arc<RangeSelectPropertyDecoder>,
}

impl RegisterRangeSelectItemModelPropertyEvent<'_> {
    /// Register a numeric `range_dispatch` property decoder under `id` (NeoForge's
    /// `RegisterRangeSelectItemModelPropertyEvent::register`).
    ///
    /// Any valid `<namespace>:<path>` identifier (including `minecraft:`) is accepted; a later
    /// registration for the same id replaces the earlier one (last-writer-wins, matching
    /// NeoForge's `LateBoundIdMapper`), and registering a built-in vanilla property id overrides
    /// that built-in when decoding item definitions.
    ///
    /// `decode` runs once per matching `range_dispatch` node on each resource load and reload with
    /// the node's JSON object converted to a [`Tag`], returning the bound per-stack numeric
    /// callback (or `Err(reason)` to reject the enclosing item definition without falling back to
    /// a built-in).
    pub fn register<D, F>(&mut self, id: Identifier, decode: D)
    where
        D: Fn(&Tag) -> Result<F, String> + Send + Sync + 'static,
        F: Fn(&dyn ClientHost, &ItemModelPropertyInput<'_>) -> f32 + Send + Sync + 'static,
    {
        self.properties.push(RangeSelectPropertyRegistration {
            id,
            decode: Arc::new(move |config| {
                let bound: Arc<RangeSelectPropertyFn> = Arc::new(decode(config)?);
                Ok(bound)
            }),
        });
    }
}

trait SelectCaseKeyOps: std::any::Any + std::fmt::Debug + Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
    fn dyn_eq(&self, other: &dyn SelectCaseKeyOps) -> bool;
}

impl<T: Eq + std::fmt::Debug + Send + Sync + 'static> SelectCaseKeyOps for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn dyn_eq(&self, other: &dyn SelectCaseKeyOps) -> bool {
        other
            .as_any()
            .downcast_ref::<T>()
            .is_some_and(|rhs| self == rhs)
    }
}

/// An opaque, type-erased `when` case value decoded by a registered `select` property
/// (`register_select_item_model_property`) and compared by equality against the property's
/// per-stack result.
// no wire: retained process-local select item model case key.
#[derive(Clone, Debug)]
pub struct SelectCaseKey(Arc<dyn SelectCaseKeyOps>);

impl SelectCaseKey {
    /// Wrap any `'static + Eq + Debug + Send + Sync` value as a `select` case key.
    pub fn new<T: Eq + std::fmt::Debug + Send + Sync + 'static>(value: T) -> Self {
        Self(Arc::new(value))
    }

    /// Borrow the inner value when its concrete type matches `T`.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.0.as_any().downcast_ref::<T>()
    }
}

impl PartialEq for SelectCaseKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.dyn_eq(&*other.0)
    }
}
impl Eq for SelectCaseKey {}

/// A decode-time parser for one `when` value of a bound `select` property instance.
// no wire: retained process-local select item model case decoder callback.
pub type SelectCaseDecoder = dyn Fn(&Tag) -> Result<SelectCaseKey, String> + Send + Sync;

/// A bound categorical `select` property instance produced at item definition decode time and
/// invoked per stack (`SelectItemModelProperty.get`).
// no wire: retained process-local select item model property callback.
pub type SelectPropertyFn =
    dyn Fn(&dyn ClientHost, &ItemModelPropertyInput<'_>) -> Option<SelectCaseKey> + Send + Sync;

/// A decode-time factory bound at `register_select_item_model_property` that parses a `select`
/// node's JSON object (as a [`Tag`]) once at resource load and returns both its `when` case
/// decoder and its per-stack property callback.
// no wire: retained process-local select item model property decoder callback.
pub type SelectPropertyDecoder =
    dyn Fn(&Tag) -> Result<(Arc<SelectCaseDecoder>, Arc<SelectPropertyFn>), String> + Send + Sync;

/// A staged categorical item model property decoder; dispatch attributes it to the registering mod.
// no wire: retained process-local select item model property registration.
#[doc(hidden)]
#[non_exhaustive]
pub struct SelectPropertyRegistration {
    /// The select property identifier.
    pub id: Identifier,
    /// The decode-time factory invoked once per matching `select` node during resource load.
    pub decode: Arc<SelectPropertyDecoder>,
}

impl RegisterSelectItemModelPropertyEvent<'_> {
    /// Register a categorical `select` property decoder under `id` (NeoForge's
    /// `RegisterSelectItemModelPropertyEvent::register`).
    ///
    /// Any valid [`Identifier`] (including `minecraft:`) is accepted; a later registration for the
    /// same id replaces the earlier one (last-writer-wins, matching NeoForge's
    /// `LateBoundIdMapper`), and registering a built-in vanilla property id overrides that
    /// built-in when decoding item definitions.
    ///
    /// `decode` runs once per matching `select` node on each resource load and reload with the
    /// node's JSON object converted to a [`Tag`], returning a pair `(decode_case, eval)`:
    /// `decode_case` validates and decodes each `cases[*].when` entry (as a [`Tag`]) into the
    /// property's case type `T`, and `eval` evaluates the stack into `Option<T>` for equality
    /// comparison against the decoded `when` values. Returning `Err(reason)` from `decode` or
    /// `decode_case` refuses the enclosing item definition without falling back to a built-in.
    pub fn register<T, D, C, F>(&mut self, id: Identifier, decode: D)
    where
        T: Eq + std::fmt::Debug + Send + Sync + 'static,
        D: Fn(&Tag) -> Result<(C, F), String> + Send + Sync + 'static,
        C: Fn(&Tag) -> Result<T, String> + Send + Sync + 'static,
        F: Fn(&dyn ClientHost, &ItemModelPropertyInput<'_>) -> Option<T> + Send + Sync + 'static,
    {
        self.properties.push(SelectPropertyRegistration {
            id,
            decode: Arc::new(move |config| {
                let (decode_case, eval) = decode(config)?;
                let case_decoder: Arc<SelectCaseDecoder> =
                    Arc::new(move |when| decode_case(when).map(SelectCaseKey::new));
                let property_fn: Arc<SelectPropertyFn> =
                    Arc::new(move |host, input| eval(host, input).map(SelectCaseKey::new));
                Ok((case_decoder, property_fn))
            }),
        });
    }
}
