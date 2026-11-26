use crate::game::ServerGame;
use std::{collections::BTreeMap, sync::Arc};
use sunrise_sim::lan::data::SyncData;

impl ServerGame<'_> {
    pub(in crate::game) fn server_registry_sync(&self) -> Option<Arc<SyncData>> {
        self.common.native_lifecycle.epoch()?;
        self.server
            .lan
            .as_ref()
            .map(|host| host.hub.lan.shared.sync())
            .or_else(|| {
                // The same lazy, frozen/current table the server's next connection receives. Never
                // reconstruct a dynamic registry from the prepared files at a mod call boundary.
                sunrise_gameplay_common::game::lan_common::sync::sync_data(
                    &self.common.options.resources,
                    &self.common.res.core.item_data,
                    self.items(),
                    &self.pack_files(),
                )
                .ok()
            })
    }
}

/// TagLoader binds mod-registry tags against the already decoded candidate, before publication.
/// No file's name can create an entry: required missing members invalidate that tag only.
pub(in crate::game) fn bind_datapack_tags(
    root: &std::path::Path,
    overlay: &sunrise_world::loot::PackOverlay<'_>,
    candidate: &mut mod_api::DatapackCandidate,
) -> Result<(), String> {
    for declaration in mod_api::__private::datapack_registry_declarations() {
        let (namespace, path) = declaration
            .key
            .split_once(':')
            .ok_or("invalid mod registry key")?;
        let folder = format!("{namespace}/{path}");
        let known: std::collections::BTreeSet<_> = candidate
            .entries(&declaration.key)
            .map_err(|error| format!("mod registry tags: {error:?}"))?
            .into_iter()
            .collect();
        let tags = load_registry_tags(root, overlay, &folder, |id| known.contains(id))?;
        candidate
            .bind_tags(&declaration.key, tags)
            .map_err(|error| format!("mod registry tags: {error:?}"))?;
    }
    Ok(())
}

pub(in crate::game) fn load_registry_tags(
    root: &std::path::Path,
    overlay: &sunrise_world::loot::PackOverlay<'_>,
    folder: &str,
    known: impl Fn(&str) -> bool,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    use sunrise_world::{damage_source::resource_files, worldgen::tags::TagDefinitions};
    let tag_folder = format!("tags/{folder}");
    let mut files: BTreeMap<String, Vec<Vec<u8>>> = resource_files(root, &tag_folder)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|(id, bytes)| (id, vec![bytes]))
        .collect();
    for (id, versions) in overlay.tag_kind(folder) {
        files
            .entry(id)
            .or_default()
            .extend(versions.iter().cloned());
    }
    let mut files: Vec<_> = files.into_iter().collect();
    files.sort_by_cached_key(|(id, _)| {
        let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
        (format!("{path}.json"), namespace.to_owned())
    });
    let mut definitions = TagDefinitions::default();
    for (id, versions) in files {
        for bytes in versions {
            if let Ok(json) = std::str::from_utf8(&bytes) {
                // TagLoader retains lower-priority members when a tag codec fails.
                let _ = definitions.apply_json(&id, "registry tags", json);
            }
        }
    }
    definitions
        .build(|id, _| known(id).then(|| id.to_owned()))
        .map(|built| built.tags)
        .map_err(|error| error.to_string())
}

use sunrise_gameplay_common::game::tables::GameItems as _;

use sunrise_gameplay_common::game::tables::WorldTablesExt as _;
