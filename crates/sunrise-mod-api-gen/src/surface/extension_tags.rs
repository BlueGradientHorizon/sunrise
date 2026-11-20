//! Extension tag records come from the same explicit inputs as the package and book catalogs.
use mod_surface::ExtensionTag;
use serde::de::DeserializeOwned;
use std::{collections::BTreeSet, fs, path::Path};

pub(super) const INPUTS: &str = "crates/sunrise-mod-vocab/extension-tag-inputs.json";

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn read(root: &Path) -> Result<Vec<ExtensionTag>, String> {
    let inputs: Vec<String> = read_json(&root.join(INPUTS))?;
    let mut tags = Vec::new();
    for input in inputs {
        tags.extend(read_json::<Vec<ExtensionTag>>(&root.join(input))?);
    }
    let mut seen = BTreeSet::new();
    for tag in &tags {
        if !tag.id.starts_with("sunrise:")
            || tag.id.starts_with("sunrise:class/")
            || tag.registry.is_empty()
            || tag.doc.is_empty()
            || !seen.insert((&tag.registry, &tag.id))
        {
            return Err(format!(
                "invalid or duplicate extension tag {}/{}",
                tag.registry, tag.id
            ));
        }
    }
    tags.sort_by(|a, b| (&a.registry, &a.id).cmp(&(&b.registry, &b.id)));
    Ok(tags)
}
