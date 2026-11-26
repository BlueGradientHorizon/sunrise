//! One checked implementation for config, game bulk data and world bulk data.
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, Metadata, OpenOptions};
use mod_api::{
    CONFIG_FILE_MAX, CONFIG_LIST_MAX, ConfigEntry, ConfigEntryKind, ConfigFileError, ConfigFiles,
    ConfigPath, ConfigStat, DATA_FILE_MAX,
};
use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

struct GameConfigFiles {
    bulk: bool,
}

static CONFIG_FILES: GameConfigFiles = GameConfigFiles { bulk: false };
static DATA_FILES: GameConfigFiles = GameConfigFiles { bulk: true };

/// A root selected by the game, never supplied by a mod. Bulk roots bind the
/// mod's directory implicitly; config keeps its established shared spelling.
pub(in crate::game) struct FileRoot {
    pub(in crate::game) path: PathBuf,
    pub(in crate::game) bulk: bool,
}

impl FileRoot {
    fn cap(&self) -> u64 {
        if self.bulk {
            DATA_FILE_MAX
        } else {
            CONFIG_FILE_MAX
        }
    }
}

/// The calling mod's own files in the game's `config/` folder, beside
/// `options.txt` (beside `server.properties` on a dedicated server):
/// its folder `config/<mod id>/` and the files beside it named
/// `<mod id>.<ext>` or `<mod id>-<rest>`, as Fabric and NeoForge mods
/// keep theirs. The service is bound to the mod whose hook or part is
/// running, so a mod never names its id and can't reach another mod's
/// files; a world's `serverconfig/` is read only through
/// `config_loading`.
pub(in crate::game) fn config_files() -> &'static dyn ConfigFiles {
    &CONFIG_FILES
}
pub(in crate::game) fn data_files() -> &'static dyn ConfigFiles {
    &DATA_FILES
}

/// Whether a mod with the `sunrise-mod.toml` id `id` is installed: a
/// native or content package (Fabric's `FabricLoader.isModLoaded`,
/// NeoForge's `ModList.isLoaded`). The list is fixed at startup.
pub(in crate::game) fn is_loaded(id: &str) -> bool {
    mod_api::__private::loaded_mods().get(id).is_some()
}

fn io(error: std::io::Error) -> ConfigFileError {
    match error.kind() {
        ErrorKind::NotFound => ConfigFileError::NotFound,
        _ => ConfigFileError::Io(error.to_string()),
    }
}

/// Open every directory without following links. Keeping the parent handle
/// makes subsequent opens and atomic replacement safe even if a directory is
/// renamed or replaced between operations.
fn resolve(
    root: &FileRoot,
    path: &ConfigPath,
    create: bool,
) -> Result<(Dir, String), ConfigFileError> {
    let mod_id =
        mod_api::content::crash::current_native_mod().ok_or(ConfigFileError::NoCallingMod)?;
    if !root.bulk && !path.owned_by(mod_id) {
        return Err(ConfigFileError::NotOwned);
    }
    let mut directory =
        Dir::open_ambient_dir(&root.path, cap_std::ambient_authority()).map_err(io)?;
    let mut components = root
        .bulk
        .then_some("sunrise-mod-data")
        .into_iter()
        .chain(root.bulk.then_some(mod_id))
        .chain(path.as_str().split('/').filter(|part| !part.is_empty()))
        .peekable();
    while let Some(part) = components.next() {
        match directory.symlink_metadata(part) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(ConfigFileError::Symlink),
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(io(e)),
        }
        if components.peek().is_none() {
            return Ok((directory, part.to_owned()));
        }
        if create {
            match directory.create_dir(part) {
                Ok(()) => {}
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
                Err(e) => return Err(io(e)),
            }
        }
        directory = directory.open_dir_nofollow(part).map_err(io)?;
    }
    Err(ConfigFileError::NotOwned)
}

fn stat_of(meta: &Metadata) -> ConfigStat {
    let modified_millis = meta
        .modified()
        .ok()
        .and_then(|t| {
            t.duration_since(cap_std::time::SystemTime::from_std(UNIX_EPOCH))
                .ok()
        })
        .map_or(0, |d| d.as_millis() as u64);
    let (kind, len) = if meta.is_dir() {
        (ConfigEntryKind::Folder, 0)
    } else {
        (ConfigEntryKind::File, meta.len())
    };
    ConfigStat::from(mod_api::__private::built::ConfigStat {
        kind,
        len,
        modified_millis,
    })
}

pub(in crate::game) fn read(
    root: &FileRoot,
    file: &ConfigPath,
) -> Result<Vec<u8>, ConfigFileError> {
    let (directory, name) = resolve(root, file, false)?;
    let meta = directory.symlink_metadata(&name).map_err(io)?;
    if !meta.is_file() {
        return Err(ConfigFileError::NotAFile);
    }
    if meta.len() > root.cap() {
        return Err(ConfigFileError::TooLarge);
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let opened = directory.open_with(&name, &options).map_err(io)?;
    if !opened.metadata().map_err(io)?.is_file() {
        return Err(ConfigFileError::NotAFile);
    }
    let mut bytes = Vec::new();
    // One byte past the cap detects growth without an unbounded allocation.
    opened
        .take(root.cap() + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() as u64 > root.cap() {
        return Err(ConfigFileError::TooLarge);
    }
    Ok(bytes)
}

/// Neither the implicit bulk root nor a config mod folder can be a file.
fn refuse_root_folder(root: &FileRoot, file: &ConfigPath) -> Result<(), ConfigFileError> {
    if file.as_str().is_empty()
        || (!root.bulk && Some(file.as_str()) == mod_api::content::crash::current_native_mod())
    {
        return Err(ConfigFileError::NotAFile);
    }
    Ok(())
}

/// What is at `name` in `directory`, without following a link; None where nothing is.
fn file_meta(directory: &Dir, name: &str) -> Result<Option<Metadata>, ConfigFileError> {
    match directory.symlink_metadata(name) {
        Ok(meta) if meta.file_type().is_symlink() => Err(ConfigFileError::Symlink),
        Ok(meta) => Ok(Some(meta)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(e)),
    }
}

/// Whether `name` is the temporary file of a write, `<name>.<pid>-<n>.tmp`.
fn is_write_temp(name: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    name.strip_suffix(".tmp")
        .and_then(|rest| rest.rsplit_once('.'))
        .and_then(|(_, tail)| tail.split_once('-'))
        .is_some_and(|(pid, n)| digits(pid) && digits(n))
}

/// Written beside the file, then renamed over it. The temporary name starts
/// with the file's own, so it stays the mod's own; `list` leaves it out, so a
/// scan never meets a half-written file, nor one a crash left behind.
pub(in crate::game) fn write(
    root: &FileRoot,
    file: &ConfigPath,
    bytes: &[u8],
) -> Result<(), ConfigFileError> {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    if bytes.len() as u64 > root.cap() {
        return Err(ConfigFileError::TooLarge);
    }
    refuse_root_folder(root, file)?;
    let (directory, name) = resolve(root, file, true)?;
    match file_meta(&directory, &name)? {
        Some(meta) if !meta.is_file() => return Err(ConfigFileError::NotAFile),
        _ => {}
    }
    let temp = format!(
        "{name}.{}-{}.tmp",
        std::process::id(),
        WRITES.fetch_add(1, Ordering::Relaxed)
    );
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let written = directory
        .open_with(&temp, &options)
        .and_then(|mut f| f.write_all(bytes).and_then(|()| f.sync_all()))
        .and_then(|()| directory.rename(&temp, &directory, &name));
    if written.is_err() {
        let _ = directory.remove_file(&temp);
    }
    written.map_err(io)
}

pub(in crate::game) fn remove(root: &FileRoot, file: &ConfigPath) -> Result<bool, ConfigFileError> {
    refuse_root_folder(root, file)?;
    let (directory, name) = match resolve(root, file, false) {
        Err(ConfigFileError::NotFound) => return Ok(false),
        resolved => resolved?,
    };
    match file_meta(&directory, &name)? {
        None => Ok(false),
        Some(meta) if !meta.is_file() => Err(ConfigFileError::NotAFile),
        Some(_) => match directory.remove_file(&name) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io(e)),
        },
    }
}

pub(in crate::game) fn list(
    root: &FileRoot,
    folder: &ConfigPath,
    after: Option<&str>,
    limit: u32,
) -> Result<Vec<ConfigEntry>, ConfigFileError> {
    if limit == 0
        || limit as usize > CONFIG_LIST_MAX
        || after.is_some_and(|name| !ConfigPath::is_valid_name(name))
    {
        return Err(ConfigFileError::InvalidPage);
    }
    let (parent, name) = resolve(root, folder, false)?;
    if !parent.symlink_metadata(&name).map_err(io)?.is_dir() {
        return Err(ConfigFileError::NotAFolder);
    }
    let directory = parent.open_dir_nofollow(&name).map_err(io)?;
    // Keep only the next bounded page, not every tile in the directory.
    let mut entries = BTreeMap::new();
    for entry in directory.entries().map_err(io)? {
        let entry = entry.map_err(io)?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if after.is_some_and(|cursor| name.as_str() <= cursor)
            || is_write_temp(&name)
            || !ConfigPath::is_valid_name(&name)
            || (entries.len() == limit as usize
                && entries
                    .last_key_value()
                    .is_some_and(|(last, _)| &name >= last))
        {
            continue;
        }
        let meta = directory.symlink_metadata(&name).map_err(io)?;
        if !meta.is_file() && !meta.is_dir() {
            continue;
        }
        entries.insert(name, stat_of(&meta));
        if entries.len() > limit as usize {
            entries.pop_last();
        }
    }
    Ok(entries
        .into_iter()
        .map(|(name, stat)| {
            ConfigEntry::from(mod_api::__private::built::ConfigEntry { name, stat })
        })
        .collect())
}

pub(in crate::game) fn stat(
    root: &FileRoot,
    path: &ConfigPath,
) -> Result<Option<ConfigStat>, ConfigFileError> {
    match resolve(root, path, false).and_then(|(dir, name)| dir.symlink_metadata(name).map_err(io))
    {
        Ok(meta) if meta.file_type().is_symlink() => Err(ConfigFileError::Symlink),
        Ok(meta) => Ok(Some(stat_of(&meta))),
        Err(ConfigFileError::NotFound) => Ok(None),
        Err(e) => Err(e),
    }
}

impl GameConfigFiles {
    fn root(&self) -> Result<FileRoot, ConfigFileError> {
        Ok(FileRoot {
            path: if self.bulk {
                crate::game::option_paths::game_dir()
            } else {
                crate::game::native_config_dir().map_err(io)?
            },
            bulk: self.bulk,
        })
    }
}

macro_rules! files {
    ($owner:ty, $root:expr) => {
        impl ConfigFiles for $owner {
            fn read(&self, file: &ConfigPath) -> Result<Vec<u8>, ConfigFileError> {
                read(&$root(self)?, file)
            }
            fn write(&self, file: &ConfigPath, bytes: &[u8]) -> Result<(), ConfigFileError> {
                write(&$root(self)?, file, bytes)
            }
            fn list(
                &self,
                folder: &ConfigPath,
                after: Option<&str>,
                limit: u32,
            ) -> Result<Vec<ConfigEntry>, ConfigFileError> {
                list(&$root(self)?, folder, after, limit)
            }
            fn stat(&self, path: &ConfigPath) -> Result<Option<ConfigStat>, ConfigFileError> {
                stat(&$root(self)?, path)
            }
            fn remove(&self, file: &ConfigPath) -> Result<bool, ConfigFileError> {
                remove(&$root(self)?, file)
            }
        }
    };
}

files!(GameConfigFiles, GameConfigFiles::root);
