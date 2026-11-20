// exports: none (reached inside the crate only)
use crate::{
    __private::{DeclarationKind, DeclarationRefusal},
    Answered, DeclarationError,
};

/// The package inventory carries the same namespace set as content files and
/// the package loader's collision fence. A package always owns its own ID.
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

/// Validate and qualify a declaration using the package namespace inventory.
pub fn owned_declaration_id(owner: &str, name: &str) -> Result<String, String> {
    validate_owned_id(owner, qualified(owner, name))
        .map_err(|error| format!("mod {owner} declares {error}"))
}

fn validate_owned_id(owner: &str, id: String) -> Result<String, DeclarationError> {
    let Some(parsed) = crate::Identifier::parse_namespaced(&id) else {
        return Err(DeclarationError::InvalidId { id });
    };
    let namespace = parsed.namespace();
    if !owns_namespace(owner, namespace) {
        return Err(DeclarationError::UnownedNamespace {
            id,
            namespace: namespace.to_owned(),
        });
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
    output: &mut Vec<Answered<D>>,
    mut declaration: D,
) -> Result<(), DeclarationError> {
    let (id, references) = declaration.names();
    if !crate::registration_open() {
        let error = DeclarationError::RegistrationClosed { id: id.clone() };
        log::warn!("mod {mod_id} {:?} declaration refused: {error}", D::KIND);
        return Err(error);
    }
    *id = validate_owned_id(mod_id, qualified(mod_id, id)).inspect_err(|error| {
        context
            .declaration_refusals
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(DeclarationRefusal::new(mod_id, D::KIND, error.clone()));
    })?;
    for reference in references.into_iter().flatten() {
        *reference = qualified(mod_id, reference);
    }
    output.push(Answered {
        answer: declaration,
        mod_id,
    });
    Ok(())
}

impl crate::RegistrationContext {
    /// Refuse this mod's registration with a diagnostic, without panicking.
    /// The first reason wins. Reasons exceeding 4096 UTF-8 bytes are truncated
    /// at a character boundary; shorter reasons are preserved verbatim.
    /// All this mod's declarations from this registration are discarded, including
    /// later declarations, without adding their validation errors to the report.
    /// Other mods' declarations are unchanged. Returning from `Hooks::register` immediately
    /// is recommended.
    pub fn refuse(&self, reason: &str) {
        let mut reason_slot = self.self_refusal.lock().unwrap_or_else(|p| p.into_inner());
        if reason_slot.is_none() {
            *reason_slot = Some(super::registration_refusal::bounded_text(reason).to_owned());
        }
    }

    /// Report a loading problem that doesn't stop this mod: a missing optional integration,
    /// a setting it corrected. `key` is a translation key in the mod's own `lang/` files and
    /// `args` its `%s` arguments; the Mods screen shows it in the player's language, and the
    /// log names the key. The first 16 warnings are kept, each cut as `refuse` cuts.
    pub fn warn(&self, key: &str, args: &[&str]) {
        let mut warnings = self.warnings.lock().unwrap_or_else(|p| p.into_inner());
        if warnings.len() < super::registration_refusal::MAX_LOADING_WARNINGS {
            warnings.push(super::registration_refusal::LoadingMessage::new(key, args));
        }
    }
}

/// The engine consumes block/item declaration ownership refusals before loading
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
