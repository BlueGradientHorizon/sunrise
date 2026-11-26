

#![cfg(test)]
#[cfg(test)]
pub(crate) mod tests {

use sunrise_client_gameplay::game::client_side::native_block_entity_draw::tests::draw;

use super::*;

/// A mod registers a layer (`register_layer_definitions`) at start: the game bakes it by its
    /// id and draws it, before and after a resource reload, beside vanilla's layers. The cube's
    /// south face, (0, 0, 1) to (1, 1, 1) in its part's frame, sits a block east of the part's
    /// origin and then the frame's 10.
    #[test]
    #[ignore = "daily"]
    fn a_registered_layer_bakes_and_draws_before_and_after_a_reload() {
        use mod_api::{CubeListBuilder, LayerDefinition, MeshDefinition, PartPose};
        use sunrise_core::geometry::Direction as Face;
        struct Golem;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Golem {
            fn register_layer_definitions(
                &self,
                _: &mod_api::RegistrationContext,
                event: &mut mod_api::RegisterLayerDefinitionsEvent<'_>,
            ) {
                event.register("golem_mod:golem#main", || {
                    let mut mesh = MeshDefinition::new();
                    let root = mesh.root();
                    mesh.add_or_replace_child(
                        root,
                        "body",
                        CubeListBuilder::create().add_box_faces(
                            [0.0; 3],
                            [16.0; 3],
                            &[Face::South],
                        ),
                        PartPose::offset(16.0, 0.0, 0.0),
                    );
                    LayerDefinition::create(mesh, 64, 64)
                });
            }
        }
        let drawn = |game: &sunrise_client_gameplay::game::ClientGame<'_>| {
            let part = game
                .bake_layer("golem_mod:golem#main")
                .expect("the mod's layer bakes");
            let mut d = draw();
            assert!(game.native_draw_model_part(&mut d, &part, "entity/zombie/zombie", 0xFF_FFFF));
            d.into_vertices().iter().map(|v| v.pos).collect::<Vec<_>>()
        };
        let face = [
            [11.0, 0.0, 1.0],
            [12.0, 0.0, 1.0],
            [12.0, 1.0, 1.0],
            [11.0, 1.0, 1.0],
        ];
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new("golem_mod", Box::new(Golem))].into();
        mod_platform::testing::with_mods(mods, || {
            crate::game::server_side::native_entity_refs::tests::with_client_game(|game| {
                sunrise_client_gameplay::game::client_side::mods_client::register_layer_definitions(
                    sunrise_gameplay_common::game::execution_role::ExecutionRole::Client,
                    &game.vanilla_root(),
                )
                .unwrap();
                assert_eq!(drawn(game), face);
                assert!(game.bake_layer("golem_mod:other#main").is_none());
                assert!(game.bake_layer("minecraft:zombie#main").is_some());
                game.spawn_pack_reload(sunrise_client_gameplay::game::client_side::packs::ReloadKind::Selection)
                    .unwrap();
                let started = std::time::Instant::now();
                while game.client.pack_reload.0.is_some() {
                    assert!(
                        started.elapsed().as_secs() < 300,
                        "the reload never finished"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    game.drive_pack_reload();
                }
                assert_eq!(drawn(game), face, "the same layer after the reload");
            });
        });
    }
}
