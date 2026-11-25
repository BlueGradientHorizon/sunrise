#[allow(unused_imports)]
pub(in crate::game::client_side) use super::ModsState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::package_files::OpenError;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::package_files::PackageContent;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::package_files::StartupAdmission;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::package_files::Visibility;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::package_files::sources;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::mods::package_files::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_mod_package::{
    Content, Manifest,
    content::{AdmittedContentSource, ContentSource, ReadError},
    manifest::MissingJar,
    selection::SelectedPackage,
};

impl StartupAdmission {
    pub(in crate::game) fn matches_selection(&self, selected: &Arc<[SelectedPackage]>) -> bool {
        Arc::ptr_eq(&self.selected, selected)
    }
}
