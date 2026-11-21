use super::*;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use mod_api::__private::game::CallbackOwner;
use mod_api::{SettingAccessError, SettingKey, SettingKind, SettingSpec, SettingValue};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{ErrorKind, Write};
use std::sync::atomic::{AtomicU64, Ordering};

/// The single client-owned store for native mod settings registered through
/// `SettingsRegistrar::register_setting`.
///
/// Live values are initialized from the resource generation's `LoopContext`
/// snapshot, updated atomically on Options > Mods writes and `set_setting`
/// calls, refreshed on pack/resource reloads from that reload's captured
/// `LoopContext` snapshot (never a second disk scan), and refreshed for a
/// single mod when its Options > Mods page opens.
#[derive(Default)]
pub(in crate::game) struct NativeSettings {
    /// Cached live values by `(owner, key)` for every registered setting.
    values: BTreeMap<(&'static str, &'static str), SettingValue>,
    /// Whether live values have been populated from a captured generation yet.
    initialized: bool,
    /// Per-owner panic containment for `setting_changed` dispatch.
    /// A panic disables only that mod's `setting_changed` callback for the
    /// session; phase refusals report each time without disabling it.
    callback_owners: BTreeMap<&'static str, CallbackOwner>,
}

impl NativeSettings {
    /// Populate live values from a captured `LoopContext` snapshot.
    ///
    /// On the first call (initial load), establishes the baseline values and
    /// returns no change notifications. On subsequent calls (reloads), updates
    /// live values from the same captured snapshot and returns each `(owner, key)`
    /// whose effective value changed (comparing `Range` floats by bits so
    /// `-0.0` and `+0.0` are distinct).
    pub(in crate::game) fn sync_from_loop_context(
        &mut self,
        context: &sunrise_world::native_load_context::LoopContext,
    ) -> Vec<(&'static str, &'static str)> {
        let mut changed = Vec::new();
        let notify = self.initialized;
        self.initialized = true;
        let mut next = BTreeMap::new();
        for (owner, spec) in mod_api::__private::settings() {
            let key = spec.key.as_str();
            let raw = context.setting(owner, key);
            let value = spec.read(raw);
            if notify {
                let previous = self
                    .values
                    .get(&(*owner, key))
                    .copied()
                    .unwrap_or_else(|| spec.default_value());
                if !previous.exact_eq(value) {
                    changed.push((*owner, key));
                }
            }
            next.insert((*owner, key), value);
        }
        self.values = next;
        changed
    }

    /// Refresh one mod's live values from disk using the same symlink-refusing
    /// regular-file admission policy as load snapshots (`capture_settings`).
    /// Returns every key of `owner` whose effective value changed.
    pub(in crate::game) fn refresh_owner_from_disk(
        &mut self,
        root: &Path,
        owner: &str,
    ) -> Vec<(&'static str, &'static str)> {
        let captured = sunrise_world::native_load_context::capture_settings(root, [owner]);
        let owner_map = captured.get(owner);
        let notify = self.initialized;
        let mut changed = Vec::new();
        for (registered_owner, spec) in mod_api::__private::settings() {
            if *registered_owner != owner {
                continue;
            }
            let key = spec.key.as_str();
            let raw = owner_map.and_then(|map| map.get(key)).map(String::as_str);
            let value = spec.read(raw);
            let slot = (*registered_owner, key);
            let previous = self
                .values
                .get(&slot)
                .copied()
                .unwrap_or_else(|| spec.default_value());
            self.values.insert(slot, value);
            if notify && !previous.exact_eq(value) {
                changed.push(slot);
            }
        }
        changed
    }

    /// Read a registered setting's current live value without touching the
    /// filesystem (falling back to the spec's default if the store has not yet
    /// been initialized).
    pub(in crate::game) fn value(
        &self,
        owner: &'static str,
        spec: &'static SettingSpec,
    ) -> SettingValue {
        self.values
            .get(&(owner, spec.key.as_str()))
            .copied()
            .unwrap_or_else(|| spec.default_value())
    }

    /// Validate, persist, and publish a new value for a registered setting.
    ///
    /// Returns `Ok(true)` when the value changed (by `SettingValue::exact_eq`),
    /// `Ok(false)` when it already matched without writing or notifying, or an
    /// explicit `SettingAccessError` when validation or persistence failed
    /// (keeping the previous live value untouched on failure).
    fn commit(
        &mut self,
        root: &Path,
        owner: &'static str,
        spec: &'static SettingSpec,
        value: SettingValue,
    ) -> Result<bool, SettingAccessError> {
        let encoded = encode_setting(spec, value)?;
        let current = self.value(owner, spec);
        if current.exact_eq(value) {
            return Ok(false);
        }
        persist_setting(root, owner, &spec.key, &encoded)
            .map_err(|_| SettingAccessError::IoFailed)?;
        self.values.insert((owner, spec.key.as_str()), value);
        Ok(true)
    }

    /// Return the session-scoped `CallbackOwner` for `owner`'s `setting_changed`
    /// callback.
    pub(in crate::game) fn callback_owner(&mut self, owner: &'static str) -> CallbackOwner {
        self.callback_owners
            .entry(owner)
            .or_insert_with(|| CallbackOwner::for_mod(owner))
            .clone()
    }
}

/// Check that `value` has the right kind and falls within `spec`'s registered
/// bounds, returning its canonical persisted string form.
fn encode_setting(spec: &SettingSpec, value: SettingValue) -> Result<String, SettingAccessError> {
    let kind_matches = matches!(
        (&spec.kind, value),
        (SettingKind::Toggle { .. }, SettingValue::Toggle(_))
            | (SettingKind::Choice { .. }, SettingValue::Choice(_))
            | (SettingKind::Range { .. }, SettingValue::Range(_))
            | (SettingKind::Color { .. }, SettingValue::Color(_))
    );
    if !kind_matches {
        return Err(SettingAccessError::WrongKind);
    }
    spec.store(value).ok_or(SettingAccessError::InvalidValue)
}

/// Validate that `component` is a single safe normal path component.
fn is_plain_component(component: &str) -> bool {
    let mut components = Path::new(component).components();
    matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
}

/// Persist `value` at `<root>/<owner>/<key>` without following symlinks at the
/// root, mod directory, or setting file, writing to a temporary sibling file
/// and atomically renaming it into place.
fn persist_setting(root: &Path, owner: &str, key: &str, value: &str) -> std::io::Result<()> {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    if !is_plain_component(owner) || !is_plain_component(key) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "unsafe setting path component",
        ));
    }
    let root_dir = open_or_create_dir_nofollow(root)?;
    let owner_dir = open_or_create_child_dir_nofollow(&root_dir, owner)?;
    match owner_dir.symlink_metadata(key) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "setting target is not a regular file",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let temp = format!(
        ".{key}.{}-{}.tmp",
        std::process::id(),
        WRITES.fetch_add(1, Ordering::Relaxed)
    );
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let result = owner_dir
        .open_with(&temp, &options)
        .and_then(|mut file| {
            file.write_all(value.as_bytes())
                .and_then(|()| file.sync_all())
        })
        .and_then(|()| owner_dir.rename(&temp, &owner_dir, key));
    if result.is_err() {
        let _ = owner_dir.remove_file(&temp);
    }
    result
}

/// Ensure `root` exists (creating ancestors if needed) and open it as a
/// capability handle without following a symlink at `root` itself.
fn open_or_create_dir_nofollow(root: &Path) -> std::io::Result<Dir> {
    let parent = root.parent().ok_or_else(|| {
        std::io::Error::new(ErrorKind::InvalidInput, "settings root has no parent")
    })?;
    let name = root
        .file_name()
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidInput, "settings root has no name"))?;
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)?;
    }
    let parent_dir = Dir::open_ambient_dir(
        if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        },
        cap_std::ambient_authority(),
    )?;
    open_or_create_child_dir_nofollow(&parent_dir, name)
}

fn open_or_create_child_dir_nofollow(parent: &Dir, name: impl AsRef<Path>) -> std::io::Result<Dir> {
    let name = name.as_ref();
    match parent.symlink_metadata(name) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "settings directory is a symlink or not a directory",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => match parent.create_dir(name) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        },
        Err(error) => return Err(error),
    }
    parent.open_dir_nofollow(name)
}

/// Collect the mod ids whose settings should be captured in a client
/// `LoopContext` snapshot: enabled client mods plus any mod with registered
/// client settings.
pub(in crate::game) fn client_setting_owners<'a>(
    enabled_client_ids: impl IntoIterator<Item = &'a str>,
) -> Vec<&'a str> {
    let mut owners: BTreeSet<&'a str> = enabled_client_ids.into_iter().collect();
    for (owner, _) in mod_api::__private::settings() {
        owners.insert(*owner);
    }
    owners.into_iter().collect()
}

/// Convert a `(spec, value)` pair to the Options > Mods widget value.
pub(super) fn ui_setting_value(spec: &SettingSpec, value: SettingValue) -> NativeSettingValue {
    match (&spec.kind, value) {
        (_, SettingValue::Toggle(on)) => NativeSettingValue::Toggle(on),
        (SettingKind::Choice { options, .. }, SettingValue::Choice(index)) => {
            NativeSettingValue::Choice {
                options: options.clone(),
                index,
            }
        }
        (SettingKind::Range { min, max, step, .. }, SettingValue::Range(value)) => {
            NativeSettingValue::Range {
                min: *min,
                max: *max,
                step: *step,
                value,
            }
        }
        (_, SettingValue::Color(rgb)) => NativeSettingValue::Color(rgb),
        _ => NativeSettingValue::Toggle(false),
    }
}

/// Typed client settings reads and writes on `Game` using owner-bound
/// `SettingKey` handles (vanilla `OptionInstance`, NeoForge `ModConfigSpec`).
impl Game<'_> {
    /// Ensure a client `Game` has initialized `native_settings` from its
    /// active `loop_context` before reading or mutating.
    fn ensure_native_settings_initialized(&self) -> bool {
        self.options.execution_role.is_client()
    }

    fn resolve_setting_key(
        &self,
        key: SettingKey,
    ) -> Result<(&'static str, &'static SettingSpec), SettingAccessError> {
        if !self.ensure_native_settings_initialized() {
            return Err(SettingAccessError::Unavailable);
        }
        mod_api::__private::setting_entry(key)
            .map(|(owner, spec)| (*owner, spec))
            .ok_or(SettingAccessError::UnknownKey)
    }

    /// Read a client setting through its owner-bound `SettingKey`.
    pub(in crate::game) fn setting(
        &self,
        key: SettingKey,
    ) -> Result<SettingValue, SettingAccessError> {
        let (owner, spec) = self.resolve_setting_key(key)?;
        Ok(self.native_settings.value(owner, spec))
    }

    /// Write a client setting through its owner-bound `SettingKey`, persisting
    /// to disk, updating live state and any open Options > Mods row, and
    /// notifying the owning mod synchronously on change.
    pub(in crate::game) fn set_setting(
        &mut self,
        key: SettingKey,
        value: SettingValue,
    ) -> Result<bool, SettingAccessError> {
        let (owner, spec) = self.resolve_setting_key(key)?;
        self.apply_native_setting_write(&Self::mod_data_root(), owner, spec, value)
    }

    /// Read a toggle setting through its owner-bound `SettingKey`.
    pub(in crate::game) fn setting_bool(
        &self,
        key: SettingKey,
    ) -> Result<bool, SettingAccessError> {
        match self.setting(key)? {
            SettingValue::Toggle(on) => Ok(on),
            _ => Err(SettingAccessError::WrongKind),
        }
    }

    /// Read a choice setting's index through its owner-bound `SettingKey`.
    pub(in crate::game) fn setting_choice(
        &self,
        key: SettingKey,
    ) -> Result<u32, SettingAccessError> {
        match self.setting(key)? {
            SettingValue::Choice(index) => {
                u32::try_from(index).map_err(|_| SettingAccessError::InvalidValue)
            }
            _ => Err(SettingAccessError::WrongKind),
        }
    }

    /// Read a range setting's number through its owner-bound `SettingKey`.
    pub(in crate::game) fn setting_f64(&self, key: SettingKey) -> Result<f64, SettingAccessError> {
        match self.setting(key)? {
            SettingValue::Range(value) => Ok(value),
            _ => Err(SettingAccessError::WrongKind),
        }
    }

    /// Read a colour setting (`0xRRGGBB`) through its owner-bound `SettingKey`.
    pub(in crate::game) fn setting_color(
        &self,
        key: SettingKey,
    ) -> Result<u32, SettingAccessError> {
        match self.setting(key)? {
            SettingValue::Color(rgb) => Ok(rgb),
            _ => Err(SettingAccessError::WrongKind),
        }
    }

    /// Shared commit path for both `ClientHost::set_setting` and Options > Mods
    /// UI writes: validates, persists, updates live state + UI row, and fires
    /// `setting_changed` synchronously through per-owner panic containment.
    pub(super) fn apply_native_setting_write(
        &mut self,
        root: &Path,
        owner: &'static str,
        spec: &'static SettingSpec,
        value: SettingValue,
    ) -> Result<bool, SettingAccessError> {
        let changed = self.native_settings.commit(root, owner, spec, value)?;
        if changed {
            self.sync_native_setting_ui_row(owner, spec, value);
            self.notify_setting_changed(owner, spec.key.as_str());
        }
        Ok(changed)
    }

    /// Synchronize the live client settings store from the active
    /// `LoopContext` snapshot (single capture per reload) and notify owners of
    /// every value that changed on reload.
    pub(in crate::game) fn sync_native_settings_from_loop_context(&mut self) {
        if !self.options.execution_role.is_client() {
            return;
        }
        let was_initialized = self.native_settings.initialized;
        let changed = self
            .native_settings
            .sync_from_loop_context(&self.loop_context);
        if !was_initialized {
            // Initial startup sync: align any Options > Mods rows built during
            // `start_mods()` with the captured baseline without firing
            // `setting_changed`.
            for (owner, spec) in mod_api::__private::settings() {
                let value = self.native_settings.value(owner, spec);
                self.sync_native_setting_ui_row(owner, spec, value);
            }
            return;
        }
        for (owner, key) in changed {
            if let Some(spec) = mod_api::__private::setting(owner, key) {
                let value = self.native_settings.value(owner, spec);
                self.sync_native_setting_ui_row(owner, spec, value);
            }
            self.notify_setting_changed(owner, key);
        }
    }

    /// Refresh one mod's live settings from disk (used when opening that mod's
    /// Options > Mods page from either Options > Mods or the recipe browser)
    /// and notify on any values that changed externally.
    pub(super) fn refresh_native_settings_owner(&mut self, root: &Path, owner: &str) {
        if !self.options.execution_role.is_client() {
            return;
        }
        let changed = self.native_settings.refresh_owner_from_disk(root, owner);
        for (changed_owner, key) in changed {
            if let Some(spec) = mod_api::__private::setting(changed_owner, key) {
                let value = self.native_settings.value(changed_owner, spec);
                self.sync_native_setting_ui_row(changed_owner, spec, value);
            }
            self.notify_setting_changed(changed_owner, key);
        }
    }

    /// Keep the Options > Mods row for `(owner, spec.key)` aligned with `value`
    /// whenever the setting changes through code or a reload.
    fn sync_native_setting_ui_row(&mut self, owner: &str, spec: &SettingSpec, value: SettingValue) {
        let Some(row) = self
            .settings
            .mods
            .installed
            .iter_mut()
            .find(|row| row.id == owner)
        else {
            return;
        };
        let Some(page) = row.native.as_mut() else {
            return;
        };
        if let Some(setting) = page.settings.iter_mut().find(|s| s.key == spec.key) {
            setting.value = ui_setting_value(spec, value);
        }
    }

    /// Deliver `setting_changed` synchronously for `owner`, contained per owner
    /// so a panic disables only that mod's setting callback for the session and
    /// surfaces an error toast, while phase refusals report without disabling.
    pub(super) fn notify_setting_changed(&mut self, owner: &'static str, key: &str) {
        let callback_owner = self.native_settings.callback_owner(owner);
        let _ = callback_owner.call("setting_changed", "setting_changed", || {
            self.native_setting_changed(owner, key);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{
        ClientHost, Hooks, ModEntry, Registrar, RegistrationContext, SettingAccessError,
        SettingKey, SettingSpec, SettingValue,
    };
    use std::sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use sunrise_world::native_load_context::{DataLoadOrigin, LoadContext, LoadInputs, LoopSide};

    struct RegisteredMod {
        toggle: OnceLock<SettingKey>,
        range: OnceLock<SettingKey>,
        choice: OnceLock<SettingKey>,
        color: OnceLock<SettingKey>,
        seen: Arc<Mutex<Vec<(String, SettingValue)>>>,
        panic_on_change: Arc<AtomicBool>,
    }

    impl RegisteredMod {
        fn new() -> Self {
            Self {
                toggle: OnceLock::new(),
                range: OnceLock::new(),
                choice: OnceLock::new(),
                color: OnceLock::new(),
                seen: Arc::new(Mutex::new(Vec::new())),
                panic_on_change: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    #[mod_api::__subscriptions]
    impl Hooks for RegisteredMod {
        fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
            let _ = self.toggle.set(
                r.register_setting(SettingSpec::toggle("on", "test.on", false))
                    .unwrap(),
            );
            let _ = self.range.set(
                r.register_setting(SettingSpec::range(
                    "scale",
                    "test.scale",
                    -2.0,
                    2.0,
                    0.5,
                    0.0,
                ))
                .unwrap(),
            );
            let _ = self.choice.set(
                r.register_setting(SettingSpec::choice("mode", "test.mode", ["a", "b"], 0))
                    .unwrap(),
            );
            let _ = self.color.set(
                r.register_setting(SettingSpec::color("tint", "test.tint", 0x11_2233))
                    .unwrap(),
            );
        }

        fn setting_changed(
            &self,
            host: &mut dyn ClientHost,
            event: &mut mod_api::SettingChangedEvent<'_>,
        ) {
            if self.panic_on_change.load(Ordering::Relaxed) {
                panic!("simulated setting_changed failure");
            }
            let key = match event.key {
                "on" => *self.toggle.get().unwrap(),
                "scale" => *self.range.get().unwrap(),
                "mode" => *self.choice.get().unwrap(),
                "tint" => *self.color.get().unwrap(),
                other => panic!("unexpected setting key {other}"),
            };
            let current = host.setting(key).unwrap();
            self.seen
                .lock()
                .unwrap()
                .push((event.key.to_owned(), current));
        }
    }

    struct OtherMod {
        toggle: OnceLock<SettingKey>,
        calls: Arc<AtomicUsize>,
    }

    #[mod_api::__subscriptions]
    impl Hooks for OtherMod {
        fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
            let _ = self.toggle.set(
                r.register_setting(SettingSpec::toggle("on", "other.on", true))
                    .unwrap(),
            );
        }

        fn setting_changed(
            &self,
            _: &mut dyn ClientHost,
            _: &mut mod_api::SettingChangedEvent<'_>,
        ) {
            self.calls.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn persist_setting_confines_paths_and_refuses_symlinks_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("settings");
        fs::create_dir_all(root.join("b")).unwrap();
        fs::write(root.join("b/secret"), "secret").unwrap();

        assert!(persist_setting(&root, "a", "../b/secret", "stolen").is_err());
        assert!(persist_setting(&root, "../b", "secret", "stolen").is_err());
        assert_eq!(fs::read_to_string(root.join("b/secret")).unwrap(), "secret");

        persist_setting(&root, "a", "on", "true").unwrap();
        assert_eq!(fs::read_to_string(root.join("a/on")).unwrap(), "true");

        fs::remove_file(root.join("a/on")).unwrap();
        fs::create_dir(root.join("a/on")).unwrap();
        assert!(persist_setting(&root, "a", "on", "false").is_err());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("b/secret"), root.join("a/link")).unwrap();
            assert!(persist_setting(&root, "a", "link", "overwrite").is_err());
            assert_eq!(fs::read_to_string(root.join("b/secret")).unwrap(), "secret");
        }
    }

    #[test]
    #[ignore = "daily"]
    fn single_owner_reads_writes_reloads_exact_f64_and_contains_callback_panics() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("settings");
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::write(root.join("alpha/on"), "true").unwrap();

        let alpha = RegisteredMod::new();
        let alpha_seen = alpha.seen.clone();
        let alpha_panic = alpha.panic_on_change.clone();
        let beta_calls = Arc::new(AtomicUsize::new(0));
        let beta = OtherMod {
            toggle: OnceLock::new(),
            calls: beta_calls.clone(),
        };

        let mods: mod_api::Mods = vec![
            ModEntry::new("alpha", Box::new(alpha)),
            ModEntry::new("beta", Box::new(beta)),
        ]
        .into();

        mod_platform::testing::with_mods(mods.clone(), || {
            mod_platform::__private::part_tests::install_parts_fixture(&mods).unwrap();
            let alpha_on = mod_api::__private::setting_key_by_name("alpha", "on").unwrap();
            let alpha_scale = mod_api::__private::setting_key_by_name("alpha", "scale").unwrap();
            let alpha_mode = mod_api::__private::setting_key_by_name("alpha", "mode").unwrap();
            let alpha_tint = mod_api::__private::setting_key_by_name("alpha", "tint").unwrap();
            let beta_on = mod_api::__private::setting_key_by_name("beta", "on").unwrap();
            let foreign = mod_api::__private::setting_key("ghost", 999);

            crate::game::native_entity_refs::tests::with_client_game(|game| {
                // First capture establishes baseline without firing setting_changed.
                let load = LoadContext::new(
                    DataLoadOrigin::ClientResourceLoad,
                    Arc::new(LoadInputs::new(BTreeMap::new(), root.clone())),
                    client_setting_owners([]),
                );
                game.loop_context = Arc::new(load.captured_loop_context(LoopSide::Client));
                game.sync_native_settings_from_loop_context();
                assert_eq!(game.setting_bool(alpha_on), Ok(true));
                assert_eq!(game.setting_f64(alpha_scale), Ok(0.0));
                assert_eq!(game.setting_choice(alpha_mode), Ok(0));
                assert_eq!(game.setting_color(alpha_tint), Ok(0x11_2233));
                assert_eq!(game.setting_bool(beta_on), Ok(true));
                assert!(alpha_seen.lock().unwrap().is_empty());

                // Unknown key and wrong-kind reads return typed errors.
                assert_eq!(game.setting(foreign), Err(SettingAccessError::UnknownKey));
                assert_eq!(
                    game.setting_f64(alpha_on),
                    Err(SettingAccessError::WrongKind)
                );

                // Authoritative write persists, updates live state, and notifies synchronously.
                let (owner, spec) = mod_api::__private::setting_entry(alpha_on)
                    .map(|(owner, spec)| (*owner, spec))
                    .unwrap();
                assert_eq!(
                    game.apply_native_setting_write(
                        &root,
                        owner,
                        spec,
                        SettingValue::Toggle(false)
                    ),
                    Ok(true)
                );
                assert_eq!(game.setting_bool(alpha_on), Ok(false));
                assert_eq!(fs::read_to_string(root.join("alpha/on")).unwrap(), "false");
                assert_eq!(
                    *alpha_seen.lock().unwrap(),
                    vec![("on".to_owned(), SettingValue::Toggle(false))]
                );
                assert_eq!(beta_calls.load(Ordering::Relaxed), 0);

                // Rewriting the same value is a no-op with no notification.
                assert_eq!(
                    game.apply_native_setting_write(
                        &root,
                        owner,
                        spec,
                        SettingValue::Toggle(false)
                    ),
                    Ok(false)
                );
                assert_eq!(alpha_seen.lock().unwrap().len(), 1);

                // Exact f64 comparison: -0.0 over +0.0 is treated as a real change
                // (matching vanilla OptionInstance.set's Objects.equals on boxed Double).
                let (scale_owner, scale_spec) = mod_api::__private::setting_entry(alpha_scale)
                    .map(|(owner, spec)| (*owner, spec))
                    .unwrap();
                assert_eq!(
                    game.apply_native_setting_write(
                        &root,
                        scale_owner,
                        scale_spec,
                        SettingValue::Range(-0.0),
                    ),
                    Ok(true)
                );
                let live_scale = game.setting_f64(alpha_scale).unwrap();
                assert_eq!(live_scale.to_bits(), (-0.0f64).to_bits());
                assert_eq!(alpha_seen.lock().unwrap().len(), 2);

                // Invalid kind or out-of-range write refuses without changing live state.
                assert_eq!(
                    game.apply_native_setting_write(
                        &root,
                        scale_owner,
                        scale_spec,
                        SettingValue::Range(99.0),
                    ),
                    Err(SettingAccessError::InvalidValue)
                );
                assert_eq!(
                    game.apply_native_setting_write(
                        &root,
                        scale_owner,
                        scale_spec,
                        SettingValue::Toggle(true),
                    ),
                    Err(SettingAccessError::WrongKind)
                );
                assert_eq!(
                    game.setting_f64(alpha_scale).unwrap().to_bits(),
                    (-0.0f64).to_bits()
                );

                // External disk edit is ignored until reload or Options > Mods refresh,
                // then picked up from a single LoopContext snapshot and notified.
                fs::write(root.join("alpha/on"), "true").unwrap();
                assert_eq!(game.setting_bool(alpha_on), Ok(false));
                let reload = LoadContext::new(
                    DataLoadOrigin::ClientResourceLoad,
                    Arc::new(LoadInputs::new(BTreeMap::new(), root.clone())),
                    client_setting_owners([]),
                );
                game.loop_context = Arc::new(reload.captured_loop_context(LoopSide::Client));
                game.sync_native_settings_from_loop_context();
                assert_eq!(game.setting_bool(alpha_on), Ok(true));
                assert_eq!(alpha_seen.lock().unwrap().len(), 3);

                // Single-owner page refresh picks up external edits and notifies.
                fs::write(root.join("alpha/mode"), "1").unwrap();
                game.refresh_native_settings_owner(&root, "alpha");
                assert_eq!(game.setting_choice(alpha_mode), Ok(1));
                assert_eq!(alpha_seen.lock().unwrap().len(), 4);

                // A panic inside alpha's setting_changed disables only alpha's callback
                // for the session while keeping beta's notifications working.
                let _ = mod_api::__private::game::take_callback_panic_notices();
                alpha_panic.store(true, Ordering::Relaxed);
                assert_eq!(
                    game.apply_native_setting_write(
                        &root,
                        owner,
                        spec,
                        SettingValue::Toggle(false)
                    ),
                    Ok(true)
                );
                let notices = mod_api::__private::game::take_callback_panic_notices();
                assert!(notices.iter().any(|n| n.mod_id == "alpha"));

                // Even after clearing the panic flag, alpha's disabled callback does not run again,
                // while live persistence still works and beta still receives its own changes.
                alpha_panic.store(false, Ordering::Relaxed);
                assert_eq!(
                    game.apply_native_setting_write(&root, owner, spec, SettingValue::Toggle(true)),
                    Ok(true)
                );
                assert_eq!(alpha_seen.lock().unwrap().len(), 4);
                let (beta_owner, beta_spec) = mod_api::__private::setting_entry(beta_on)
                    .map(|(owner, spec)| (*owner, spec))
                    .unwrap();
                assert_eq!(
                    game.apply_native_setting_write(
                        &root,
                        beta_owner,
                        beta_spec,
                        SettingValue::Toggle(false),
                    ),
                    Ok(true)
                );
                assert_eq!(beta_calls.load(Ordering::Relaxed), 1);
            });
        });
    }
}
