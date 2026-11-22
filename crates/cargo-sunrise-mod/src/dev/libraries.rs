//! Select the add-on's library projects before invoking the native build.
use crate::registry_source::{Registry, write_files};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use sunrise_mod_package::{
    Manifest, Package,
    index::{self, Index, Kind, native::Bundle, resolve},
    manifest::Side,
};

struct LocalSource {
    path: PathBuf,
    package: Package,
}

pub(super) struct Libraries {
    local: BTreeMap<String, LocalSource>,
    index: Index,
    side: Side,
}

impl Libraries {
    pub(super) fn read(project: &Manifest, paths: &[&Path], side: Side) -> Result<Self, String> {
        let mut local = BTreeMap::new();
        for path in paths {
            let path =
                dunce::canonicalize(path).map_err(|e| format!("--with {}: {e}", path.display()))?;
            if !path.is_dir() && path.extension().and_then(|s| s.to_str()) != Some("smod") {
                return Err(format!(
                    "--with {}: expected a native project folder or .smod",
                    path.display()
                ));
            }
            let package =
                Package::read(&path).map_err(|e| format!("--with {}: {e}", path.display()))?;
            let id = &package.manifest.id;
            if !package.is_native() {
                return Err(format!(
                    "--with {}: {id} is not a native library",
                    path.display()
                ));
            }
            if *id == project.id || local.contains_key(id) {
                return Err(format!(
                    "duplicate mod id {id} in --with sources and the project"
                ));
            }
            local.insert(id.clone(), LocalSource { path, package });
        }
        // Walk libraries supplied with --with, including explicit optional libraries. Only an
        // unsupplied required edge needs the index.
        let mut todo = vec![project];
        let mut seen = BTreeSet::new();
        let mut index = Index::default();
        'local_closure: while let Some(manifest) = todo.pop() {
            for dep in
                sunrise_mod_package::dependencies::from_manifest(&manifest.dependencies, side)
            {
                if let Some(source) = local.get(&dep.id) {
                    if seen.insert(source.package.manifest.id.as_str()) {
                        todo.push(&source.package.manifest);
                    }
                } else if !dep.optional && dep.id != project.id {
                    index = Registry::default().read_index().map_err(|error| format!(
                        "{} needs {} {}. Supply the library with --with <path> or use the Sunrise mod registry. Could not read the mod registry: {error}",
                        manifest.id, dep.id, dep.requirement
                    ))?;
                    break 'local_closure;
                }
            }
        }
        Ok(Self { local, index, side })
    }

    fn plan(&self, project: &Manifest, game: &str) -> Result<resolve::ManifestPlan<'_>, String> {
        let archives: BTreeMap<_, _> = self
            .local
            .iter()
            .filter(|(_, source)| !source.path.is_dir())
            .map(|(id, source)| index::sha256_file(&source.path).map(|hash| (id, hash)))
            .collect::<Result<_, _>>()?;
        let locals: Vec<_> = self
            .local
            .iter()
            .map(|(id, source)| resolve::LocalMod {
                manifest: &source.package.manifest,
                sha256: archives.get(id).map(String::as_str),
            })
            .collect();
        resolve::for_manifest(&self.index, project, &locals, game, self.side, |_, _| true)
            .map_err(|error| format!("{}: {error}. Supply the library with --with <path> or use a matching version in the Sunrise mod registry.", project.id))
    }

    pub(super) fn stage(
        &self,
        project: &Manifest,
        game: &str,
        cache: &Path,
        root: &Path,
    ) -> Result<BTreeMap<String, PathBuf>, String> {
        let selected = self.plan(project, game)?;
        let registry = Registry::default();
        let mut origins = BTreeMap::new();
        for id in selected.local {
            let source = &self.local[&id];
            let into = root.join(&id);
            let files = if source.path.is_dir() {
                origins.insert(id.clone(), source.path.clone());
                directory_files(&source.package, &source.path)?
            } else {
                Bundle::read(&source.path)?.files
            };
            write_files(&into, files)?;
            println!(
                "Using library {id} {} from {}",
                source.package.manifest.version,
                source.path.display()
            );
        }
        for (project, row) in selected.registry {
            if project.kind == Kind::NativeMod {
                let source = registry.native_source(cache, &self.index, project, row)?;
                write_files(&root.join(&project.id), source.files)?;
                println!(
                    "Using library {} {} from the Sunrise mod registry",
                    project.id, row.version
                );
            }
        }
        Ok(origins)
    }
}

/// Published SDKs compare the library's recorded uses, just as the install path does. Keep the
/// original pin in the source snapshot; only the SDK used for this particular build changes.
pub(super) fn check_fit(project: &Path, sdk: &super::NativeSdk, root: &Path) -> Result<(), String> {
    use sunrise_native_build::{
        check::{self, Unlisted, Verdict},
        contract::CONTRACT,
    };
    let build = &sdk.index.build.commit;
    let mut build_data = None;
    for bundle in sunrise_native_build::scan_bundles(root)? {
        let pin = &bundle.native().sunrise;
        if pin == build {
            continue;
        }
        let id = &bundle.manifest.id;
        let mut plan = check::Plan {
            contract: CONTRACT,
            mods: bundle.dir.clone(),
            build: build.clone(),
            surface: Some(sdk.dir.join(crate::sdk_fetch::SURFACE)),
            build_data: build_data.clone(),
        };
        let refused =
            |why: String| format!("library {id} pins {pin} and cannot be built for {build}: {why}");
        let judge = |plan: &check::Plan| {
            check::check(plan)?
                .mods
                .into_iter()
                .next()
                .ok_or_else(|| "native-check returned no verdict for this library".to_owned())
        };
        let mut row = judge(&plan).map_err(refused)?;
        if row.needs == Some("build-data") {
            build_data = Some(crate::sdk_fetch::build_data(project, build).map_err(refused)?);
            plan.build_data = build_data.clone();
            row = judge(&plan).map_err(refused)?;
        }
        match row.verdict {
            Some(verdict) if verdict.usable() => (),
            Some(Verdict::DoesNotFit { misfits, .. }) => {
                return Err(refused(
                    misfits
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; "),
                ));
            }
            Some(Verdict::NoList { why, hooks, .. }) => {
                let detail = match why {
                    Unlisted::Missing => "it has no uses list".into(),
                    Unlisted::Unreadable => "its uses list is unreadable".into(),
                    Unlisted::NewerSchema => "its uses list requires a newer tool".into(),
                    Unlisted::OtherBuild => "its uses list is for another build".into(),
                    Unlisted::HookVersions => {
                        format!("its uses list omits versions for {}", hooks.join(", "))
                    }
                    Unlisted::GameCode => "its uses list omits declared game code".into(),
                };
                return Err(refused(format!(
                    "{detail}; build or repin the library against this SDK"
                )));
            }
            Some(Verdict::Exact { why, .. }) => {
                return Err(refused(format!(
                    "it declares {why}, which needs the exact SDK"
                )));
            }
            _ => {
                return Err(refused(row.refusal.unwrap_or_else(|| {
                    format!(
                        "native-check needs {}",
                        row.needs.unwrap_or("a fit verdict")
                    )
                })));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

fn directory_files(package: &Package, dir: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut files = package.directory_files(dir)?;
    let uses = mod_surface::uses::USES_FILE;
    match crate::bounded::read_bytes(
        &dir.join(uses),
        mod_surface::uses::MAX_USES_BYTES as u64,
        crate::bounded::Symlinks::Refuse,
    ) {
        Ok(table) => {
            files.insert(uses.into(), table);
        }
        Err(error) if error.is_not_found() => (),
        Err(error) => return Err(error.into()),
    }
    Ok(files)
}
