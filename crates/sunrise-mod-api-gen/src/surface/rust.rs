//! Rust as the surface writes it: one normalising function for types and signatures (paths
//! relative to `mod_api`, lifetimes kept, one spacing), and each type's wire layout description
//! for the shape function.
use crate::index::{Found, Index, ItemId, OutsideSeam};
use mod_source::layouts::{WireForm, Wires, param_names};
use mod_surface::shape::Shape;
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::{BTreeSet, HashMap, HashSet};
use syn::visit_mut::VisitMut;
use syn::{Item, Type};

/// The crates whose paths are written by their last segment (`Arc`, `HashMap`, `Duration`).
const STD: &[&str] = &["std", "core", "alloc"];

/// `tokens` with one spacing: a space after `,`, `;` and a lone `:`, around `->`, `=`, `+` and
/// `=>`, between two words, after a word-ending `>`, `)` or `]` before a word; none elsewhere.
/// A comma before a closing bracket or the end is dropped.
pub(super) fn print(tokens: TokenStream) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum Last {
        Start,
        Word,
        Close,
        Glue,
        Spaced,
    }
    fn walk(tokens: TokenStream, out: &mut String, last: &mut Last) {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        let mut i = 0;
        while i < trees.len() {
            match &trees[i] {
                TokenTree::Ident(ident) => {
                    if matches!(*last, Last::Word | Last::Close) {
                        out.push(' ');
                    }
                    out.push_str(&ident.to_string());
                    *last = Last::Word;
                }
                TokenTree::Literal(literal) => {
                    if matches!(*last, Last::Word | Last::Close) {
                        out.push(' ');
                    }
                    out.push_str(&literal.to_string());
                    *last = Last::Word;
                }
                TokenTree::Group(group) => {
                    let (open, close) = match group.delimiter() {
                        Delimiter::Parenthesis => ("(", ")"),
                        Delimiter::Bracket => ("[", "]"),
                        Delimiter::Brace => (" { ", " }"),
                        Delimiter::None => ("", ""),
                    };
                    out.push_str(open);
                    let mut inner = Last::Start;
                    walk(group.stream(), out, &mut inner);
                    out.push_str(close);
                    *last = Last::Close;
                }
                TokenTree::Punct(punct) => {
                    // Gather a joint run (`::`, `->`, `'a`).
                    let mut run = punct.as_char().to_string();
                    let mut joint = punct.spacing() == Spacing::Joint;
                    while joint && i + 1 < trees.len() {
                        let TokenTree::Punct(next) = &trees[i + 1] else {
                            break;
                        };
                        if punct.as_char() == '\'' {
                            break;
                        }
                        run.push(next.as_char());
                        joint = next.spacing() == Spacing::Joint;
                        i += 1;
                    }
                    let end = trees.get(i + 1).is_none()
                        || matches!(trees.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '>');
                    match run.as_str() {
                        "," if end => {}
                        "," | ";" | ":" => {
                            out.push_str(&run);
                            out.push(' ');
                            *last = Last::Spaced;
                        }
                        "->" | "=" | "+" | "=>" => {
                            if !out.ends_with(' ') && !out.is_empty() {
                                out.push(' ');
                            }
                            out.push_str(&run);
                            out.push(' ');
                            *last = Last::Spaced;
                        }
                        "'" => {
                            if *last == Last::Word {
                                out.push(' ');
                            }
                            out.push('\'');
                            // The lifetime's name follows as a word glued to the quote.
                            *last = Last::Glue;
                        }
                        ">" => {
                            out.push('>');
                            *last = Last::Close;
                        }
                        "&" | "&&" if *last == Last::Word => {
                            out.push(' ');
                            out.push_str(&run);
                            *last = Last::Glue;
                        }
                        _ => {
                            out.push_str(&run);
                            *last = Last::Glue;
                        }
                    }
                }
            }
            i += 1;
        }
    }
    let mut out = String::new();
    walk(tokens, &mut out, &mut Last::Start);
    out.trim().to_owned()
}

/// Rewrites every path of a type or signature to the path a mod writes, collecting the ids of
/// the API types and traits it names.
pub(super) struct Paths<'p, 'a> {
    pub(super) index: &'p mut Index<'a>,
    pub(super) public: &'p HashMap<ItemId, Vec<String>>,
    pub(super) module: usize,
    /// Generic parameters in scope (and `Self`), written as they are.
    pub(super) params: Vec<String>,
    pub(super) refs: BTreeSet<String>,
    pub(super) check: Option<(&'p ExternalTypes<'p>, ItemId)>,
    /// Private aliases already followed while checking this public item's types.
    pub(super) aliases: HashSet<ItemId>,
}

/// A visitor cannot return an error. Keep the first disallowed path and its owning item.
pub(super) struct ExternalTypes<'a> {
    outside: &'a OutsideSeam,
    public: &'a HashMap<ItemId, Vec<String>>,
    first: std::cell::RefCell<Option<(ItemId, Vec<String>)>>,
}

impl<'a> ExternalTypes<'a> {
    pub(super) fn new(outside: &'a OutsideSeam, public: &'a HashMap<ItemId, Vec<String>>) -> Self {
        Self {
            outside,
            public,
            first: Default::default(),
        }
    }

    fn inspect(
        &self,
        index: &mut Index<'_>,
        module: usize,
        id: ItemId,
        path: &[String],
        rooted: bool,
    ) {
        if let Some(path) = self.outside.external_path(index, module, path, rooted) {
            self.first.borrow_mut().get_or_insert_with(|| (id, path));
        }
    }

    pub(super) fn finish(&self, index: &Index<'_>) -> Result<(), String> {
        if let Some((id, path)) = self.first.take() {
            let owner = self.public.get(&id).map_or_else(
                || {
                    let (_, name) =
                        super::walk::defined(index.item(id)).expect("a named surface item");
                    format!("{}::{name}", super::module_name(index, id.module))
                },
                |public| format!("mod_api::{}", public.join("::")),
            );
            return Err(OutsideSeam::error(&owner, &path));
        }
        Ok(())
    }
}

/// The id prefix of a public item that a signature can name.
pub(super) fn ref_kind(item: &Item) -> Option<&'static str> {
    match item {
        Item::Struct(_) | Item::Enum(_) | Item::Union(_) | Item::Type(_) => Some("type"),
        Item::Trait(_) => Some("trait"),
        _ => None,
    }
}

impl Paths<'_, '_> {
    fn inspect_private_alias(&mut self, id: ItemId, arguments: &syn::PathArguments) {
        let Item::Type(mut alias) = self.index.item(id).clone() else {
            return;
        };
        if !self.aliases.insert(id) {
            return;
        }
        // Explicit type arguments replace positional defaults. Inspecting a replaced default
        // would reject a safe use such as `Alias<u32>` just because `Alias<T = Outside>` has an
        // unused outside default.
        let supplied_types = match arguments {
            syn::PathArguments::AngleBracketed(arguments) => arguments
                .args
                .iter()
                .filter(|argument| matches!(argument, syn::GenericArgument::Type(_)))
                .count(),
            _ => 0,
        };
        let mut type_index = 0;
        for param in &mut alias.generics.params {
            if let syn::GenericParam::Type(param) = param {
                if type_index < supplied_types {
                    param.default = None;
                }
                type_index += 1;
            }
        }
        let old_module = self.module;
        let old_params = std::mem::replace(&mut self.params, param_names(&alias.generics));
        let old_refs = std::mem::take(&mut self.refs);
        self.module = id.module;
        self.visit_generics_mut(&mut alias.generics);
        let mut ty = *alias.ty;
        self.visit_type_mut(&mut ty);
        self.module = old_module;
        self.params = old_params;
        self.refs = old_refs;
        self.aliases.remove(&id);
    }

    fn rewrite(&mut self, path: &mut syn::Path) {
        let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        let rooted = path.leading_colon.is_some();
        if path.leading_colon.is_none()
            && segments.len() == 1
            && self.params.iter().any(|p| *p == segments[0])
            || segments.first().is_some_and(|s| s == "Self")
        {
            return;
        }
        let found = if rooted {
            // `::` starts at the extern prelude. Let the outside-seam lookup handle dependency
            // aliases without allowing a local module of the same name to shadow them.
            Ok(Found::External(segments.clone()))
        } else {
            self.index.resolve_path(self.module, &segments)
        };
        let written = match found {
            Ok(Found::Item(id)) => {
                if let Some((check, owner)) = self.check {
                    check.inspect(self.index, self.module, owner, &segments, rooted);
                }
                match self.public.get(&id) {
                    Some(public) => {
                        if let Some(kind) = ref_kind(self.index.item(id)) {
                            self.refs.insert(format!("{kind}:{}", public.join("::")));
                        }
                        public.clone()
                    }
                    None => {
                        let arguments = path
                            .segments
                            .last()
                            .map(|segment| segment.arguments.clone())
                            .unwrap_or(syn::PathArguments::None);
                        self.inspect_private_alias(id, &arguments);
                        vec![segments.last().cloned().unwrap_or_default()]
                    }
                }
            }
            Ok(Found::External(full)) => {
                if let Some((check, id)) = self.check {
                    check.inspect(self.index, self.module, id, &full, rooted);
                }
                if STD.contains(&full[0].as_str()) {
                    vec![full.last().cloned().unwrap_or_default()]
                } else {
                    full
                }
            }
            Err(_) => {
                if let Some((check, id)) = self.check {
                    check.inspect(self.index, self.module, id, &segments, rooted);
                }
                segments.clone()
            }
        };
        if written == segments {
            return;
        }
        let arguments = path
            .segments
            .last()
            .map(|s| s.arguments.clone())
            .unwrap_or(syn::PathArguments::None);
        let mut rewritten: syn::Path =
            syn::parse_str(&written.join("::")).unwrap_or_else(|_| path.clone());
        if let Some(last) = rewritten.segments.last_mut() {
            last.arguments = arguments;
        }
        rewritten.leading_colon = None;
        *path = rewritten;
    }
}

impl VisitMut for Paths<'_, '_> {
    fn visit_type_path_mut(&mut self, ty: &mut syn::TypePath) {
        syn::visit_mut::visit_type_path_mut(self, ty);
        if let Some(qself) = &ty.qself {
            let trait_path: Vec<String> = ty
                .path
                .segments
                .iter()
                .take(qself.position)
                .map(|segment| segment.ident.to_string())
                .collect();
            if let Some((check, id)) = self.check {
                check.inspect(
                    self.index,
                    self.module,
                    id,
                    &trait_path,
                    ty.path.leading_colon.is_some(),
                );
            }
        } else {
            self.rewrite(&mut ty.path);
        }
    }
    fn visit_trait_bound_mut(&mut self, bound: &mut syn::TraitBound) {
        syn::visit_mut::visit_trait_bound_mut(self, bound);
        self.rewrite(&mut bound.path);
    }
    fn visit_attribute_mut(&mut self, _: &mut syn::Attribute) {}
}

/// The game's crates: types cross as their derive or hand-written impl says, and an opaque type
/// is written with the public paths of the API.
pub(super) struct GameWires<'f> {
    forms: &'f HashMap<ItemId, WireForm>,
    ids: &'f HashMap<ItemId, Vec<String>>,
    check: Option<&'f ExternalTypes<'f>>,
    item: std::cell::Cell<Option<ItemId>>,
}

impl<'f> GameWires<'f> {
    pub(super) fn new(
        forms: &'f HashMap<ItemId, WireForm>,
        ids: &'f HashMap<ItemId, Vec<String>>,
        check: Option<&'f ExternalTypes<'f>>,
    ) -> Self {
        Self {
            forms,
            ids,
            check,
            item: Default::default(),
        }
    }
}

impl Wires for GameWires<'_> {
    fn form(&self, id: ItemId, attrs: &[syn::Attribute], derives: &[String]) -> Option<WireForm> {
        self.item.set(Some(id));
        super::wire_forms::form_of(id, attrs, derives, self.forms)
    }
    fn path(&self, id: ItemId) -> Option<&[String]> {
        self.ids.get(&id).map(Vec::as_slice)
    }
    fn opaque(&self, index: &mut Index<'_>, module: usize, ty: &Type, params: &[String]) -> Shape {
        let mut ty = ty.clone();
        let mut paths = Paths {
            index,
            public: self.ids,
            module,
            params: params.to_vec(),
            refs: BTreeSet::new(),
            check: self.check.zip(self.item.get()),
            aliases: HashSet::new(),
        };
        paths.visit_type_mut(&mut ty);
        Shape::Opaque(print(ty.to_token_stream()))
    }
}
