

#![cfg(test)]
use sunrise_gameplay_common::game::interface::client_reload::texture_atlases::tests::Maps;

#[cfg(test)]
pub(crate) mod tests {

use sunrise_gameplay_common::game::interface::client_reload::texture_atlases::tests::maps;

use sunrise_gameplay_common::game::interface::client_reload::texture_atlases::tests::player;

use sunrise_gameplay_common::game::interface::client_reload::texture_atlases::tests::presets;

use super::*;

/// The registered atlas's sprite is in the GUI atlas at its own size after the first load,
    /// drawable by its name, and gone after a reload that no longer registers it.
    #[test]
    #[ignore = "daily"]
    fn a_mod_atlas_sprite_is_stitched_into_the_gui_atlas_until_a_reload_drops_it() {
        let kept = std::sync::Arc::default();
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "maps_mod",
            Box::new(Maps(std::sync::Arc::clone(&kept))),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            sunrise_gameplay::game::native_entity_refs::tests::with_client_game(|game| {
                game.texture_atlases_stitched();
                let player = player(&maps());
                assert_eq!(
                    game.gui_sprite_pixels(&player.to_string()),
                    Some([8.0, 8.0])
                );
                assert_eq!(
                    game.gui_sprite_pixels(&sunrise_gameplay_common::game::client_side::client_reload::texture_atlases::tests::player(&presets()).to_string()),
                    Some([8.0, 8.0]),
                    "a sprite under an atlas named presets is stitched too"
                );
                {
                    let kept = kept.lock().unwrap();
                    assert_eq!(kept.0, 1);
                    assert_eq!(
                        kept.1.last().copied().flatten().map(|r| r.size),
                        Some([8, 8])
                    );
                }
                game.spawn_pack_reload(crate::game::packs::ReloadKind::Selection)
                    .unwrap();
                let started = std::time::Instant::now();
                while game.side.pack_reload.0.is_some() {
                    assert!(
                        started.elapsed().as_secs() < 300,
                        "the reload never finished"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    game.drive_pack_reload();
                }
                let kept = kept.lock().unwrap();
                assert_eq!(kept.0, 2);
                assert_eq!(
                    kept.1.last().copied(),
                    Some(None),
                    "looked up in the new atlas"
                );
                assert_eq!(game.gui_sprite_pixels(&player.to_string()), None);
            });
        });
    }
}
