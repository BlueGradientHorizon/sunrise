pub(in crate::game::client_side) use crate::game::mods::package_files::StartupAdmission;
use std::sync::Arc;
use sunrise_mod_package::selection::SelectedPackage;

impl StartupAdmission {
    pub(in crate::game) fn matches_selection(&self, selected: &Arc<[SelectedPackage]>) -> bool {
        Arc::ptr_eq(&self.selected, selected)
    }
}
