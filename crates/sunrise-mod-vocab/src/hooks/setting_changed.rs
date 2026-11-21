//! The `setting_changed` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0020

pub trait Hooks {
    /// Setting `key` of mod `mod_id` (`SettingsRegistrar::register_setting`) changed through
    /// Options > Mods, [`ConfigClientHost::set_setting`](crate::prelude::ConfigClientHost::set_setting),
    /// or a client settings reload; the new value is already published and persisted, so read it
    /// with [`ConfigClientHost::setting`](crate::prelude::ConfigClientHost::setting) or the typed
    /// getters using the [`SettingKey`](crate::SettingKey) returned at registration. Only mod
    /// `mod_id` hears it: a mod is told of its own settings, never another's.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_setting_changed",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        owner = "mod_id",
        neoforge = "ModConfigEvent.Reloading (a ModConfigSpec value set on the config screen)"
    )]
    fn setting_changed(mod_id: &str, key: &str);
}
