//! The model loaders a mod registers (`register_model_loaders`, NeoForge's
//! `ModelEvent.RegisterLoaders`) and the geometry they bake.
use crate::{Direction, Identifier, RegisterModelLoadersEvent, Tag};
use std::{collections::BTreeMap, sync::Arc};

/// One quad a model loader adds to a model whose `loader` it registered
/// (`register_model_loaders`), baked with the model's elements: turned with the
/// blockstate variant, culled and shaded as element faces are.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: mods construct quads and the game reads every field
// no wire: baked in-process by a registered native model loader.
pub struct ModelLoaderQuad {
    /// Corners in block units (0 to 1 across the block), counter-clockwise seen from the front,
    /// as an obj model's are.
    pub positions: [[f32; 3]; 4],
    /// Each corner's texture coordinates, 0 to 1 across the sprite.
    pub uvs: [[f32; 2]; 4],
    /// The texture: a key of the model's `textures` (`side`, or `#side`).
    pub texture: String,
    /// The face whose solid neighbour hides the quad; `None` never culls.
    pub cull: Option<Direction>,
    /// The tint index a block tint source colours it by; -1 is none.
    pub tint_index: i32,
    /// Whether it takes directional shading.
    pub shade: bool,
}

/// Borrowed input passed to a registered model loader callback when a model is baked.
// no wire: borrowed model loader bake input.
// exhaustive: the game constructs this view; mods only read it.
pub struct ModelLoaderInput<'a> {
    /// The model's identifier (`mymod:block/pillar`).
    pub model: &'a str,
    /// The model's whole JSON object converted to a `Tag`.
    pub config: &'a Tag,
    /// The model's resolved texture slots (`slot -> sprite identifier`), including inherited slots.
    pub textures: &'a BTreeMap<String, String>,
}

/// A retained model loader bake callback bound at `register_model_loaders`.
// no wire: retained process-local model loader callback.
pub type ModelLoaderFn =
    dyn Fn(&ModelLoaderInput<'_>) -> Result<Vec<ModelLoaderQuad>, String> + Send + Sync;

impl RegisterModelLoadersEvent<'_> {
    /// Register a model loader under `id` (NeoForge's `ModelEvent.RegisterLoaders`).
    ///
    /// Any valid `<namespace>:<path>` identifier is accepted; duplicates and collisions with the
    /// built-in loaders (`neoforge:composite`, `neoforge:obj`) are rejected as registration errors.
    /// The loader callback runs once per model on each resource load and reload, never per frame.
    pub fn register<F>(&mut self, id: Identifier, loader: F)
    where
        F: Fn(&ModelLoaderInput<'_>) -> Result<Vec<ModelLoaderQuad>, String>
            + Send
            + Sync
            + 'static,
    {
        self.loaders.push(ModelLoaderRegistration {
            id,
            bake: Arc::new(loader),
        });
    }
}

/// A staged model loader callback; dispatch attributes it to the registering mod.
// no wire: retained process-local model loader callback.
#[doc(hidden)]
#[non_exhaustive]
pub struct ModelLoaderRegistration {
    /// The loader identifier.
    pub id: Identifier,
    /// The bake callback invoked when a model naming this loader is resolved.
    pub bake: Arc<ModelLoaderFn>,
}
