//! Entering a world without blocking the window: the deferred entry, world creation, the world
//! resources' background load, and the world's own content tables.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use std::sync::atomic::{AtomicU64, Ordering};

pub(in crate::game) static BIOME_KEYS_SESSION: AtomicU64 = AtomicU64::new(1);

pub(in crate::game) fn next_biome_keys_session() -> u64 {
    BIOME_KEYS_SESSION.fetch_add(1, Ordering::Relaxed)
}

impl Game<'_> {
    /// The entries of the mods' datapack registries in the winning pack files: decoded with
    /// each registry's schema and checked as one candidate, so a bad entry fails the load
    /// before any table is published. `None` when no mod declared a registry: nothing is
    /// scanned. A file whose `neoforge:conditions` fail is absent; the key is dropped before
    /// the schema sees the file.
    pub(in crate::game) fn datapack_candidate(
        &self,
        stack: &functions::PackStack,
        conditions: &sunrise_world::conditions::LoadConditions,
    ) -> Result<Option<mod_api::DatapackCandidate>> {
        let declarations = mod_api::__private::datapack_registry_declarations();
        if declarations.is_empty() {
            return Ok(None);
        }
        let keys: Vec<(&str, &str)> = declarations
            .iter()
            .filter_map(|declaration| declaration.key.split_once(':'))
            .collect();
        // Content files come before the world's enabled packs, lowest priority first.
        let packs: Vec<_> = std::iter::once(&stack.content)
            .chain(stack.enabled_packs())
            .collect();
        let files: Vec<_> =
            sunrise_client::datapack::registry_files(&packs, &keys, |path, bytes| {
                // Malformed JSON is kept for the registry's own decode diagnostic.
                serde_json::from_slice::<serde_json::Value>(bytes)
                    .map_or(true, |json| conditions.allows_file(&json, path))
            })
            .map_err(|error| {
                format!(
                    "datapack registry {} entry {} (pack {}, file {}): {:?}",
                    error.registry, error.entry, error.pack, error.file, error.reason
                )
            })?
            .into_iter()
            .collect();
        mod_api::DatapackCandidate::build(declarations, &files)
            .map(Some)
            .map_err(|error| {
                format!(
                    "datapack registry {} entry {} (pack {}, file {}): {:?}",
                    error.registry, error.entry, error.pack, error.file, error.reason
                )
                .into()
            })
    }

    /// Sends each joined Sunrise client the snapshot of the set just published, built from the
    /// offer owned by its connection. A refused replacement ends that connection with its reason.
    pub(in crate::game) fn send_datapack_snapshots(&mut self) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        for guest in host
            .hub
            .guests
            .iter()
            .filter(|g| g.placed && g.participates_online() && g.brand == "sunrise")
        {
            sunrise_client::datapack_net::reload_connection(&guest.sender);
        }
    }
    /// Close guest stonecutter windows before publishing a reloaded recipe generation.
    pub(in crate::game) fn close_stonecutters_for_reload(&mut self) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let open: Vec<_> = host
            .menus
            .iter()
            .filter_map(|(&conn, menus)| {
                menus
                    .station
                    .as_ref()
                    .filter(|menu| menu.kind == super::stations::Kind::Stonecutter)
                    .map(|_| conn)
            })
            .collect();
        for conn in open {
            self.guest_close_menus_and_tell(conn);
        }
    }
}
