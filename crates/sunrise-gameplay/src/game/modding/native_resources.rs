//! Resource reads for native mods through the game's selected pack stacks.
use sunrise_gameplay_common::game::side_ops::modapi_modding::SideOps as ModdingSideOps;
use mod_api::{Identifier, PackType};
use std::collections::BTreeSet;
use sunrise_assets::resource_source::{ResourceSource, read_winning_resource};

impl<__Side: sunrise_gameplay_common::game::Side> crate::game::Game<'_, __Side> {
    /// `pack_type`'s top folder and its stack, lowest priority first. Client
    /// resources are already resolved into `PackResources::source` by
    /// `packs::resources` (filters and overrides included, each pack's copy kept
    /// as a layer); server data is stacked here in vanilla's priority order: the
    /// game's own data unless `minecraft:vanilla` is disabled, then the mods'
    /// content root, then the selected world packs (later overrides earlier).
    /// None on a server for client resources: only a client selects asset packs.
    fn resource_stack_sources(
        &self,
        pack_type: PackType,
    ) -> Option<(&'static str, Vec<&ResourceSource>)> {
        Some(if pack_type == PackType::ClientResources {
            if !self.options.execution_role.is_client() {
                return None;
            }
            ("assets", vec![&self.pack.source])
        } else {
            let mut sources = Vec::new();
            if !self.side_vanilla_data_disabled() {
                sources.push(&self.pack.data_source);
            }
            sources.push(&self.pack.data_changes);
            if self.options.execution_role.is_logical_server() {
                ModdingSideOps::side_append_function_pack_sources(&self.side, &mut sources);
            }
            ("data", sources)
        })
    }

    /// `ResourceManager.getResource`: the winning bytes of `identifier` in `pack_type`'s stack.
    pub(in crate::game) fn resource(
        &self,
        pack_type: PackType,
        identifier: &Identifier,
    ) -> Option<Vec<u8>> {
        let (top, sources) = self.resource_stack_sources(pack_type)?;
        let name = format!("{top}/{}/{}", identifier.namespace(), identifier.path());
        read_winning_resource(&sources, &name, mod_api::MAX_RESOURCE_BYTES)
    }

    /// `ResourceManager.getResourceStack`: every copy of `identifier`, lowest first.
    pub(in crate::game) fn resource_stack(
        &self,
        pack_type: PackType,
        identifier: &Identifier,
    ) -> Vec<Vec<u8>> {
        let Some((top, sources)) = self.resource_stack_sources(pack_type) else {
            return Vec::new();
        };
        let name = format!("{top}/{}/{}", identifier.namespace(), identifier.path());
        sources
            .iter()
            .flat_map(|source| source.stack(&name))
            .filter_map(|copy| copy.read_limited(mod_api::MAX_RESOURCE_BYTES).ok())
            .collect()
    }

    /// `ResourceManager.listResources`: the names under `directory` in every namespace whose
    /// path ends in `suffix`, in `Identifier.compareTo` order (path, then namespace).
    pub(in crate::game) fn list_resources(
        &self,
        pack_type: PackType,
        directory: &str,
        suffix: &str,
    ) -> Vec<Identifier> {
        let Some((top, sources)) = self.resource_stack_sources(pack_type) else {
            return Vec::new();
        };
        let mut found = BTreeSet::new();
        for source in sources {
            for namespace in source.namespaces(top) {
                let root = format!("{top}/{namespace}/");
                let under = match directory {
                    "" => root.clone(),
                    directory => format!("{root}{directory}/"),
                };
                found.extend(
                    source
                        .list(&under)
                        .iter()
                        .filter_map(|name| name.strip_prefix(&root))
                        .filter(|path| path.ends_with(suffix) && !path.ends_with(".mcmeta"))
                        .filter_map(|path| Identifier::from_parts(&namespace, path).ok()),
                );
            }
        }
        found.into_iter().collect()
    }
}

use sunrise_gameplay_common::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;
