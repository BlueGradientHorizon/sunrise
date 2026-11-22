use super::{BlockEntity, CacheOptions, Game, Vertex, in_own_process};
use glam::DVec3;
use sunrise_client::{
    camera::CameraType, entities::ArmorStand, items::ItemStack, motion::PlayerMotion,
    signs::SignData,
};
use sunrise_core::geometry::Vec3;
use sunrise_protocol::{
    connection::Pipe,
    packets::play::{self, Clientbound as C},
};

#[test]
#[ignore = "daily"]
fn sign_outline_follows_scope_and_camera_subject_in_both_draw_paths() {
    in_own_process(
        "game::sign_outline_tests::sign_outline_follows_scope_and_camera_subject_in_both_draw_paths",
        || super::native_entity_refs::tests::with_client_game(check_signs),
    );
}

fn check_signs(game: &mut Game<'_>) {
    const POS: [i32; 3] = [0, 0, 0];
    const NEAR: DVec3 = DVec3::new(0.5, 0.5, 0.5);
    const FAR: DVec3 = DVec3::new(18.5, 0.5, 0.5);
    const EDGE: DVec3 = DVec3::new(16.5, 0.5, 0.5);
    const PARTIAL: f32 = 0.25;
    const SUBJECT: i32 = 31;
    let (client, _server) = Pipe::pair();
    game.connect_local(client, "SignOutline").unwrap();
    let motion = game.motion;
    let player = || PlayerMotion::new(motion, Vec3::new(FAR.x, FAR.y, FAR.z)).unwrap();
    game.inventory[0] = Some(ItemStack::new("minecraft:spyglass", 1));

    let mut observed = Vec::new();
    for (block, hanging) in [
        ("minecraft:oak_sign", false),
        ("minecraft:oak_hanging_sign", true),
    ] {
        let state = game.pack.assets.registry.default_state(block).unwrap();
        let sign = |color, glowing| {
            let mut sign = SignData {
                hanging,
                ..Default::default()
            };
            sign.front.lines[0] = "A".into();
            sign.front.color = color;
            sign.front.glowing = glowing;
            BlockEntity::Sign(sign)
        };
        // One red glyph. Holding the sign data, position and light fixed across the
        // series exercises cached vertices on both sides of an outline transition.
        game.world.block_entities.insert((0, 0, 0), sign(14, true));
        let cases = [
            (
                "first_person_active_scope",
                CameraType::FirstPerson,
                true,
                Some(FAR),
                FAR,
                9,
            ),
            (
                "third_person_active_scope",
                CameraType::ThirdPersonBack,
                true,
                Some(FAR),
                NEAR,
                1,
            ),
            (
                "passive_spyglass",
                CameraType::FirstPerson,
                false,
                Some(FAR),
                NEAR,
                1,
            ),
            (
                "just_inside",
                CameraType::FirstPerson,
                false,
                Some(DVec3::new(EDGE.x.next_down(), EDGE.y, EDGE.z)),
                FAR,
                9,
            ),
            (
                "exact_distance",
                CameraType::FirstPerson,
                false,
                Some(EDGE),
                NEAR,
                1,
            ),
            (
                "eye_height",
                CameraType::FirstPerson,
                false,
                Some(DVec3::new(0.5, 16.25, 0.5)),
                FAR,
                9,
            ),
            ("no_subject", CameraType::FirstPerson, false, None, NEAR, 1),
            (
                "scoping_without_subject",
                CameraType::FirstPerson,
                true,
                None,
                FAR,
                9,
            ),
            (
                "local_camera",
                CameraType::ThirdPersonFront,
                false,
                None,
                FAR,
                9,
            ),
        ];
        for cached in [true, false] {
            for (name, camera_type, scoping, subject, visual, expected) in cases {
                game.player = Some(player());
                game.camera_type = camera_type;
                game.using = scoping.then_some((0, 0));
                game.armor_stands.clear();
                let player_id = game.remote.as_ref().unwrap().entity_id;
                game.handle_packet(C::SetCamera(play::SetCamera {
                    camera_id: player_id,
                }))
                .unwrap();
                if let Some(subject) = subject {
                    let at = Vec3::new(subject.x, subject.y, subject.z);
                    let mut stand = ArmorStand::new(SUBJECT as u32, at, 0.0);
                    // The previous position deliberately disagrees for a threshold case.
                    if name == "exact_distance" {
                        stand.previous = Vec3::new(NEAR.x, NEAR.y, NEAR.z);
                    }
                    game.armor_stands.push(stand);
                    game.handle_packet(C::SetCamera(play::SetCamera { camera_id: SUBJECT }))
                        .unwrap();
                }
                let visual = match name {
                    "eye_height" => {
                        game.view.eye_height = 1.7775;
                        game.view.eye_height_old = game.view.eye_height;
                        let (camera, _, _) = game.camera_view(PARTIAL).unwrap();
                        assert!((camera - NEAR).length_squared() > 16.0 * 16.0);
                        camera
                    }
                    "exact_distance" => {
                        let (camera, _, _) = game.camera_view(PARTIAL).unwrap();
                        assert!((camera - NEAR).length_squared() < 16.0 * 16.0);
                        camera
                    }
                    "no_subject" => {
                        game.player = None;
                        visual
                    }
                    "scoping_without_subject" => {
                        // Loss of the held object before a new camera subject is available.
                        game.armor_stands
                            .push(ArmorStand::new(SUBJECT as u32, Vec3::ZERO, 0.0));
                        game.handle_packet(C::SetCamera(play::SetCamera { camera_id: SUBJECT }))
                            .unwrap();
                        game.armor_stands.clear();
                        visual
                    }
                    "local_camera" => {
                        game.player.as_mut().unwrap().position = Vec3::new(NEAR.x, NEAR.y, NEAR.z);
                        visual
                    }
                    _ => visual,
                };
                let quads = render_glyphs(game, (POS, state.0), visual, PARTIAL, cached);
                println!("{block} {cached} {name}: {quads}");
                observed.push((block, cached, name, quads, expected));
            }
        }
        // Without the distance or scope predicate, black glowing still has its eight
        // outline copies. Plain text never has them, even nearby.
        game.player = Some(player());
        game.using = None;
        for (name, color, glowing, at, expected) in [
            ("black_glowing_far", 15, true, FAR, 9),
            ("plain_near", 14, false, NEAR, 1),
        ] {
            game.player.as_mut().unwrap().position = Vec3::new(at.x, at.y, at.z);
            game.world
                .block_entities
                .insert((0, 0, 0), sign(color, glowing));
            for cached in [true, false] {
                let quads = render_glyphs(game, (POS, state.0), at, PARTIAL, cached);
                println!("{block} {cached} {name}: {quads}");
                observed.push((block, cached, name, quads, expected));
            }
        }
    }
    for (block, cached, name, actual, expected) in observed {
        assert_eq!(actual, expected, "{block} {name}, cached={cached}");
    }
}

fn render_glyphs(
    game: &Game<'_>,
    block: ([i32; 3], u32),
    visual: DVec3,
    partial: f32,
    cached: bool,
) -> usize {
    let renderer = game.res.block_entity_render();
    let blocks = [block];
    let mut out: Vec<Vertex> = Vec::new();
    game.with_block_entity_scene(visual, partial, &|_, _, _, _| true, |scene| {
        if cached {
            renderer.render_with_gpu_cache_options(
                scene,
                blocks.iter(),
                std::iter::empty(),
                CacheOptions {
                    signs: true,
                    ..Default::default()
                },
                0,
                |_| None,
                &mut out,
            );
        } else {
            renderer.render_with_cache_options_and_callbacks(
                scene,
                blocks.iter(),
                CacheOptions::default(),
                0,
                None,
                &mut out,
                |_, _, _| {},
                |_, _, _, _| {},
            );
        }
    });
    let glyph = game.gui.atlas.font.glyph_styled('A', &Default::default());
    let cell = glyph.rect.expect("the prepared font has a glyph for A");
    out.as_chunks::<4>()
        .0
        .iter()
        .filter(|q| q[0].normal == [0; 4] && q[0].uv == [cell.u0, cell.v1])
        .count()
}
