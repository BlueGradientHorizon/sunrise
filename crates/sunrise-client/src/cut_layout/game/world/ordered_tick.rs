

#![cfg(test)]
#[cfg(test)]
pub(crate) mod tests {
/// Hears every entity tick, naming `named` to `entity_tick_each`.
    struct Displays {
        named: &'static [&'static str],
        heard: Heard,
    }




/// A listener's calls, in order: the phase and the entities told.
    type Heard = std::sync::Arc<std::sync::Mutex<Vec<(&'static str, Vec<u32>)>>>;
#[mod_api::__subscriptions]
    impl mod_api::Hooks for Displays {
        fn entity_tick_listened(
            &self,
            _: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickListenedEvent,
        ) {
            event.listened = true;
        }
        fn entity_tick_pre(
            &self,
            _: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickPreEvent<'_>,
        ) {
            let entities = event.entities.to_vec();
            self.heard.lock().unwrap().push(("pre", entities));
        }
        fn entity_tick_post(
            &self,
            _: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickPostEvent<'_>,
        ) {
            let entities = event.entities.to_vec();
            self.heard.lock().unwrap().push(("post", entities));
        }
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            for kind in self.named {
                r.entity_tick_each(kind);
            }
        }
    }


use std::sync::Arc;

use sunrise_core::position::ChunkPos;

use sunrise_level::chunk::Chunk;

use sunrise_sim::join_seq::{JoinList, JoinSeq, Joined as _};

#[test]
    #[ignore = "daily"]
    fn dispatch_ticks_both_owners_of_each_located_kind() {
        super::super::native_entity_refs::tests::with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            game.block_ticks
                .add_empty_chunk(ChunkPos::new(0, 0))
                .unwrap();
            for x in [4, 12] {
                assert!(game.run_command(&format!("summon tnt {x} 70 4 {{fuse:80}}")));
                assert!(game.run_command(&format!(
                    "summon falling_block {x} 70 8 {{BlockState:{{Name:\"minecraft:sand\"}},Time:1}}"
                )));
                assert!(game.run_command(&format!("summon experience_orb {x} 70 12 {{Value:1}}")));
            }
            // Existing residents whose spawn packets were already announced.
            game.side.ordered.announced = sunrise_sim::join_seq::JoinSeq::MAX;
            game.side.tickets.0.update(None);
            game.begin_ordered_ticks();
            game.run_ordered_ticks(sunrise_sim::join_seq::JoinSeq::MAX, false)
                .unwrap();
            assert_eq!(
                game.tnts.iter().map(|t| t.fuse).collect::<Vec<_>>(),
                [79, 79]
            );
            assert_eq!(
                game.falling.iter().map(|f| f.age).collect::<Vec<_>>(),
                [2, 2]
            );
            assert_eq!(game.orbs.iter().map(|o| o.age).collect::<Vec<_>>(), [1, 1]);
        });
    }
/// Two text displays, which the server keeps without simulating them, through a level's
    /// entity phase: their ids and what the listener heard.
    fn display_ticks(named: &'static [&'static str]) -> (Vec<u32>, Vec<(&'static str, Vec<u32>)>) {
        let heard = Heard::default();
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "display-ticks",
            Box::new(Displays {
                named,
                heard: heard.clone(),
            }),
        )]
        .into();
        let mut ids = Vec::new();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                game.build_registries();
                game.set_allow_commands(true);
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                game.block_ticks
                    .add_empty_chunk(ChunkPos::new(0, 0))
                    .unwrap();
                for x in [4, 12] {
                    assert!(
                        game.run_command(&format!("summon text_display {x} 70 4 {{text:\"x\"}}"))
                    );
                }
                ids = game.side.others.0.iter().map(|o| o.id).collect();
                game.side.tickets.0.update(None);
                game.begin_ordered_ticks();
                game.run_ordered_ticks(sunrise_sim::join_seq::JoinSeq::MAX, false)
                    .unwrap();
                game.native_others_tick_family(game.side.ordered.told, game.side.ordered.born);
            });
        });
        let heard = std::mem::take(&mut *heard.lock().unwrap());
        (ids, heard)
    }
/// Displays of a type a mod named are told one by one at their place in the join order, each
    /// Pre right before its Post.
    #[test]
    #[ignore = "daily"]
    fn named_displays_are_told_at_their_place_in_the_join_order() {
        let (ids, heard) = display_ticks(&["minecraft:text_display"]);
        assert_eq!(ids.len(), 2);
        assert_eq!(
            heard,
            [
                ("pre", vec![ids[0]]),
                ("post", vec![ids[0]]),
                ("pre", vec![ids[1]]),
                ("post", vec![ids[1]]),
            ]
        );
    }
/// With no display type named, the displays are told once as a family; the walk itself
    /// offers none of them.
    #[test]
    #[ignore = "daily"]
    fn unnamed_displays_are_told_once_for_the_family() {
        let (ids, heard) = display_ticks(&[]);
        assert_eq!(heard, [("pre", ids.clone()), ("post", ids)]);
    }
}
