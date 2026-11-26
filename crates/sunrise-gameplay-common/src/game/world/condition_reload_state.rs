use super::*;

pub struct StagedWorldTables {
    pub tables: Option<WorldTables>,
    pub advancements: Arc<sunrise_sim::advancements::Tree>,
    pub health: sunrise_sim::health::HealthResources,
    pub datapack_registries: Option<mod_api::DatapackRegistrySet>,
}

pub struct PreparedWorldReload {
    pub structure_spawns: sunrise_sim::mobs::spawn_selection::StructureSpawns,
    pub advancements: Arc<sunrise_sim::advancements::Tree>,
    pub tables: Option<WorldTables>,
    pub health: sunrise_sim::health::HealthResources,
    pub parts: PreparedParts,
    pub wire: Option<Arc<sunrise_sim::lan::data::SyncData>>,
    pub wire_tags: sunrise_sim::lan::data::WorldTags,
    pub datapack_registries: Option<mod_api::DatapackRegistrySet>,
}
