//! Fold resource-pack tag layers, then bind the resulting entries to a registry.
use crate::loot::PackOverlay;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::{io, path::Path};

/// A tag's entries over its layers, in pack order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagLayers(Vec<Entry>);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    id: String,
    required: bool,
    remove: bool,
}

/// A tag file entry's id: a string, or an object's `id` (`TagEntry.CODEC`).
pub fn entry_id(entry: &Value) -> Option<&str> {
    entry.as_str().or_else(|| entry.get("id")?.as_str())
}

impl TagLayers {
    /// The layers of one file.
    pub fn from_file(file: &Value) -> Self {
        let mut layers = Self::default();
        layers.push(file);
        layers
    }

    /// One more file, over the layers below it (conditions already
    /// checked): `replace` drops them, then its `values` and `remove` follow.
    /// Inside `values`, an entry of `{"id": ..., "remove": true}`
    /// (`TagLoader.EntryWithSource.remove`) removes like a `remove` entry.
    pub fn push(&mut self, file: &Value) {
        if file.get("replace").and_then(Value::as_bool) == Some(true) {
            self.0.clear();
        }
        for (field, remove) in [("values", false), ("remove", true)] {
            for entry in file
                .get(field)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(id) = entry_id(entry) else {
                    continue;
                };
                let (prefix, name) = id.strip_prefix('#').map_or(("", id), |name| ("#", name));
                let id = if name.contains(':') {
                    format!("{prefix}{name}")
                } else {
                    format!("{prefix}minecraft:{name}")
                };
                self.0.push(Entry {
                    id,
                    required: entry
                        .get("required")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    remove: remove || entry.get("remove").and_then(Value::as_bool) == Some(true),
                });
            }
        }
    }

    /// The `#tags` the entries name, adding or removing.
    pub fn tags(&self) -> impl Iterator<Item = &str> {
        self.0.iter().filter_map(|entry| entry.id.strip_prefix('#'))
    }

    /// Bind one tag to registry IDs and already bound nested tags. A missing
    /// required addition fails the entire tag; missing optional entries and
    /// removals never do. Members retain their insertion order.
    pub fn build(
        &self,
        contains: impl Fn(&str) -> bool,
        mut members: impl FnMut(&str) -> Option<Vec<String>>,
    ) -> Option<Vec<String>> {
        let mut out = Vec::new();
        for entry in &self.0 {
            let ids = match entry.id.strip_prefix('#') {
                Some(tag) => members(tag),
                None => contains(&entry.id).then(|| vec![entry.id.clone()]),
            };
            let Some(ids) = ids else {
                if entry.required && !entry.remove {
                    return None;
                }
                continue;
            };
            for id in ids {
                if entry.remove {
                    out.retain(|kept| kept != &id);
                } else if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        Some(out)
    }

    /// Bind the registry's complete tag stack before publishing it. Admit all
    /// required dependencies before optional ones, dropping edges that would
    /// close a cycle. Build each tag once, after its admitted dependencies.
    pub fn bind(
        kind: &str,
        raw: &HashMap<String, Self>,
        contains: impl Fn(&str) -> bool,
    ) -> HashMap<String, Vec<String>> {
        let mut names: Vec<_> = raw.keys().map(String::as_str).collect();
        names.sort_unstable();
        let mut dependencies: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut pending = Vec::new();
        let mut seen = HashSet::new();
        for required in [true, false] {
            for &name in &names {
                for id in raw[name].0.iter().filter_map(|entry| {
                    ((entry.required && !entry.remove) == required)
                        .then(|| entry.id.strip_prefix('#'))
                        .flatten()
                }) {
                    if !raw.contains_key(id) {
                        continue;
                    }
                    pending.clear();
                    seen.clear();
                    pending.push(id);
                    let mut cyclic = false;
                    while let Some(next) = pending.pop() {
                        if next == name {
                            cyclic = true;
                            break;
                        }
                        if seen.insert(next) {
                            pending.extend(dependencies.get(next).into_iter().flatten().copied());
                        }
                    }
                    if !cyclic {
                        dependencies.entry(name).or_default().push(id);
                    }
                }
            }
        }

        let mut bound: HashMap<String, Vec<String>> = HashMap::new();
        let mut scheduled = HashSet::new();
        let mut pending = Vec::new();
        for name in names {
            pending.push((name, false));
            while let Some((name, ready)) = pending.pop() {
                if ready {
                    match raw[name].build(&contains, |id| bound.get(id).cloned()) {
                        Some(members) => {
                            bound.insert(name.to_owned(), members);
                        }
                        None => log::warn!(
                            "Sunrise: couldn't bind {kind} tag {name}: missing required member"
                        ),
                    }
                } else if scheduled.insert(name) {
                    pending.push((name, true));
                    pending.extend(
                        dependencies
                            .get(name)
                            .into_iter()
                            .flatten()
                            .rev()
                            .map(|&id| (id, false)),
                    );
                }
            }
        }
        bound
    }
}

/// Read a registry's resource and world-pack tag stack, then bind it against
/// the registry admitted by the owning load. Conditions do not gate tag files.
pub fn load(
    root: &Path,
    kind: &str,
    overlay: &PackOverlay<'_>,
    contains: impl Fn(&str) -> bool,
) -> io::Result<HashMap<String, Vec<String>>> {
    let mut raw = HashMap::new();
    let data = root.join("data");
    for namespace in crate::json_folder::namespaces(&data) {
        let dir = data.join(&namespace).join("tags").join(kind);
        let folder = match crate::json_folder::read(&dir) {
            Ok(folder) => folder,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for file in &folder.files {
            let value = serde_json::from_slice(file.bytes()?)?;
            raw.insert(
                format!("{namespace}:{}", file.name),
                TagLayers::from_file(&value),
            );
        }
    }
    for (id, files) in overlay.tag_kind(kind) {
        for bytes in files {
            if let Ok(value) = serde_json::from_slice(bytes) {
                raw.entry(id.clone()).or_default().push(&value);
            }
        }
    }
    Ok(TagLayers::bind(kind, &raw, contains))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn removals_apply_in_layer_order_and_replace_drops_them() {
        let mut things = TagLayers::from_file(&json!({"values": ["stone", "dirt"]}));
        things.push(&json!({"values": [], "remove": ["dirt", "#test:other"]}));
        things.push(&json!({"values": ["sand", "gravel", {"id":"stone", "remove":true}]}));
        let members = |tag: &str| (tag == "test:other").then(|| vec!["minecraft:sand".into()]);
        assert_eq!(
            things.build(|_| true, members).unwrap(),
            ["minecraft:sand", "minecraft:gravel"]
        );
        things.push(&json!({"replace": true, "values": ["dirt"]}));
        assert_eq!(things.build(|_| true, members).unwrap(), ["minecraft:dirt"]);
    }

    #[test]
    fn required_dependencies_take_priority_over_optional_cycles() {
        let raw = HashMap::from([
            (
                "test:a".into(),
                TagLayers::from_file(&json!({"values": ["#test:b", "sand"]})),
            ),
            (
                "test:b".into(),
                TagLayers::from_file(
                    &json!({"values": [{"id":"#test:a", "required":false}, "dirt"]}),
                ),
            ),
        ]);
        let bound = TagLayers::bind("block", &raw, |_| true);
        assert_eq!(bound["test:a"], ["minecraft:dirt", "minecraft:sand"]);
        assert_eq!(bound["test:b"], ["minecraft:dirt"]);
    }
}
