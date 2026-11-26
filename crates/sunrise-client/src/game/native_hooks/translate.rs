use super::*;

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The selected language's text for a translation key (`Component.translatable`'s lookup), `None`
    /// when no language layer defines it.
    pub(in crate::game) fn translate(&self, key: &str) -> Option<String> {
        sunrise_client_core::lang::has(key).then(|| sunrise_client_core::lang::get(key))
    }
}
