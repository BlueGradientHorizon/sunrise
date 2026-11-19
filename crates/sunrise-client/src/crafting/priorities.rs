//! Pack-layered `data/<namespace>/recipe_priorities.json` (NeoForge's public
//! recipe-priorities format). Order the shared sources before any table is built.
use super::{ResolvedSources, registration_key};
use crate::Result;
use sunrise_world::{
    conditions::LoadConditions, loot::PackOverlay,
};
use serde_json::Value;
use std::{cmp::Reverse, collections::HashMap, fs, path::Path};

#[derive(Default)]
struct Priorities(HashMap<String, i32>);

impl Priorities {
    fn layer(&mut self, bytes: &[u8], conditions: &LoadConditions) {
        let parsed = || -> std::result::Result<_, String> {
            let file: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
            if !conditions.allows_file(&file, "recipe_priorities.json") {
                return Ok(None);
            }
            let replace = match file.get("replace") {
                None => false,
                Some(Value::Bool(value)) => *value,
                _ => return Err("replace must be a boolean".into()),
            };
            let mut entries = HashMap::new();
            if let Some(values) = file.get("entries") {
                let values = values.as_object().ok_or("entries must be an object")?;
                for (id, value) in values {
                    let priority = value
                        .as_i64()
                        .and_then(|v| i32::try_from(v).ok())
                        .ok_or_else(|| {
                            format!("priority for {id} must be a signed 32-bit integer")
                        })?;
                    entries.insert(sunrise_items::recipe_sources::qualified(id), priority);
                }
            }
            Ok(Some((replace, entries)))
        };
        match parsed() {
            Ok(Some((replace, entries))) => {
                if replace {
                    self.0.clear();
                }
                // Unknown ids are intentionally retained, including conditioned-out recipes.
                self.0.extend(entries);
            }
            Ok(None) => {}
            Err(e) => log::warn!("recipe_priorities.json not loaded: {e}"),
        }
    }

    fn load(root: &Path, overlay: &PackOverlay) -> Result<Self> {
        let inventory = sunrise_mod_package::enabled::EnabledMods::read(root)?;
        let conditions = overlay
            .conditions
            .with_mod_inventory(inventory.server_ids());
        let mut priorities = Self::default();
        let mut namespaces: Vec<_> = match fs::read_dir(root.join("data")) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        namespaces.sort();
        for namespace in namespaces {
            let path = namespace.join("recipe_priorities.json");
            match fs::read(&path) {
                Ok(bytes) => priorities.layer(&bytes, &conditions),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => log::warn!("{} not loaded: {e}", path.display()),
            }
        }
        for (_, layers) in overlay
            .files
            .iter()
            .filter(|(key, _)| key == "recipe_priorities")
        {
            for bytes in layers {
                priorities.layer(bytes, &conditions);
            }
        }
        Ok(priorities)
    }

    fn get(&self, name: &str) -> i32 {
        if name.contains(':') {
            self.0.get(name).copied().unwrap_or(0)
        } else {
            self.0
                .get(&format!("minecraft:{name}"))
                .copied()
                .unwrap_or(0)
        }
    }
}

impl ResolvedSources {
    /// Order `sources` once for a load: descending priority, then vanilla's registration order.
    pub(super) fn order_by_priority(
        sources: &mut [(String, Value)],
        root: &Path,
        overlay: &PackOverlay,
    ) -> Result<()> {
        let priorities = Priorities::load(root, overlay)?;
        sources.sort_by_cached_key(|(name, _)| {
            (Reverse(priorities.get(name)), registration_key(name))
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests;
