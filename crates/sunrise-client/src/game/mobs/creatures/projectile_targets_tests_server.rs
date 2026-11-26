use crate::game::commands::Target;
use crate::game::creatures::projectile_targets_tests::TrackedTarget;
use sunrise_core::geometry::{Aabb, Vec3};

pub(in crate::game) fn clear_entities(game: &mut crate::game::ServerGame<'_>) {
    game.mobs.clear();
    game.dropped.clear();
    game.boats.clear();
    game.carts.clear();
    game.armor_stands.clear();
    game.tnts.clear();
    game.falling.clear();
    game.orbs.clear();
    game.creatures.projectiles.clear();
    game.rockets.clear();
    game.tridents.clear();
    game.item_frames.clear();
    game.paintings.clear();
    game.cushions.clear();
    game.side.others.0.clear();
    game.side.knot_ids.clear();
    game.side.broken_knots.clear();
    game.resident_entities.knots.clear();
    game.reset_entity_residency();
    game.bind_local_player_residency();
}

pub(in crate::game) fn summon_target(
    game: &mut crate::game::ServerGame<'_>,
    target: &str,
) -> TrackedTarget {
    match target {
        "armor_stand" => {
            assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {}"));
            TrackedTarget::Stand(game.armor_stands.last().unwrap().id)
        }
        "armor_stand_equipped" => {
            assert!(game.run_command(
                r#"summon minecraft:armor_stand 3.5 64.0 4.5 {equipment:{head:{id:"minecraft:iron_helmet",count:1}}}"#
            ));
            TrackedTarget::Stand(game.armor_stands.last().unwrap().id)
        }
        "armor_stand_marker" => {
            assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {Marker:1b}"));
            TrackedTarget::Stand(game.armor_stands.last().unwrap().id)
        }
        "armor_stand_small" | "armor_stand_small_high" => {
            assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {Small:1b}"));
            TrackedTarget::Stand(game.armor_stands.last().unwrap().id)
        }
        "armor_stand_nogravity" => {
            assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {NoGravity:1b}"));
            TrackedTarget::Stand(game.armor_stands.last().unwrap().id)
        }
        "armor_stand_invisible" => {
            assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {Invisible:1b}"));
            TrackedTarget::Stand(game.armor_stands.last().unwrap().id)
        }
        "painting" => {
            assert!(game.run_command(
                r#"summon minecraft:painting 3.5 64.5 4.5 {block_pos:[I;3,64,4],facing:1b,variant:"minecraft:kebab"}"#
            ));
            TrackedTarget::Painting(game.paintings.last().unwrap().id)
        }
        "item_frame" => {
            assert!(game.run_command(
                r#"summon minecraft:item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b,Item:{id:"minecraft:diamond",count:1}}"#
            ));
            TrackedTarget::Frame(game.item_frames.last().unwrap().id)
        }
        "item_frame_empty" => {
            assert!(game.run_command(
                "summon minecraft:item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b}"
            ));
            TrackedTarget::Frame(game.item_frames.last().unwrap().id)
        }
        "item_frame_fixed" => {
            assert!(game.run_command(
                r#"summon minecraft:item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b,Fixed:1b,Item:{id:"minecraft:diamond",count:1}}"#
            ));
            TrackedTarget::Frame(game.item_frames.last().unwrap().id)
        }
        "glow_item_frame" => {
            assert!(game.run_command(
                r#"summon minecraft:glow_item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b,Item:{id:"minecraft:diamond",count:1}}"#
            ));
            TrackedTarget::Frame(game.item_frames.last().unwrap().id)
        }
        "glow_item_frame_empty" => {
            assert!(game.run_command(
                "summon minecraft:glow_item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b}"
            ));
            TrackedTarget::Frame(game.item_frames.last().unwrap().id)
        }
        "leash_knot" => {
            assert!(game.run_command("summon minecraft:leash_knot 3.5 64.375 4.5 {}"));
            TrackedTarget::ExplicitKnot(game.side.others.0.last().unwrap().id)
        }
        "leash_knot_tied" => {
            assert!(game.run_command(
                "summon minecraft:pig 3.5 64.0 6.5 {NoAI:1b,Silent:1b,leash:[I;3,64,4]}"
            ));
            let pig_id = game.mobs.last().unwrap().id;
            game.tick_server(false).unwrap();
            let id = *game
                .side
                .knot_ids
                .get(&[3, 64, 4])
                .expect("natural leash knot created on fence");
            TrackedTarget::NaturalKnot { id, pig_id }
        }
        "falling_block" => {
            assert!(game.run_command(
                r#"summon minecraft:falling_block 3.5 65.0 4.5 {BlockState:"minecraft:sand",Time:1,Motion:[0.0d,0.0d,0.0d]}"#
            ));
            TrackedTarget::Falling(game.falling.last().unwrap().id)
        }
        "cushion" => {
            assert!(game.run_command(
                r#"summon minecraft:cushion 3.5 64.0 4.5 {block_pos:[I;3,64,4],color:"white"}"#
            ));
            TrackedTarget::Cushion(game.cushions.last().unwrap().id)
        }
        other => panic!("unexpected witness target {other}"),
    }
}

pub(in crate::game) fn target_alive(
    game: &crate::game::ServerGame<'_>,
    target: TrackedTarget,
) -> bool {
    match target {
        TrackedTarget::Stand(id) => game.armor_stands.iter().any(|a| a.id == id),
        TrackedTarget::Painting(id) => game.paintings.iter().any(|p| p.id == id),
        TrackedTarget::Frame(id) => game.item_frames.iter().any(|f| f.id == id),
        TrackedTarget::Cushion(id) => game.cushions.iter().any(|c| c.id == id),
        TrackedTarget::Falling(id) => game.falling.iter().any(|f| f.id == id),
        TrackedTarget::ExplicitKnot(id) => game.side.others.0.iter().any(|o| o.id == id),
        TrackedTarget::NaturalKnot { id, .. } => {
            game.resident_entities.knots.iter().any(|k| k.id == id)
        }
    }
}

pub(in crate::game) fn target_bounds(
    game: &crate::game::ServerGame<'_>,
    target: TrackedTarget,
) -> Aabb {
    match target {
        TrackedTarget::Stand(id) => {
            let a = game.armor_stands.iter().find(|a| a.id == id).unwrap();
            if a.marker {
                Aabb::new(a.position, a.position)
            } else {
                a.bounds()
            }
        }
        TrackedTarget::Painting(id) => game
            .selector_target_bounds(Target::Painting(id))
            .expect("painting bounds"),
        TrackedTarget::Frame(id) => game
            .selector_target_bounds(Target::Frame(id))
            .expect("frame bounds"),
        TrackedTarget::Cushion(id) => game
            .selector_target_bounds(Target::Cushion(id))
            .expect("cushion bounds"),
        TrackedTarget::Falling(id) => game
            .selector_target_bounds(Target::Falling(id))
            .expect("falling block bounds"),
        TrackedTarget::ExplicitKnot(id) => {
            let o = game.side.others.0.iter().find(|o| o.id == id).unwrap();
            let b = sunrise_core::position::BlockPos::containing(o.position);
            let k = sunrise_sim::mobs::leash::knot_state([b.x, b.y, b.z]);
            let half = k.width / 2.0;
            Aabb::new(
                Vec3::new(k.position.x - half, k.position.y, k.position.z - half),
                Vec3::new(
                    k.position.x + half,
                    k.position.y + k.height,
                    k.position.z + half,
                ),
            )
        }
        TrackedTarget::NaturalKnot { id, .. } => game
            .selector_target_bounds(Target::Knot(id))
            .expect("natural knot bounds"),
    }
}
