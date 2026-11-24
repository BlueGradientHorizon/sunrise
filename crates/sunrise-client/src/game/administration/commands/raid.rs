//! `RaidCommand`: `/raid start|stop|check|spawnleader|setomen|glow|sound`.
#[allow(unused_imports)]
use super::{Game, R, Text, err};
#[allow(unused_imports)]
use sunrise_client::{
    command::{Arg, Context},
    mobs::raider,
};
#[allow(unused_imports)]
use sunrise_core::random::RandomSource;

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::Game;
    use crate::game::native_entity_refs::tests::with_game;
    use sunrise_client::{command::vanilla, motion::PlayerMotion};
    use sunrise_core::geometry::Vec3;

    /// Runs a line of the dev tree (`/raid` is registered only there).
    fn run_dev(game: &mut Game<'_>, line: &str) -> i32 {
        let d = vanilla::build_with(true);
        let parse = d.parse(line, &game.ui.registries);
        let (ctx, _) = parse.executable().expect(line);
        let (ctx, root) = (ctx.clone(), parse.context.clone());
        game.run_chain(&root, &ctx).expect(line)
    }

    #[test]
    #[ignore = "daily"]
    fn raid_check_finds_a_raid_counting_down_and_the_horn_is_local_only() {
        with_game(|game| {
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
            // RaidCommand needs the owner's permission level (3).
            game.ui.registries.permission = 4;
            game.server.chat_capture = Some(Vec::new());
            game.audio.0.capture = Some(Vec::new());
            let (id, _) = game
                .raiding
                .0
                .raids
                .create_or_extend([0, 64, 0], 2, None, 3);
            assert!(!game.raiding.0.raids.get(id).unwrap().started);

            assert_eq!(run_dev(game, "raid check"), 1);
            let lines: Vec<String> = game
                .server
                .chat_capture
                .take()
                .unwrap()
                .iter()
                .map(|t| t.plain())
                .collect();
            assert_eq!(
                lines,
                [
                    "Found a started raid!",
                    "Num groups spawned: 0 Raid omen level: 0 Num mobs: 0 Raid health: 0.0 / 0.0",
                ]
            );

            run_dev(game, "raid sound global");
            assert!(game.audio.0.capture.as_ref().unwrap().is_empty());
            run_dev(game, "raid sound local");
            let played: Vec<_> = game
                .audio
                .0
                .capture
                .take()
                .unwrap()
                .into_iter()
                .map(|s| (s.event, s.position, s.volume))
                .collect();
            assert_eq!(
                played,
                [(
                    "minecraft:event.raid.horn".to_owned(),
                    [5.5, 64.0, 0.5],
                    2.0
                )]
            );
        });
    }
}
