//! The crates the mod API's signatures name, indexed by [`mod_source::index`].
pub(crate) use mod_source::index::{Found, Index, ItemId, default_build, uses};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::Path,
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
    crates: HashSet<String>,
    indexed: Vec<WorkspaceCrate>,
    /// Dependency names as Rust spells them, keyed by the indexed crate that declares them.
    dependencies: HashMap<String, HashMap<String, String>>,
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
        for (indexed_name, package) in member_manifests {
            let mut aliases = HashMap::new();
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
        }
        Ok(Self {
            crates,
            indexed,
            dependencies,
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
        if let Some(target) = Self::indexed_module(index, module, path) {
            let target = &index.modules[target];
            if !self.crates.contains(&target.krate) {
                return None;
            }
            return Some(
                std::iter::once(target.krate.to_owned())
                    .chain(target.path.iter().cloned())
                    .collect(),
            );
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
        let root = index
            .modules
            .iter()
            .find(|candidate| candidate.krate == *krate && candidate.path.is_empty())?;
        if let Some(real) = root.items.iter().find_map(|item| {
            let Item::ExternCrate(item) = item else {
                return None;
            };
            if !default_build(&item.attrs) {
                return None;
            }
            let alias = item
                .rename
                .as_ref()
                .map_or_else(|| item.ident.to_string(), |(_, name)| name.to_string());
            if alias != *first {
                return None;
            }
            let name = item.ident.to_string();
            self.crates
                .contains(&name)
                .then_some(name.clone())
                .or_else(|| self.dependencies.get(krate)?.get(&name).cloned())
        }) {
            return Some(std::iter::once(real).chain(rest.iter().cloned()).collect());
        }
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
        Self::indexed_item_path(index, id)
    }

    fn indexed_item_path(index: &Index<'_>, id: crate::index::ItemId) -> Option<Vec<String>> {
        let module = &index.modules[id.module];
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

    fn item_at_path(index: &mut Index<'_>, path: &[String]) -> Option<crate::index::ItemId> {
        let (krate, relative) = path.split_first()?;
        let (name, parent) = relative.split_last()?;
        let module = index.module(krate, parent).ok()?;
        match index.resolve(module, name).ok()? {
            Found::Item(id) => Some(id),
            Found::External(_) => None,
        }
    }

    fn imported_item(
        &self,
        index: &mut Index<'_>,
        module: usize,
        path: &[String],
        rooted: bool,
    ) -> Option<crate::index::ItemId> {
        if let Some(path) = self.external_path(index, module, path, rooted) {
            return Self::item_at_path(index, &path);
        }
        match index.resolve_path(module, path).ok()? {
            Found::Item(id) => Some(id),
            Found::External(path) => Self::item_at_path(index, &path),
        }
    }

    fn has_named_type_import(&self, index: &mut Index<'_>, module: usize, name: &str) -> bool {
        let mut imports = Vec::new();
        for item in &index.modules[module].items {
            match item {
                Item::Use(import) if default_build(&import.attrs) => {
                    uses(&import.tree, &mut Vec::new(), &mut |path, alias| {
                        if alias.as_deref() == Some(name) {
                            imports.push((path, import.leading_colon.is_some()));
                        }
                    });
                }
                Item::ExternCrate(import) if default_build(&import.attrs) => {
                    let alias = import
                        .rename
                        .as_ref()
                        .map_or_else(|| import.ident.to_string(), |(_, name)| name.to_string());
                    if alias == name {
                        return true;
                    }
                }
                _ => {}
            }
        }
        imports.into_iter().any(|(path, rooted)| {
            self.external_module_target(index, module, &path, rooted)
                .is_some()
                || self
                    .imported_item(index, module, &path, rooted)
                    .is_some_and(|id| type_item(index.item(id)).is_some())
        })
    }

    fn record_ambiguous_binding(&self, name: &str, first: &[String], second: &[String]) {
        if first == second {
            return;
        }
        let first = first.join("::");
        let second = second.join("::");
        self.resolution_error.borrow_mut().get_or_insert_with(|| {
            format!(
                "ambiguous type-namespace binding `{name}`: external import `{first}` may collide with workspace glob binding `{second}`; qualify the intended path explicitly"
            )
        });
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

    /// Finds an exported module binding, following public aliases and globs. A private use in
    /// the importing crate can then name this module without making its own import public.
    fn exported_external_module(
        &self,
        index: &mut Index<'_>,
        module: usize,
        name: &str,
        seen: &mut HashSet<(usize, String)>,
    ) -> Option<usize> {
        if !seen.insert((module, name.to_owned())) {
            return None;
        }
        let items = index.modules.get(module)?.items.clone();
        let krate = index.modules[module].krate.clone();
        let mut module_path = index.modules[module].path.clone();
        for item in &items {
            if let Item::Mod(item) = item
                && item.ident == name
                && default_build(&item.attrs)
            {
                if matches!(item.vis, Visibility::Public(_)) {
                    module_path.push(name.to_owned());
                    return index.module(&krate, &module_path).ok();
                }
                // A private module shadows a same-named module imported by a glob.
                return None;
            }
        }

        for item in &items {
            let Item::Use(import) = item else { continue };
            if !matches!(import.vis, Visibility::Public(_)) || !default_build(&import.attrs) {
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
                if let Some(target) = self.external_module_target(
                    index,
                    module,
                    &path,
                    import.leading_colon.is_some(),
                ) {
                    return Some(target);
                }
            }
            for path in globs {
                if let Some(target) = self.external_module_target(
                    index,
                    module,
                    &path,
                    import.leading_colon.is_some(),
                ) && let Some(target) = self.exported_external_module(index, target, name, seen)
                {
                    return Some(target);
                }
            }
        }
        None
    }

    /// Resolves a `use` target to an indexed module in an outside workspace crate.
    fn external_module_target(
        &self,
        index: &mut Index<'_>,
        module: usize,
        path: &[String],
        rooted: bool,
    ) -> Option<usize> {
        let target = if let Some(path) = self.external_path(index, module, path, rooted) {
            let (krate, path) = path.split_first()?;
            index.module(krate, path).ok()?
        } else {
            Self::indexed_module(index, module, path)?
        };
        self.crates
            .contains(&index.modules.get(target)?.krate)
            .then_some(target)
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

    fn globbed_external_module(
        &self,
        index: &mut Index<'_>,
        module: usize,
        name: &str,
    ) -> Option<usize> {
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
            if let Some(target) = self.external_module_target(index, module, &glob, rooted)
                && let Some(target) =
                    self.exported_external_module(index, target, name, &mut HashSet::new())
            {
                return Some(target);
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
        if let Ok(found) = index.resolve_type(scope, first) {
            match found {
                Found::External(external) => {
                    let workspace_binding =
                        self.globbed_external_path(index, scope, first).or_else(|| {
                            self.globbed_external_module(index, scope, first)
                                .map(|module| {
                                    let module = &index.modules[module];
                                    std::iter::once(module.krate.to_owned())
                                        .chain(module.path.iter().cloned())
                                        .collect()
                                })
                        });
                    let workspace_binding = workspace_binding?;
                    self.record_ambiguous_binding(first, &external, &workspace_binding);
                    if external != workspace_binding {
                        return None;
                    }
                }
                // Values and types occupy separate Rust namespaces: a local constant cannot
                // shadow a same-named type imported by a glob in a signature.
                Found::Item(id)
                    if !self.crates.contains(&index.modules[id.module].krate)
                        && type_item(index.item(id)).is_some() =>
                {
                    if id.module != scope
                        && !self.has_named_type_import(index, scope, first)
                        && let Some(workspace_binding) =
                            self.globbed_external_path(index, scope, first)
                        && let Some(indexed_binding) = Self::indexed_item_path(index, id)
                    {
                        self.record_ambiguous_binding(first, &indexed_binding, &workspace_binding);
                    }
                    return None;
                }
                Found::Item(id) if self.crates.contains(&index.modules[id.module].krate) => {
                    return is_public_external_type(index, id)
                        .then(|| self.indexed_external_path(index, id))
                        .flatten();
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
            if self
                .imported_item(index, scope, &imported, rooted)
                .is_some_and(|id| matches!(index.item(id), Item::Const(_) | Item::Static(_)))
            {
                if !self.has_named_type_import(index, scope, first)
                    && let Some(mut resolved) = self.globbed_external_path(index, scope, first)
                {
                    resolved.extend(rest.iter().cloned());
                    return Some(resolved);
                }
                return None;
            }
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
        if let Some(target) = self.globbed_external_module(index, scope, first) {
            if !rest.is_empty() {
                return self.resolve_external(index, target, rest, seen);
            }
            let target = &index.modules[target];
            return Some(
                std::iter::once(target.krate.to_owned())
                    .chain(target.path.iter().cloned())
                    .collect(),
            );
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

#[cfg(test)]
mod tests {
    use super::{Index, OutsideSeam};
    use std::fs;

    #[test]
    fn private_workspace_glob_resolves_a_module_exported_through_alias_and_glob() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"consumer\", \"engine\"]\nresolver = \"3\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("consumer/src")).unwrap();
        fs::create_dir_all(root.join("engine/src")).unwrap();
        fs::write(
            root.join("consumer/Cargo.toml"),
            "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nengine = { package = \"engine-library\", path = \"../engine\" }\n",
        )
        .unwrap();
        fs::write(root.join("consumer/src/lib.rs"), "use engine::*;\n").unwrap();
        fs::write(
            root.join("engine/Cargo.toml"),
            "[package]\nname = \"engine-library\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\nname = \"engine_library\"\n",
        )
        .unwrap();
        fs::write(
            root.join("engine/src/lib.rs"),
            "mod private { pub mod bridge { pub mod nested { pub struct Hidden; } } }\npub use private::bridge as bridge_alias;\npub use bridge_alias::*;\n",
        )
        .unwrap();

        let outside = OutsideSeam::read(root).unwrap();
        let mut index = Index::new(root, &[], &[]);
        outside.add_to_index(&mut index).unwrap();
        let consumer = index.module("consumer", &[]).unwrap();
        let path = outside.external_path(
            &mut index,
            consumer,
            &["nested", "Hidden"].map(str::to_owned),
            false,
        );
        let path = path.expect("the private workspace glob resolves the exported module binding");

        assert_eq!(
            path,
            ["engine_library", "private", "bridge", "nested", "Hidden"]
        );
    }

    #[test]
    fn external_import_and_globbed_module_collision_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"consumer\", \"engine\"]\nresolver = \"3\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("consumer/src")).unwrap();
        fs::create_dir_all(root.join("engine/src")).unwrap();
        fs::write(
            root.join("consumer/Cargo.toml"),
            "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nengine = { package = \"engine-library\", path = \"../engine\" }\n",
        )
        .unwrap();
        fs::write(
            root.join("consumer/src/lib.rs"),
            "use core::option::Option as nested;\nuse engine::*;\n",
        )
        .unwrap();
        fs::write(
            root.join("engine/Cargo.toml"),
            "[package]\nname = \"engine-library\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\nname = \"engine_library\"\n",
        )
        .unwrap();
        fs::write(
            root.join("engine/src/lib.rs"),
            "pub mod nested { pub struct Hidden; }\n",
        )
        .unwrap();

        let outside = OutsideSeam::read(root).unwrap();
        let mut index = Index::new(root, &[], &[]);
        outside.add_to_index(&mut index).unwrap();
        let consumer = index.module("consumer", &[]).unwrap();
        assert_eq!(
            outside.external_path(
                &mut index,
                consumer,
                &["nested", "Hidden"].map(str::to_owned),
                false,
            ),
            None
        );
        let error = outside
            .take_resolution_error()
            .expect("the collision must be reported through the resolver error path");
        assert!(
            error.contains("ambiguous type-namespace binding `nested`"),
            "{error}"
        );
        assert!(error.contains("engine_library::nested"), "{error}");
        assert!(
            error.contains("qualify the intended path explicitly"),
            "{error}"
        );
    }

    #[test]
    fn rooted_path_resolves_root_extern_crate_alias() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"consumer\", \"engine\"]\nresolver = \"3\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("consumer/src")).unwrap();
        fs::create_dir_all(root.join("engine/src")).unwrap();
        fs::write(
            root.join("consumer/Cargo.toml"),
            "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nengine_library = { package = \"engine-library\", path = \"../engine\" }\n",
        )
        .unwrap();
        fs::write(
            root.join("consumer/src/lib.rs"),
            "extern crate engine_library as engine;\n",
        )
        .unwrap();
        fs::write(
            root.join("engine/Cargo.toml"),
            "[package]\nname = \"engine-library\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\nname = \"engine_library\"\n",
        )
        .unwrap();
        fs::write(root.join("engine/src/lib.rs"), "pub struct Secret;\n").unwrap();

        let outside = OutsideSeam::read(root).unwrap();
        let mut index = Index::new(root, &[], &[]);
        outside.add_to_index(&mut index).unwrap();
        let consumer = index.module("consumer", &[]).unwrap();

        assert_eq!(
            outside.external_path(
                &mut index,
                consumer,
                &["engine", "Secret"].map(str::to_owned),
                true,
            ),
            Some(["engine_library", "Secret"].map(str::to_owned).to_vec())
        );
    }

    #[test]
    fn allowed_type_and_workspace_glob_collision_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"consumer\", \"engine\"]\nresolver = \"3\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("consumer/src")).unwrap();
        fs::create_dir_all(root.join("engine/src")).unwrap();
        fs::create_dir_all(root.join("api/src")).unwrap();
        fs::write(
            root.join("consumer/Cargo.toml"),
            "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nengine = { package = \"engine-library\", path = \"../engine\" }\n",
        )
        .unwrap();
        fs::write(
            root.join("consumer/src/lib.rs"),
            "use allowed_api::*;\nuse engine::*;\n",
        )
        .unwrap();
        fs::write(
            root.join("engine/Cargo.toml"),
            "[package]\nname = \"engine-library\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\nname = \"engine_library\"\n",
        )
        .unwrap();
        fs::write(root.join("engine/src/lib.rs"), "pub struct Secret;\n").unwrap();
        fs::write(root.join("api/src/lib.rs"), "pub struct Secret;\n").unwrap();

        let outside = OutsideSeam::read(root).unwrap();
        let mut index = Index::new(root, &[], &[]);
        outside.add_to_index(&mut index).unwrap();
        index.add_crate("allowed_api", "api/src");
        let consumer = index.module("consumer", &[]).unwrap();

        assert_eq!(
            outside.external_path(&mut index, consumer, &["Secret".to_owned()], false,),
            None
        );
        let error = outside
            .take_resolution_error()
            .expect("the allowed and workspace glob bindings must be reported as ambiguous");
        assert!(
            error.contains("ambiguous type-namespace binding `Secret`"),
            "{error}"
        );
        assert!(error.contains("allowed_api::Secret"), "{error}");
        assert!(error.contains("engine_library::Secret"), "{error}");
    }
}
