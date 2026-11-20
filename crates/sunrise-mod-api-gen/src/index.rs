//! The crates the mod API's signatures name, indexed by [`mod_source::index`].
pub(crate) use mod_source::index::{Found, Index, ItemId, uses};
use std::{collections::HashSet, path::Path};

/// The crates whose items mod API signatures name: crate name and source directory.
pub(crate) const CRATES: &[(&str, &str)] = &[
    ("mod_api", "crates/sunrise-mod-vocab/src"),
    ("sunrise_core", "crates/sunrise-core/src"),
    ("sunrise_mod_runtime", "crates/sunrise-mod-runtime/src"),
];

/// The mod API's type index, read below `root`.
pub(crate) fn game<'a>(root: &'a Path, included: &'a [(&'a str, &'a str)]) -> Index<'a> {
    Index::new(root, included, CRATES).expanding(crate::exports::expand)
}

/// The workspace libraries that the public API cannot treat as opaque third-party types.
pub(crate) struct OutsideSeam(HashSet<String>);

impl OutsideSeam {
    pub(crate) fn read(root: &Path) -> Result<Self, String> {
        let manifest = crate::replace::manifest(root)?;
        let members = manifest
            .get("workspace")
            .and_then(|workspace| workspace.get("members"))
            .and_then(toml::Value::as_array)
            .ok_or("Cargo.toml: no workspace.members array")?;
        let mut outside = HashSet::new();
        for member in members {
            let member = member
                .as_str()
                .ok_or("Cargo.toml: workspace member is not a path")?;
            // The facade is added only for the public walk, not the capability index.
            if member == "crates/sunrise-mod-api"
                || CRATES
                    .iter()
                    .any(|(_, src)| Path::new(src).parent() == Some(Path::new(member)))
            {
                continue;
            }
            let package = crate::replace::manifest(&root.join(member))?;
            let name = package
                .get("lib")
                .and_then(|lib| lib.get("name"))
                .or_else(|| package.get("package").and_then(|p| p.get("name")))
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("{member}/Cargo.toml: no crate name"))?;
            outside.insert(name.replace('-', "_"));
        }
        Ok(Self(outside))
    }

    pub(crate) fn rejects(&self, path: &[String]) -> bool {
        path.first().is_some_and(|root| self.0.contains(root))
    }

    pub(crate) fn error(item: &str, path: &[String]) -> String {
        format!(
            "{item}: `{}` names a workspace crate outside the mod API type index",
            path.join("::")
        )
    }
}
