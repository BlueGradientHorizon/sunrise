//! The Rust items a set of crates' signatures name, found as rustc finds them: through each
//! module's items, `use`s and glob imports, into the crates `sunrise-mod-api` builds on.
use std::{collections::HashMap, fs, path::Path};
use syn::{Attribute, Item, Meta, UseTree};

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
    /// A path outside the indexed crates (`std::…`, `serde_json::…`), as written.
    External(Vec<String>),
}

#[derive(Clone, Copy)]
enum Namespace {
    Any,
    Type,
}

/// The indexed crates' modules, parsed as they are first reached.
pub struct Index<'a> {
    root: &'a Path,
    /// The crates it indexes: crate name and source directory below `root`.
    crates: &'static [(&'static str, &'static str)],
    /// Sources included into an inline module with `include!`, by file name.
    included: &'a [(&'a str, &'a str)],
    pub modules: Vec<Module>,
    by_path: HashMap<(String, Vec<String>), usize>,
    /// How deep `resolve` is in following imports.
    depth: usize,
    /// Every source file read, for the build script to track.
    pub read: std::cell::RefCell<Vec<std::path::PathBuf>>,
    /// Rewrites a source file's text before it is parsed: the game's generated module lists.
    expand: fn(&Path, &str) -> Result<String, String>,
    /// Crates added for one consumer beside its crates ([`Index::add_crate`]).
    extra: Vec<(String, String)>,
    /// A crate's root file where it is not `lib.rs` (a library's `[lib] path`).
    root_files: HashMap<String, String>,
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
            depth: 0,
            read: Default::default(),
            extra: Vec::new(),
            root_files: HashMap::new(),
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
        self.resolve_in_namespace(module, name, Namespace::Any)
    }

    /// What type `name` means in `module`, following named and glob imports in the type namespace.
    /// A named import of a value does not hide a type provided by a glob import.
    pub fn resolve_type(&mut self, module: usize, name: &str) -> Result<Found, String> {
        self.resolve_in_namespace(module, name, Namespace::Type)
    }

    fn resolve_in_namespace(
        &mut self,
        module: usize,
        name: &str,
        namespace: Namespace,
    ) -> Result<Found, String> {
        self.depth += 1;
        if self.depth > 64 {
            self.depth = 0;
            let m = &self.modules[module];
            return Err(format!(
                "{name} in {}::{} imports itself",
                m.krate,
                m.path.join("::")
            ));
        }
        let found = self.lookup(module, name, namespace, &mut Vec::new());
        self.depth = self.depth.saturating_sub(1);
        found?.ok_or_else(|| {
            let m = &self.modules[module];
            format!("{name} not found in {}::{}", m.krate, m.path.join("::"))
        })
    }

    fn lookup(
        &mut self,
        module: usize,
        name: &str,
        namespace: Namespace,
        seen: &mut Vec<usize>,
    ) -> Result<Option<Found>, String> {
        if seen.contains(&module) {
            return Ok(None);
        }
        seen.push(module);
        let items = &self.modules[module].items;
        if let Some(item) = items.iter().position(|item| {
            default_build(definition_attrs(item)) && defines(item, name, namespace)
        }) {
            return Ok(Some(Found::Item(ItemId { module, item })));
        }
        let mut named = Vec::new();
        let mut globs = Vec::new();
        for item in items {
            if let Item::Use(item) = item
                && default_build(&item.attrs)
            {
                uses(
                    &item.tree,
                    &mut Vec::new(),
                    &mut |path, alias| match alias {
                        Some(alias) if alias == name => named.push(path),
                        None => globs.push(path),
                        _ => {}
                    },
                );
            }
        }
        if matches!(namespace, Namespace::Any)
            && let Some(path) = named.first()
        {
            return self.resolve_path(module, path).map(Some);
        }
        let mut external_named = None;
        for path in named {
            match self.resolve_path(module, &path) {
                Ok(found @ Found::Item(id)) if is_type_item(self.item(id)) => {
                    return Ok(Some(found));
                }
                Ok(found @ Found::External(_)) => {
                    // The index cannot tell whether an unindexed external name is a value or a
                    // type. Try indexed glob types first; preserve the external path as fallback.
                    external_named.get_or_insert(found);
                }
                Ok(Found::Item(_)) | Err(_) => {}
            }
        }
        for glob in globs {
            if let Some((krate, path)) = self.absolute(module, &glob)
                && let Ok(target) = self.module(krate, &path)
                && let Some(found) = self.lookup(target, name, namespace, seen)?
            {
                if let Some(external) = external_named.take() {
                    // Without indexing the external crate, its named binding could be either a
                    // value or a type. Preserve it so the API boundary can fail closed if a glob
                    // also supplies a type with this name.
                    return Ok(Some(external));
                }
                return Ok(Some(found));
            }
        }
        Ok(external_named)
    }

    /// What the path `path`, written in `module`, names.
    pub fn resolve_path(&mut self, module: usize, path: &[String]) -> Result<Found, String> {
        if path.len() == 1 {
            return self.resolve(module, &path[0]);
        }
        let (last, parent) = path.split_last().unwrap();
        if let Some(target) = self.module_path(module, parent, &mut Vec::new())? {
            return self.resolve(target, last);
        }
        // A non-module path prefix can still be an external crate/type alias.
        if let Ok(Found::External(prefix)) = self.resolve(module, &path[0])
            && prefix.len() > 1
        {
            let mut full = prefix;
            full.extend(path[1..].iter().cloned());
            return Ok(Found::External(full));
        }
        Ok(Found::External(path.to_vec()))
    }

    /// What the signature type path `path`, written in `module`, names.
    pub fn resolve_type_path(&mut self, module: usize, path: &[String]) -> Result<Found, String> {
        if path.len() == 1 {
            return self.resolve_type(module, &path[0]);
        }
        let (last, parent) = path.split_last().unwrap();
        if let Some(target) = self.module_path(module, parent, &mut Vec::new())? {
            return self.resolve_type(target, last);
        }
        // A non-module path prefix can still be an external crate/type alias.
        if let Ok(Found::External(prefix)) = self.resolve_type(module, &path[0])
            && prefix.len() > 1
        {
            let mut full = prefix;
            full.extend(path[1..].iter().cloned());
            return Ok(Found::External(full));
        }
        Ok(Found::External(path.to_vec()))
    }

    /// Resolves `path` as a module if its segments name declared modules, following local
    /// `use` aliases. Missing source for a declared module is an error; a type or unknown name is
    /// simply not a module.
    pub fn resolve_module_path(
        &mut self,
        module: usize,
        path: &[String],
    ) -> Result<Option<usize>, String> {
        self.module_path(module, path, &mut Vec::new())
    }

    /// Resolves a path to a module, including `use` aliases of local modules. It checks each
    /// segment against a module declaration so a type prefix such as `Engine::Assoc` remains an
    /// external path instead of being mistaken for a source module.
    fn module_path(
        &mut self,
        module: usize,
        path: &[String],
        seen: &mut Vec<(usize, String)>,
    ) -> Result<Option<usize>, String> {
        if let Some((krate, path)) = self.absolute(module, path) {
            let mut target = self.module(krate.clone(), &[])?;
            let mut prefix = Vec::new();
            for (position, segment) in path.iter().enumerate() {
                let is_module = self.modules[target]
                    .items
                    .iter()
                    .any(|item| {
                        matches!(item, Item::Mod(item) if item.ident == segment.as_str() && default_build(&item.attrs))
                    });
                if !is_module {
                    if krate == self.modules[module].krate {
                        return self.module_path(target, &path[position..], seen);
                    }
                    return Ok(None);
                }
                prefix.push(segment.clone());
                target = self.module(krate.clone(), &prefix)?;
            }
            return Ok(Some(target));
        }

        let (first, rest) = path.split_first().ok_or("an empty module path")?;
        if !seen.contains(&(module, first.clone())) {
            seen.push((module, first.clone()));
            let mut imported = None;
            for item in &self.modules[module].items {
                match item {
                    Item::Use(item) if default_build(&item.attrs) => {
                        uses(&item.tree, &mut Vec::new(), &mut |path, alias| {
                            if alias.as_deref() == Some(first.as_str()) {
                                imported = Some(path);
                            }
                        })
                    }
                    Item::ExternCrate(item) if default_build(&item.attrs) => {
                        let alias = item
                            .rename
                            .as_ref()
                            .map_or_else(|| item.ident.to_string(), |(_, name)| name.to_string());
                        if alias == *first {
                            imported = Some(vec![item.ident.to_string()]);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(mut imported) = imported {
                imported.extend(rest.iter().cloned());
                let resolved = self.module_path(module, &imported, seen)?;
                seen.pop();
                return Ok(resolved);
            }
            seen.pop();
        }
        Ok(None)
    }

    /// `path`, written in `module`, as a module of an indexed crate, if it is one.
    pub fn absolute(&self, module: usize, path: &[String]) -> Option<(String, Vec<String>)> {
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
            name => (self.crate_name(name)?, Vec::new()),
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
        Item::Static(item) => item.ident == name,
        _ => false,
    }
}

fn is_type_item(item: &Item) -> bool {
    matches!(
        item,
        Item::Struct(_) | Item::Enum(_) | Item::Type(_) | Item::Trait(_) | Item::Union(_)
    )
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

fn definition_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        _ => &[],
    }
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
}
