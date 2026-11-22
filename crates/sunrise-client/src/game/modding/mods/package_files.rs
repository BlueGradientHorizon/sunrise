//! Each enabled package's content files, handed to registration so any native mod can list and
//! read them (`RegistrationContext::package_files`) before content loading reads them itself.
use super::ModsState;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use sunrise_mod_package::content::{ContentSource, ReadError};

/// One package's content files, listed the first time a mod asks, by content loading's own walk.
struct PackageContent {
    path: PathBuf,
    listed: OnceLock<Result<(Arc<[String]>, ContentSource), String>>,
}

impl PackageContent {
    fn listed(&self) -> Result<&(Arc<[String]>, ContentSource), String> {
        self.listed
            .get_or_init(|| {
                ContentSource::open(&self.path).map(|(source, names)| (names.into(), source))
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}

impl mod_api::PackageFileSource for PackageContent {
    fn names(&self) -> Result<Arc<[String]>, String> {
        self.listed().map(|(names, _)| names.clone())
    }
    fn read(&self, name: &str) -> Result<Vec<u8>, mod_api::PackageReadError> {
        self.listed()
            .map_err(mod_api::PackageReadError::Read)?
            .1
            .reader()
            .read(name)
            .map_err(|error| match error {
                ReadError::NotFound(_) => mod_api::PackageReadError::NotFound,
                ReadError::Other(reason) => mod_api::PackageReadError::Read(reason),
            })
    }
}

/// The content files of every enabled package, native and content-only, by mod id: the packages
/// `RegistrationContext::mods` lists.
pub(in crate::game) fn sources(
    mods: &ModsState,
) -> BTreeMap<String, Arc<dyn mod_api::PackageFileSource>> {
    let native = mods
        .native
        .packages()
        .iter()
        .map(|package| (&package.manifest.id, &package.path));
    let content = mods
        .selected
        .iter()
        .map(|selected| (&selected.package.manifest.id, &selected.path));
    native
        .chain(content)
        .map(|(id, path)| {
            let source: Arc<dyn mod_api::PackageFileSource> = Arc::new(PackageContent {
                path: path.clone(),
                listed: OnceLock::new(),
            });
            (id.clone(), source)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::PackageFileSource as _;

    fn package(path: PathBuf) -> PackageContent {
        PackageContent {
            path,
            listed: OnceLock::new(),
        }
    }

    /// A folder package is listed once, by content loading's walk, and read live; a package that
    /// cannot be opened gives its reason to every call.
    #[test]
    fn a_package_is_listed_once_and_read_as_content_loading_reads_it() {
        let folder = tempfile::tempdir().unwrap();
        let script = folder.path().join("data/demo/lumen/main.luau");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(&script, b"return 1").unwrap();
        let demo = package(folder.path().to_owned());
        assert_eq!(&*demo.names().unwrap(), ["data/demo/lumen/main.luau"]);
        std::fs::write(&script, b"return 2").unwrap();
        std::fs::write(folder.path().join("data/demo/lumen/later.luau"), b"").unwrap();
        assert_eq!(demo.read("data/demo/lumen/main.luau").unwrap(), b"return 2");
        assert_eq!(&*demo.names().unwrap(), ["data/demo/lumen/main.luau"]);
        assert!(demo.read("data/demo/../demo/lumen/main.luau").is_err());

        let missing = package(folder.path().join("missing.smod"));
        let reason = missing.names().unwrap_err();
        assert_eq!(
            missing.read("data/demo/lumen/main.luau"),
            Err(mod_api::PackageReadError::Read(reason))
        );
    }
}
