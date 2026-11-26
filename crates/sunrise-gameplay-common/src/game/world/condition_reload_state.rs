use super::*;

pub(super) struct StagedWorldTables {
    pub(super) tables: Option<WorldTables>,
    pub(super) advancements: Arc<sunrise_sim::advancements::Tree>,
    pub(super) health: sunrise_sim::health::HealthResources,
    pub(super) datapack_registries: Option<mod_api::DatapackRegistrySet>,
}

pub struct PreparedWorldReload {
    pub(super) structure_spawns: sunrise_sim::mobs::spawn_selection::StructureSpawns,
    pub(super) advancements: Arc<sunrise_sim::advancements::Tree>,
    pub(super) tables: Option<WorldTables>,
    pub(super) health: sunrise_sim::health::HealthResources,
    pub(super) parts: PreparedParts,
    pub(super) wire: Option<Arc<sunrise_sim::lan::data::SyncData>>,
    pub(super) wire_tags: sunrise_sim::lan::data::WorldTags,
    pub(super) datapack_registries: Option<mod_api::DatapackRegistrySet>,
}
