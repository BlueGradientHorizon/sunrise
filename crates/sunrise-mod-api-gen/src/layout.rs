//! Native hook placement discovered from engine modules, used by source tooling.
use crate::catalog::{Catalog, DECLARATIONS};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};
use syn::{Item, spanned::Spanned};

fn normalize_area_stem(stem: &str) -> &str {
    stem.strip_suffix("_server")
        .or_else(|| stem.strip_suffix("_client"))
        .unwrap_or(stem)
}

/// Return catalog areas and lossless item line ranges for the source splitter.
pub fn native_hook_layout(root: &Path, source: &str) -> Result<Value, String> {
    let mut engine_areas = BTreeMap::new();
    for path in sunrise_game_discovery::native_hook_files(root)? {
        let raw_area = path.file_stem().unwrap().to_string_lossy().into_owned();
        let area = normalize_area_stem(&raw_area).to_owned();
        let file = syn::parse_file(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        for item in file.items {
            match item {
                Item::Impl(item) => {
                    for method in item.items {
                        if let syn::ImplItem::Fn(method) = method {
                            let name = method.sig.ident.to_string();
                            if name.starts_with("native_")
                                && let Some(previous) =
                                    engine_areas.insert(name.clone(), area.clone())
                                && previous != area
                            {
                                return Err(format!("{name}: found in both {previous} and {area}"));
                            }
                        }
                    }
                }
                Item::Fn(func) => {
                    let name = func.sig.ident.to_string();
                    if name.starts_with("native_")
                        && let Some(previous) = engine_areas.insert(name.clone(), area.clone())
                        && previous != area
                    {
                        return Err(format!("{name}: found in both {previous} and {area}"));
                    }
                }
                _ => {}
            }
        }
    }
    let mut areas = BTreeMap::new();
    for area in Catalog::modules(&root.join(DECLARATIONS))? {
        let path = root.join(DECLARATIONS).join(format!("{area}.rs"));
        let catalog = Catalog::parse(&[(
            path.display().to_string(),
            fs::read_to_string(&path).map_err(|e| e.to_string())?,
        )])?;
        for target in catalog.hooks.iter().map(|h| &h.target) {
            if target.starts_with("sunrise_client::game::native_hooks::") {
                let name = target.rsplit("::").next().unwrap().to_owned();
                let engine_area = engine_areas.get(&name).ok_or_else(|| {
                    format!("{name}: catalog target is missing from engine modules")
                })?;
                areas.insert(name, engine_area.clone());
            }
        }
    }
    let file = syn::parse_file(source).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for item in file.items {
        let span = item.span();
        if let Item::Impl(item) = &item {
            let methods = item
                .items
                .iter()
                .map(|method| {
                    let name = match method {
                        syn::ImplItem::Fn(method) => method.sig.ident.to_string(),
                        _ => String::new(),
                    };
                    let span = method.span();
                    json!({"name": name, "start": span.start().line, "end": span.end().line})
                })
                .collect::<Vec<_>>();
            items.push(
                json!({"start": span.start().line, "end": span.end().line, "methods": methods}),
            );
        } else {
            items.push(json!({"start": span.start().line, "end": span.end().line}));
        }
    }
    Ok(json!({"areas": areas, "items": items}))
}

#[cfg(test)]
mod tests {
    use crate::{SourceLayout, catalog::BindingKind, gameplay::Routing};
    use std::{collections::BTreeMap, fs, path::Path};
    use syn::{ImplItem, Item, TraitItem, Type};

    #[test]
    fn native_hooks_live_in_their_catalog_area() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let catalog = Routing::load(&root).unwrap().catalog(&root).unwrap();
        let mut modules = BTreeMap::<_, Vec<Item>>::new();
        for file in SourceLayout::read(&root)
            .unwrap()
            .files(&root, false)
            .unwrap()
        {
            if file.module.contains("::native_hooks") {
                let source = fs::read_to_string(&file.path).unwrap();
                modules
                    .entry(file.module)
                    .or_default()
                    .extend(syn::parse_file(&source).unwrap().items);
            }
        }
        let mut checked = 0;
        for hook in catalog.hooks {
            let Some(binding) = hook.gameplay_binding().unwrap() else {
                continue;
            };
            checked += 1;
            let module = format!("{}::{}", binding.package.domain().key, binding.module);
            let items = modules
                .get(&module)
                .unwrap_or_else(|| panic!("missing module {module}"));
            assert!(
                items.iter().any(|item| match (item, &binding.kind) {
                    (Item::Fn(function), BindingKind::Free | BindingKind::Borrowed(_)) => {
                        function.sig.ident == binding.function
                    }
                    (Item::Trait(item), BindingKind::Shared) => {
                        item.ident == "GameHooks" && item.items.iter().any(|member| {
                            matches!(member, TraitItem::Fn(method) if method.sig.ident == binding.function && method.default.is_some())
                        })
                    }
                    (Item::Impl(item), BindingKind::Nominal | BindingKind::Scoped(_)) => {
                        let owner = match &binding.kind {
                            BindingKind::Scoped(owner) => owner.ident.to_string(),
                            _ => binding.package.game_type().to_owned(),
                        };
                        item.trait_.is_none()
                            && matches!(&*item.self_ty, Type::Path(ty) if ty.path.segments.last().is_some_and(|segment| segment.ident == owner))
                            && item.items.iter().any(|member| {
                                matches!(member, ImplItem::Fn(method) if method.sig.ident == binding.function)
                            })
                    }
                    _ => false,
                }),
                "missing catalog target {}",
                hook.target,
            );
        }
        assert!(checked > 0);
    }
}
