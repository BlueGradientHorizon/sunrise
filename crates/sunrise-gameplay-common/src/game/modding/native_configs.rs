//! Native mods' config files, loaded and reloaded (NeoForge's `ModConfig`
//! and `ModConfigEvent`, docs/modding/src/how-to/native-mods.md). A mod lists its configs
//! (`native_register_configs`), each a file of a kind: `Client` on the
//! window, `Common` once per process (the window, or a dedicated server),
//! `Server` on the server for each world it opens, which a world may
//! override in its `serverconfig/` folder. The game writes a missing file's
//! default text, reads it and hands the text to the mod
//! (`native_config_loading`); the mod parses it. Once a second it looks at
//! each file's modification time and hands changed text over again
//! (`native_config_reloading`). A server's configs are dropped when it stops
//! (`native_config_unloading`).

use crate::game::CommonGame;
use mod_api::ConfigPath;
#[allow(unused_imports)]
pub use mod_api::game::ConfigFile;
pub use std::fs;
pub use std::path::{Path, PathBuf};
pub use std::time::{Duration, Instant};

/// How often the files' modification times are read.
pub const CHECK_EVERY: Duration = Duration::from_secs(1);

/// A loaded config, where it was read and what its file looked like then.
pub struct Tracked {
    pub config: ConfigFile,
    pub(in crate::game) path: PathBuf,
}

/// One game's configs: the window's, a server thread's or a dedicated
/// server's.
#[derive(Default)]
pub struct NativeConfigs {
    pub(in crate::game) started: bool,
    pub files: Vec<Tracked>,
    pub(in crate::game) next_check: Option<Instant>,
}

/// The file `config` is read from: the world's `serverconfig/` copy of a
/// server config when there is one, else `config_dir`'s, written from the
/// default when missing.
pub(in crate::game) fn locate(
    config: &mod_api::Config,
    config_dir: &Path,
    world: Option<&Path>,
) -> std::io::Result<PathBuf> {
    if config.kind == mod_api::ConfigKind::Server
        && let Some(world) = world
    {
        let own = world.join("serverconfig").join(&config.file);
        if own.is_file() {
            return Ok(own);
        }
    }
    let path = config_dir.join(&config.file);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &config.default)?;
    }
    Ok(path)
}

impl NativeConfigs {
    /// Reads `configs` of `kinds` (a duplicate path, or a file outside the
    /// mod's own in `config/` as `Host::config_files` reads them, is skipped)
    /// and answers each loaded file.
    pub fn load(
        &mut self,
        configs: Vec<mod_api::Config>,
        kinds: &[mod_api::ConfigKind],
        config_dir: &Path,
        world: Option<&Path>,
    ) -> Vec<ConfigFile> {
        self.started = true;
        let mut loaded = Vec::new();
        for config in configs.into_iter().filter(|c| kinds.contains(&c.kind)) {
            if !ConfigPath::new(&config.file).is_ok_and(|file| file.owned_by(&config.mod_id)) {
                log::warn!(
                    "Sunrise: mod {} config {:?} is not one of its files in config/",
                    config.mod_id,
                    config.file
                );
                continue;
            }
            let path = match locate(&config, config_dir, world) {
                Ok(path) => path,
                Err(e) => {
                    log::warn!("Sunrise: mod {} config {}: {e}", config.mod_id, config.file);
                    continue;
                }
            };
            if self.files.iter().any(|t| t.path == path) {
                log::warn!(
                    "Sunrise: mod {} config {} is registered twice",
                    config.mod_id,
                    path.display()
                );
                continue;
            }
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) => {
                    log::warn!(
                        "Sunrise: mod {} config {}: {e}",
                        config.mod_id,
                        path.display()
                    );
                    continue;
                }
            };
            let file = ConfigFile::from(mod_api::__private::built::ConfigFile {
                mod_id: config.mod_id,
                kind: config.kind,
                file: config.file,
                text,
            });
            loaded.push(file.clone());
            self.files.push(Tracked { config: file, path });
        }
        loaded
    }

    /// The files whose text changed since they were read, read again; a
    /// file that is gone or unreadable keeps its last text. The text is
    /// compared, not the file's modification time: a save inside the same
    /// timestamp tick as the load (a mod writing its config from
    /// `config_loading`) keeps the time and the length and still changed.
    pub fn changed(&mut self) -> Vec<ConfigFile> {
        let mut changed = Vec::new();
        for tracked in &mut self.files {
            let Ok(text) = fs::read_to_string(&tracked.path) else {
                continue;
            };
            if text != tracked.config.text {
                tracked.config.text = text;
                changed.push(tracked.config.clone());
            }
        }
        changed
    }

    /// Whether a second passed since the last look at the files.
    pub fn due(&mut self, now: Instant) -> bool {
        if self.files.is_empty() || self.next_check.is_some_and(|t| now < t) {
            return false;
        }
        self.next_check = Some(now + CHECK_EVERY);
        true
    }

    /// Drops the server's configs and answers them.
    pub fn drop_server(&mut self) -> Vec<ConfigFile> {
        let (server, rest) = std::mem::take(&mut self.files)
            .into_iter()
            .partition(|t| t.config.kind == mod_api::ConfigKind::Server);
        self.files = rest;
        server.into_iter().map(|t| t.config).collect()
    }
}

pub trait NativeConfigsGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// Asks the mods for their configs and loads those of `kinds`.
     fn native_configs_load(
        &mut self,
        kinds: &[mod_api::ConfigKind],
        world: Option<&Path>,
    ) {
        let config_dir = match crate::game::native_config_dir() {
            Ok(dir) => dir,
            Err(e) => {
                log::warn!("Sunrise: mod configs: {e}");
                return;
            }
        };
        let mut configs = Vec::new();
        self.native_register_configs(&mut configs);
        if configs.is_empty() {
            self.native_configs.started = true;
            return;
        }
        for file in self.native_configs.load(configs, kinds, &config_dir, world) {
            self.native_config_loading(&file);
        }
    }

    /// The window's `Client` and `Common` configs, once: after client setup
    /// and before its title, connection or ordinary runtime callbacks.
     fn native_configs_client_start(&mut self) {
        if !self.native_configs.started {
            use mod_api::ConfigKind::{Client, Common};
            self.native_configs_load(&[Client, Common], None);
        }
    }

    /// A server about to open `world`: a dedicated server loads its
    /// `Common` configs too, a single player's server thread only `Server`.
     fn native_configs_server_start(&mut self, world: &Path, dedicated: bool) {
        use mod_api::ConfigKind::{Common, Server};
        let kinds: &[mod_api::ConfigKind] = if dedicated && !self.native_configs.started {
            &[Common, Server]
        } else {
            &[Server]
        };
        self.native_configs_load(kinds, Some(world));
    }

    /// Hands changed files over again, at most once a second.
     fn native_configs_poll(&mut self) {
        if !self.native_configs.due(Instant::now()) {
            return;
        }
        for file in self.native_configs.changed() {
            self.native_config_reloading(&file);
        }
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeConfigsGameExt1<'__sg> for G {}


use crate::game::native_hooks::mod_configs::GameHooks as _;
