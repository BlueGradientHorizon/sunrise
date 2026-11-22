//! The content index captured for each package at startup, shared by registration and declaration
//! loading.
use super::ModsState;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use sunrise_mod_package::{
    Content, Manifest,
    content::{ContentSource, ReadError},
    manifest::MissingJar,
    selection::SelectedPackage,
};

pub(super) struct PackageContent {
    path: PathBuf,
    opened: Result<ContentSource, String>,
}

impl PackageContent {
    pub(super) fn read_declaring(
        &self,
        manifest: &Manifest,
        declared: &[String],
    ) -> Result<Content, String> {
        Content::read_opened(
            self.opened.as_ref().map_err(Clone::clone)?,
            manifest,
            declared,
        )
    }
}

impl mod_api::PackageFileSource for PackageContent {
    fn names(&self) -> Result<Arc<[String]>, String> {
        self.opened
            .as_ref()
            .map(ContentSource::names)
            .map_err(Clone::clone)
    }

    fn read(&self, name: &str) -> Result<Vec<u8>, mod_api::PackageReadError> {
        self.opened
            .as_ref()
            .map_err(|error| mod_api::PackageReadError::Read(error.clone()))?
            .reader()
            .read(name)
            .map_err(|error| match error {
                ReadError::NotFound(_) => mod_api::PackageReadError::NotFound,
                ReadError::Other(reason) => mod_api::PackageReadError::Read(reason),
            })
    }
}

pub(in crate::game) struct StartupAdmission {
    /// The startup selection whose JAR requirements were checked; a later resource reload may
    /// have selected different packages.
    selected: Arc<[SelectedPackage]>,
    native: Arc<sunrise_mod_package::runtime::Runtime>,
    pub(super) refused: Vec<MissingJar>,
    admitted: BTreeMap<String, Arc<PackageContent>>,
}

impl StartupAdmission {
    pub(super) fn open(mods: &ModsState, refused: Vec<MissingJar>) -> Self {
        let native = mods
            .native
            .packages()
            .iter()
            .map(|p| (&p.manifest.id, &p.path));
        let content = mods
            .selected
            .iter()
            .map(|p| (&p.package.manifest.id, &p.path));
        let admitted = native
            .chain(content)
            .filter(|(id, _)| !refused.iter().any(|missing| missing.owners.contains(id)))
            .map(|(id, path)| {
                (
                    id.clone(),
                    Arc::new(PackageContent {
                        path: path.clone(),
                        opened: ContentSource::open(path),
                    }),
                )
            })
            .collect();
        Self {
            selected: mods.selected.clone(),
            native: mods.native.clone(),
            refused,
            admitted,
        }
    }

    pub(in crate::game) fn native_packages(
        &self,
    ) -> &[sunrise_mod_package::packages::AdjacentManifest] {
        // Every validated native package is admitted; a missing JAR on one is fatal.
        self.native.packages()
    }

    pub(in crate::game) fn content_packages(&self) -> impl Iterator<Item = &SelectedPackage> {
        self.selected
            .iter()
            .filter(|p| self.admitted.contains_key(&p.package.manifest.id))
    }

    pub(super) fn matches_selection(&self, selected: &Arc<[SelectedPackage]>) -> bool {
        Arc::ptr_eq(&self.selected, selected)
    }

    pub(super) fn package(&self, id: &str, path: &Path) -> Option<&PackageContent> {
        self.admitted
            .get(id)
            .filter(|source| source.path == path)
            .map(Arc::as_ref)
    }
}

/// The sources for the same admitted packages as `RegistrationContext::mods`.
pub(in crate::game) fn sources(
    admission: &StartupAdmission,
) -> BTreeMap<String, Arc<dyn mod_api::PackageFileSource>> {
    admission
        .admitted
        .iter()
        .map(|(id, source)| {
            let source: Arc<dyn mod_api::PackageFileSource> = source.clone();
            (id.clone(), source)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{
        api_values::TryIntoApi,
        mods::{ContentRequest, read_contents, validate_startup_packages},
    };
    use mod_api::{
        __private::game::RegistrationContextNew, PackageFilesError, PackageReadError,
        RegistrationContext,
    };
    use std::fs;
    use sunrise_mod_package::{mod_list::NATIVE_DIR, runtime::Runtime};

    const SCRIPT: &str = "data/demo/lumen/main.luau";
    const NATIVE_MODEL: &str = "assets/native_demo/models/item/gem.json";

    fn folder(root: &Path, id: &str, extra: &str) -> PathBuf {
        let path = root.join(id);
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("sunrise-mod.toml"),
            format!("id='{id}'\nname='{id}'\nversion='1.0.0'\napi='0.2'\nside='both'\n{extra}"),
        )
        .unwrap();
        path
    }

    fn write(path: &Path, name: &str, bytes: &[u8]) {
        let file = path.join(name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }

    fn with_registration(admission: &StartupAdmission, f: impl FnOnce(RegistrationContext)) {
        let packages = admission
            .native_packages()
            .iter()
            .map(|p| p.try_into_api().unwrap())
            .chain(
                admission
                    .content_packages()
                    .map(|p| p.package.try_into_api().unwrap()),
            )
            .collect();
        let data = mod_api::__private::RegistrationData::new(
            Default::default(),
            Default::default(),
            mod_api::InstanceDirectory::from_path(Path::new("unused")).unwrap(),
            mod_api::RuntimeEnvironment::new(
                mod_api::Dist::Client,
                "26.3".into(),
                "test".into(),
                false,
            ),
        )
        .with_package_files(sources(admission));
        mod_platform::__private::with_test_package_inventory(packages, || {
            mod_platform::__private::with_test_registration_data(data, || {
                let _call = mod_api::__private::enter("reader", "register");
                f(RegistrationContext::new());
            });
        });
    }

    #[test]
    fn registration_files_and_metadata_follow_startup_admission() {
        use mod_api::__private::native::BuiltMod;
        const BUILT: &[BuiltMod] = &[BuiltMod {
            id: "native_demo",
            version: "1.0.0",
            side: mod_api::Side::Both,
        }];
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let demo = folder(root, "demo", "");
        write(&demo, SCRIPT, b"return 1");
        let missing = folder(
            root,
            "missing",
            &format!(
                "[[jar]]\nid='original'\nversion='1'\nsha512='{}'\n",
                "a".repeat(128)
            ),
        );
        write(&missing, "data/missing/lumen/main.luau", b"return 2");
        let native = folder(
            &root.join(NATIVE_DIR),
            "native_demo",
            &format!(
                "capabilities=['registries']\n[native]\ngame='26.3'\nsunrise='{}'\nmixins='{}'\n",
                "a".repeat(40),
                "b".repeat(40)
            ),
        );
        write(
            &native,
            "Cargo.toml",
            b"[package]\nname='native_demo'\nversion='1.0.0'\n",
        );
        write(&native, "src/lib.rs", b"");
        write(&native, NATIVE_MODEL, b"{}");

        let mut mods = ModsState {
            native: Arc::new(Runtime::load(&root.join("sunrise-client"), BUILT).unwrap()),
            dev: vec![demo, missing],
            ..Default::default()
        };
        mods.capture_code_switches(&["native_demo".to_owned()].into());
        mods.select(root, &Default::default());
        validate_startup_packages(&mods).unwrap();
        let admission = mods.admit_startup_packages(&root.join("jars")).unwrap();
        with_registration(&admission, |cx| {
            assert_eq!(cx.mods().ids().collect::<Vec<_>>(), ["native_demo", "demo"]);
            let demo = cx.package_files("demo").unwrap();
            assert_eq!(demo.list("").collect::<Vec<_>>(), [SCRIPT]);
            assert_eq!(demo.read(SCRIPT).unwrap(), b"return 1");
            let native = cx.package_files("native_demo").unwrap();
            assert_eq!(native.list("assets").collect::<Vec<_>>(), [NATIVE_MODEL]);
            assert_eq!(native.read(NATIVE_MODEL).unwrap(), b"{}");
            assert_eq!(
                cx.package_files("missing").err(),
                Some(PackageFilesError::NotEnabled)
            );
        });
    }

    #[test]
    fn registration_and_declaration_loading_share_the_admission_index_and_read_loose_bytes_live() {
        let dir = tempfile::tempdir().unwrap();
        let demo = folder(dir.path(), "demo", "");
        write(&demo, SCRIPT, b"return 1");
        let mut mods = ModsState {
            dev: vec![demo.clone()],
            ..Default::default()
        };
        mods.select(dir.path(), &Default::default());
        validate_startup_packages(&mods).unwrap();
        let admission = mods
            .admit_startup_packages(&dir.path().join("jars"))
            .unwrap();
        let later = "data/demo/lumen/later.luau";
        write(&demo, later, b"return 3");
        write(&demo, SCRIPT, b"return 2");

        let loaded = read_contents(&[ContentRequest {
            path: &demo,
            manifest: &mods.selected[0].package.manifest,
            declared: vec!["demo:registered_item".into()],
            admitted: mods
                .startup_admission()
                .and_then(|a| a.package("demo", &demo)),
        }])
        .pop()
        .unwrap()
        .unwrap();
        with_registration(&admission, |cx| {
            let files = cx.package_files("demo").unwrap();
            assert_eq!(files.list("").collect::<Vec<_>>(), [SCRIPT]);
            assert_eq!(files.list("").collect::<Vec<_>>(), loaded.files.as_ref());
            assert_eq!(files.read(later), Err(PackageReadError::NotFound));
            assert_eq!(files.read(SCRIPT).unwrap(), b"return 2");
            assert_eq!(loaded.file(SCRIPT).unwrap(), b"return 2");
            fs::remove_file(demo.join(SCRIPT)).unwrap();
            assert_eq!(files.read(SCRIPT), Err(PackageReadError::NotFound));
        });
    }
}
