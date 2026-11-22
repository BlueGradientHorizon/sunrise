use super::{HashMap, HashSet, PackOverlay, Path, Result};
use std::sync::Arc;

/// One generation of bound entity type tags. The ordered view is sent to
/// clients; the membership and type bits answer gameplay queries.
#[derive(Clone, Default)]
pub struct EntityTypeTags {
    ids: Arc<[String]>,
    ordered: HashMap<String, Vec<String>>,
    members: Arc<HashMap<String, HashSet<String>>>,
    bits: Vec<u8>,
}

impl EntityTypeTags {
    pub const REGISTRY: &str = "minecraft:entity_type";
    pub const BURN_IN_DAYLIGHT: u8 = 1;
    pub const POWDER_SNOW_WALKABLE: u8 = 2;

    /// The component registry's entity type index, including content-mod types.
    pub fn registry_ids(
        rows: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Arc<[String]>> {
        let mut ids = rows
            .keys()
            .map(|name| sunrise_core::identifier::Identifier::parse(name).map(|id| id.to_string()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.sort_unstable();
        ids.dedup();
        Ok(ids.into())
    }

    pub fn ids(&self) -> &Arc<[String]> {
        &self.ids
    }

    /// Bind pack tags to the prepared or frozen entity registry's sorted IDs.
    pub fn load(root: &Path, overlay: &PackOverlay, ids: Arc<[String]>) -> Result<Self> {
        let ordered = sunrise_world::tag_layers::load(root, "entity_type", overlay, |name| {
            ids.binary_search_by(|id| id.as_str().cmp(name)).is_ok()
        })?;
        Ok(Self::from_ordered(ordered, ids))
    }

    pub fn from_ordered(ordered: HashMap<String, Vec<String>>, ids: Arc<[String]>) -> Self {
        let members: HashMap<String, HashSet<String>> = ordered
            .iter()
            .map(|(tag, values)| (tag.clone(), values.iter().cloned().collect()))
            .collect();
        let daylight = members.get("minecraft:burn_in_daylight");
        let powder_snow = members.get("minecraft:powder_snow_walkable_mobs");
        let bits = ids
            .iter()
            .map(|id| {
                (u8::from(daylight.is_some_and(|tag| tag.contains(id))) * Self::BURN_IN_DAYLIGHT)
                    | (u8::from(powder_snow.is_some_and(|tag| tag.contains(id)))
                        * Self::POWDER_SNOW_WALKABLE)
            })
            .collect();
        Self {
            ids,
            ordered,
            members: Arc::new(members),
            bits,
        }
    }

    pub fn ordered(&self) -> &HashMap<String, Vec<String>> {
        &self.ordered
    }

    pub fn members(&self) -> &Arc<HashMap<String, HashSet<String>>> {
        &self.members
    }

    pub fn contains(&self, tag: &str, id: &str) -> bool {
        self.members
            .get(tag)
            .is_some_and(|members| members.contains(id))
    }

    #[inline]
    pub fn has_bit(&self, slot: Option<usize>, flag: u8) -> bool {
        slot.and_then(|slot| self.bits.get(slot))
            .is_some_and(|bits| bits & flag != 0)
    }
}
