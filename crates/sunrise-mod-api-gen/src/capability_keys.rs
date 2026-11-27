//! Provider roles and built-in capability metadata derived from the public keys.
use quote::ToTokens;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use syn::Item;

/// A capability key's shape: what its providers are registered by, its handler trait as
/// written (`ResourceHandler<ItemResource>`), and an entity key's lookup context.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct KeyShape {
    pub(crate) by: &'static str,
    pub(crate) handler: String,
    pub(crate) context: String,
}

fn is_const_name(name: &str) -> bool {
    name.chars().any(|c| c.is_ascii_uppercase()) && !name.chars().any(|c| c.is_ascii_lowercase())
}

fn collect_use_targets(
    prefix: &mut Vec<String>,
    tree: &syn::UseTree,
    out: &mut Vec<(Vec<String>, String, String)>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_targets(prefix, &path.tree, out);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let ident = name.ident.to_string();
            if ident != "self" {
                out.push((prefix.clone(), ident.clone(), ident));
            }
        }
        syn::UseTree::Rename(rename) => {
            let orig = rename.ident.to_string();
            if orig != "self" {
                out.push((prefix.clone(), orig, rename.rename.to_string()));
            }
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_targets(prefix, item, out);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn resolve_module_file(
    src_root: &Path,
    current_mod: &[String],
    segments: &[String],
) -> Option<PathBuf> {
    let mut resolved: Vec<String> = match segments.first().map(String::as_str) {
        Some("crate") => Vec::new(),
        Some("self") => current_mod.to_vec(),
        Some("super") => {
            let mut base = current_mod.to_vec();
            let mut rest = segments;
            while rest.first().map(String::as_str) == Some("super") {
                base.pop()?;
                rest = &rest[1..];
            }
            base.extend(rest.iter().cloned());
            return file_for_module(src_root, &base);
        }
        Some(_) if current_mod.is_empty() => Vec::new(),
        Some(_) => current_mod.to_vec(),
        None => return None,
    };
    let start = usize::from(matches!(
        segments.first().map(String::as_str),
        Some("crate" | "self")
    ));
    resolved.extend(segments[start..].iter().cloned());
    file_for_module(src_root, &resolved)
}

fn file_for_module(src_root: &Path, module: &[String]) -> Option<PathBuf> {
    if module.is_empty() {
        return None;
    }
    let mut base = src_root.to_path_buf();
    for seg in module {
        base.push(seg);
    }
    let file = base.with_extension("rs");
    if file.is_file() {
        return Some(file);
    }
    let dir_mod = base.join("mod.rs");
    if dir_mod.is_file() {
        return Some(dir_mod);
    }
    None
}

fn module_segments_for(src_root: &Path, path: &Path) -> Vec<String> {
    let Ok(rel) = path.strip_prefix(src_root) else {
        return Vec::new();
    };
    let mut segs: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if let Some(last) = segs.last_mut() {
        if last == "mod.rs" || last == "lib.rs" {
            segs.pop();
        } else if let Some(stem) = last.strip_suffix(".rs") {
            *last = stem.to_owned();
        }
    }
    segs
}

fn const_key_shape(item: &syn::ItemConst) -> Result<Option<KeyShape>, String> {
    let syn::Type::Path(path) = &*item.ty else {
        return Ok(None);
    };
    let Some(last) = path.path.segments.last() else {
        return Ok(None);
    };
    let by = match last.ident.to_string().as_str() {
        "CapabilityKey" => "Block",
        "ItemCapabilityKey" => "Item",
        "EntityCapabilityKey" => "Entity",
        _ => return Ok(None),
    };
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return Ok(None);
    };
    let mut args = args.args.iter();
    let Some(syn::GenericArgument::Type(syn::Type::TraitObject(object))) = args.next() else {
        return Ok(None);
    };
    let context = match args.next() {
        Some(syn::GenericArgument::Type(ty)) => written(ty),
        _ => "()".to_string(),
    };
    Ok(Some(KeyShape {
        by,
        handler: trait_name(object)?,
        context,
    }))
}

fn resolve_const_shape(
    src_root: &Path,
    file_path: &Path,
    name: &str,
    cache: &mut BTreeMap<PathBuf, syn::File>,
    visited: &mut BTreeSet<(PathBuf, String)>,
) -> Result<Option<KeyShape>, String> {
    if !visited.insert((file_path.to_path_buf(), name.to_owned())) {
        return Ok(None);
    }
    if !cache.contains_key(file_path) {
        let source =
            crate::read_input(file_path).map_err(|e| format!("{}: {e}", file_path.display()))?;
        let parsed =
            syn::parse_file(&source).map_err(|e| format!("{}: {e}", file_path.display()))?;
        cache.insert(file_path.to_path_buf(), parsed);
    }
    let parsed = cache.get(file_path).unwrap().clone();
    for item in &parsed.items {
        if let Item::Const(c) = item
            && matches!(c.vis, syn::Visibility::Public(_))
            && c.ident == name
        {
            return const_key_shape(c);
        }
    }
    let current_mod = module_segments_for(src_root, file_path);
    let mut reexports = Vec::new();
    for item in &parsed.items {
        if let Item::Use(u) = item
            && matches!(u.vis, syn::Visibility::Public(_))
        {
            let mut prefix = Vec::new();
            collect_use_targets(&mut prefix, &u.tree, &mut reexports);
        }
    }
    for (segments, orig_name, exported_name) in reexports {
        if exported_name != name {
            continue;
        }
        if let Some(target) = resolve_module_file(src_root, &current_mod, &segments)
            && let Some(shape) = resolve_const_shape(src_root, &target, &orig_name, cache, visited)?
        {
            return Ok(Some(shape));
        }
    }
    Ok(None)
}

/// Every exported capability declaration, resolved directly from local constant `pub use`s
/// in `mod_api_exports_root.rs` without indexing every root export.
fn key_declarations_from_exports(
    root: &Path,
    exports_root: &str,
) -> Result<(Vec<(String, KeyShape)>, Vec<PathBuf>), String> {
    let src_root = root.join("crates/sunrise-mod-vocab/src");
    let parsed = syn::parse_file(exports_root).map_err(|e| e.to_string())?;
    let mut targets = Vec::new();
    for item in &parsed.items {
        if let Item::Use(item) = item
            && matches!(item.vis, syn::Visibility::Public(_))
        {
            let mut prefix = Vec::new();
            collect_use_targets(&mut prefix, &item.tree, &mut targets);
        }
    }
    let mut cache = BTreeMap::new();
    let mut declarations = Vec::new();
    for (segments, orig_name, exported_name) in targets {
        if !is_const_name(&orig_name) {
            continue;
        }
        let Some(file_path) = resolve_module_file(&src_root, &[], &segments) else {
            continue;
        };
        let mut visited = BTreeSet::new();
        if let Some(shape) =
            resolve_const_shape(&src_root, &file_path, &orig_name, &mut cache, &mut visited)?
        {
            declarations.push((exported_name, shape));
        }
    }
    Ok((declarations, cache.into_keys().collect()))
}

/// Distinct provider shapes, in the order their keys are exported.
pub(crate) fn key_shapes(root: &Path) -> Result<Vec<KeyShape>, String> {
    let exports = crate::exports::sources(root)?;
    let exports_root = exports
        .get("mod_api_exports_root.rs")
        .map(String::as_str)
        .unwrap_or("");
    let (declarations, _) = key_declarations_from_exports(root, exports_root)?;
    let mut shapes = Vec::new();
    for (_, shape) in declarations {
        if !shapes.contains(&shape) {
            shapes.push(shape);
        }
    }
    Ok(shapes)
}

impl KeyShape {
    /// Its provider trait, as an owner (`BlockCapabilityProvider<dyn ResourceHandler<ItemResource>>`).
    pub(crate) fn provider(&self) -> String {
        match self.by {
            "Entity" => format!(
                "EntityCapabilityProvider<dyn {}, {}>",
                self.handler, self.context
            ),
            by => format!("{by}CapabilityProvider<dyn {}>", self.handler),
        }
    }
}

/// A trait object's trait as the generator names it: its last path segment, with its type
/// arguments as written (`ResourceHandler<ItemResource>`).
fn trait_name(object: &syn::TypeTraitObject) -> Result<String, String> {
    object
        .bounds
        .iter()
        .find_map(|bound| match bound {
            syn::TypeParamBound::Trait(bound) => bound.path.segments.last().map(|s| {
                let args = match &s.arguments {
                    syn::PathArguments::AngleBracketed(args) => format!(
                        "<{}>",
                        args.args.iter().map(written).collect::<Vec<_>>().join(", ")
                    ),
                    _ => String::new(),
                };
                format!("{}{args}", s.ident)
            }),
            _ => None,
        })
        .ok_or_else(|| format!("dyn {}", object.to_token_stream()))
}

/// Tokens as written, without the spaces `quote` puts around punctuation
/// (`Option<Direction>`, `dyn ResourceHandler<ItemResource>`).
fn written(tokens: &impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .replace(" < ", "<")
        .replace("< ", "<")
        .replace(" <", "<")
        .replace(" >", ">")
        .replace(" ,", ",")
        .replace("( )", "()")
}

pub(crate) const OUTPUT: &str = "builtin_capability_keys.rs";

fn render_keys(declarations: &[(String, KeyShape)]) -> String {
    let entries = declarations
        .iter()
        .map(|(name, shape)| {
            format!(
                "(ProvidedBy::{}, crate::{name}.name, crate::{name}.binding()),",
                shape.by
            )
        })
        .collect::<String>();
    format!(
        "// @generated by sunrise-mod-api-gen from mod_api's capability keys; do not edit.\n\
         pub(crate) fn builtin_capabilities() -> impl Iterator<Item = (ProvidedBy, &'static str, (TypeId, &'static str))> {{\n\
         [{entries}].into_iter()\n}}\n"
    )
}

/// The built-in capability metadata, reusing the already-assembled root export fragment.
pub(crate) fn generate_from_exports(root: &Path, exports_root: &str) -> Result<String, String> {
    let (declarations, _) = key_declarations_from_exports(root, exports_root)?;
    Ok(render_keys(&declarations))
}

/// The built-in capability metadata derived from the public capability keys.
pub(crate) fn generate(root: &Path) -> Result<String, String> {
    let exports = crate::exports::sources(root)?;
    let exports_root = exports
        .get("mod_api_exports_root.rs")
        .map(String::as_str)
        .unwrap_or("");
    generate_from_exports(root, exports_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scan_all_capability_consts(dir: &Path, out: &mut BTreeMap<String, KeyShape>) {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                scan_all_capability_consts(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let source = fs::read_to_string(&path).unwrap();
                let parsed = syn::parse_file(&source).unwrap();
                for item in &parsed.items {
                    if let Item::Const(c) = item
                        && matches!(c.vis, syn::Visibility::Public(_))
                        && let Some(shape) = const_key_shape(c).unwrap()
                    {
                        let prev = out.insert(c.ident.to_string(), shape);
                        assert!(
                            prev.is_none(),
                            "duplicate public capability key const {}",
                            c.ident
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn narrowed_capability_key_pass_matches_exhaustive_vocab_scan() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let exports = crate::exports::sources(&root).unwrap();
        let (narrowed, read_paths) =
            key_declarations_from_exports(&root, &exports["mod_api_exports_root.rs"]).unwrap();
        let narrowed_map: BTreeMap<_, _> = narrowed.into_iter().collect();
        let mut exhaustive = BTreeMap::new();
        scan_all_capability_consts(&root.join("crates/sunrise-mod-vocab/src"), &mut exhaustive);
        assert_eq!(
            narrowed_map, exhaustive,
            "narrowed capability_keys pass disagreed with exhaustive vocab scan"
        );
        assert!(!narrowed_map.is_empty());
        let camera = root.join("crates/sunrise-mod-vocab/src/client/camera.rs");
        assert!(
            !read_paths.contains(&camera),
            "non-constant file {} should not be opened by capability_keys",
            camera.display()
        );
    }
}
