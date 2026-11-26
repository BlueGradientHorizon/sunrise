use super::*;

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
impl Game<'_> {
    /// The selected language's text for a translation key (`Component.translatable`'s lookup), `None`
    /// when no language layer defines it.
    pub(in crate::game) fn translate(&self, key: &str) -> Option<String> {
        sunrise_client::lang::has(key).then(|| sunrise_client::lang::get(key))
    }
    /// The calling mod's run-time translation entries for language `code`
    /// (`lang::set_mod_entries`), checked against the limits `ConfigClientHost::set_translations`
    /// names.
    pub(in crate::game) fn set_translations(
        &self,
        code: &str,
        entries: Vec<(String, String)>,
    ) -> Option<usize> {
        const MAX_ENTRIES: usize = 65_536;
        const MAX_KEY: usize = 256;
        const MAX_TEXT: usize = 32_767;
        let mod_id = mod_api::content::crash::current_native_mod()?;
        let code_ok = (1..=16).contains(&code.len())
            && code
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !code_ok
            || entries.len() > MAX_ENTRIES
            || entries
                .iter()
                .any(|(key, text)| key.is_empty() || key.len() > MAX_KEY || text.len() > MAX_TEXT)
        {
            return None;
        }
        Some(sunrise_client::lang::set_mod_entries(
            mod_id,
            code,
            entries.into_iter().collect(),
        ))
    }
    /// Drops all of the calling mod's run-time translation entries.
    pub(in crate::game) fn clear_translations(&self) {
        if let Some(mod_id) = mod_api::content::crash::current_native_mod() {
            sunrise_client::lang::clear_mod_entries(mod_id);
        }
    }
}
