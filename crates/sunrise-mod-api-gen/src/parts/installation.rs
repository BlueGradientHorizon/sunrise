//! Owned authoring inputs generated from the private lookup storage graph.
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use syn::{Item, ItemStruct, ItemType, Type, Visibility, visit::Visit, visit_mut::VisitMut};

#[derive(Default)]
struct Schema {
    records: BTreeMap<String, (ItemStruct, syn::Path)>,
    aliases: BTreeMap<String, ItemType>,
    owners: BTreeMap<String, String>,
    imports: BTreeMap<String, BTreeMap<String, String>>,
    loaded: BTreeSet<String>,
    selected: BTreeSet<String>,
    duplicates: BTreeSet<String>,
}

fn derived(record: &str, field: &str) -> bool {
    matches!(
        (record, field),
        ("Parts", "hooks")
            | ("Attached", "panicked")
            | (
                "Installed",
                "epoch"
                    | "namespace_authority"
                    | "hook_bits"
                    | "tag_hook_bits"
                    | "resolved_menus"
                    | "entity_type_defaults"
            )
    )
}

// The generator has syntax but no trait resolver. Recognize the Copy types this
// storage schema uses, including compositions of those types.
fn is_copy(ty: &Type) -> bool {
    match ty {
        Type::Reference(reference) => reference.mutability.is_none(),
        Type::Tuple(tuple) => tuple.elems.iter().all(is_copy),
        Type::Path(path) if path.qself.is_none() => {
            let segments = &path.path.segments;
            let last = segments.last().unwrap();
            match segments.len() {
                1 if last.ident == "Option" => {
                    matches!(&last.arguments, syn::PathArguments::AngleBracketed(args)
                        if args.args.len() == 1
                            && matches!(args.args.first(), Some(syn::GenericArgument::Type(inner)) if is_copy(inner)))
                }
                1 => matches!(
                    last.ident.to_string().as_str(),
                    "bool"
                        | "char"
                        | "usize"
                        | "isize"
                        | "u8"
                        | "u16"
                        | "u32"
                        | "u64"
                        | "u128"
                        | "i8"
                        | "i16"
                        | "i32"
                        | "i64"
                        | "i128"
                        | "f32"
                        | "f64"
                        | "HandlerThread"
                        | "PayloadProtocol"
                        | "PayloadDirection"
                        | "VanillaStages"
                ),
                3 => {
                    segments[0].ident == "std"
                        && segments[1].ident == "any"
                        && last.ident == "TypeId"
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn resolve_relative_module(owner: &str, segments: &[String]) -> Option<String> {
    let mut base: Vec<&str> = owner.split("::").collect();
    match segments.first().map(String::as_str) {
        Some("crate") => {
            let mut out = vec!["crate".to_owned()];
            out.extend(segments[1..].iter().cloned());
            Some(out.join("::"))
        }
        Some("self") => {
            let mut out: Vec<String> = base.into_iter().map(str::to_owned).collect();
            out.extend(segments[1..].iter().cloned());
            Some(out.join("::"))
        }
        Some("super") => {
            let mut rest = segments;
            while rest.first().map(String::as_str) == Some("super") {
                if base.len() <= 1 {
                    return None;
                }
                base.pop();
                rest = &rest[1..];
            }
            let mut out: Vec<String> = base.into_iter().map(str::to_owned).collect();
            out.extend(rest.iter().cloned());
            Some(out.join("::"))
        }
        _ => None,
    }
}

fn module_file_path(src_root: &Path, module: &str) -> Option<PathBuf> {
    let rest = module.strip_prefix("crate::")?;
    let mut base = src_root.to_path_buf();
    for seg in rest.split("::") {
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

fn collect_use_imports(
    owner: &str,
    prefix: &mut Vec<String>,
    tree: &syn::UseTree,
    out: &mut BTreeMap<String, String>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_imports(owner, prefix, &path.tree, out);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let ident = name.ident.to_string();
            if ident != "self"
                && let Some(module) = resolve_relative_module(owner, prefix)
            {
                out.insert(ident, module);
            }
        }
        syn::UseTree::Rename(rename) => {
            if rename.ident != "self"
                && let Some(module) = resolve_relative_module(owner, prefix)
            {
                out.insert(rename.rename.to_string(), module);
            }
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_imports(owner, prefix, item, out);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn child_mod_path(parent_file: &Path, module: &syn::ItemMod) -> Option<PathBuf> {
    for attr in &module.attrs {
        if !attr.path().is_ident("path") {
            continue;
        }
        if let syn::Meta::NameValue(value) = &attr.meta
            && let syn::Expr::Lit(value) = &value.value
            && let syn::Lit::Str(rel) = &value.lit
        {
            let candidate = parent_file.parent()?.join(rel.value());
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

impl Schema {
    fn ingest(
        &mut self,
        src_root: &Path,
        path: &Path,
        owner: &str,
        parsed: &syn::File,
        follow_child_mods: bool,
    ) -> Result<(), String> {
        if !self.loaded.insert(owner.to_owned()) {
            return Ok(());
        }
        let mut uses = BTreeMap::new();
        let mut child_mods = Vec::new();
        for item in &parsed.items {
            match item {
                Item::Struct(item) => {
                    let name = item.ident.to_string();
                    // Installed is the root Parts epoch, not another domain's
                    // unrelated installation wrapper with the same local name.
                    if name == "Installed"
                        && !matches!(
                            owner,
                            "crate::parts::shared" | "crate::parts::shared::registration_state"
                        )
                    {
                        continue;
                    }
                    // Only private storage, plus these supported opaque lookup owners,
                    // needs a mirror. Public semantic API values pass through unchanged.
                    if !matches!(item.vis, Visibility::Public(_))
                        || matches!(
                            name.as_str(),
                            "Attached" | "Parts" | "TooltipAppenderPart" | "Payloads"
                        )
                    {
                        let target_name = if name == "Installed" {
                            "crate::parts::shared::Installed".to_string()
                        } else {
                            format!("{owner}::{name}")
                        };
                        let target = syn::parse_str(&target_name).map_err(|e| e.to_string())?;
                        self.owners.insert(name.clone(), owner.to_owned());
                        if self
                            .records
                            .insert(name.clone(), (item.clone(), target))
                            .is_some()
                        {
                            self.duplicates.insert(name);
                        }
                    }
                }
                Item::Type(item) if !matches!(item.vis, Visibility::Public(_)) => {
                    self.owners.insert(item.ident.to_string(), owner.to_owned());
                    self.aliases.insert(item.ident.to_string(), item.clone());
                }
                Item::Use(item) => {
                    let mut prefix = Vec::new();
                    collect_use_imports(owner, &mut prefix, &item.tree, &mut uses);
                }
                Item::Mod(module) if follow_child_mods && module.content.is_none() => {
                    if let Some(child_path) = child_mod_path(path, module) {
                        child_mods.push((child_path, format!("{owner}::{}", module.ident)));
                    }
                }
                _ => {}
            }
        }
        if !uses.is_empty() {
            self.imports.insert(owner.to_owned(), uses);
        }
        for (child_path, child_owner) in child_mods {
            self.load_file(src_root, &child_path, &child_owner, true)?;
        }
        Ok(())
    }

    fn load_file(
        &mut self,
        src_root: &Path,
        path: &Path,
        owner: &str,
        follow_child_mods: bool,
    ) -> Result<(), String> {
        if self.loaded.contains(owner) {
            return Ok(());
        }
        let source = crate::read_input(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let parsed = syn::parse_file(&source).map_err(|e| format!("{}: {e}", path.display()))?;
        self.ingest(src_root, path, owner, &parsed, follow_child_mods)
    }

    fn load_module(&mut self, src_root: &Path, module: &str) -> Result<(), String> {
        if self.loaded.contains(module) {
            return Ok(());
        }
        let Some(path) = module_file_path(src_root, module) else {
            return Ok(());
        };
        self.load_file(src_root, &path, module, false)
    }

    fn seed(
        &mut self,
        src_root: &Path,
        parsed_parts: &[(PathBuf, String, syn::File)],
    ) -> Result<(), String> {
        let parts_mod = src_root.join("parts/mod.rs");
        if parts_mod.is_file() {
            self.load_file(src_root, &parts_mod, "crate::parts", false)?;
        }
        if !parsed_parts.is_empty() {
            for (path, owner, file) in parsed_parts {
                self.ingest(src_root, path, owner, file, true)?;
            }
        } else {
            let parts_dir = src_root.join("parts");
            if parts_dir.is_dir() {
                for kind in crate::read_module_files(&parts_dir)? {
                    let path = parts_dir.join(format!("{kind}.rs"));
                    self.load_file(src_root, &path, &format!("crate::parts::{kind}"), true)?;
                }
            }
        }
        Ok(())
    }

    fn resolve_imported_module(&self, owner: &str, ident: &str) -> Option<String> {
        let mut cur = owner;
        loop {
            if let Some(module) = self.imports.get(cur).and_then(|map| map.get(ident)) {
                return Some(module.clone());
            }
            let (parent, _) = cur.rsplit_once("::")?;
            cur = parent;
        }
    }

    fn select(&mut self, src_root: &Path, name: &str) -> Result<(), String> {
        if !self.selected.insert(name.to_string()) {
            return Ok(());
        }
        let owner = self.owners.get(name).cloned().unwrap_or_default();
        let mut refs = Vec::new();
        if let Some((record, _)) = self.records.get(name) {
            for field in &record.fields {
                if field
                    .ident
                    .as_ref()
                    .is_some_and(|id| derived(name, &id.to_string()))
                {
                    continue;
                }
                TypeRefs(&mut refs).visit_type(&field.ty);
            }
        } else if let Some(alias) = self.aliases.get(name) {
            TypeRefs(&mut refs).visit_type(&alias.ty);
        }
        let mut dependencies = BTreeSet::new();
        for segments in refs {
            let ident = segments.last().unwrap().clone();
            if segments.len() >= 2 {
                if let Some(module) =
                    resolve_relative_module(&owner, &segments[..segments.len() - 1])
                {
                    self.load_module(src_root, &module)?;
                }
            } else if let Some(module) = self.resolve_imported_module(&owner, &ident) {
                self.load_module(src_root, &module)?;
            }
            dependencies.insert(ident);
        }
        for dependency in dependencies {
            if self.records.contains_key(&dependency) || self.aliases.contains_key(&dependency) {
                self.select(src_root, &dependency)?;
            }
        }
        Ok(())
    }

    fn input_type(&self, ty: &Type) -> Type {
        let mut input = ty.clone();
        Inputs(&self.selected).visit_type_mut(&mut input);
        input
    }

    fn changed(&self, ty: &Type) -> bool {
        let mut names = BTreeSet::new();
        Names(&mut names).visit_type(ty);
        names.iter().any(|name| self.selected.contains(name))
    }

    fn convert(&self, value: TokenStream, ty: &Type) -> Result<TokenStream, String> {
        if !self.changed(ty) {
            return Ok(value);
        }
        match ty {
            Type::Path(path) => {
                let segment = path.path.segments.last().unwrap();
                let name = segment.ident.to_string();
                if self.records.contains_key(&name) && self.selected.contains(&name) {
                    return Ok(quote!((#value).into()));
                }
                let args = match &segment.arguments {
                    syn::PathArguments::AngleBracketed(args) => args
                        .args
                        .iter()
                        .filter_map(|arg| {
                            if let syn::GenericArgument::Type(ty) = arg {
                                Some(ty.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                if let Some(alias) = self.aliases.get(&name) {
                    let replacements = alias
                        .generics
                        .type_params()
                        .map(|p| p.ident.to_string())
                        .zip(args)
                        .collect();
                    let mut expanded = (*alias.ty).clone();
                    Substitute(&replacements).visit_type_mut(&mut expanded);
                    return self.convert(value, &expanded);
                }
                match (name.as_str(), args.as_slice()) {
                    ("Vec", [element]) => {
                        let element = self.convert(quote!(element), element)?;
                        Ok(quote!((#value).into_iter().map(|element| #element).collect()))
                    }
                    ("HashMap" | "BTreeMap", [key, element]) => {
                        let key = self.convert(quote!(key), key)?;
                        let element = self.convert(quote!(element), element)?;
                        Ok(
                            quote!((#value).into_iter().map(|(key, element)| (#key, #element)).collect()),
                        )
                    }
                    ("Option", [element]) => {
                        let element = self.convert(quote!(element), element)?;
                        Ok(quote!((#value).map(|element| #element)))
                    }
                    ("Arc", [element]) => {
                        let element =
                            self.convert(quote!(std::sync::Arc::unwrap_or_clone(#value)), element)?;
                        Ok(quote!(std::sync::Arc::new(#element)))
                    }
                    ("Box", [element]) => {
                        let element = self.convert(quote!(*#value), element)?;
                        Ok(quote!(Box::new(#element)))
                    }
                    _ => Err(format!("unsupported owned storage container {name}")),
                }
            }
            Type::Array(array) => {
                let element = self.convert(quote!(element), &array.elem)?;
                Ok(quote!((#value).map(|element| #element)))
            }
            Type::Tuple(tuple) => {
                let converted = tuple
                    .elems
                    .iter()
                    .enumerate()
                    .map(|(index, ty)| {
                        let index = syn::Index::from(index);
                        self.convert(quote!((#value).#index), ty)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(quote!((#(#converted,)*)))
            }
            _ => Err("unsupported private storage field shape".into()),
        }
    }

    fn emit(&self) -> Result<TokenStream, String> {
        let mut output = TokenStream::new();
        for name in &self.selected {
            let input_name = format_ident!("{name}Input");
            if let Some(alias) = self.aliases.get(name) {
                let mut input = alias.clone();
                input.ident = input_name;
                input.vis = syn::parse_quote!(pub);
                *input.ty = self.input_type(&alias.ty);
                input.attrs.clear();
                output.extend(quote!(#[doc(hidden)] #input));
                continue;
            }
            let (record, target) = &self.records[name];
            let generics = &record.generics;
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            let mut fields = Vec::new();
            let mut conversions = Vec::new();
            let mut clones = Vec::new();
            let mut reattached = Vec::new();
            let mut reverse = Vec::new();
            for field in &record.fields {
                let field_name = field
                    .ident
                    .as_ref()
                    .ok_or("lookup records must have named fields")?;
                if derived(name, &field_name.to_string()) {
                    conversions.push(quote!(#field_name: Default::default()));
                    continue;
                }
                let ty = self.input_type(&field.ty);
                let attrs = &field.attrs;
                fields.push(quote!(#(#attrs)* pub #field_name: #ty));
                let converted = self.convert(quote!(input.#field_name), &field.ty)?;
                conversions.push(quote!(#field_name: #converted));
                let copied = is_copy(&field.ty);
                reverse.push(if copied {
                    quote!(#field_name: input.#field_name)
                } else {
                    quote!(#field_name: input.#field_name.clone())
                });
                reattached.push(if field_name == "part" {
                    quote!(part)
                } else if copied {
                    quote!(#field_name: self.#field_name)
                } else {
                    quote!(#field_name: self.#field_name.clone())
                });
                clones.push(if copied {
                    quote!(#field_name: self.#field_name)
                } else {
                    quote!(#field_name: self.#field_name.clone())
                });
            }
            let mut derives = Vec::new();
            for attr in &record.attrs {
                if attr.path().is_ident("derive") {
                    let paths = attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
                        .map_err(|e| e.to_string())?;
                    derives.extend(
                        paths
                            .into_iter()
                            .filter(|path| path.is_ident("Clone") || path.is_ident("Default")),
                    );
                }
            }
            let derive = if derives.is_empty() {
                quote!()
            } else {
                quote!(#[derive(#(#derives),*)])
            };
            output.extend(quote! {
                #[doc = "No Wire: owned native registration metadata contains callbacks and mod identities."]
                #[doc(hidden)] #derive
                pub struct #input_name #generics #where_clause { #(#fields,)* }
                impl #impl_generics From<#input_name #ty_generics> for #target #ty_generics #where_clause {
                    fn from(input: #input_name #ty_generics) -> Self { Self { #(#conversions,)* } }
                }
            });
            // Narrow testing read views reuse these leaf conversions without exposing Installed.
            if !matches!(name.as_str(), "Parts" | "TagParts" | "Installed")
                && record.fields.iter().all(|field| !self.changed(&field.ty))
            {
                let mut reverse_generics = generics.clone();
                for field in &record.fields {
                    let ty = &field.ty;
                    reverse_generics
                        .make_where_clause()
                        .predicates
                        .push(syn::parse_quote!(#ty: Clone));
                }
                let (reverse_impl, _, reverse_where) = reverse_generics.split_for_impl();
                output.extend(quote! {
                    impl #reverse_impl From<&#target #ty_generics> for #input_name #ty_generics #reverse_where {
                        fn from(input: &#target #ty_generics) -> Self { Self { #(#reverse,)* } }
                    }
                });
            }
            // Deriving Clone would incorrectly impose P: Clone on Arc<P>.
            if name == "Attached" {
                output.extend(quote! {
                    impl #impl_generics #input_name #ty_generics #where_clause {
                        /// Retain ownership metadata when expanding one Part into another view.
                        pub fn with_part<Q: ?Sized>(&self, part: Arc<Q>) -> AttachedInput<Q> {
                            AttachedInput { #(#reattached,)* }
                        }
                    }
                    impl #impl_generics Clone for #input_name #ty_generics #where_clause {
                        fn clone(&self) -> Self { Self { #(#clones,)* } }
                    }
                });
            }
        }
        Ok(output)
    }
}

fn local_path(path: &syn::TypePath) -> bool {
    path.path.segments.len() == 1
        || matches!(
            path.path
                .segments
                .first()
                .unwrap()
                .ident
                .to_string()
                .as_str(),
            "crate" | "self" | "super"
        )
}

struct TypeRefs<'a>(&'a mut Vec<Vec<String>>);
impl<'ast> Visit<'ast> for TypeRefs<'_> {
    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        if local_path(path) {
            self.0.push(
                path.path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect(),
            );
        }
        syn::visit::visit_type_path(self, path);
    }
}

struct Names<'a>(&'a mut BTreeSet<String>);
impl<'ast> Visit<'ast> for Names<'_> {
    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        if local_path(path) {
            self.0
                .insert(path.path.segments.last().unwrap().ident.to_string());
        }
        syn::visit::visit_type_path(self, path);
    }
}
struct Inputs<'a>(&'a BTreeSet<String>);
impl VisitMut for Inputs<'_> {
    fn visit_type_path_mut(&mut self, path: &mut syn::TypePath) {
        syn::visit_mut::visit_type_path_mut(self, path);
        let segment = path.path.segments.last().unwrap();
        if local_path(path) && self.0.contains(&segment.ident.to_string()) {
            let mut segment = segment.clone();
            segment.ident = format_ident!("{}Input", segment.ident);
            path.path = syn::Path {
                leading_colon: None,
                segments: [segment].into_iter().collect(),
            };
        }
    }
}
struct Substitute<'a>(&'a BTreeMap<String, Type>);
impl VisitMut for Substitute<'_> {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(path) = ty
            && path.path.segments.len() == 1
            && let Some(value) = self.0.get(&path.path.segments[0].ident.to_string())
        {
            *ty = value.clone();
        } else {
            syn::visit_mut::visit_type_mut(self, ty);
        }
    }
}

pub(super) fn source_with(
    root: &Path,
    parsed_parts: &[(PathBuf, String, syn::File)],
) -> Result<String, String> {
    let src_root = root.join("crates/sunrise-mod-vocab/src");
    let mut schema = Schema::default();
    schema.seed(&src_root, parsed_parts)?;
    if !schema.records.contains_key("Installed") {
        return Ok(String::new());
    }
    schema.select(&src_root, "Installed")?;
    if let Some(name) = schema.selected.intersection(&schema.duplicates).next() {
        return Err(format!("ambiguous private storage record {name}"));
    }
    let records = schema.emit()?;
    let directory = root.join("crates/sunrise-mod-vocab/src/parts/installation_tokens");
    let mut factories = TokenStream::new();
    if directory.exists() {
        for name in crate::read_module_files(&directory)? {
            let file_path = directory.join(format!("{name}.rs"));
            let path = file_path.to_string_lossy().into_owned();
            let module = format_ident!("token_{name}");
            let source = crate::read_input(&file_path).map_err(|e| e.to_string())?;
            let parsed = syn::parse_file(&source).map_err(|e| e.to_string())?;
            factories.extend(quote! { #[path = #path] mod #module; });
            for item in parsed.items {
                if let Item::Fn(function) = item
                    && matches!(function.vis, Visibility::Public(_))
                {
                    let name = function.sig.ident;
                    let conditions = function.attrs.iter().filter(|attr| {
                        attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr")
                    });
                    factories.extend(quote! { #(#conditions)* pub use #module::#name; });
                }
            }
        }
    }
    let aliases = schema
        .selected
        .iter()
        .map(|name| {
            let input = format_ident!("{name}Input");
            let original = format_ident!("{name}");
            quote!(#input as #original)
        })
        .collect::<Vec<_>>();
    let parsed = syn::parse2(quote! {
        /// Implementation-only owned inputs; not a supported mod construction surface.
        #[doc(hidden)]
        pub mod installation {
            use super::*;
            #[allow(unused_imports)] use crate::*;
            pub use super::registration_access::{install_input as install, parts_installed};
            #records
            #factories
        }
        /// Imports for the concrete authoring half, generated from the same storage graph.
        #[doc(hidden)]
        #[macro_export]
        macro_rules! __mod_parts_authoring_imports {
            () => {
                use $crate::*;
                use $crate::__private::parts_installation::{#(#aliases),*};
            };
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(prettyplease::unparse(&parsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    #[ignore = "daily"]
    fn a_foreign_author_constructs_nested_inputs_without_access_to_lookup_fields() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/parts-owned-inputs-{}",
            std::process::id()
        ));
        let dir = root.join("crates/sunrise-mod-vocab/src");
        fs::create_dir_all(dir.join("parts")).unwrap();
        let storage = r#"
            use std::{collections::HashMap, sync::Arc};
            pub struct Attached<P: ?Sized> { pub(crate) owner: &'static str, pub(crate) part: Arc<P> }
            pub(crate) struct Holder<P> { pub(crate) provider: P }
            pub(crate) type Rows<P> = HashMap<Box<str>, Vec<Holder<P>>>;
            #[derive(Clone)] pub(crate) struct Registered {
                pub(crate) owner: &'static str, pub(crate) owner_index: usize,
                pub(crate) part: Arc<dyn crate::Part>,
            }
            #[derive(Clone, Default)] pub struct Payloads {
                pub(crate) phases: [HashMap<String, Registered>; 2],
            }
            #[derive(Default)] pub struct Parts {
                pub(crate) values: Vec<Attached<dyn crate::Part>>,
                pub(crate) rows: Rows<Arc<dyn crate::Part>>,
                pub(crate) payloads: Option<Arc<Payloads>>,
                pub(crate) hooks: Vec<u8>,
            }
            #[derive(Default)] pub(crate) struct TagParts {
                pub(crate) entries: Vec<(Box<str>, Attached<dyn crate::Part>)>,
            }
            #[derive(Default)] pub(crate) struct Installed {
                pub(crate) keys: Parts, pub(crate) tags: TagParts,
                pub(crate) metadata: Vec<crate::meta::Info>,
                pub(crate) hook_bits: Vec<bool>, pub(crate) tag_hook_bits: Vec<bool>,
            }
        "#;
        let metadata = r#"
            pub(crate) struct Info { pub(crate) id: Box<str>, pub(crate) hooks: Vec<u32> }
        "#;
        fs::write(dir.join("parts/shared.rs"), storage).unwrap();
        fs::write(dir.join("meta.rs"), metadata).unwrap();
        let generated = source_with(&root, &[]).unwrap();
        let native = format!(
            r#"
            pub trait Part: Send + Sync {{}} impl Part for u32 {{}}
            mod meta {{ {metadata} }}
            mod parts {{
                use std::{{collections::HashMap, sync::Arc}};
                pub(crate) mod shared {{ {storage} }}
                use shared::*;
                mod registration_access {{
                    pub fn parts_installed() -> bool {{ false }}
                    pub fn install_input(input: super::installation::InstalledInput) {{
                        let _: super::shared::Installed = input.into();
                    }}
                }}
                {generated}
            }}
            pub use parts::installation;
            pub use parts::shared::{{Attached, Payloads}};
            pub fn inspect(input: installation::InstalledInput) -> (usize, usize, String, Vec<u32>, bool) {{
                let expected = input.keys.payloads.as_ref().unwrap().phases[0]["sample"].part.clone();
                let stored: parts::shared::Installed = input.into();
                let phases = &stored.keys.payloads.as_ref().unwrap().phases;
                assert_eq!(phases[0]["sample"].owner, "after");
                assert_eq!(phases[1]["sample"].owner, "owner");
                assert_eq!(phases[0]["sample"].owner_index, 11);
                assert_eq!(phases[1]["sample"].owner_index, 19);
                assert!(std::sync::Arc::ptr_eq(&phases[0]["sample"].part, &expected));
                assert!(std::sync::Arc::ptr_eq(&phases[1]["sample"].part, &expected));
                assert_eq!(stored.keys.values[0].owner, "owner");
                assert_eq!(stored.tags.entries[0].0.as_ref(), "tag");
                let item = &stored.metadata[0];
                (stored.keys.values.len(), stored.keys.rows.len(), item.id.to_string(), item.hooks.clone(),
                 stored.keys.hooks.is_empty() && stored.hook_bits.is_empty() && stored.tag_hook_bits.is_empty())
            }}
        "#
        );
        fs::write(root.join("native.rs"), native).unwrap();
        let output = std::process::Command::new("rustc")
            .args(["--edition=2024", "--crate-type=rlib", "--crate-name=lookup"])
            .arg(root.join("native.rs"))
            .arg("-o")
            .arg(root.join("liblookup.rlib"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::write(root.join("author.rs"), r#"
            use lookup::installation::*;
            use std::{collections::HashMap, sync::Arc};
            #[test] fn owned_metadata_survives_installation() {
                let part: Arc<dyn lookup::Part> = Arc::new(5u32);
                let attached = AttachedInput { owner: "owner", part: part.clone() };
                let cloned = attached.clone();
                assert!(Arc::ptr_eq(&attached.part, &cloned.part));
                let checkpoint = Arc::new(PayloadsInput {
                    phases: [11, 19].map(|owner_index| HashMap::from([("sample".into(), RegisteredInput {
                        owner: "owner", owner_index, part: part.clone(),
                    })])),
                });
                let mut payloads = checkpoint.clone();
                Arc::make_mut(&mut payloads).phases[0].get_mut("sample").unwrap().owner = "after";
                let retained = payloads.clone();
                let input = InstalledInput {
                    keys: PartsInput {
                        payloads: Some(payloads),
                        values: vec![attached, cloned],
                        rows: HashMap::from([("row".into(), vec![HolderInput { provider: part.clone() }])]),
                    },
                    tags: TagPartsInput { entries: vec![("tag".into(), AttachedInput { owner: "tag", part })] },
                    metadata: vec![InfoInput { id: "metadata".into(), hooks: vec![7, 8] }],
                };
                assert_eq!(lookup::inspect(input), (2, 1, "metadata".to_owned(), vec![7, 8], true));
                assert_eq!(checkpoint.phases[0]["sample"].owner, "owner");
                assert_eq!(retained.phases[0]["sample"].owner, "after");
            }
        "#).unwrap();
        let binary = root.join(format!("author{}", std::env::consts::EXE_SUFFIX));
        let output = std::process::Command::new("rustc")
            .args(["--edition=2024", "--test"])
            .arg(root.join("author.rs"))
            .arg("--extern")
            .arg(format!("lookup={}", root.join("liblookup.rlib").display()))
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = std::process::Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        for construction in [
            r#"lookup::Attached { owner: "not public", part: std::sync::Arc::new(5u32) }"#,
            "lookup::Payloads { phases: Default::default() }",
        ] {
            fs::write(
                root.join("forbidden.rs"),
                format!("fn main() {{ let _ = {construction}; }}"),
            )
            .unwrap();
            let output = std::process::Command::new("rustc")
                .arg("--edition=2024")
                .arg(root.join("forbidden.rs"))
                .arg("--extern")
                .arg(format!("lookup={}", root.join("liblookup.rlib").display()))
                .arg("-o")
                .arg(root.join("forbidden"))
                .output()
                .unwrap();
            assert!(
                !output.status.success(),
                "lookup fields became public: {construction}"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
