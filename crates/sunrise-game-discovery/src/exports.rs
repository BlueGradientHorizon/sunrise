//! The source-only module fragments of the API vocabulary, shared with its generator.
use quote::ToTokens;
use std::{collections::BTreeMap, fs, path::Path};
use syn::{Item, parse::Parser};

/// Generated export fragment filenames share this prefix.
pub const PREFIX: &str = "mod_api_exports_";

/// Parse a literal `concat!(env!(var), "...")` include token stream, substituting `replacement` for `env!(var)`.
/// Returns `Ok(None)` if the tokens do not match that exact shape.
pub fn env_concat_include(
    tokens: proc_macro2::TokenStream,
    var: &str,
    replacement: &str,
) -> Result<Option<String>, String> {
    let Ok(syn::Expr::Macro(expression)) = syn::parse2::<syn::Expr>(tokens) else {
        return Ok(None);
    };
    if !expression.mac.path.is_ident("concat") {
        return Ok(None);
    }
    let arguments = <syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>>::parse_terminated
        .parse2(expression.mac.tokens)
        .map_err(|e| e.to_string())?;
    let mut path = String::new();
    let mut saw_var = false;
    for argument in arguments {
        match argument {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) => path.push_str(&value.value()),
            syn::Expr::Macro(env_macro)
                if env_macro.mac.path.is_ident("env")
                    && syn::parse2::<syn::LitStr>(env_macro.mac.tokens.clone())
                        .is_ok_and(|name| name.value() == var) =>
            {
                saw_var = true;
                path.push_str(replacement);
            }
            _ => return Ok(None),
        }
    }
    Ok(saw_var.then_some(path))
}

fn collect(
    root: &Path,
    scope: &str,
    items: Vec<Item>,
    out: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    for mut item in items {
        if let Item::Mod(module) = &mut item {
            if let Some((_, items)) = module.content.take() {
                let next = match (scope, module.ident.to_string().as_str()) {
                    ("root", "__private") => "private".to_owned(),
                    ("private", name @ ("built" | "game" | "reload")) => format!("private_{name}"),
                    _ => {
                        return Err(format!(
                            "unsupported export scope {scope}::{}",
                            module.ident
                        ));
                    }
                };
                if !module.attrs.is_empty() {
                    return Err("export scope containers must not carry attributes".into());
                }
                collect(root, &next, items, out)?;
                continue;
            }
            if scope != "root" {
                return Err("out-of-line export modules belong at the API root".into());
            }
            let path = root
                .join("crates/sunrise-mod-vocab/src")
                .join(format!("{}.rs", module.ident));
            let path = if path.is_file() {
                path
            } else {
                path.with_extension("").join("mod.rs")
            };
            let path = path.to_string_lossy();
            module.attrs.push(syn::parse_quote!(#[path = #path]));
        } else if !matches!(item, Item::Use(_)) {
            return Err("export fragments contain only uses and module declarations".into());
        }
        let code = out.entry(scope.to_owned()).or_default();
        code.push_str(&item.to_token_stream().to_string());
        code.push('\n');
    }
    Ok(())
}

/// Collect lexical scope fragments and whether the real generator must append Parts/Host exports.
/// `areas` preserves the caller's catalog ordering; source inventories use the shared sorted enumerator.
pub fn fragments(
    root: &Path,
    areas: Vec<String>,
) -> Result<(BTreeMap<String, String>, bool, bool), String> {
    let directory = root.join("crates/sunrise-mod-vocab/src/exports");
    let mut out = [
        "root",
        "private",
        "private_built",
        "private_game",
        "private_reload",
    ]
    .map(|scope| (scope.to_owned(), String::new()))
    .into_iter()
    .collect();
    let mut has_parts = false;
    let mut has_host = false;
    for area in areas {
        let path = directory.join(format!("{area}.rs"));
        let source = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file = syn::parse_file(&source).map_err(|e| format!("{}: {e}", path.display()))?;
        has_host |= file.items.iter().any(|item| matches!(item, Item::Mod(module) if module.ident == "host" && module.content.is_none()));
        has_parts |= file.items.iter().any(|item| matches!(item, Item::Mod(module) if module.ident == "parts" && module.content.is_none()));
        collect(root, "root", file.items, &mut out)?;
    }
    Ok((out, has_parts, has_host))
}
