//! The Rust items a set of crates' signatures name, found as rustc finds them: through each
//! module's items, `use`s and glob imports, into the crates `sunrise-mod-api` builds on.
use std::{collections::HashMap, fs, path::Path};
use syn::{Attribute, Item, Meta, UseTree, Visibility};

fn target_cfg(name: &str) -> Option<String> {
    let environment = format!("CARGO_CFG_{}", name.to_ascii_uppercase());
    if let Ok(value) = std::env::var(environment) {
        return Some(value);
    }
    match name {
        "target_os" => Some(std::env::consts::OS.to_owned()),
        "target_arch" => Some(std::env::consts::ARCH.to_owned()),
        "target_family" => Some(
            if cfg!(unix) {
                "unix"
            } else if cfg!(windows) {
                "windows"
            } else {
                "other"
            }
            .to_owned(),
        ),
        "target_pointer_width" => Some(usize::BITS.to_string()),
        "target_endian" if cfg!(target_endian = "little") => Some("little".to_owned()),
        "target_endian" if cfg!(target_endian = "big") => Some("big".to_owned()),
        _ => None,
    }
}

fn target_flag(name: &str, fallback: bool) -> bool {
    std::env::var_os(format!("CARGO_CFG_{}", name.to_ascii_uppercase())).is_some() || fallback
}

fn cfg_active(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => {
            let Some(name) = path.get_ident().map(ToString::to_string) else {
                return true;
            };
            match name.as_str() {
                "test" => false,
                "unix" | "windows" => target_cfg("target_family")
                    .is_none_or(|family| family.split(',').any(|item| item == name)),
                "debug_assertions" => target_flag("debug_assertions", cfg!(debug_assertions)),
                "target_thread_local" => target_flag(
                    "target_thread_local",
                    target_cfg("target_family").is_some_and(|family| {
                        family
                            .split(',')
                            .any(|name| name == "unix" || name == "windows")
                    }),
                ),
                _ => true,
            }
        }
        Meta::NameValue(pair) => {
            let Some(name) = pair.path.get_ident().map(ToString::to_string) else {
                return true;
            };
            if name == "feature" {
                return false;
            }
            if !name.starts_with("target_") {
                return true;
            }
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(expected),
                ..
            }) = &pair.value
            else {
                return true;
            };
            target_cfg(&name).is_none_or(|actual| {
                actual
                    .split(',')
                    .any(|item| item.trim() == expected.value())
            })
        }
        Meta::List(list) => {
            let Ok(inner) = list.parse_args_with(
                syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
            ) else {
                return true;
            };
            if list.path.is_ident("any") {
                inner.iter().any(cfg_active)
            } else if list.path.is_ident("all") {
                inner.iter().all(cfg_active)
            } else if list.path.is_ident("not") {
                !inner.iter().all(cfg_active)
            } else {
                true
            }
        }
    }
}

fn attribute_is_active(meta: &Meta) -> bool {
    match meta {
        Meta::List(list) if list.path.is_ident("cfg") => list
            .parse_args::<Meta>()
            .map(|condition| cfg_active(&condition))
            .unwrap_or(true),
        Meta::List(list) if list.path.is_ident("cfg_attr") => {
            let Ok(args) = list.parse_args_with(
                syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
            ) else {
                return true;
            };
            let Some(condition) = args.first() else {
                return true;
            };
            !cfg_active(condition) || args.iter().skip(1).all(attribute_is_active)
        }
        _ => true,
    }
}

/// Whether source items are present in the default build for Cargo's target.
pub fn default_build(attrs: &[Attribute]) -> bool {
    attrs.iter().all(|attr| attribute_is_active(&attr.meta))
}

/// One module: its crate, its path below the crate root, and its items.
pub struct Module {
    pub krate: String,
    pub path: Vec<String>,
    pub items: Vec<Item>,
}

/// A named item a signature refers to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ItemId {
    pub module: usize,
    pub item: usize,
}

/// Where a name leads.
pub enum Found {
    Item(ItemId),
    /// A path outside the indexed crates, with the scope and root of the original spelling.
    External {
        module: usize,
        path: Vec<String>,
        rooted: bool,
    },
}

/// The outcome of looking up a name without picking an arbitrary glob binding.
pub enum Binding {
    Missing,
    Found(Found),
    Ambiguous(ItemId, ItemId),
}

impl Binding {
    fn require(self, index: &Index<'_>, module: usize, name: &str) -> Result<Found, String> {
        match self {
            Self::Found(found) => Ok(found),
            Self::Ambiguous(first, second) => Err(index.ambiguous_type(name, first, second)),
            Self::Missing => {
                let m = &index.modules[module];
                Err(format!(
                    "{name} not found in {}::{}",
                    m.krate,
                    m.path.join("::")
                ))
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Namespace {
    Any,
    Type,
}

type ActiveLookups = Vec<(usize, String, Namespace)>;

/// The indexed crates' modules, parsed as they are first reached.
pub struct Index<'a> {
    root: &'a Path,
    /// The crates it indexes: crate name and source directory below `root`.
    crates: &'static [(&'static str, &'static str)],
    /// Sources included into an inline module with `include!`, by file name.
    included: &'a [(&'a str, &'a str)],
    pub modules: Vec<Module>,
    by_path: HashMap<(String, Vec<String>), usize>,
    /// Every source file read, for the build script to track.
    pub read: std::cell::RefCell<Vec<std::path::PathBuf>>,
    /// Rewrites a source file's text before it is parsed: the game's generated module lists.
    expand: fn(&Path, &str) -> Result<String, String>,
    /// Crates added for one consumer beside its crates ([`Index::add_crate`]).
    extra: Vec<(String, String)>,
    /// A crate's root file where it is not `lib.rs` (a library's `[lib] path`).
    root_files: HashMap<String, String>,
    /// Cargo names for dependencies whose source is in this index, by importing crate.
    dependency_aliases: HashMap<String, HashMap<String, String>>,
}

/// The file a module's `#[path = "..."]` names.
fn path_attribute(attrs: &[syn::Attribute]) -> Option<String> {
    fn path(meta: &Meta) -> Option<String> {
        match meta {
            Meta::NameValue(value) if value.path.is_ident("path") => match &value.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(file),
                    ..
                }) => Some(file.value()),
                _ => None,
            },
            Meta::List(list) if list.path.is_ident("cfg_attr") => {
                let args = list
                    .parse_args_with(
                        syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                    )
                    .ok()?;
                let condition = args.first()?;
                cfg_active(condition)
                    .then(|| args.iter().skip(1).find_map(path))
                    .flatten()
            }
            _ => None,
        }
    }

    attrs.iter().find_map(|attr| path(&attr.meta))
}

/// The directory below the crate root that holds `path`'s last module, with its slash.
fn parent_dir(path: &[String]) -> String {
    path.split_last().map_or(String::new(), |(_, dir)| {
        dir.iter().map(|d| format!("{d}/")).collect()
    })
}

impl<'a> Index<'a> {
    pub fn new(
        root: &'a Path,
        included: &'a [(&'a str, &'a str)],
        crates: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            root,
            crates,
            expand: |_, source| Ok(source.to_owned()),
            included,
            modules: Vec::new(),
            by_path: HashMap::new(),
            read: Default::default(),
            extra: Vec::new(),
            root_files: HashMap::new(),
            dependency_aliases: HashMap::new(),
        }
    }

    /// Names the file `krate`'s root module lives in, below its source directory.
    pub fn set_root_file(&mut self, krate: &str, file: &str) {
        self.root_files.insert(krate.to_owned(), file.to_owned());
    }

    /// Reads each source file through `expand` before parsing it.
    pub fn expanding(mut self, expand: fn(&Path, &str) -> Result<String, String>) -> Self {
        self.expand = expand;
        self
    }

    /// Indexes one more crate, for this index only: the surface walk reads the facade crate
    /// (`sunrise_mod_api`), which no generated output resolves through.
    pub fn add_crate(&mut self, name: &str, dir: &str) {
        if self.crate_dir(name).is_none() {
            self.extra.push((name.to_owned(), dir.to_owned()));
        }
    }

    pub fn add_dependency_aliases(&mut self, aliases: &HashMap<String, HashMap<String, String>>) {
        self.dependency_aliases.extend(aliases.clone());
    }

    fn extern_crate_name(&self, module: usize, name: &str) -> Option<String> {
        self.dependency_aliases
            .get(&self.modules[module].krate)
            .and_then(|aliases| aliases.get(name))
            .cloned()
            .or_else(|| self.crate_name(name))
    }

    fn crate_name(&self, name: &str) -> Option<String> {
        self.crates
            .iter()
            .find(|(krate, _)| *krate == name)
            .map(|(krate, _)| (*krate).to_owned())
            .or_else(|| {
                self.extra
                    .iter()
                    .find(|(krate, _)| krate == name)
                    .map(|(krate, _)| krate.clone())
            })
    }

    fn crate_dir(&self, name: &str) -> Option<(&str, &str)> {
        self.crates
            .iter()
            .find(|(krate, _)| *krate == name)
            .map(|(krate, dir)| (*krate, *dir))
            .or_else(|| {
                self.extra
                    .iter()
                    .find(|(krate, _)| krate == name)
                    .map(|(krate, dir)| (krate.as_str(), dir.as_str()))
            })
    }

    pub fn item(&self, id: ItemId) -> &Item {
        &self.modules[id.module].items[id.item]
    }

    /// The owning crate, defining module and original name of an indexed item.
    pub fn item_path(&self, id: ItemId) -> Option<Vec<String>> {
        let module = &self.modules[id.module];
        let name = match self.item(id) {
            Item::Const(item) => &item.ident,
            Item::Enum(item) => &item.ident,
            Item::Fn(item) => &item.sig.ident,
            Item::Static(item) => &item.ident,
            Item::Struct(item) => &item.ident,
            Item::Trait(item) => &item.ident,
            Item::Type(item) => &item.ident,
            Item::Union(item) => &item.ident,
            _ => return None,
        };
        Some(
            std::iter::once(module.krate.clone())
                .chain(module.path.iter().cloned())
                .chain(std::iter::once(name.to_string()))
                .collect(),
        )
    }

    /// Describes the conflicting definitions at a use of an ambiguous name.
    pub fn ambiguous_type(&self, name: &str, first: ItemId, second: ItemId) -> String {
        let first = self.item_path(first).expect("a named definition");
        let second = self.item_path(second).expect("a named definition");
        ambiguous_glob(name, &first, &second)
    }

    /// The module at `path` in `krate`, parsed on first use.
    pub fn module(&mut self, krate: impl AsRef<str>, path: &[String]) -> Result<usize, String> {
        let requested = krate.as_ref();
        let krate = self
            .crate_name(requested)
            .ok_or_else(|| format!("{requested} is not a crate the mod API builds on"))?;
        if let Some(&id) = self.by_path.get(&(krate.clone(), path.to_vec())) {
            return Ok(id);
        }
        let items = match path.split_last() {
            None => {
                let file = self
                    .root_files
                    .get(krate.as_str())
                    .map_or("lib.rs", String::as_str);
                self.parse(&krate, file)?
            }
            Some((last, parent)) => {
                let parent = self.module(&krate, parent)?;
                let declarations = &self.modules[parent].items;
                let has_declaration = declarations
                    .iter()
                    .any(|item| matches!(item, Item::Mod(item) if item.ident == last.as_str()));
                let declared = declarations.iter().find_map(|item| match item {
                    Item::Mod(item)
                        if item.ident == last.as_str() && default_build(&item.attrs) =>
                    {
                        Some((item.content.clone(), path_attribute(&item.attrs)))
                    }
                    _ => None,
                });
                if declared.is_none() && has_declaration {
                    return Err(format!(
                        "{}::{} is excluded from the default build",
                        krate,
                        path.join("::")
                    ));
                }
                match declared {
                    Some((Some((_, items)), _)) => self.expand_includes(items)?,
                    // A generated `#[path]` is absolute.
                    Some((None, Some(file))) if Path::new(&file).is_absolute() => {
                        self.parse(&krate, &file)?
                    }
                    // `#[path]` is relative to the declaring file's directory: `parent.rs`
                    // sits beside it, `parent/mod.rs` below.
                    Some((None, Some(file))) => {
                        let beside = parent_dir(&path[..path.len() - 1]);
                        let below = parent_dir(path);
                        self.parse(&krate, &format!("{beside}{file}"))
                            .or_else(|_| self.parse(&krate, &format!("{below}{file}")))?
                    }
                    _ => {
                        let dir = path.join("/");
                        self.parse(&krate, &format!("{dir}.rs"))
                            .or_else(|_| self.parse(&krate, &format!("{dir}/mod.rs")))?
                    }
                }
            }
        };
        let id = self.modules.len();
        self.modules.push(Module {
            krate: krate.clone(),
            path: path.to_vec(),
            items,
        });
        self.by_path.insert((krate, path.to_vec()), id);
        Ok(id)
    }

    fn parse(&self, krate: &str, file: &str) -> Result<Vec<Item>, String> {
        let dir = self.crate_dir(krate).expect("a known crate").1;
        let path = self.root.join(dir).join(file);
        let source = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.read.borrow_mut().push(path.clone());
        let source = (self.expand)(self.root, &source)?;
        let file = syn::parse_file(&source).map_err(|e| format!("{}: {e}", path.display()))?;
        self.expand_includes(file.items)
    }

    /// `items` with each `include!` of a generated file replaced by that file's items.
    fn expand_includes(&self, items: Vec<Item>) -> Result<Vec<Item>, String> {
        let mut out = Vec::new();
        for item in items {
            match &item {
                Item::Macro(mac) if mac.mac.path.is_ident("include") => {
                    let tokens = mac.mac.tokens.to_string();
                    // The wire impls are generated from what this reads, so it never reads them.
                    if tokens.contains("wire_") {
                        continue;
                    }
                    let (name, source) = self
                        .included
                        .iter()
                        .find(|(name, _)| tokens.contains(name))
                        .ok_or_else(|| format!("include!({tokens}) is not a generated file"))?;
                    let file = syn::parse_file(source).map_err(|e| format!("{name}: {e}"))?;
                    out.extend(file.items);
                }
                _ => out.push(item),
            }
        }
        Ok(out)
    }

    /// What `name` means in `module`: an item defined there, a `use` of it, or a glob import
    /// that provides it.
    pub fn resolve(&mut self, module: usize, name: &str) -> Result<Found, String> {
        self.lookup(
            module,
            name,
            Namespace::Any,
            &mut vec![module],
            &mut Vec::new(),
        )?
        .require(self, module, name)
    }

    /// What type `name` means in `module`, following named and glob imports in the type namespace.
    /// A named import of a value does not hide a type provided by a glob import.
    pub fn resolve_type(&mut self, module: usize, name: &str) -> Result<Found, String> {
        self.visible_type_binding(module, name, module)?
            .require(self, module, name)
    }

    /// The type binding that `importer` can bring into scope by globbing `module`.
    pub fn visible_type_binding(
        &mut self,
        module: usize,
        name: &str,
        importer: usize,
    ) -> Result<Binding, String> {
        self.lookup(
            module,
            name,
            Namespace::Type,
            &mut vec![importer],
            &mut Vec::new(),
        )
    }

    fn lookup(
        &mut self,
        module: usize,
        name: &str,
        namespace: Namespace,
        holders: &mut Vec<usize>,
        active: &mut ActiveLookups,
    ) -> Result<Binding, String> {
        // Cut just this branch, leaving the other globs and all active callers intact.
        if active.len() >= 64
            || active
                .iter()
                .any(|(scope, binding, ns)| *scope == module && binding == name && *ns == namespace)
        {
            return Ok(Binding::Missing);
        }
        active.push((module, name.to_owned(), namespace));
        let found = self.lookup_binding(module, name, namespace, holders, active);
        active.pop();
        found
    }

    /// A glob imports only bindings visible to the original importer and every module
    /// holding an intermediate glob. A hidden binding still shadows globs in its namespace.
    fn lookup_binding(
        &mut self,
        module: usize,
        name: &str,
        namespace: Namespace,
        holders: &mut Vec<usize>,
        active: &mut ActiveLookups,
    ) -> Result<Binding, String> {
        let items = &self.modules[module].items;
        let reachable = |vis| {
            holders
                .iter()
                .all(|&holder| self.visible_from(module, vis, holder))
        };
        let mut shadowed = false;
        for (item, definition_item) in items.iter().enumerate() {
            if let Some((attrs, visibility)) = definition(definition_item)
                && default_build(attrs)
                && defines(definition_item, name, namespace)
            {
                if reachable(visibility) {
                    return Ok(Binding::Found(Found::Item(ItemId { module, item })));
                }
                shadowed = true;
            }
        }
        let mut named = Vec::new();
        let mut globs = Vec::new();
        for item in items {
            if let Item::Use(item) = item
                && default_build(&item.attrs)
            {
                let open = reachable(&item.vis);
                uses(
                    &item.tree,
                    &mut Vec::new(),
                    &mut |path, alias| match alias {
                        Some(alias) if alias == name => {
                            named.push((path, item.leading_colon.is_some(), open))
                        }
                        None if open => globs.push((path, item.leading_colon.is_some())),
                        _ => {}
                    },
                );
            }
        }
        if matches!(namespace, Namespace::Any)
            && let Some((path, rooted, open)) = named.first()
        {
            return if *open {
                self.resolve_path_in_namespace(module, path, *rooted, Namespace::Any, active)
            } else {
                Ok(Binding::Missing)
            };
        }
        let mut external_named = None;
        for (path, rooted, open) in named {
            match self.resolve_path_in_namespace(module, &path, rooted, Namespace::Type, active) {
                Ok(found @ Binding::Found(Found::Item(_))) if open => return Ok(found),
                Ok(Binding::Found(found @ Found::External { .. })) if open => {
                    // An unindexed import's namespace is unknown. Preserve the external path
                    // so the caller can handle that without mistaking it for a local glob type.
                    external_named.get_or_insert(found);
                }
                Ok(ambiguous @ Binding::Ambiguous(..)) if open => return Ok(ambiguous),
                Ok(Binding::Found(_) | Binding::Ambiguous(..)) => shadowed = true,
                Ok(Binding::Missing) | Err(_) => {}
            }
        }
        if shadowed {
            return Ok(Binding::Missing);
        }
        if let Some(found) = external_named {
            return Ok(Binding::Found(found));
        }
        let mut candidate = None;
        let mut indexed = None;
        for (glob, rooted) in globs {
            let Some(target) = self.module_path(module, &glob, rooted, &mut Vec::new())? else {
                continue;
            };
            holders.push(module);
            let found = self.lookup(target, name, namespace, holders, active);
            holders.pop();
            match found? {
                Binding::Found(found) => {
                    if matches!(namespace, Namespace::Any) {
                        return Ok(Binding::Found(found));
                    }
                    if let Found::Item(id) = found {
                        let first = *indexed.get_or_insert(id);
                        if first != id {
                            return Ok(Binding::Ambiguous(first, id));
                        }
                    }
                    candidate.get_or_insert(found);
                }
                ambiguous @ Binding::Ambiguous(..) => return Ok(ambiguous),
                Binding::Missing => {}
            }
        }
        Ok(candidate.map_or(Binding::Missing, Binding::Found))
    }

    /// A restricted item is visible only in the named module and its descendants.
    fn visible_from(&self, module: usize, visibility: &Visibility, importer: usize) -> bool {
        if module == importer {
            return true;
        }
        let scope = &self.modules[module];
        let importer = &self.modules[importer];
        match visibility {
            Visibility::Public(_) => true,
            Visibility::Inherited => {
                scope.krate == importer.krate && importer.path.starts_with(&scope.path)
            }
            Visibility::Restricted(visibility) => {
                let path = visibility
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>();
                self.local_absolute(module, &path)
                    .is_some_and(|(krate, path)| {
                        krate == importer.krate && importer.path.starts_with(&path)
                    })
            }
        }
    }

    /// What the path `path`, written in `module`, names.
    pub fn resolve_path(&mut self, module: usize, path: &[String]) -> Result<Found, String> {
        self.resolve_path_in_namespace(module, path, false, Namespace::Any, &mut Vec::new())?
            .require(self, module, path.last().map_or("", String::as_str))
    }

    /// What the signature type path `path`, written in `module`, names.
    pub fn resolve_type_path(&mut self, module: usize, path: &[String]) -> Result<Found, String> {
        self.resolve_path_in_namespace(module, path, false, Namespace::Type, &mut Vec::new())?
            .require(self, module, path.last().map_or("", String::as_str))
    }

    /// What a `::`-rooted import names, skipping local bindings of the crate name.
    pub fn resolve_rooted_path(&mut self, module: usize, path: &[String]) -> Result<Found, String> {
        self.resolve_path_in_namespace(module, path, true, Namespace::Any, &mut Vec::new())?
            .require(self, module, path.last().map_or("", String::as_str))
    }

    fn resolve_path_in_namespace(
        &mut self,
        module: usize,
        path: &[String],
        rooted: bool,
        namespace: Namespace,
        active: &mut ActiveLookups,
    ) -> Result<Binding, String> {
        let (last, parent) = path.split_last().ok_or("an empty item path")?;
        if !rooted && parent.is_empty() {
            return self.lookup(module, last, namespace, &mut vec![module], active);
        }
        if !parent.is_empty()
            && let Some(target) = self.module_path(module, parent, rooted, &mut Vec::new())?
        {
            return self.lookup(target, last, namespace, &mut vec![target], active);
        }
        // A non-module path prefix can still be an external crate/type alias.
        if !rooted {
            match self.lookup(module, &path[0], namespace, &mut vec![module], active)? {
                Binding::Found(Found::External {
                    module,
                    path: mut prefix,
                    rooted,
                }) if rooted || prefix.len() > 1 => {
                    prefix.extend(path[1..].iter().cloned());
                    return Ok(Binding::Found(Found::External {
                        module,
                        path: prefix,
                        rooted,
                    }));
                }
                ambiguous @ Binding::Ambiguous(..) => return Ok(ambiguous),
                _ => {}
            }
        }
        Ok(Binding::Found(Found::External {
            module,
            path: path.to_vec(),
            rooted,
        }))
    }

    /// Resolves `path` as a module if its segments name declared modules, following local
    /// `use` aliases. Missing source for a declared module is an error; a type or unknown name is
    /// simply not a module.
    pub fn resolve_module_path(
        &mut self,
        module: usize,
        path: &[String],
    ) -> Result<Option<usize>, String> {
        self.module_path(module, path, false, &mut Vec::new())
    }

    /// Resolves a module under an indexed crate named explicitly in the extern prelude.
    pub fn resolve_rooted_module_path(
        &mut self,
        module: usize,
        path: &[String],
    ) -> Result<Option<usize>, String> {
        self.module_path(module, path, true, &mut Vec::new())
    }

    /// Resolves a path to a module, including `use` aliases of local modules. It checks each
    /// segment against a module declaration so a type prefix such as `Engine::Assoc` remains an
    /// external path instead of being mistaken for a source module.
    fn module_path(
        &mut self,
        module: usize,
        path: &[String],
        rooted: bool,
        seen: &mut Vec<(usize, String)>,
    ) -> Result<Option<usize>, String> {
        let (first, rest) = path.split_first().ok_or("an empty module path")?;
        let mut absolute = if rooted {
            None
        } else {
            self.local_absolute(module, path)
        };
        // A named module alias is a local type-namespace binding, ahead of Cargo's
        // extern prelude. Resolve the alias itself before following the remaining segments.
        if !rooted && absolute.is_none() && !seen.contains(&(module, first.clone())) {
            let mut imported = None;
            for item in &self.modules[module].items {
                match item {
                    Item::Use(item) if default_build(&item.attrs) => {
                        uses(&item.tree, &mut Vec::new(), &mut |path, alias| {
                            if alias.as_deref() == Some(first.as_str()) {
                                imported = Some((path, item.leading_colon.is_some()));
                            }
                        })
                    }
                    Item::ExternCrate(item) if default_build(&item.attrs) => {
                        let alias = item
                            .rename
                            .as_ref()
                            .map_or_else(|| item.ident.to_string(), |(_, name)| name.to_string());
                        if alias == *first {
                            imported = Some((vec![item.ident.to_string()], true));
                        }
                    }
                    _ => {}
                }
            }
            if let Some((imported, rooted)) = imported {
                seen.push((module, first.clone()));
                let target = self.module_path(module, &imported, rooted, seen);
                seen.pop();
                if let Some(target) = target? {
                    let Module { krate, path, .. } = &self.modules[target];
                    absolute = Some((krate.clone(), path.iter().chain(rest).cloned().collect()));
                }
            }
        }
        let Some((krate, path)) = absolute.or_else(|| {
            self.extern_crate_name(module, first)
                .map(|krate| (krate, rest.to_vec()))
        }) else {
            return Ok(None);
        };

        let mut target = self.module(&krate, &[])?;
        let mut prefix = Vec::new();
        for (position, segment) in path.iter().enumerate() {
            let is_module = self.modules[target].items.iter().any(|item| {
                matches!(item, Item::Mod(item) if item.ident == segment.as_str() && default_build(&item.attrs))
            });
            if !is_module {
                if krate == self.modules[module].krate {
                    return self.module_path(target, &path[position..], false, seen);
                }
                return Ok(None);
            }
            prefix.push(segment.clone());
            target = self.module(&krate, &prefix)?;
        }
        Ok(Some(target))
    }

    /// A path to a declared local module, or one starting at `crate`, `self` or `super`.
    fn local_absolute(&self, module: usize, path: &[String]) -> Option<(String, Vec<String>)> {
        let m = &self.modules[module];
        let (first, rest) = path.split_first()?;
        let mut base = match first.as_str() {
            "crate" => (m.krate.clone(), Vec::new()),
            "self" => (m.krate.clone(), m.path.clone()),
            "super" => (
                m.krate.clone(),
                m.path[..m.path.len().checked_sub(1)?].to_vec(),
            ),
            name if m.items.iter().any(|item| {
                matches!(item, Item::Mod(item) if item.ident == name && default_build(&item.attrs))
            }) =>
            {
                let mut path = m.path.clone();
                path.push(name.to_string());
                (m.krate.clone(), path)
            }
            _ => return None,
        };
        for segment in rest {
            if segment == "super" {
                base.1.pop()?;
            } else {
                base.1.push(segment.clone());
            }
        }
        Some(base)
    }
}

/// Describes two distinct definitions imported into a type namespace through globs.
pub fn ambiguous_glob(name: &str, first: &[String], second: &[String]) -> String {
    let first = first.join("::");
    let second = second.join("::");
    format!(
        "ambiguous type-namespace binding `{name}`: glob imports supply both `{first}` and `{second}`; qualify the intended path explicitly"
    )
}

/// Whether `item` defines `name` in the requested namespace.
fn defines(item: &Item, name: &str, namespace: Namespace) -> bool {
    if matches!(namespace, Namespace::Type) {
        return is_type_item_named(item, name);
    }
    match item {
        Item::Struct(item) => item.ident == name,
        Item::Enum(item) => item.ident == name,
        Item::Type(item) => item.ident == name,
        Item::Trait(item) => item.ident == name,
        Item::Union(item) => item.ident == name,
        Item::Const(item) => item.ident == name,
        Item::Fn(item) => item.sig.ident == name,
        Item::Static(item) => item.ident == name,
        _ => false,
    }
}

fn is_type_item_named(item: &Item, name: &str) -> bool {
    match item {
        Item::Struct(item) => item.ident == name,
        Item::Enum(item) => item.ident == name,
        Item::Type(item) => item.ident == name,
        Item::Trait(item) => item.ident == name,
        Item::Union(item) => item.ident == name,
        _ => false,
    }
}

fn definition(item: &Item) -> Option<(&[Attribute], &Visibility)> {
    let (attrs, visibility) = match item {
        Item::Const(item) => (&item.attrs, &item.vis),
        Item::Fn(item) => (&item.attrs, &item.vis),
        Item::Enum(item) => (&item.attrs, &item.vis),
        Item::Struct(item) => (&item.attrs, &item.vis),
        Item::Static(item) => (&item.attrs, &item.vis),
        Item::Trait(item) => (&item.attrs, &item.vis),
        Item::Type(item) => (&item.attrs, &item.vis),
        Item::Union(item) => (&item.attrs, &item.vis),
        _ => return None,
    };
    Some((attrs, visibility))
}

/// Each path `tree` imports, with the name it binds (`None` for a glob).
pub fn uses(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    f: &mut impl FnMut(Vec<String>, Option<String>),
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            uses(&path.tree, prefix, f);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let mut path = prefix.clone();
            if name.ident != "self" {
                path.push(name.ident.to_string());
            }
            let alias = path.last().cloned();
            f(path, alias);
        }
        UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            if rename.ident != "self" {
                path.push(rename.ident.to_string());
            }
            f(path, Some(rename.rename.to_string()));
        }
        UseTree::Glob(_) => f(prefix.clone(), None),
        UseTree::Group(group) => {
            for tree in &group.items {
                uses(tree, prefix, f);
            }
        }
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    use std::fs;

    /// Resolve a name in a module of a one-crate test index.
    fn resolves(lib: &str, module: &[&str], name: &str) -> Option<Vec<String>> {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("k/src")).unwrap();
        fs::write(dir.path().join("k/src/lib.rs"), lib).unwrap();
        let mut index = Index::new(dir.path(), &[], &[("k", "k/src")]);
        let path: Vec<String> = module.iter().map(|m| (*m).to_owned()).collect();
        let module = index.module("k", &path).unwrap();
        match index.resolve(module, name) {
            Ok(Found::Item(id)) => Some(index.modules[id.module].path.clone()),
            Ok(Found::External { path, .. }) => panic!("{name} left the crate: {path:?}"),
            Err(_) => None,
        }
    }

    #[test]
    fn default_build_uses_debug_and_thread_local_cfgs() {
        let debug: Attribute = syn::parse_quote!(#[cfg(debug_assertions)]);
        assert_eq!(default_build(&[debug]), cfg!(debug_assertions));

        let thread_local: Attribute = syn::parse_quote!(#[cfg(target_thread_local)]);
        let family_supports_thread_local = target_cfg("target_family").is_some_and(|family| {
            family
                .split(',')
                .any(|name| name == "unix" || name == "windows")
        });
        assert_eq!(
            default_build(&[thread_local]),
            target_flag("target_thread_local", family_supports_thread_local)
        );

        let test_only: Attribute = syn::parse_quote!(#[cfg(test)]);
        assert!(!default_build(&[test_only]));
    }

    #[test]
    fn module_uses_the_active_cfg_attr_path() {
        const CRATES: &[(&str, &str)] = &[("engine", "engine/src")];
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("engine/src/platform")).unwrap();
        fs::write(
            root.join("engine/src/lib.rs"),
            "#[cfg_attr(unix, cfg_attr(all(), path = \"platform/unix.rs\"))]\n#[cfg_attr(windows, path = \"platform/windows.rs\")]\n#[cfg_attr(not(any(unix, windows)), path = \"platform/other.rs\")]\npub mod platform;\n",
        )
        .unwrap();
        fs::write(
            root.join("engine/src/platform/unix.rs"),
            "pub struct UnixPlatform;\n",
        )
        .unwrap();
        fs::write(
            root.join("engine/src/platform/windows.rs"),
            "pub struct WindowsPlatform;\n",
        )
        .unwrap();
        fs::write(
            root.join("engine/src/platform/other.rs"),
            "pub struct OtherPlatform;\n",
        )
        .unwrap();

        let mut index = Index::new(root, &[], CRATES);
        let module = index.module("engine", &["platform".to_owned()]).unwrap();
        let expected = if cfg!(windows) {
            "WindowsPlatform"
        } else if cfg!(unix) {
            "UnixPlatform"
        } else {
            "OtherPlatform"
        };
        assert!(matches!(
            index.resolve(module, expected),
            Ok(Found::Item(_))
        ));
    }

    #[test]
    fn type_resolution_skips_named_const_and_static_imports_before_globs() {
        const CRATES: &[(&str, &str)] = &[("consumer", "consumer/src"), ("engine", "engine/src")];
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("consumer/src")).unwrap();
        fs::create_dir_all(root.join("engine/src")).unwrap();
        fs::write(
            root.join("consumer/src/lib.rs"),
            "const LOCAL_CONST: u8 = 0;\nstatic LOCAL_STATIC: u8 = 0;\nuse crate::LOCAL_CONST as ConstName;\nuse crate::LOCAL_STATIC as StaticName;\nuse engine::*;\n",
        )
        .unwrap();
        fs::write(
            root.join("engine/src/lib.rs"),
            "pub struct ConstName;\npub struct StaticName;\n",
        )
        .unwrap();

        let mut index = Index::new(root, &[], CRATES);
        let consumer = index.module("consumer", &[]).unwrap();
        for name in ["ConstName", "StaticName"] {
            let Found::Item(id) = index.resolve_type(consumer, name).unwrap() else {
                panic!("{name} should resolve to the globbed type");
            };
            assert_eq!(index.modules[id.module].krate, "engine");
            assert!(matches!(index.item(id), Item::Struct(item) if item.ident == name));
        }

        let Found::Item(id) = index.resolve(consumer, "LOCAL_STATIC").unwrap() else {
            panic!("the static should remain resolvable as a value name");
        };
        assert!(matches!(index.item(id), Item::Static(item) if item.ident == "LOCAL_STATIC"));
    }

    #[test]
    fn type_resolution_prefers_a_named_type_import_over_a_glob() {
        const CRATES: &[(&str, &str)] = &[("consumer", "consumer/src"), ("engine", "engine/src")];
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("consumer/src")).unwrap();
        fs::create_dir_all(root.join("engine/src")).unwrap();
        fs::write(
            root.join("consumer/src/lib.rs"),
            "pub struct Explicit;\nuse crate::Explicit as Secret;\nuse engine::*;\n",
        )
        .unwrap();
        fs::write(root.join("engine/src/lib.rs"), "pub struct Secret;\n").unwrap();

        let mut index = Index::new(root, &[], CRATES);
        let consumer = index.module("consumer", &[]).unwrap();
        let Found::Item(id) = index
            .resolve_type_path(consumer, &["Secret".to_owned()])
            .unwrap()
        else {
            panic!("the explicit named type import should resolve to an indexed item");
        };
        assert_eq!(index.modules[id.module].krate, "consumer");
        assert!(matches!(index.item(id), Item::Struct(item) if item.ident == "Explicit"));
    }

    #[test]
    fn mixed_named_and_glob_cycles_leave_later_globs_available() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("k/src")).unwrap();
        fs::write(
            dir.path().join("k/src/lib.rs"),
            "mod left { pub use crate::middle::{Name, Shown}; }
             mod right { pub use crate::middle::{Name, Shown}; }
             mod middle { pub use crate::left::*; pub use crate::right::*; pub use crate::later::*; }
             mod later { pub struct Shown; }",
        )
        .unwrap();
        let mut index = Index::new(dir.path(), &[], &[("k", "k/src")]);
        let root = index.module("k", &[]).unwrap();
        assert!(
            index
                .resolve_type_path(root, &["middle".into(), "Name".into()])
                .is_err()
        );
        let Found::Item(id) = index
            .resolve_type_path(root, &["middle".into(), "Shown".into()])
            .unwrap()
        else {
            panic!("the later glob should supply Shown");
        };
        assert_eq!(index.modules[id.module].path, ["later"]);
    }

    #[test]
    fn a_glob_of_a_glob_imports_only_what_every_glob_holder_can_see() {
        let lib = "
            pub mod z {
                pub mod y { pub(super) struct Hidden; pub struct Shown; }
                pub mod w { use crate::*; }
            }
            pub use z::y::*;
        ";
        assert_eq!(
            resolves(lib, &["z", "w"], "Shown"),
            Some(vec!["z".into(), "y".into()])
        );
        assert_eq!(resolves(lib, &["z", "w"], "Hidden"), None);
    }

    #[test]
    fn a_definition_hidden_from_the_importer_still_shadows_its_modules_globs() {
        let lib = "
            pub mod a { struct Thing; pub use crate::b::*; }
            pub mod b { pub struct Thing; pub struct Other; }
            pub mod c { use crate::a::*; }
        ";
        assert_eq!(resolves(lib, &["c"], "Other"), Some(vec!["b".into()]));
        assert_eq!(resolves(lib, &["c"], "Thing"), None);
    }
}
