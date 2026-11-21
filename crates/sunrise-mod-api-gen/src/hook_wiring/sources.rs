use crate::source;
use mod_source::modules::{ModuleVariant, cfg_conditions, evaluate_cfg, walk_module_variants};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};
use syn::Item;

/// A non-test cfg assignment. A first pass discovers a relevant unknown predicate; the check
/// then repeats with each value. Unrelated crates and modules don't split the check's builds.
#[derive(Clone, Default)]
pub(super) struct Build {
    pub(super) values: BTreeMap<String, bool>,
    pub(super) unknown: RefCell<Option<String>>,
}

impl Build {
    fn permits(&self, conditions: &[String]) -> bool {
        for condition in conditions {
            let mut missing = None;
            match evaluate_cfg(condition, |atom| {
                if atom == "test" {
                    Some(false)
                } else if let Some(value) = self.values.get(atom) {
                    Some(*value)
                } else {
                    missing.get_or_insert_with(|| atom.to_owned());
                    None
                }
            }) {
                Some(false) => return false,
                None if missing.is_some() => {
                    if self.unknown.borrow().is_none() {
                        *self.unknown.borrow_mut() = missing;
                    }
                }
                _ => {}
            }
        }
        true
    }

    pub(super) fn enabled(&self, attrs: &[syn::Attribute]) -> bool {
        self.permits(&cfg_conditions(attrs))
    }
}

type ModuleItems = Vec<(Vec<String>, Vec<Item>)>;

/// Index packages by Cargo's library roots (including client-lib's out-of-package root).
/// Cache parsed bodies only for modules the check inspects.
pub(super) struct Sources<'a> {
    pub(super) root: &'a Path,
    files: HashMap<String, Result<Vec<ModuleVariant>, String>>,
    modules: HashMap<(String, String), ModuleItems>,
}

impl<'a> Sources<'a> {
    pub(super) fn new(root: &'a Path) -> Self {
        Self {
            root,
            files: HashMap::new(),
            modules: HashMap::new(),
        }
    }

    pub(super) fn items<'s>(
        &'s mut self,
        build: &'s Build,
        package: &str,
        module: &str,
    ) -> Result<Option<impl Iterator<Item = &'s Item>>, String> {
        let key = (package.to_owned(), module.to_owned());
        if !self.modules.contains_key(&key) {
            let root = self.root;
            let parse = |path: &Path, text: &str| {
                let parsed = if package == "sunrise_mod_vocab" {
                    syn::parse_file(&crate::exports::expand(root, text)?)
                } else {
                    syn::parse_file(text)
                };
                parsed.map_err(|error| format!("{}: {error}", path.display()))
            };
            let files = self.files.entry(package.to_owned()).or_insert_with(|| {
                let (_, lib) = source::crate_root(root, package)?;
                let dir = lib
                    .parent()
                    .expect("a library file has a folder")
                    .to_owned();
                walk_module_variants(package.to_owned(), lib, dir, false, false, parse)
            });
            let path = if module.is_empty() {
                package.to_owned()
            } else {
                format!("{package}::{module}")
            };
            let mut variants = Vec::new();
            for variant in files
                .as_ref()
                .map_err(Clone::clone)?
                .iter()
                .filter(|v| v.file.module == path)
            {
                let path = &variant.file.path;
                let text = fs::read_to_string(path)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                let mut items = parse(path, &text)?.items;
                for &index in &variant.inline {
                    // The walker recorded these indices in the same parsed file.
                    let Item::Mod(declaration) = items.swap_remove(index) else {
                        unreachable!()
                    };
                    items = declaration.content.expect("recorded inline module").1;
                }
                variants.push((variant.file.cfg.clone(), items));
            }
            self.modules.insert(key.clone(), variants);
        }
        let variants = &self.modules[&key];
        // Resolve aliases and globs only if there isn't a declared module at this path.
        if !variants.iter().any(|(cfg, _)| build.permits(cfg)) {
            return Ok(None);
        }
        Ok(Some(
            variants
                .iter()
                .filter(|(cfg, _)| build.permits(cfg))
                .flat_map(|(_, items)| items),
        ))
    }
}
