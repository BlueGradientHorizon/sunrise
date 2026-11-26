//! Language map lookup for session and dedicated-server translation resolution.
use sunrise_protocol::text::Language;

/// The language a session resolves server-sent components with: the selected
/// one as `lang` loaded it (`en_us` under it, packs included), so a menu title
/// the server sends as a key follows the language screen. A process with no
/// language set (a dedicated server's log) reads en_us.json.
pub fn load_language(resources: &std::path::Path) -> Language {
    if let Some(lang) = crate::lang::current() {
        return lang.map().clone();
    }
    // No language screen ran (a dedicated server's log): en_us, with
    // Sunrise's own strings included — the net thread resolves keys with
    // this map (`Reason::keyed`).
    sunrise_client_core::lang::Language::load(&[&resources.to_path_buf()], "en_us")
        .map()
        .clone()
}
