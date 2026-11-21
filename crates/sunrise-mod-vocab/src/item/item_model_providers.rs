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

/// A decode-time factory bound at `register_item_models` that parses an item model node's JSON
/// object (as a [`Tag`]) once at resource load and returns the per-stack update callback.
// no wire: retained process-local item model decoder callback.
pub type ItemModelDecoder = dyn Fn(&Tag) -> Result<Arc<ItemModelFn>, String> + Send + Sync;

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
    /// Any valid `<namespace>:<path>` identifier (including `minecraft:`) is accepted; a later
    /// registration for the same id replaces the earlier one (last-writer-wins, matching
    /// NeoForge's `LateBoundIdMapper`), and registering a built-in vanilla or engine item model
    /// type id overrides that built-in when decoding item definitions.
    ///
    /// `decode` runs once per matching node on each resource load and reload with the node's JSON
    /// object converted to a [`Tag`], returning the bound per-stack layer callback (or
    /// `Err(reason)` to reject the enclosing item definition without falling back to a built-in).
    pub fn register<D, F>(&mut self, id: Identifier, decode: D)
    where
        D: Fn(&Tag) -> Result<F, String> + Send + Sync + 'static,
        F: Fn(&dyn ClientHost, &ItemModelUpdateInput<'_>, &mut Vec<ItemModelLayer>)
            + Send
            + Sync
            + 'static,
    {
        self.models.push(ItemModelRegistration {
            id,
            decode: Arc::new(move |config| {
                let bound: Arc<ItemModelFn> = Arc::new(decode(config)?);
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

/// A bound categorical `select` property instance produced at item definition decode time and
/// invoked per stack (`SelectItemModelProperty.get`).
// no wire: retained process-local select item model property callback.
pub type SelectPropertyFn =
    dyn Fn(&dyn ClientHost, &ItemModelPropertyInput<'_>) -> Option<String> + Send + Sync;

/// A decode-time factory bound at `register_select_item_model_property` that parses a `select`
/// node's JSON object (as a [`Tag`]) once at resource load and returns the per-stack categorical
/// property callback.
// no wire: retained process-local select item model property decoder callback.
pub type SelectPropertyDecoder =
    dyn Fn(&Tag) -> Result<Arc<SelectPropertyFn>, String> + Send + Sync;

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
    /// Any valid `<namespace>:<path>` identifier (including `minecraft:`) is accepted; a later
    /// registration for the same id replaces the earlier one (last-writer-wins, matching
    /// NeoForge's `LateBoundIdMapper`), and registering a built-in vanilla property id overrides
    /// that built-in when decoding item definitions.
    ///
    /// `decode` runs once per matching `select` node on each resource load and reload with the
    /// node's JSON object converted to a [`Tag`], returning the bound per-stack categorical
    /// callback (or `Err(reason)` to reject the enclosing item definition without falling back to
    /// a built-in).
    pub fn register<D, F>(&mut self, id: Identifier, decode: D)
    where
        D: Fn(&Tag) -> Result<F, String> + Send + Sync + 'static,
        F: Fn(&dyn ClientHost, &ItemModelPropertyInput<'_>) -> Option<String>
            + Send
            + Sync
            + 'static,
    {
        self.properties.push(SelectPropertyRegistration {
            id,
            decode: Arc::new(move |config| {
                let bound: Arc<SelectPropertyFn> = Arc::new(decode(config)?);
                Ok(bound)
            }),
        });
    }
}
