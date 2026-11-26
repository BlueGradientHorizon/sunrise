#![cfg(test)]
#[cfg(test)]
pub(crate) mod tests {

    use super::*;

    use mod_api::BlockEdit;
    use mod_api::BlockRun;

    use sunrise_level::chunk::Chunk;

    #[test]
    #[ignore = "daily"]
    fn typed_bulk_late_refusals_preserve_the_world_and_emit_no_changes() {
        use mod_api::{
            StateConstraint, StatePredicate, StatePredicateRecord, StatePredicateTarget,
        };
        use sunrise_gameplay_common::game::api_values::TryIntoApi;
        sunrise_gameplay::game::native_entity_refs::tests::with_game(|game| {
            // The shared fixture is unticked; expose the dedicated-server bulk surface.
            game.options.dedicated = Some(Vec::new());
            let dimension = game.dim.id().to_owned();
            game.world
                .chunks
                .insert((0, 0), std::sync::Arc::new(Chunk::empty(0, 0)));
            let registry = game.pack.assets.registry;
            let air = registry.default_state("minecraft:air").unwrap();
            let stone = registry.default_state("minecraft:stone").unwrap();
            let api_air = air.try_into_api().unwrap();
            let api_stone = stone.try_into_api().unwrap();
            let metadata = mod_api::__private::block_definitions().unwrap();
            let good = StatePredicate::block(metadata, api_air.block(), &[]).unwrap();
            let bad = StatePredicate::from(StatePredicateRecord::new(
                StatePredicateTarget::Block(api_air.block()),
                vec![StateConstraint::new("unknown", "true")],
            ));
            let mut edit = BlockEdit {
                palette: vec![api_stone],
                filters: vec![good, bad],
                runs: vec![BlockRun {
                    x: 0,
                    z: 0,
                    y: 64,
                    height: 1,
                    surface: None,
                    block: 0,
                    only: Some(0),
                }],
            };
            game.world.block_log = Some(Vec::new());
            assert_eq!(game.world.block(0, 64, 0), air);
            let refused = game.edit_blocks(&dimension, &edit);
            assert_eq!(
                game.world.block(0, 64, 0),
                air,
                "late filter refusal must preserve the world"
            );
            assert!(game.world.block_log.as_ref().unwrap().is_empty());
            assert!(refused.is_err());

            edit.filters.pop();
            edit.palette
                .push(mod_api::__private::block_state_from_ordinal(
                    api_stone.block(),
                    u16::MAX,
                ));
            let refused = game.edit_blocks(&dimension, &edit);
            assert_eq!(
                game.world.block(0, 64, 0),
                air,
                "late palette refusal must preserve the world"
            );
            assert!(game.world.block_log.as_ref().unwrap().is_empty());
            assert!(refused.is_err());

            edit.palette.pop();
            assert_eq!(game.edit_blocks(&dimension, &edit), Ok(1));
            assert_eq!(
                game.world.block(0, 64, 0),
                stone,
                "the same valid run must actually mutate"
            );
            game.world.block_log = None;
        });
    }
}
