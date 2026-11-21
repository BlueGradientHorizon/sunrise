//! Model loaders native mods register (NeoForge's `ModelEvent.RegisterLoaders`): a model whose
//! `loader` names a custom loader keeps its JSON object, and when a [`LoaderGeometry`] provider
//! recognizes that loader id, the geometry it answers is baked with the model's elements. The
//! provider comes from the client, so this crate never reaches the mod API or keeps a second
//! registration table.
use crate::model::Direction;
use serde_json::Value;
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "model_loaders_tests.rs"]
mod tests;

/// One quad a registered loader adds, checked by the client before it gets here.
#[derive(Clone, Debug, PartialEq)]
pub struct LoaderQuad {
    /// Corners in block units, as an obj model's are.
    pub positions: [[f32; 3]; 4],
    /// Each corner's texture coordinates, 0 to 1 across the sprite.
    pub uvs: [[f32; 2]; 4],
    /// A key of the model's `textures`, without `#`.
    pub texture: String,
    /// The face whose solid neighbour hides it; `None` never culls.
    pub cull: Option<Direction>,
    pub tint_index: i32,
    pub shade: bool,
}

/// The quads a registered loader answers for a model, asked once per `(model, config, resolved
/// textures)` in a library. `model` names the model for the log and `textures` carries its
/// resolved texture slots (`slot -> sprite identifier`).
///
/// Returns `Ok(None)` when no provider is registered, `Ok(Some(quads))` when a provider
/// successfully handled the model (including zero quads), or `Err(reason)` when it failed.
pub trait LoaderGeometry: Send + Sync {
    fn quads(
        &self,
        loader: &str,
        model: &str,
        config: &Value,
        textures: &BTreeMap<String, String>,
    ) -> Result<Option<Vec<LoaderQuad>>, String>;
}
