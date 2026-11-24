//! Built-in `c:` and `neoforge:` convention tags embedded in `sunrise-world`.
//!
//! Loaded once from `crates/sunrise-world/data/convention_tags.json` — the same
//! file that generates `mod_api::tags::c::*` — and injected as the lowest layer
//! beneath vanilla files, data packs and mods so `values`, `remove` and
//! `replace: true` layer on top identically to vanilla tags.

use super::TagLayers;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

const DATA: &str = include_str!("../../data/convention_tags.json");

#[derive(Deserialize)]
struct SpecFile {
    registries: Vec<RegistrySpec>,
}

#[derive(Deserialize)]
struct RegistrySpec {
    registry: String,
    tags: Vec<TagSpec>,
}

#[derive(Deserialize)]
struct TagSpec {
    #[serde(rename = "const")]
    const_name: String,
    id: String,
    values: Vec<String>,
}

/// One built-in convention tag from Sunrise's canonical data file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConventionTag {
    /// Rust constant name in `mod_api::tags::c::<module>`.
    pub const_name: String,
    /// Canonical tag identifier (for example `"c:ingots"` or `"neoforge:is_magic"`).
    pub id: String,
    /// Raw tag entries (`"minecraft:iron_ingot"`, `"#c:ingots/iron"`, ...).
    pub values: Vec<String>,
}

struct Parsed {
    by_registry: BTreeMap<String, Vec<ConventionTag>>,
    names_by_registry: HashMap<String, HashSet<String>>,
    layers_by_registry: HashMap<String, HashMap<String, TagLayers>>,
    json_by_registry: HashMap<String, Vec<(String, String)>>,
}

static PARSED: OnceLock<Parsed> = OnceLock::new();

fn parsed() -> &'static Parsed {
    PARSED.get_or_init(|| {
        let spec: SpecFile =
            serde_json::from_str(DATA).expect("built-in convention_tags.json must parse");
        let mut by_registry = BTreeMap::new();
        let mut names_by_registry = HashMap::new();
        let mut layers_by_registry = HashMap::new();
        let mut json_by_registry = HashMap::new();
        for reg in spec.registries {
            let mut tags = Vec::with_capacity(reg.tags.len());
            let mut names = HashSet::with_capacity(reg.tags.len());
            let mut layers = HashMap::with_capacity(reg.tags.len());
            let mut jsons = Vec::with_capacity(reg.tags.len());
            for tag in reg.tags {
                let json_value = serde_json::json!({ "values": &tag.values });
                let layer = TagLayers::from_file(json_value.clone())
                    .expect("built-in convention tag entries must be valid");
                let json_text = serde_json::to_string(&json_value)
                    .expect("built-in convention tag JSON must serialize");
                names.insert(tag.id.clone());
                layers.insert(tag.id.clone(), layer);
                jsons.push((tag.id.clone(), json_text));
                tags.push(ConventionTag {
                    const_name: tag.const_name,
                    id: tag.id,
                    values: tag.values,
                });
            }
            by_registry.insert(reg.registry.clone(), tags);
            names_by_registry.insert(reg.registry.clone(), names);
            layers_by_registry.insert(reg.registry.clone(), layers);
            json_by_registry.insert(reg.registry, jsons);
        }
        Parsed {
            by_registry,
            names_by_registry,
            layers_by_registry,
            json_by_registry,
        }
    })
}

/// Every built-in convention tag definition grouped by registry path (`"block"`,
/// `"item"`, `"worldgen/biome"`, ...).
pub fn all() -> &'static BTreeMap<String, Vec<ConventionTag>> {
    &parsed().by_registry
}

/// Whether `id` (for example `"c:ingots"` or `"neoforge:is_magic"`) is a built-in
/// convention tag for registry `kind`.
pub fn has_tag(kind: &str, id: &str) -> bool {
    parsed()
        .names_by_registry
        .get(kind)
        .is_some_and(|names| names.contains(id))
}

/// Base `TagLayers` map for registry `kind`, cloned from the parsed built-in
/// convention definitions so disk files and pack overlays layer on top.
pub fn layers(kind: &str) -> HashMap<String, TagLayers> {
    parsed()
        .layers_by_registry
        .get(kind)
        .cloned()
        .unwrap_or_default()
}

/// Built-in convention tags for registry `kind` as `(tag_id, json_text)` pairs,
/// for loaders that apply raw JSON tag definitions (`damage_source`, `worldgen`).
pub fn json_entries(kind: &str) -> &'static [(String, String)] {
    parsed()
        .json_by_registry
        .get(kind)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
