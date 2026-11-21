//! Model loaders native mods register (NeoForge's `ModelEvent.RegisterLoaders`): a model whose
//! `loader` names one keeps its JSON object, and the geometry its mod answers is baked with the
//! model's elements. The answer comes through [`LoaderGeometry`], which the client supplies, so
//! this crate never reaches the mod API.
use crate::model::Direction;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::RwLock,
};

#[cfg(test)]
#[path = "model_loaders_tests.rs"]
mod tests;

static TYPES: RwLock<BTreeSet<String>> = RwLock::new(BTreeSet::new());

/// Whether `id` is a valid canonical `<namespace>:<path>` identifier with non-empty namespace and
/// path.
fn is_valid_id(id: &str) -> bool {
    let Some((namespace, path)) = id.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && crate::json::identifier(id).is_ok_and(|parsed| parsed == id)
}

/// Keeps the loader ids native mods registered, before resources load: any valid canonical
/// `<namespace>:<path>` identifier (including `minecraft:`) is accepted; malformed ids, built-in
/// loader collisions (`neoforge:composite`, `neoforge:obj`), and duplicates are logged as errors
/// and left out. Returns the ids kept, in registration order.
pub fn register(ids: Vec<String>) -> Vec<String> {
    let mut kept = Vec::new();
    let mut set = TYPES.write().unwrap_or_else(|e| e.into_inner());
    for id in ids {
        if !is_valid_id(&id) {
            log::error!("model loader {id:?} is not a valid <namespace>:<path> identifier");
        } else if matches!(id.as_str(), "neoforge:composite" | "neoforge:obj") {
            log::error!("model loader {id} collides with a built-in model loader");
        } else if !set.insert(id.clone()) {
            log::error!("model loader {id} is registered twice");
        } else {
            kept.push(id);
        }
    }
    kept
}

/// Whether a native mod registered `id` as a model loader.
pub fn is_registered(id: &str) -> bool {
    TYPES.read().unwrap_or_else(|e| e.into_inner()).contains(id)
}

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

/// The quads a registered loader answers for a model, asked once per model each time a library
/// resolves it. `model` names the model for the log and `textures` carries its resolved texture
/// slots (`slot -> sprite identifier`).
pub trait LoaderGeometry: Send + Sync {
    fn quads(
        &self,
        loader: &str,
        model: &str,
        config: &Value,
        textures: &BTreeMap<String, String>,
    ) -> Vec<LoaderQuad>;
}
