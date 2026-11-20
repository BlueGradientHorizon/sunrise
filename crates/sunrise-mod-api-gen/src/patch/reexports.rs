//! The source-level part of path resolution that can prove a copy crosses a restricted `use`.
//! Leave external crates, macro expansions and ambiguous bindings to the compiler.

use super::{Item, Source, Visit};
use crate::source::unraw;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use syn::{Attribute, ItemUse, UseTree, Visibility};

pub(super) type Scopes = HashMap<String, Rc<Scope>>;

#[derive(Default)]
pub(super) struct Scope {
    items: HashMap<String, Binding>,
    imports: HashMap<String, Option<Import>>,
    globs: Vec<Import>,
    opaque: bool,
}

struct Import {
    path: Vec<String>,
    external: bool,
    conditional: bool,
    limit: Option<String>,
}

#[derive(Clone)]
struct Binding {
    module: Option<String>,
    // Which module and its descendants can see this name in the original source. `None` is pub.
    limit: Option<String>,
    restricted_import: bool,
}

fn within(module: &str, ancestor: &str) -> bool {
    module == ancestor
        || module
            .strip_prefix(ancestor)
            .is_some_and(|tail| tail.starts_with("::"))
}

fn limit(module: &str, vis: &Visibility) -> Option<String> {
    match vis {
        Visibility::Public(_) => None,
        Visibility::Inherited => Some(module.to_owned()),
        Visibility::Restricted(vis) => {
            let path: Vec<_> = vis.path.segments.iter().map(|s| unraw(&s.ident)).collect();
            let (base, tail) = relative(module, &path);
            Some(
                tail.iter()
                    .fold(base.to_owned(), |base, name| format!("{base}::{name}")),
            )
        }
    }
}

/// The starting scope and the rest of a path, after consuming Rust's relative qualifiers.
fn relative<'a>(module: &'a str, mut path: &'a [String]) -> (&'a str, &'a [String]) {
    let Some((first, tail)) = path.split_first() else {
        return (module, path);
    };
    match first.as_str() {
        "crate" => (module.split("::").next().unwrap_or(module), tail),
        "self" => (module, tail),
        "super" => {
            let mut base = module;
            while let Some(("super", tail)) = path.split_first().map(|(s, tail)| (s.as_str(), tail))
            {
                let Some((parent, _)) = base.rsplit_once("::") else {
                    break;
                };
                base = parent;
                path = tail;
            }
            (base, path)
        }
        _ => (module, path),
    }
}

fn conditional(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
}

fn imports(item: &ItemUse, mut insert: impl FnMut(Option<String>, Vec<String>)) {
    fn walk(
        tree: &UseTree,
        prefix: &mut Vec<String>,
        insert: &mut impl FnMut(Option<String>, Vec<String>),
    ) {
        let leaf = |name: &syn::Ident, mut path: Vec<String>| {
            if name != "self" {
                path.push(unraw(name));
            }
            path
        };
        match tree {
            UseTree::Path(next) => {
                prefix.push(unraw(&next.ident));
                walk(&next.tree, prefix, insert);
                prefix.pop();
            }
            UseTree::Name(name) => {
                let path = leaf(&name.ident, prefix.clone());
                if let Some(name) = path.last() {
                    insert(Some(name.clone()), path);
                }
            }
            UseTree::Rename(rename) if rename.rename != "_" => {
                insert(
                    Some(unraw(&rename.rename)),
                    leaf(&rename.ident, prefix.clone()),
                );
            }
            UseTree::Rename(_) => {}
            UseTree::Glob(_) => insert(None, prefix.clone()),
            UseTree::Group(group) => {
                for tree in &group.items {
                    walk(tree, prefix, insert);
                }
            }
        }
    }
    walk(&item.tree, &mut Vec::new(), &mut insert);
}

impl Scope {
    fn read<'a>(module: &str, items: impl IntoIterator<Item = &'a Item>) -> Self {
        let mut scope = Self::default();
        for item in items {
            let (ident, vis, attrs) = match item {
                Item::Use(item) if !mod_source::modules::test_only(&item.attrs) => {
                    let limit = limit(module, &item.vis);
                    imports(item, |name, path| {
                        let import = Import {
                            path,
                            external: item.leading_colon.is_some(),
                            conditional: conditional(&item.attrs),
                            limit: limit.clone(),
                        };
                        if let Some(name) = name {
                            let import = (!import.conditional).then_some(import);
                            scope
                                .imports
                                .entry(name)
                                .and_modify(|entry| *entry = None)
                                .or_insert(import);
                        } else {
                            scope.globs.push(import);
                        }
                    });
                    continue;
                }
                Item::Mod(i) => (&i.ident, &i.vis, &i.attrs),
                Item::Fn(i) => (&i.sig.ident, &i.vis, &i.attrs),
                Item::Const(i) => (&i.ident, &i.vis, &i.attrs),
                Item::Static(i) => (&i.ident, &i.vis, &i.attrs),
                Item::Struct(i) => (&i.ident, &i.vis, &i.attrs),
                Item::Enum(i) => (&i.ident, &i.vis, &i.attrs),
                Item::Union(i) => (&i.ident, &i.vis, &i.attrs),
                Item::Type(i) => (&i.ident, &i.vis, &i.attrs),
                Item::Trait(i) => (&i.ident, &i.vis, &i.attrs),
                Item::TraitAlias(i) => (&i.ident, &i.vis, &i.attrs),
                Item::ExternCrate(i) => (
                    i.rename.as_ref().map_or(&i.ident, |(_, name)| name),
                    &i.vis,
                    &i.attrs,
                ),
                Item::Macro(i) if i.ident.is_none() => {
                    scope.opaque |= !mod_source::modules::test_only(&i.attrs);
                    continue;
                }
                _ => continue,
            };
            if !mod_source::modules::test_only(attrs) {
                let name = unraw(ident);
                scope.items.insert(
                    name.clone(),
                    Binding {
                        module: matches!(item, Item::Mod(_)).then(|| format!("{module}::{name}")),
                        limit: limit(module, vis),
                        restricted_import: false,
                    },
                );
            }
        }
        scope
    }
}

// Not found is distinct from being unable to tell what an opaque glob supplies.
struct Unresolved;
type Lookup = Result<Option<Binding>, Unresolved>;

struct Walk<'a> {
    source: &'a mut Source,
    // `use super::*` and sibling globs can reach the very name already being looked up.
    active: HashSet<(String, String)>,
}

impl Walk<'_> {
    fn scope(&mut self, module: &str) -> Option<Rc<Scope>> {
        if let Some(scope) = self.source.cache.scopes.get(module) {
            return Some(Rc::clone(scope));
        }
        let scope = Rc::new(Scope::read(module, &self.source.module(module).ok()?.items));
        self.source
            .cache
            .scopes
            .insert(module.to_owned(), Rc::clone(&scope));
        Some(scope)
    }

    fn imported_module(&mut self, scope: &str, import: &Import) -> Option<String> {
        if import.external || import.conditional {
            return None;
        }
        let (module, tail) = relative(scope, &import.path);
        let mut module = module.to_owned();
        for name in tail {
            module = self.binding(&module, name, true).ok().flatten()?.module?;
        }
        Some(module)
    }

    fn binding(&mut self, module: &str, name: &str, follow: bool) -> Lookup {
        let scope = self.scope(module).ok_or(Unresolved)?;
        if let Some(item) = scope.items.get(name) {
            return Ok(Some(item.clone()));
        }
        // The game module walker knows about build-generated declarations too.
        let child = format!("{module}::{name}");
        if self.source.files.contains_key(&child) {
            return Ok(Some(Binding {
                module: Some(child),
                limit: Some(module.to_owned()),
                restricted_import: false,
            }));
        }
        if scope.imports.is_empty() && scope.globs.is_empty() {
            return if scope.opaque {
                Err(Unresolved)
            } else {
                Ok(None)
            };
        }
        let key = (module.to_owned(), name.to_owned());
        if !self.active.insert(key.clone()) {
            return Ok(None);
        }
        let binding = self.import_binding(&scope, module, name, follow);
        self.active.remove(&key);
        binding
    }

    fn import_binding(&mut self, scope: &Scope, module: &str, name: &str, follow: bool) -> Lookup {
        if let Some(import) = scope.imports.get(name) {
            let import = import.as_ref().ok_or(Unresolved)?;
            return Ok(Some(Binding {
                module: if follow {
                    self.imported_module(module, import)
                } else {
                    None
                },
                limit: import.limit.clone(),
                restricted_import: import.limit.is_some(),
            }));
        }
        if scope.opaque {
            return Err(Unresolved);
        }
        let mut found = None;
        for import in &scope.globs {
            // A glob we cannot read (including an external crate) might provide this name too.
            let from = self.imported_module(module, import).ok_or(Unresolved)?;
            if let Some(mut binding) = self.binding(&from, name, follow)?
                && binding
                    .limit
                    .as_ref()
                    .is_none_or(|limit| within(module, limit))
            {
                if found.is_some() {
                    return Err(Unresolved);
                }
                binding.restricted_import |= import.limit.is_some();
                if let Some(limit) = &import.limit
                    && binding.limit.as_ref().is_none_or(|old| within(limit, old))
                {
                    binding.limit = Some(limit.clone());
                }
                found = Some(binding);
            }
        }
        Ok(found)
    }

    fn blocked(&mut self, origin: &str, path: &[String]) -> Option<String> {
        let (module, tail) = relative(origin, path);
        let mut module = module.to_owned();
        for (index, name) in tail.iter().enumerate() {
            let binding = self
                .binding(&module, name, index + 1 < tail.len())
                .ok()
                .flatten()?;
            if binding
                .limit
                .as_ref()
                .is_some_and(|limit| !within(origin, limit))
            {
                return None;
            }
            // The copy's module and its ancestors are mirrors, which repeat their imports.
            if binding.restricted_import && !within(origin, &module) {
                return Some(module);
            }
            module = binding.module?;
        }
        None
    }
}

pub(super) fn check_body(
    source: &mut Source,
    module: &str,
    body: &syn::Block,
) -> Result<(), String> {
    struct Check<'a> {
        walk: Walk<'a>,
        origin: &'a str,
        local: Vec<Scope>,
        refusal: Option<String>,
    }
    impl Check<'_> {
        fn path(&mut self, parts: &[String]) {
            if self.refusal.is_some() || parts.len() < 2 {
                return;
            }
            let first = &parts[0];
            if !matches!(first.as_str(), "crate" | "self" | "super")
                && self.local.iter().any(|scope| {
                    scope.opaque
                        || !scope.globs.is_empty()
                        || scope.items.contains_key(first)
                        || scope.imports.contains_key(first)
                })
            {
                // A block-local binding can shadow the module's name. Don't guess its target.
                return;
            }
            if let Some(module) = self.walk.blocked(self.origin, parts) {
                self.refusal = Some(format!(
                    "its body names `{}`, which crosses a non-public `use` in `{module}`",
                    parts.join("::")
                ));
            }
        }
    }
    impl<'ast> Visit<'ast> for Check<'_> {
        fn visit_block(&mut self, block: &'ast syn::Block) {
            let mut items = block
                .stmts
                .iter()
                .filter_map(|stmt| match stmt {
                    syn::Stmt::Item(item) => Some(item),
                    _ => None,
                })
                .peekable();
            if items.peek().is_some() {
                self.local.push(Scope::read(self.origin, items));
                syn::visit::visit_block(self, block);
                self.local.pop();
            } else {
                syn::visit::visit_block(self, block);
            }
        }

        fn visit_path(&mut self, path: &'ast syn::Path) {
            if self.refusal.is_some() {
                return;
            }
            if path.leading_colon.is_none() && path.segments.len() > 1 {
                let parts: Vec<_> = path.segments.iter().map(|s| unraw(&s.ident)).collect();
                self.path(&parts);
            }
            syn::visit::visit_path(self, path);
        }

        fn visit_item_use(&mut self, item: &'ast ItemUse) {
            if item.leading_colon.is_none() && !conditional(&item.attrs) {
                imports(item, |_, parts| self.path(&parts));
            }
        }

        // An inline module nested inside the body has its own scope, not the copy's scope.
        fn visit_item_mod(&mut self, _: &'ast syn::ItemMod) {}
    }
    let mut check = Check {
        walk: Walk {
            source,
            active: HashSet::new(),
        },
        origin: module,
        local: Vec::new(),
        refusal: None,
    };
    check.visit_block(body);
    check.refusal.map_or(Ok(()), Err)
}
