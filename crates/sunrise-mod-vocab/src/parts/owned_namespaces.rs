// exports: none (reached inside the crate only)
use crate::{
    __private::{DeclarationKind, DeclarationRefusal},
    Answered, DeclarationError,
};

/// The package inventory carries the same namespace set as content files and
/// the package loader's collision fence. A package always owns its own ID.
/// Keep the cross-crate export for runtime level creation until P3-36.
pub fn owns_namespace(owner: &str, namespace: &str) -> bool {
    owner == namespace
        || crate::LoadedMods::new()
            .get(owner)
            .is_some_and(|package| package.namespaces().any(|owned| owned == namespace))
}

/// `name` with its namespace: a bare name is in mod `owner`'s own, for a declared id and
/// for the block and item references a declaration names (`block_item`, `pick_item`,
/// `wall_block`, `base_block`, `crafting_remainder`, `Item::block`) alike.
pub fn qualified(owner: &str, name: &str) -> String {
    if name.contains(':') {
        name.to_owned()
    } else {
        format!("{owner}:{name}")
    }
}

/// Qualify a bare declaration name with the mod ID and validate the resulting identifier.
pub fn declaration_id(owner: &str, name: &str) -> Result<String, String> {
    validate_declaration_id(qualified(owner, name))
        .map_err(|error| format!("mod {owner} declares {error}"))
}

// Removed in P3-04 step 3 once Part registrar callers use declaration_id.
pub fn owned_declaration_id(owner: &str, name: &str) -> Result<String, String> {
    declaration_id(owner, name)
}

fn validate_declaration_id(id: String) -> Result<String, DeclarationError> {
    if crate::Identifier::parse_namespaced(&id).is_none() {
        return Err(DeclarationError::InvalidId { id });
    }
    Ok(id)
}

/// A declaration a mod's `blocks` or `items` hook adds: its id, and the names of other
/// content it references.
pub(crate) trait Declaration {
    const KIND: DeclarationKind;

    /// The declared id and its optional references.
    fn names(&mut self) -> (&mut String, impl IntoIterator<Item = &mut Option<String>>);
}

impl Declaration for crate::Block {
    const KIND: DeclarationKind = DeclarationKind::Block;

    fn names(&mut self) -> (&mut String, impl IntoIterator<Item = &mut Option<String>>) {
        (
            &mut self.id,
            [
                &mut self.block_item,
                &mut self.pick_item,
                &mut self.wall_block,
                &mut self.base_block,
            ],
        )
    }
}

impl Declaration for crate::Item {
    const KIND: DeclarationKind = DeclarationKind::Item;

    fn names(&mut self) -> (&mut String, impl IntoIterator<Item = &mut Option<String>>) {
        (
            &mut self.id,
            [&mut self.crafting_remainder, &mut self.block],
        )
    }
}

/// Validate at the call site and keep the dispatch owner with the declaration.
pub(crate) fn submit_declaration<D: Declaration>(
    context: &crate::RegistrationContext,
    mod_id: &'static str,
    submit: &mut dyn FnMut(Answered<D>) -> Result<(), DeclarationError>,
    mut declaration: D,
) -> Result<(), DeclarationError> {
    if !crate::registration_open() {
        let error = DeclarationError::RegistrationClosed {
            id: declaration.names().0.clone(),
        };
        log::warn!("mod {mod_id} {:?} declaration refused: {error}", D::KIND);
        return Err(error);
    }
    let result = (|| {
        let (id, references) = declaration.names();
        *id = validate_declaration_id(qualified(mod_id, id))?;
        for reference in references.into_iter().flatten() {
            *reference = qualified(mod_id, reference);
        }
        submit(Answered {
            answer: declaration,
            mod_id,
        })
    })();
    result.inspect_err(|error| {
        context
            .declaration_refusals
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(DeclarationRefusal::new(mod_id, D::KIND, error.clone()));
    })
}

/// The engine consumes block/item declaration refusals before loading
/// resources.
// hidden: engine admission plumbing, re-exported through __private for the loader.
#[doc(hidden)]
pub fn take_declaration_refusals(context: &crate::RegistrationContext) -> Vec<DeclarationRefusal> {
    std::mem::take(
        &mut *context
            .declaration_refusals
            .lock()
            .unwrap_or_else(|p| p.into_inner()),
    )
}
