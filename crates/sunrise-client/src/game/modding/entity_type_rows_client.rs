use crate::game::entity_type_rows::EntityTypeRows;

impl EntityTypeRows {
    /// Whether `id` is an entity type.
    pub(in crate::game) fn contains(&self, id: &str) -> bool {
        self.ids.binary_search_by(|t| t.as_str().cmp(id)).is_ok()
    }
    /// `EntityType.getDimensions()` of `id`: its width, height and eye height.
    pub(in crate::game) fn dimensions(&self, id: &str) -> Option<[f32; 3]> {
        let i = self
            .dimensions
            .binary_search_by(|(t, _)| t.as_str().cmp(id))
            .ok()?;
        Some(self.dimensions[i].1)
    }
    /// Whether `id` is a living entity type.
    pub(in crate::game) fn is_living(&self, id: &str) -> bool {
        self.living.binary_search_by(|t| t.as_str().cmp(id)).is_ok()
    }
}
