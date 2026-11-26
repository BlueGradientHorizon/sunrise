use std::sync::Arc;
use sunrise_level::datapack::DataPack;

/// The world's pack stack: the built-in pack (unless disabled), the packs
/// of `<world>/datapacks` and the resources root's feature packs, with the
/// enabled ids from `datapacks.json` (`PackRepository.reload`).
pub struct PackStack {
    pub builtin: Option<DataPack>,
    pub content: DataPack,
    pub packs: Vec<Arc<DataPack>>,
    pub enabled: Vec<String>,
    /// The feature flags the stack was selected for: the incoming world's, also on its first
    /// open, before `self.level` holds them (what `neoforge:feature_flags_enabled` reads).
    pub features: Vec<String>,
}
impl PackStack {
    /// The enabled packs in `enabled` (the selected) order, later overriding
    /// earlier: `PackRepository.openAllSelected` keeps the selected order.
    pub fn enabled_packs(&self) -> impl Iterator<Item = &DataPack> {
        self.enabled
            .iter()
            .filter_map(|id| self.packs.iter().find(|p| &p.id == id).map(Arc::as_ref))
    }
}
