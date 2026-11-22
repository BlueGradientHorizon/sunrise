//! Load-scoped registration and per-candidate condition lookup.
use super::Game;
use mod_api::__private::game::{
    CallbackOwner, RegistrationContextNew, datapack_entry_id, datapack_registry_directory,
};
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};
use sunrise_client::datapack::RegistryFiles;
use sunrise_world::conditions::ConditionContextRead;

pub(super) type ConditionTable =
    Arc<BTreeMap<mod_api::Identifier, (CallbackOwner, Arc<dyn mod_api::LoadCondition>)>>;
impl Game<'_> {
    pub(super) fn begin_condition_reload(
        &mut self,
        stack: &super::functions::PackStack,
        resources: &std::path::Path,
    ) -> Result<
        super::server_reload::PreparedServerReload,
        mod_api::__private::reload::ReloadStageError,
    > {
        use mod_api::__private::reload::ReloadStageError;
        if self.mods.load_condition_candidate.is_none() {
            let inventory = sunrise_mod_package::enabled::EnabledMods::read(resources)
                .map_err(|error| ReloadStageError::DataSource(error.to_string()))?;
            self.mods.load_condition_candidate = Some(
                conditions_for_mods(&self.mods, &inventory)
                    .map_err(ReloadStageError::DataSource)?,
            );
        }
        self.prepare_server_reload(stack)
    }
    pub(super) fn prepare_pack_condition_reload(
        &mut self,
        stack: &super::functions::PackStack,
    ) -> Result<
        super::server_reload::PreparedServerReload,
        mod_api::__private::reload::ReloadStageError,
    > {
        self.mods.load_condition_candidate = None;
        self.begin_condition_reload(stack, &self.options.resources.clone())
    }

    pub(super) fn stage_condition_inventory(
        &mut self,
    ) -> sunrise_client::Result<Option<sunrise_assets::resource_source::ResourceRoot>> {
        self.protect_condition_root();
        self.mods.load_condition_candidate = None;
        let inventory = sunrise_mod_package::enabled::EnabledMods::read(&self.options.resources)?;
        self.mods.load_condition_candidate = Some(conditions_for_mods(&self.mods, &inventory)?);
        self.stage_enabled_inventory()
    }

    pub(super) fn protect_condition_root(&mut self) {
        self.mods.active_condition_root = Some(sunrise_client::mod_content::retain_root(
            &self.options.resources,
        ));
    }

    pub(super) fn condition_root_needs_reload(&self) -> bool {
        self.mods
            .load_condition_candidate
            .as_ref()
            .is_some_and(|view| view.has_registered())
            && (!self.mods.selected.is_empty()
                || !self.mods.dev.is_empty()
                || !self.mods.native.packages().is_empty())
    }

    /// The conditions of a load from `root` over the candidate pack `files`.
    pub(super) fn registered_load_conditions(
        &self,
        inventory: &sunrise_mod_package::enabled::EnabledMods,
        stack: &super::functions::PackStack,
        root: &std::path::Path,
        files: &[(String, Vec<Vec<u8>>)],
    ) -> Result<(sunrise_world::conditions::LoadConditions, RegistryFiles), String> {
        // `ICondition.IContext`: the world's feature flags and every registry's
        // entries as this candidate holds them. Tags join later, once this load has bound them.
        let features = || stack.features.iter().map(String::as_str);
        // Admission of a pack entry reads every condition but the registries this builds. It
        // shares the load's verdicts, so a condition that reads no registry runs once per file.
        let base = conditions_for_mods(&self.mods, inventory)?;
        let admission = base.clone().with_features(features());
        // DataStore's fixed content folders omit mod-declared registry folders.
        // Include those entries from this candidate's packs before conditions read them.
        let declarations = mod_api::__private::datapack_registry_declarations();
        let registry_files =
            candidate_registry_files(stack, declarations).map_err(|error| format!("{error:?}"))?;
        let registries =
            candidate_registries(root, files, &registry_files, declarations, &admission);
        let conditions = base.with_features(features()).with_registries(
            registries
                .iter()
                .map(|(registry, ids)| (registry.as_str(), ids.iter().map(String::as_str))),
        );
        Ok((conditions, registry_files))
    }
}

/// Select the physical winners used by both the condition context and registry decoding.
fn candidate_registry_files(
    stack: &super::functions::PackStack,
    declarations: &[mod_api::DatapackRegistryDeclaration],
) -> Result<RegistryFiles, Box<mod_api::DatapackLoadError>> {
    let keys: Vec<_> = declarations
        .iter()
        .map(|declaration| declaration.key.as_str())
        .collect();
    let packs: Vec<_> = std::iter::once(&stack.content)
        .chain(stack.enabled_packs())
        .collect();
    sunrise_client::datapack::registry_files(&packs, &keys)
}

/// Every registry's entries as the candidate load holds them, which `neoforge:registered` reads:
/// the built-in ones the prepared root exports (`command-registries.json`, never what an earlier
/// load added), and the entries the candidate's pack `files` add to a registry that takes them
/// (vanilla's, or a datapack registry a mod declared). So an id a pack adds counts on its first
/// load, and one a pack no longer has stops counting on the next.
fn candidate_registries(
    root: &std::path::Path,
    files: &[(String, Vec<Vec<u8>>)],
    declared_files: &RegistryFiles,
    declarations: &[mod_api::DatapackRegistryDeclaration],
    admission: &sunrise_world::conditions::LoadConditions,
) -> BTreeMap<String, std::collections::BTreeSet<String>> {
    let export: serde_json::Value = std::fs::read(root.join("command-registries.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mut registries: BTreeMap<String, std::collections::BTreeSet<String>> =
        export["by_registry"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(registry, entry)| {
                let ids = entry["ids"].as_array().into_iter().flatten();
                let ids = ids.filter_map(serde_json::Value::as_str).map(str::to_owned);
                (registry.clone(), ids.collect())
            })
            .collect();
    for declaration in declarations {
        registries.entry(declaration.key.clone()).or_default();
    }
    let mut by_folder: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for registry in registries.keys() {
        if let Some(directory) = datapack_registry_directory(registry) {
            by_folder
                .entry(directory.into_owned())
                .or_default()
                .push(registry.clone());
        }
    }
    // DataStore keys spell the same physical resource as `<folder>/<namespace>:<path>`.
    // Keep one winner even when the same file was also listed for a declared registry.
    let overlay_winners = files.iter().filter_map(|(key, versions)| {
        let (prefix, path) = key.split_once(':')?;
        let (directory, namespace) = prefix.rsplit_once('/')?;
        let file = format!("data/{namespace}/{directory}/{path}.json");
        (!declared_files.contains_key(&file))
            .then_some((Cow::Owned(file), versions.last()?.as_slice()))
    });
    let winners = overlay_winners.chain(
        declared_files
            .iter()
            .map(|(file, (_, bytes))| (Cow::Borrowed(file.as_str()), bytes.as_slice())),
    );
    for (file, bytes) in winners {
        let Some((_, path)) = file
            .strip_prefix("data/")
            .and_then(|rest| rest.split_once('/'))
        else {
            continue;
        };
        let mut folders = path
            .match_indices('/')
            .filter_map(|(slash, _)| by_folder.get_key_value(&path[..slash]))
            .peekable();
        if folders.peek().is_none() {
            continue;
        }
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) {
            // An admission probe is not the entry's loader. A codec error
            // omits it here; the owning loader will report it.
            let Ok(conditions) = admission.decode_file_conditions(&json) else {
                continue;
            };
            let mut reads_candidate = false;
            conditions.visit_context_reads(|read| {
                reads_candidate |= matches!(
                    read,
                    ConditionContextRead::Registered { .. } | ConditionContextRead::TagEmpty { .. }
                );
            });
            // This pass does not have the candidate's registry and tag views yet.
            if !reads_candidate
                && !conditions
                    .allows_file_isolated(admission, &file)
                    .unwrap_or(false)
            {
                continue;
            }
        }
        // A resource under nested folders, or a folder shared by two registry
        // keys, contributes the entry seen by each independent FileToIdConverter.
        for (directory, keys) in folders {
            let Some(id) = datapack_entry_id(directory, &file) else {
                continue;
            };
            for registry in keys {
                registries
                    .get_mut(registry)
                    .expect("named above")
                    .insert(id.clone());
            }
        }
    }
    registries
}

pub(super) fn conditions_for_mods(
    sources: &super::mods::ModsState,
    inventory: &sunrise_mod_package::enabled::EnabledMods,
) -> Result<sunrise_world::conditions::LoadConditions, String> {
    if let Some(candidate) = &sources.load_condition_candidate {
        return Ok(candidate.with_mod_inventory(inventory.server_ids()));
    }
    let table = sources
        .load_conditions
        .get_or_init(|| {
            let mut registrar = mod_api::LoadConditionDeclarations::new(
                sunrise_world::conditions::is_builtin_condition,
            );
            Game::native_register_load_conditions(
                &mod_api::RegistrationContext::new(),
                &mut registrar,
            );
            let entries = registrar.finish().map_err(|error| error.message)?;
            // Every candidate shares the owner of each retained registration.
            Ok(Arc::new(
                entries
                    .into_iter()
                    .map(|(id, (owner, handler))| (id, (CallbackOwner::for_mod(owner), handler)))
                    .collect(),
            ))
        })
        .clone()?;
    let conditions = sunrise_world::conditions::LoadConditions::new(inventory.server_ids());
    Ok(
        conditions.with_lookup(table.iter().map(|(id, (owner, handler))| {
            let name = id.to_string();
            let enabled = sources.code_owner_enabled(owner.id());
            let owner = owner.clone();
            let handler = handler.clone();
            (name.clone(), move |args: &serde_json::Value| {
                if !enabled {
                    return Ok(false);
                }
                let mut arguments = args.as_object().cloned().expect("decoded condition object");
                arguments.remove("type");
                let arguments = serde_json::Value::Object(arguments);
                owner
                    .call("load condition", "test_load_condition", || {
                        handler.test(&arguments)
                    })
                    .map_err(|reason| format!("condition {name}: {reason}"))?
                    .map_err(|error| format!("condition {name}: mod {}: {error}", owner.id()))
            })
        })),
    )
}

#[cfg(test)]
#[path = "load_conditions/startup_tests.rs"]
mod startup_tests;

#[cfg(test)]
#[path = "load_conditions/tests.rs"]
mod tests;

pub(super) fn content_root_with_conditions(
    root: &std::path::Path,
    jars: &std::path::Path,
    mods: &[sunrise_client::mod_content::ContentMod],
    cache: &std::path::Path,
    inventory: &sunrise_mod_package::enabled::EnabledMods,
    sources: &super::mods::ModsState,
) -> sunrise_client::Result<std::path::PathBuf> {
    let conditions = conditions_for_mods(sources, inventory)?;
    sunrise_client::mod_content::resource_root_with_conditions(
        root,
        jars,
        mods,
        cache,
        inventory,
        &conditions,
    )
}

#[cfg(test)]
#[path = "load_conditions/reload_tests.rs"]
mod reload_tests;
