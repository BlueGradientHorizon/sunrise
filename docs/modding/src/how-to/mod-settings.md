# Config files and settings

A mod keeps its options in one of two ways:

| | Config files | Settings |
| --- | --- | --- |
| Lives in | The game's `config/` folder (on a server, `serverconfig/` per world) | `sunrise-mods/.sunrise-mod-data/<mod id>/`, one file per key |
| Edited by | The player, in a text editor | The player, on the mod's page in **Options > Mods** |
| You declare it with | `Registrar::register_config` | `Registrar::register_setting` |
| You read it in | The config's `ConfigListener` (`ConfigEvent::Loading`, `Reloading`, `Unloading`) | `Host::setting` and the typed `setting_*` reads |
| NeoForge counterpart | `ModContainer.registerConfig`, `ModConfigEvent` | `ModConfigSpec` entries on the config screen |

## Config files

The startup registrar accepts `register_config(ConfigSpec::new(kind, default_text), listener)` in
`Hooks::register`. The default filename uses the registering mod's ID (`<mod id>-<kind>.toml`), and
`.file(path)` names a different native path. A filename already registered by any mod or config kind returns
`ConfigRegistrationError::Duplicate` and keeps the first declaration. The optional `Arc<ConfigListener>` is
bound to that config's owner and receives only its own `ConfigEvent` and `&ConfigSnapshot` with the owning
`mod_id`, `kind`, registered `file`, resolved on-disk `path`, and `text`.

Config files sit in the game's `config/` folder, beside `options.txt` (beside `server.properties` on a
dedicated server), as Fabric and NeoForge mods' do.

1. In `Hooks::register`, call `r.register_config(ConfigSpec::new(kind, default_text), listener)` for each
   `Client`, `Common` or `Server` config. Declarations freeze in registration order before file I/O.
2. When an owning side loads its configs (`Client` and `Common` once on the window; `Common` once and
   `Server` per world on a dedicated server; `Server` per world on an integrated server), the game selects an
   existing world `serverconfig/` override for `Server` configs when present, or the instance `config/` path
   otherwise, writes the default text only for a missing base file, reads the file, and calls only that
   registration's listener with `ConfigEvent::Loading`.
3. Once a second the game checks tracked files' text; when a tracked file's text changed, it updates the
   snapshot and calls only that registration's listener with `ConfigEvent::Reloading`. Unchanged files fire
   nothing. An actual I/O failure (including a later missing or unreadable file) is logged with owner and path
   and keeps the last successfully read snapshot without substituting another path, retrying, or latching.
4. When a server stops, each tracked `Server` config is dropped and only its registration's listener receives
   `ConfigEvent::Unloading` with its last snapshot. `Client` and `Common` configs stay until the game quits.
   Returning `Err(message)` from a listener logs the error on every call without disabling future deliveries;
   a panic disables only that registration for the rest of the session and raises an error notice naming that
   mod without aborting server shutdown.

`ConfigEvent::Loading` runs in the Config [load phase](../concepts/load-phases.md): settings reads and config
files are available, world handles and transactions are not. Polling leaves the current phase unchanged and
`ConfigEvent::Unloading` runs in World.

For files the hooks don't manage, `Host::config_files()` gives your mod its own files in `config/`. Paths retain
the ownership spelling above, such as `<mod id>/extra.toml`. The service is bound to the mod whose hook or Part
is running; you cannot reach another mod's files.

## Bulk data

Keep map tiles, waypoints, seeds and exports out of settings and config files:

- `Host::data_files()` is available on both sides, rooted at `<game directory>/sunrise-mod-data/<mod id>/`.
  A client-only mod can key its own subfolders by server or save and dimension.
- `ServerHost::world_data_files()` is server-only, rooted at `<world>/sunrise-mod-data/<mod id>/`.
  Its contents are copied, deleted and backed up with the save. Without a current world it refuses calls with
  `ConfigFileError::NoWorld`; a client has no world-folder service.

Both use the same `ConfigFiles`, `ConfigPath`, `ConfigEntry` and `ConfigFileError` types as config files. Bulk paths are relative to your implicit mod root: `tiles/overworld/0.bin`, not
`<mod id>/tiles/overworld/0.bin`. The game never scans bulk data as settings or config.

`read`, `write` and `stat` operate on checked `/`-separated paths; traversal (`..`), backslashes and symbolic links
are refused. Writes replace the whole file atomically, never exposing partial contents. Sunrise's own per-file
limits are `DATA_FILE_MAX` (64 MiB) for both bulk areas and `CONFIG_FILE_MAX` (4 MiB) for config files.
Oversized files and unavailable operations return typed errors, rather than partially writing.
`remove` deletes one file under the same confinement and returns whether it was there; it refuses a folder
with `ConfigFileError::NotAFile` and never removes one.

Walk a folder with `list(&folder, after, limit)`: start with `after = None`, then pass the last returned entry's
name as the exclusive cursor. Names are lexicographically ordered; each page has at most `limit` entries.
`limit` must be between 1 and `CONFIG_LIST_MAX` (1024), or the call returns `ConfigFileError::InvalidPage`.
Use `ConfigPath::root()` to list your own root, and stop when a page is empty.

## Settings

Register each setting from `Hooks::register` with `Registrar::register_setting(SettingSpec { … })`: a key
(`a-z`, `0-9`, `_`, `-` and `.`, not starting with `.`), translation keys for its label and tooltip, and its kind
with the default. A setting the spec check refuses, or a key you registered already, comes back as the error.
Settings are client settings and appear on your mod's page in **Options > Mods**, opened from its **Config**
button.

- Read a setting with `host.setting(mod_id, key)` or a typed read. Typed reads are cached per data folder and key.
- `host.set_setting(mod_id, key, value)` keeps a value. A successful write updates the cache; a failed one leaves
  it. This write does not fire `setting_changed`.
- When the player edits a setting, the value is kept first, then `setting_changed` tells your mod alone.

A mod reads and writes only its own settings: another mod's id, or a key its owner never registered, reads
`None` and writes nothing.

The settings folder is settings only, not bulk storage: ordinary files there are read as setting keys and
subfolders or unsafe entries still warn during snapshots. Use the bulk services above instead; their files are
not included in settings snapshots.
