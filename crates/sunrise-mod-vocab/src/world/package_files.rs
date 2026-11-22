//! An enabled package's content files, read through the registration context.
//!
//! NeoForge hands a mod any loaded mod file's contents from construction on
//! (`ModList.getModFileById(id).getFile().getContents()`, a `JarContents`: `containsFile`,
//! `readFile`, `visitContent`). Here a native mod lists and reads any enabled package's `assets/`
//! and `data/` files from registration on, so it can turn another package's files into the
//! blocks and items it registers. The view is content loading's own: the same file list, and
//! each read through the same reader and checks.
use std::sync::Arc;

/// The game's side of one enabled package's content files. A mod reaches it through
/// [`RegistrationContext::package_files`](crate::RegistrationContext::package_files), not
/// directly.
pub trait PackageFileSource: Send + Sync + 'static {
    /// Every content file (`assets/…`, `data/…`), sorted, as content loading lists them; the
    /// reason when the package's files cannot be listed.
    fn names(&self) -> Result<Arc<[String]>, String>;
    /// The bytes of `name`, one of [`names`](Self::names), read and checked as content loading
    /// reads it.
    fn read(&self, name: &str) -> Result<Vec<u8>, PackageReadError>;
}

/// A package's content files were asked for and cannot be served.
#[derive(Clone, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
#[non_exhaustive]
pub enum PackageFilesError {
    /// The call is outside any mod's registration dispatch (a test, a tool, the engine's own
    /// call).
    Unavailable,
    /// No enabled package has this id: it is not installed, switched off, or was refused.
    NotEnabled,
    /// The package's files cannot be listed: the reason, as content loading would report it.
    Unreadable(String),
}
impl std::fmt::Display for PackageFilesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => f.write_str("package files are read during a mod's registration"),
            Self::NotEnabled => f.write_str("no enabled package has this id"),
            Self::Unreadable(reason) => write!(f, "the package's files cannot be listed: {reason}"),
        }
    }
}
impl std::error::Error for PackageFilesError {}

/// A read of one package file was refused.
#[derive(Clone, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
#[non_exhaustive]
pub enum PackageReadError {
    /// The package has no content file at this path.
    NotFound,
    /// The file is listed and could not be read: the reason, as content loading would report it.
    Read(String),
}
impl std::fmt::Display for PackageReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => f.write_str("the package has no content file at this path"),
            Self::Read(reason) => write!(f, "the package file cannot be read: {reason}"),
        }
    }
}
impl std::error::Error for PackageReadError {}

/// One enabled package's content files, from
/// [`RegistrationContext::package_files`](crate::RegistrationContext::package_files). The list is
/// fixed when the handle is made; the mod may keep it and read from any thread. Each read goes to
/// the package as content loading does: a `.smod` reads the same bytes every time, a development
/// folder (`--dev-mod`) what it holds at that moment.
// exhaustive: its fields are private, so mods cannot build or destructure it
// no wire: a mod-side handle around the game's source, which crosses as its own resource.
#[derive(Clone)]
pub struct PackageFiles {
    names: Arc<[String]>,
    source: Arc<dyn PackageFileSource>,
}

impl PackageFiles {
    /// The package-relative paths that start with `folder` (`"data/lumen_demo/lumen/"`), sorted;
    /// `""` lists every file. Only `assets/…` and `data/…` files are listed.
    pub fn list(&self, folder: &str) -> impl Iterator<Item = &str> {
        let start = self.names.partition_point(|name| name.as_str() < folder);
        self.names[start..]
            .iter()
            .take_while(move |name| name.starts_with(folder))
            .map(String::as_str)
    }

    /// The bytes of the file at `path`, a package-relative path [`list`](Self::list) gives.
    /// [`NotFound`](PackageReadError::NotFound) for any other path, `..` and absolute ones
    /// included.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, PackageReadError> {
        self.names
            .binary_search_by(|name| name.as_str().cmp(path))
            .map_err(|_| PackageReadError::NotFound)?;
        self.source.read(path)
    }
}

impl crate::RegistrationContext {
    /// The content files (`assets/…`, `data/…`) of the enabled package `id`, as content loading
    /// sees them. Any enabled package may be read, the caller's own included: the same packages
    /// [`mods`](Self::mods) lists. Available from registration on, before content is loaded;
    /// keep the handle to read later.
    pub fn package_files(&self, id: &str) -> Result<PackageFiles, PackageFilesError> {
        crate::content::crash::current_native_mod().ok_or(PackageFilesError::Unavailable)?;
        let source = self
            .data
            .as_ref()
            .ok_or(PackageFilesError::Unavailable)?
            .package_source(id)
            .ok_or(PackageFilesError::NotEnabled)?;
        let names = source.names().map_err(PackageFilesError::Unreadable)?;
        Ok(PackageFiles { names, source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A package with two scripts and a model, read back as `read:<name>`; `unreadable` has a
    /// file that fails to read.
    struct Fake;
    impl PackageFileSource for Fake {
        fn names(&self) -> Result<Arc<[String]>, String> {
            Ok([
                "assets/demo/models/item/gem.json",
                "data/demo/lumen/a.luau",
                "data/demo/lumen/b.luau",
                "data/demo/lumen_extra/c.luau",
            ]
            .map(String::from)
            .into())
        }
        fn read(&self, name: &str) -> Result<Vec<u8>, PackageReadError> {
            if name == "data/demo/lumen/b.luau" {
                return Err(PackageReadError::Read(format!("{name}: gone")));
            }
            Ok(format!("read:{name}").into_bytes())
        }
    }
    /// A package whose files cannot be listed.
    struct Broken;
    impl PackageFileSource for Broken {
        fn names(&self) -> Result<Arc<[String]>, String> {
            Err("more than 65536 content files".into())
        }
        fn read(&self, _: &str) -> Result<Vec<u8>, PackageReadError> {
            unreachable!("an unlisted package is never read")
        }
    }

    /// A registration context with `demo` and `broken` enabled.
    fn context() -> crate::RegistrationContext {
        let mut sources: BTreeMap<String, Arc<dyn PackageFileSource>> = BTreeMap::new();
        sources.insert("demo".into(), Arc::new(Fake));
        sources.insert("broken".into(), Arc::new(Broken));
        let mut cx = crate::RegistrationContext::new();
        cx.data = Some(
            crate::__private::RegistrationData::new(
                Default::default(),
                Default::default(),
                crate::InstanceDirectory::Unix(b"/instance".to_vec()),
                crate::RuntimeEnvironment::new(
                    crate::Dist::Client,
                    "26.3".into(),
                    "test".into(),
                    false,
                ),
            )
            .with_package_files(sources),
        );
        cx
    }

    /// `f`, run as a call into mod `reader`'s registration.
    fn in_dispatch<T>(f: impl FnOnce() -> T) -> T {
        let outer = crate::content::crash::push(crate::content::crash::ModCall {
            id: "reader",
            hook: "register",
            part: None,
        });
        let out = f();
        crate::content::crash::pop(outer.0, outer.1);
        out
    }

    #[test]
    fn a_mod_lists_a_folder_and_reads_only_listed_files() {
        let cx = context();
        let files = in_dispatch(|| cx.package_files("demo")).unwrap();
        assert_eq!(
            files.list("data/demo/lumen/").collect::<Vec<_>>(),
            ["data/demo/lumen/a.luau", "data/demo/lumen/b.luau"]
        );
        assert_eq!(files.list("").count(), 4);
        assert_eq!(files.list("data/other/").count(), 0);
        assert_eq!(
            files.read("data/demo/lumen/a.luau").unwrap(),
            b"read:data/demo/lumen/a.luau"
        );
        assert_eq!(
            files.read("data/demo/lumen/b.luau"),
            Err(PackageReadError::Read(
                "data/demo/lumen/b.luau: gone".into()
            ))
        );
        for unlisted in [
            "data/demo/lumen/missing.luau",
            "data/demo/lumen/../lumen/a.luau",
            "/data/demo/lumen/a.luau",
            "data\\demo\\lumen\\a.luau",
            "",
        ] {
            assert_eq!(
                files.read(unlisted),
                Err(PackageReadError::NotFound),
                "{unlisted}"
            );
        }
    }

    #[test]
    fn a_package_that_is_not_enabled_or_not_listable_or_a_call_outside_dispatch_is_refused() {
        let cx = context();
        let refused = |result: Result<PackageFiles, PackageFilesError>| result.err();
        assert_eq!(
            refused(in_dispatch(|| cx.package_files("disabled"))),
            Some(PackageFilesError::NotEnabled)
        );
        assert_eq!(
            refused(in_dispatch(|| cx.package_files("broken"))),
            Some(PackageFilesError::Unreadable(
                "more than 65536 content files".into()
            ))
        );
        assert_eq!(
            refused(cx.package_files("demo")),
            Some(PackageFilesError::Unavailable)
        );
        assert_eq!(
            refused(in_dispatch(|| {
                crate::RegistrationContext::new().package_files("demo")
            })),
            Some(PackageFilesError::Unavailable)
        );
    }
}
