//! One shared startup `(kind, id) -> (owner_mod, TypeId, type_name)` identity guard
//! across capabilities, custom static registries, mod events, and data maps.
use std::{any::TypeId, collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TypeIdentityKind {
    BlockCapability,
    ItemCapability,
    EntityCapability,
    CustomRegistry,
}

impl TypeIdentityKind {
    fn describe(self) -> &'static str {
        match self {
            Self::BlockCapability | Self::ItemCapability | Self::EntityCapability => "capability",
            Self::CustomRegistry => "custom registry",
        }
    }

    fn conflict_verb(self) -> &'static str {
        match self {
            Self::BlockCapability | Self::ItemCapability | Self::EntityCapability => {
                "is answered with two handler types"
            }
            Self::CustomRegistry => "is used with two types",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeIdentity {
    type_id: Option<TypeId>,
    type_name: Arc<str>,
}

impl TypeIdentity {
    pub(crate) fn typed(type_id: TypeId, type_name: &'static str) -> Self {
        Self {
            type_id: Some(type_id),
            type_name: Arc::from(type_name),
        }
    }

    pub(crate) fn by_name(type_name: Arc<str>) -> Self {
        Self {
            type_id: None,
            type_name,
        }
    }

    fn matches(&self, other: &Self) -> bool {
        if let (Some(a), Some(b)) = (self.type_id, other.type_id)
            && a != b
        {
            return false;
        }
        self.type_name == other.type_name
    }
}

#[derive(Clone, Debug)]
struct Entry {
    owner_mod: &'static str,
    identity: TypeIdentity,
}

/// Startup table recording the first Rust type identity bound to each `(kind, id)`.
///
/// Any later registration under the same `(kind, id)` with a different type identity is
/// refused with a diagnostic naming the kind, ID, both mods, and both Rust type names.
#[derive(Clone, Default)]
pub(crate) struct TypeIdentityTable {
    entries: HashMap<(TypeIdentityKind, String), Entry>,
}

impl TypeIdentityTable {
    /// Seed a built-in binding (owned by `"the game"`) without refusing.
    pub(crate) fn seed(&mut self, kind: TypeIdentityKind, id: &str, identity: TypeIdentity) {
        self.entries.insert(
            (kind, id.to_owned()),
            Entry {
                owner_mod: "the game",
                identity,
            },
        );
    }

    /// Record `(kind, id)` for `mod_id` with `identity`, or return a refusal naming
    /// both mods and both types if an incompatible identity was already bound.
    pub(crate) fn claim(
        &mut self,
        kind: TypeIdentityKind,
        id: &str,
        mod_id: &'static str,
        identity: TypeIdentity,
    ) -> Result<(), String> {
        let key = (kind, id.to_owned());
        if let Some(existing) = self.entries.get(&key) {
            if !existing.identity.matches(&identity) {
                return Err(format!(
                    "{} {id} {}, {} by {} and {} by {mod_id}",
                    kind.describe(),
                    kind.conflict_verb(),
                    existing.identity.type_name,
                    existing.owner_mod,
                    identity.type_name,
                ));
            }
            return Ok(());
        }
        self.entries.insert(
            key,
            Entry {
                owner_mod: mod_id,
                identity,
            },
        );
        Ok(())
    }
}
