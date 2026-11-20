use super::*;
use crate::{
    EntityTypeKey, Heightmap, SpawnPlacementError, SpawnPlacementOperation,
    SpawnPlacementPredicate, SpawnPlacementType,
};

/// One spawn predicate registration, in mod registration order. The game
/// resolves the declarations at resource load before running spawn checks.
#[doc(hidden)]
// no wire: contains a native callback.
pub struct SpawnPlacementDeclaration {
    /// The canonical id of the entity type.
    pub id: String,
    /// The registrar key to give the predicate.
    pub kind: EntityTypeKey,
    /// The mod that registered the predicate, for load refusals.
    pub mod_id: &'static str,
    /// Where the type stands, if the registration sets it.
    pub placement: Option<SpawnPlacementType>,
    /// Which surface the spawner finds, if the registration sets it.
    pub heightmap: Option<Heightmap>,
    /// The native predicate to run for the type.
    pub predicate: Arc<dyn SpawnPlacementPredicate>,
    /// How to combine this predicate with those already registered.
    pub operation: SpawnPlacementOperation,
}

/// The spawn predicates mods registered, in registration order.
#[doc(hidden)]
pub fn spawn_placement_declarations() -> &'static [SpawnPlacementDeclaration] {
    installed().map_or(&[], |installed| &installed.spawn_placements)
}

/// Registrar operations for spawn placements.
pub trait SpawnPlacementsRegistrar {
    /// Register a native predicate for `kind` (NeoForge's `RegisterSpawnPlacementsEvent`).
    /// The first registration for a type without a vanilla entry must supply both `placement`
    /// and `heightmap`; its predicate becomes the original regardless of `operation`.
    /// For an existing entry, only `Replace` may supply either option, and `None` keeps
    /// that field unchanged.
    ///
    /// The last replacement runs alone, even if later registrations add `Or` or `And`
    /// predicates. Otherwise the original runs first, then the `Or` predicates until
    /// one passes, then the `And` predicates until one fails. Each list runs in registration
    /// order, and no `And` runs if neither the original nor an `Or` passed.
    ///
    /// An unavailable registrar key returns [`SpawnPlacementError::InvalidEntityType`].
    /// A type that does not load or an invalid choice of metadata refuses resource loading,
    /// naming the mod and type. `mob_spawn_placement_check` still sees the resulting answer.
    fn register_spawn_placement(
        &mut self,
        kind: EntityTypeKey,
        placement: Option<SpawnPlacementType>,
        heightmap: Option<Heightmap>,
        predicate: Arc<dyn SpawnPlacementPredicate>,
        operation: SpawnPlacementOperation,
    ) -> Result<(), SpawnPlacementError>;
}
