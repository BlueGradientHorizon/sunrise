//! The crates the mod API's signatures name, indexed by [`mod_source::index`].
pub(crate) use mod_source::index::{Found, Index, ItemId, default_build, uses};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use syn::{Attribute, Ident, Item, Visibility};

fn type_item(item: &Item) -> Option<(&Ident, &Visibility, &[Attribute])> {
    match item {
        Item::Enum(item) => Some((&item.ident, &item.vis, &item.attrs)),
        Item::Struct(item) => Some((&item.ident, &item.vis, &item.attrs)),
        Item::Trait(item) => Some((&item.ident, &item.vis, &item.attrs)),
        Item::Type(item) => Some((&item.ident, &item.vis, &item.attrs)),
        Item::Union(item) => Some((&item.ident, &item.vis, &item.attrs)),
        _ => None,
    }
}

fn is_public_external_type(index: &Index<'_>, id: crate::index::ItemId) -> bool {
    type_item(index.item(id)).is_some_and(|(_, visibility, attrs)| {
        matches!(visibility, Visibility::Public(_)) && default_build(attrs)
    })
}

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
pub(crate) struct OutsideSeam {
    root: PathBuf,
    crates: HashSet<String>,
    indexed: Vec<WorkspaceCrate>,
    /// Dependency names as Rust spells them, keyed by the indexed crate that declares them.
    dependencies: HashMap<String, HashMap<String, String>>,
    /// External dependency aliases and their Cargo package names for rustc namespace probes.
    external_libraries: HashMap<String, HashMap<String, String>>,
    resolution_error: std::cell::RefCell<Option<String>>,
}

struct WorkspaceCrate {
    name: String,
    source: String,
    root_file: Option<String>,
}

impl OutsideSeam {
    pub(crate) fn read(root: &Path) -> Result<Self, String> {
        let workspace_manifest = root.join("Cargo.toml");
        crate::rerun_if_changed(&workspace_manifest.display());
        let manifest = crate::replace::manifest(root)?;
        let workspace_members = manifest
            .get("workspace")
            .and_then(|workspace| workspace.get("members"))
            .and_then(toml::Value::as_array)
            .ok_or("Cargo.toml: no workspace.members array")?;
        let workspace_dependencies = manifest
            .get("workspace")
            .and_then(|workspace| workspace.get("dependencies"))
            .and_then(toml::Value::as_table);
        let mut crates = HashSet::new();
        let mut indexed = Vec::new();
        let mut packages = HashMap::new();
        let mut member_manifests = Vec::new();
        for member in workspace_members {
            let member = member
                .as_str()
                .ok_or("Cargo.toml: workspace member is not a path")?;
            let package_path = root.join(member);
            crate::rerun_if_changed(&package_path.join("Cargo.toml").display());
            let package = crate::replace::manifest(&package_path)?;
            let package_name = package
                .get("package")
                .and_then(|package| package.get("name"))
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("{member}/Cargo.toml: no package name"))?;
            let crate_name = package
                .get("lib")
                .and_then(|lib| lib.get("name"))
                .or_else(|| package.get("package").and_then(|p| p.get("name")))
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("{member}/Cargo.toml: no crate name"))?;
            let crate_name = crate_name.replace('-', "_");
            packages.insert(package_name.to_owned(), crate_name.clone());

            // The facade and seam crates are indexed under names their source writes, which
            // differ from some Cargo library names (`mod_api` vs `mod_vocab`).
            let indexed_name = CRATES
                .iter()
                .find(|(_, src)| Path::new(src).parent() == Some(Path::new(member)))
                .map(|(name, _)| (*name).to_owned())
                .or_else(|| {
                    (member == "crates/sunrise-mod-api").then(|| "sunrise_mod_api".to_owned())
                })
                .unwrap_or_else(|| crate_name.clone());
            let seam = member == "crates/sunrise-mod-api"
                || member == "crates/sunrise-mod-api-macros"
                || CRATES
                    .iter()
                    .any(|(_, src)| Path::new(src).parent() == Some(Path::new(member)));
            if !seam {
                crates.insert(crate_name.clone());
                let lib_path = package
                    .get("lib")
                    .and_then(|lib| lib.get("path"))
                    .and_then(toml::Value::as_str)
                    .unwrap_or("src/lib.rs");
                let source_file = package_path.join(lib_path);
                if package.get("lib").is_some() || source_file.is_file() {
                    if !source_file.is_file() {
                        return Err(format!(
                            "{member}/Cargo.toml: library source {} is missing",
                            source_file.display()
                        ));
                    }
                    let source = source_file
                        .parent()
                        .and_then(|path| path.strip_prefix(root).ok())
                        .ok_or_else(|| format!("{member}/Cargo.toml: invalid library path"))?
                        .to_string_lossy()
                        .into_owned();
                    let root_file = source_file
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned());
                    indexed.push(WorkspaceCrate {
                        name: crate_name,
                        source,
                        root_file,
                    });
                }
            }
            member_manifests.push((indexed_name, package));
        }

        let mut dependencies = HashMap::new();
        let mut external_libraries = HashMap::new();
        for (indexed_name, package) in member_manifests {
            let mut aliases = HashMap::new();
            let mut external = HashMap::new();
            let mut add = |table: Option<&toml::map::Map<String, toml::Value>>| {
                let Some(table) = table else { return };
                for (alias, spec) in table {
                    let inherited = spec
                        .as_table()
                        .and_then(|table| table.get("workspace"))
                        .and_then(toml::Value::as_bool)
                        == Some(true);
                    let spec = if inherited {
                        workspace_dependencies
                            .and_then(|workspace| workspace.get(alias))
                            .unwrap_or(spec)
                    } else {
                        spec
                    };
                    let package = spec
                        .as_table()
                        .and_then(|table| table.get("package"))
                        .and_then(toml::Value::as_str)
                        .unwrap_or(alias);
                    if let Some(crate_name) = packages.get(package)
                        && crates.contains(crate_name)
                    {
                        aliases.insert(alias.replace('-', "_"), crate_name.clone());
                    } else if !packages.contains_key(package) {
                        external.insert(alias.replace('-', "_"), package.to_owned());
                    }
                }
            };
            add(package.get("dependencies").and_then(toml::Value::as_table));
            if let Some(targets) = package.get("target").and_then(toml::Value::as_table) {
                for target in targets.values() {
                    add(target.get("dependencies").and_then(toml::Value::as_table));
                }
            }
            if !aliases.is_empty() {
                dependencies.insert(indexed_name.clone(), aliases);
            }
            if !external.is_empty() {
                external_libraries.insert(indexed_name, external);
            }
        }
        Ok(Self {
            root: root.to_path_buf(),
            crates,
            indexed,
            dependencies,
            external_libraries,
            resolution_error: Default::default(),
        })
    }

    /// Adds outside workspace crates to the lazy source index so glob imports can be resolved to
    /// actual exported names without treating primitives or prelude types as leaks.
    pub(crate) fn add_to_index(&self, index: &mut Index<'_>) -> Result<(), String> {
        for krate in &self.indexed {
            index.add_crate(&krate.name, &krate.source);
            if let Some(file) = &krate.root_file
                && file != "lib.rs"
            {
                index.set_root_file(&krate.name, file);
            }
        }

        // Resolve every source module reachable from the seam crates in each default build up
        // front. Export lookup can treat an unknown name as absent, but it must not mistake an
        // unreadable module or generated include for a confirmed absence.
        let mut reachable = HashSet::new();
        let mut crates = CRATES
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .chain(std::iter::once("sunrise_mod_api".to_owned()))
            .collect::<VecDeque<_>>();
        while let Some(name) = crates.pop_front() {
            if reachable.insert(name.clone()) {
                crates.extend(
                    self.dependencies
                        .get(&name)
                        .into_iter()
                        .flat_map(HashMap::values)
                        .cloned(),
                );
            }
        }
        for krate in self
            .indexed
            .iter()
            .filter(|krate| reachable.contains(&krate.name))
        {
            let mut pending = VecDeque::from([(krate.name.clone(), Vec::<String>::new())]);
            let mut seen = HashSet::new();
            while let Some((name, path)) = pending.pop_front() {
                if !seen.insert((name.clone(), path.clone())) {
                    continue;
                }
                let module = index.module(&name, &path)?;
                let children = index.modules[module]
                    .items
                    .iter()
                    .filter_map(|item| match item {
                        Item::Mod(child) if default_build(&child.attrs) => {
                            Some(child.ident.to_string())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                for child in children {
                    let mut child_path = path.clone();
                    child_path.push(child);
                    pending.push_back((name.clone(), child_path));
                }
            }
        }
        Ok(())
    }

    /// Resolves an indexed source path to an outside workspace crate, following Cargo and `use`
    /// aliases. The returned path uses the owning crate's real Rust library name.
    pub(crate) fn external_path(
        &self,
        index: &mut Index<'_>,
        module: usize,
        path: &[String],
        rooted: bool,
    ) -> Option<Vec<String>> {
        if rooted {
            return self.rooted_external_path(index, module, path);
        }
        if let Some(target) = Self::indexed_module(index, module, path)
            && !self.crates.contains(&index.modules[target].krate)
        {
            return None;
        }
        self.resolve_external(index, module, path, &mut HashSet::new())
    }

    /// A path to an indexed API or local module rather than an outside workspace crate.
    pub(crate) fn internal_module(
        &self,
        index: &mut Index<'_>,
        module: usize,
        path: &[String],
    ) -> Option<usize> {
        let target = Self::indexed_module(index, module, path)?;
        (!self.crates.contains(&index.modules[target].krate)).then_some(target)
    }

    /// A leading `::` starts in the extern prelude, where a local module must not shadow a
    /// dependency with the same spelling.
    fn rooted_external_path(
        &self,
        index: &Index<'_>,
        module: usize,
        path: &[String],
    ) -> Option<Vec<String>> {
        let (first, rest) = path.split_first()?;
        if self.crates.contains(first) {
            return Some(path.to_vec());
        }
        let krate = &index.modules[module].krate;
        let real = self.dependencies.get(krate)?.get(first)?;
        Some(
            std::iter::once(real.clone())
                .chain(rest.iter().cloned())
                .collect(),
        )
    }

    fn indexed_external_path(
        &self,
        index: &Index<'_>,
        id: crate::index::ItemId,
    ) -> Option<Vec<String>> {
        let module = &index.modules[id.module];
        if !self.crates.contains(&module.krate) {
            return None;
        }
        let name = match index.item(id) {
            Item::Const(item) => item.ident.to_string(),
            Item::Enum(item) => item.ident.to_string(),
            Item::Struct(item) => item.ident.to_string(),
            Item::Trait(item) => item.ident.to_string(),
            Item::Type(item) => item.ident.to_string(),
            Item::Union(item) => item.ident.to_string(),
            _ => return None,
        };
        Some(
            std::iter::once(module.krate.to_owned())
                .chain(module.path.iter().cloned())
                .chain(std::iter::once(name))
                .collect(),
        )
    }

    /// Finds a type among the names a module actually exports. `Index::resolve` deliberately
    /// sees private aliases too, which is useful inside a crate but wrong for an imported glob.
    fn exported_external_type(
        &self,
        index: &mut Index<'_>,
        module: usize,
        name: &str,
        seen: &mut HashSet<(usize, String)>,
    ) -> Option<Vec<String>> {
        if !seen.insert((module, name.to_owned())) {
            return None;
        }
        let items = index.modules.get(module)?.items.clone();
        for (position, item) in items.iter().enumerate() {
            if let Some((ident, visibility, attrs)) = type_item(item)
                && ident == name
                && default_build(attrs)
            {
                if matches!(visibility, syn::Visibility::Public(_)) {
                    return self.indexed_external_path(
                        index,
                        crate::index::ItemId {
                            module,
                            item: position,
                        },
                    );
                }
                // A compiled private type shadows imports in the type namespace.
                return None;
            }
        }

        for item in &items {
            let Item::Use(import) = item else { continue };
            if !matches!(import.vis, syn::Visibility::Public(_)) || !default_build(&import.attrs) {
                continue;
            }
            let mut named = Vec::new();
            let mut globs = Vec::new();
            uses(&import.tree, &mut Vec::new(), &mut |path, alias| {
                if alias.as_deref() == Some(name) {
                    named.push(path);
                } else if alias.is_none() {
                    globs.push(path);
                }
            });
            for path in named {
                let target = self
                    .external_path(index, module, &path, import.leading_colon.is_some())
                    .unwrap_or(path);
                if let Ok(Found::Item(id)) = index.resolve_path(module, &target)
                    && let Some(external) = self.indexed_external_path(index, id)
                    && is_public_external_type(index, id)
                {
                    return Some(external);
                }
            }
            for path in globs {
                let target = if let Some(external) =
                    self.external_path(index, module, &path, import.leading_colon.is_some())
                {
                    let (krate, inner) = external.split_first()?;
                    index.module(krate, inner).ok()
                } else {
                    Self::indexed_module(index, module, &path)
                };
                let Some(target) = target else { continue };
                if let Some(external) = self.exported_external_type(index, target, name, seen) {
                    return Some(external);
                }
            }
        }
        None
    }

    fn globbed_external_path(
        &self,
        index: &mut Index<'_>,
        module: usize,
        name: &str,
    ) -> Option<Vec<String>> {
        let mut globs = Vec::new();
        for item in &index.modules[module].items {
            if let Item::Use(item) = item
                && default_build(&item.attrs)
            {
                uses(&item.tree, &mut Vec::new(), &mut |path, alias| {
                    if alias.is_none() {
                        globs.push((path, item.leading_colon.is_some()));
                    }
                });
            }
        }
        for (glob, rooted) in globs {
            let target = if let Some(path) = self.external_path(index, module, &glob, rooted) {
                let (krate, inner) = path.split_first()?;
                index.module(krate, inner).ok()
            } else {
                Self::indexed_module(index, module, &glob)
            };
            if let Some(target) = target
                && let Some(path) =
                    self.exported_external_type(index, target, name, &mut HashSet::new())
            {
                return Some(path);
            }
        }
        None
    }

    /// Loads an indexed module named by a local path, including `crate::` and `super::` paths.
    fn indexed_module(index: &mut Index<'_>, module: usize, path: &[String]) -> Option<usize> {
        index.resolve_module_path(module, path).ok().flatten()
    }

    fn resolve_external(
        &self,
        index: &mut Index<'_>,
        module: usize,
        path: &[String],
        seen: &mut HashSet<(usize, String)>,
    ) -> Option<Vec<String>> {
        // Resolve `crate::`, `self::` and `super::` before looking for imports in that module.
        let (first, _) = path.split_first()?;
        let krate = index.modules.get(module)?.krate.clone();
        let (scope, path) = match first.as_str() {
            "crate" | "self" | "super" => {
                let mut scope_path = index.modules[module].path.clone();
                let mut consumed = 1;
                match first.as_str() {
                    "crate" => scope_path.clear(),
                    "self" => {}
                    "super" => {
                        scope_path.pop()?;
                        while path.get(consumed).is_some_and(|segment| segment == "super") {
                            scope_path.pop()?;
                            consumed += 1;
                        }
                    }
                    _ => unreachable!(),
                }
                let scope = index.modules.iter().position(|candidate| {
                    candidate.krate == krate && candidate.path == scope_path
                })?;
                (scope, &path[consumed..])
            }
            _ => (module, path),
        };
        let (first, rest) = path.split_first()?;

        // A local module shadows a crate with the same spelling. Follow it before checking the
        // external workspace crate and import aliases.
        if !rest.is_empty()
            && let Some(target) = Self::indexed_module(index, scope, std::slice::from_ref(first))
        {
            let target_module = &index.modules[target];
            if self.crates.contains(&target_module.krate) {
                return Some(
                    std::iter::once(target_module.krate.to_owned())
                        .chain(target_module.path.iter().cloned())
                        .chain(rest.iter().cloned())
                        .collect(),
                );
            }
            return self.resolve_external(index, target, rest, seen);
        }
        if let Ok(found) = index.resolve(scope, first) {
            match found {
                Found::External(external) => {
                    if let Some(workspace_type) = self.globbed_external_path(index, scope, first) {
                        let package = external.first().and_then(|alias| {
                            self.external_libraries
                                .get(&krate)
                                .and_then(|aliases| aliases.get(alias))
                        });
                        match external_import_shadows_type(
                            &self.root,
                            &external,
                            first,
                            package.map(String::as_str),
                        ) {
                            Ok(true) => return None,
                            Ok(false) => return Some(workspace_type),
                            Err(error) => {
                                self.resolution_error.borrow_mut().get_or_insert(error);
                                return None;
                            }
                        }
                    }
                    return None;
                }
                // Values and types occupy separate Rust namespaces: a local constant cannot
                // shadow a same-named type imported by a glob in a signature.
                Found::Item(id)
                    if !self.crates.contains(&index.modules[id.module].krate)
                        && type_item(index.item(id)).is_some() =>
                {
                    return None;
                }
                Found::Item(_) => {}
            }
        }
        if self.crates.contains(first) {
            return Some(path.to_vec());
        }
        let krate = index.modules.get(scope)?.krate.clone();
        if let Some(real) = self
            .dependencies
            .get(&krate)
            .and_then(|aliases| aliases.get(first))
        {
            return Some(
                std::iter::once(real.clone())
                    .chain(rest.iter().cloned())
                    .collect(),
            );
        }

        if !seen.insert((scope, first.clone())) {
            return None;
        }
        let mut named = None;
        for item in &index.modules[scope].items {
            match item {
                Item::Use(item) if default_build(&item.attrs) => {
                    uses(&item.tree, &mut Vec::new(), &mut |imported, alias| {
                        if alias.as_deref() == Some(first.as_str()) {
                            named = Some((imported, item.leading_colon.is_some()));
                        }
                    })
                }
                Item::ExternCrate(item) if default_build(&item.attrs) => {
                    let alias = item
                        .rename
                        .as_ref()
                        .map_or_else(|| item.ident.to_string(), |(_, name)| name.to_string());
                    if alias == *first {
                        named = Some((vec![item.ident.to_string()], false));
                    }
                }
                _ => {}
            }
        }
        if let Some((imported, rooted)) = named {
            if let Some(mut resolved) = self.external_path(index, scope, &imported, rooted) {
                resolved.extend(rest.iter().cloned());
                return Some(resolved);
            }
            if !rest.is_empty()
                && let Some(target) = Self::indexed_module(index, scope, &imported)
            {
                return self.resolve_external(index, target, rest, seen);
            }
            return None;
        }
        self.globbed_external_path(index, scope, first)
    }

    pub(crate) fn take_resolution_error(&self) -> Option<String> {
        self.resolution_error.borrow_mut().take()
    }

    pub(crate) fn error(item: &str, path: &[String]) -> String {
        format!(
            "{item}: `{}` names a workspace crate outside the mod API type index",
            path.join("::")
        )
    }
}

/// Whether an opaque explicit import binds in the type namespace ahead of a same-named glob type.
/// A tiny rustc probe handles external items whose namespace the source index cannot know.
fn external_import_shadows_type(
    root: &Path,
    external: &[String],
    name: &str,
    package: Option<&str>,
) -> Result<bool, String> {
    static NEXT_PROBE: AtomicU64 = AtomicU64::new(0);
    let Some(crate_name) = external.first() else {
        return Err("cannot resolve an empty external import path".to_owned());
    };
    let probe_dir = std::env::temp_dir().join(format!(
        "sunrise-type-namespace-{}-{}",
        std::process::id(),
        NEXT_PROBE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&probe_dir).map_err(|error| {
        format!(
            "cannot create temporary type namespace probe {}: {error}",
            probe_dir.display()
        )
    })?;

    let result = (|| {
        let source = probe_dir.join("probe.rs");
        let output = probe_dir.join("probe.rmeta");
        let import = external.join("::");
        let source_text = format!(
            "#![allow(dead_code, unused_imports)]\n\
             mod __sunrise_workspace {{ pub struct {name}; }}\n\
             use self::__sunrise_workspace::*;\n\
             use ::{import} as {name};\n\
             trait __SunriseWorkspaceType {{}}\n\
             impl __SunriseWorkspaceType for __sunrise_workspace::{name} {{}}\n\
             fn __sunrise_assert<T: __SunriseWorkspaceType>() {{}}\n\
             fn __sunrise_probe() {{ __sunrise_assert::<{name}>(); }}\n"
        );
        std::fs::write(&source, source_text)
            .map_err(|error| format!("cannot write type namespace probe: {error}"))?;

        let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
        command
            .arg("--edition=2024")
            .arg("--crate-type=lib")
            .arg("--emit=metadata")
            .arg("--crate-name=sunrise_type_namespace_probe")
            .arg("-o")
            .arg(&output);

        if !matches!(crate_name.as_str(), "core" | "std") {
            let directory = cargo_dependency_dir().ok_or_else(|| {
                format!("cannot find Cargo's dependency artifacts to resolve {crate_name}::{name}")
            })?;
            let artifact_name = match package {
                Some(package) => {
                    let cargo_home = cargo_home()?;
                    external_library_name(root, &cargo_home, package)?
                }
                None => crate_name.replace('-', "_"),
            };
            let artifact = external_metadata(&directory, &artifact_name).ok_or_else(|| {
                format!(
                    "cannot find Cargo metadata for external crate {crate_name} ({artifact_name})"
                )
            })?;
            command
                .arg("--extern")
                .arg(format!("{crate_name}={}", artifact.display()))
                .arg("-L")
                .arg(format!("dependency={}", directory.display()));
        }

        command.arg(&source);
        let result = command
            .output()
            .map_err(|error| format!("cannot run rustc for type namespace probe: {error}"))?;
        if result.status.success() {
            return Ok(false);
        }
        let stderr = String::from_utf8_lossy(&result.stderr);
        if ["E0107", "E0277", "E0573"]
            .iter()
            .any(|code| stderr.contains(code))
        {
            return Ok(true);
        }
        Err(format!(
            "cannot determine the type namespace for {import} as {name}: {}",
            stderr.lines().take(8).collect::<Vec<_>>().join("\n")
        ))
    })();
    let _ = std::fs::remove_dir_all(&probe_dir);
    result
}

/// Finds the Rust library target for a locked registry package. Cargo dependency aliases and
/// package names can both differ from the target name used in `lib*.rmeta`.
fn cargo_home() -> Result<PathBuf, String> {
    std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .ok_or_else(|| "cannot locate Cargo's home directory".to_owned())
}

fn external_library_name(root: &Path, cargo_home: &Path, package: &str) -> Result<String, String> {
    let lock_path = root.join("Cargo.lock");
    crate::rerun_if_changed(&lock_path.display());
    let lock_text = std::fs::read_to_string(&lock_path)
        .map_err(|error| format!("{}: {error}", lock_path.display()))?;
    let lock: toml::Value =
        toml::from_str(&lock_text).map_err(|error| format!("{}: {error}", lock_path.display()))?;
    let versions = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| {
            entry.get("name").and_then(toml::Value::as_str) == Some(package)
                && entry
                    .get("source")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|source| source.starts_with("registry+"))
        })
        .filter_map(|entry| entry.get("version").and_then(toml::Value::as_str))
        .collect::<Vec<_>>();
    let registry_sources = cargo_home.join("registry/src");
    for version in versions {
        let package_dir_name = format!("{package}-{version}");
        let registries = std::fs::read_dir(&registry_sources).map_err(|error| {
            format!(
                "cannot read Cargo registry sources {}: {error}",
                registry_sources.display()
            )
        })?;
        for registry in registries.filter_map(Result::ok) {
            let package_dir = registry.path().join(&package_dir_name);
            if package_dir.join("Cargo.toml").is_file() {
                crate::rerun_if_changed(&package_dir.join("Cargo.toml").display());
                return crate::replace::library_name(&package_dir);
            }
        }
    }
    Err(format!(
        "cannot find the locked registry source for external package {package}"
    ))
}

fn cargo_dependency_dir() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    for ancestor in executable.ancestors() {
        if ancestor.file_name().is_some_and(|name| name == "deps") && ancestor.is_dir() {
            return Some(ancestor.to_path_buf());
        }
        let directory = ancestor.join("deps");
        if directory.is_dir() {
            return Some(directory);
        }
    }
    None
}

fn external_metadata(directory: &Path, crate_name: &str) -> Option<PathBuf> {
    let prefix = format!("lib{}-", crate_name.replace('-', "_"));
    let mut matches = std::fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
                && path
                    .extension()
                    .is_some_and(|extension| extension == "rmeta" || extension == "rlib")
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|path| {
        path.extension()
            .is_some_and(|extension| extension == "rmeta")
    });
    matches.pop()
}

#[cfg(test)]
mod tests {
    use super::external_library_name;
    use std::fs;

    #[test]
    fn external_library_name_uses_the_locked_package_lib_target() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        let cargo_home = directory.path().join("cargo");
        let package_dir = cargo_home
            .join("registry/src/test-index")
            .join("md-5-0.10.6");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(
            root.join("Cargo.lock"),
            "[[package]]\nname = \"md-5\"\nversion = \"0.10.6\"\nsource = \"registry+https://example.invalid/index\"\n",
        )
        .unwrap();
        fs::write(
            package_dir.join("Cargo.toml"),
            "[package]\nname = \"md-5\"\nversion = \"0.10.6\"\n[lib]\nname = \"md5\"\n",
        )
        .unwrap();

        assert_eq!(
            external_library_name(&root, &cargo_home, "md-5").unwrap(),
            "md5"
        );
    }
}
