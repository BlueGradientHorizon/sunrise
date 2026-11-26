//! `WorldDrawContext::text` (#6173): a mod's text under its own pose, drawn as a block entity's
//! text is: turned and rolled as the pose says, full bright when glowing, ringed when outlined.
use crate::game::host::draw::GameWorldDrawContext;
use super::native_entity_refs::tests::with_client_game;
use super::world_name_tag_tests::{CAMERA, face_south};
use glam::{DVec3, Mat4, Vec3};
use mod_api::{DrawError, Text, WorldDrawContext, WorldTextOptions};
use sunrise_client::block_entity_render::vertex_light;
use sunrise_client::{mesher::Vertex, world_geometry::WorldGeometry};

/// Four blocks in front of the camera.
const ORIGIN: [f64; 3] = [0.5, 70.5, 0.5];

/// A name tag's text scale (`PoseStack.scale(0.025, -0.025, 0.025)`): a font pixel 0.025
/// blocks, y up in the world.
fn name_tag_scale() -> Mat4 {
    Mat4::from_scale(Vec3::new(0.025, -0.025, 0.025))
}

/// `m` as the row-major 3×4 a pose is given in.
fn rows(m: Mat4) -> [f32; 12] {
    m.transpose().to_cols_array()[..12].try_into().unwrap()
}

/// [`ORIGIN`] from the camera, where the pose's origin lands.
fn anchor() -> Vec3 {
    (DVec3::from(ORIGIN) - DVec3::from(CAMERA)).as_vec3()
}

/// One view's posed text at `origin`: what it draws with the name tags, then over everything.
fn texts(
    game: &mut crate::game::ClientGame<'_>,
    origin: [f64; 3],
    pose: [f32; 12],
    text: &Text,
    options: WorldTextOptions,
) -> (Result<f32, DrawError>, [Vec<Vertex>; 2]) {
    face_south(game);
    let mut draw = WorldGeometry::default();
    draw.reset(CAMERA, Mat4::IDENTITY, [0.5, 0.5]);
    let result = GameWorldDrawContext::new(game, &mut draw).text(origin, pose, text, options);
    (result, draw.labels().map(<[Vertex]>::to_vec))
}

#[test]
#[ignore = "daily"]
fn posed_text_turns_and_rolls_as_its_pose_says_and_non_finite_is_refused() {
    with_client_game(|game| {
        let text = Text::literal("Label");
        let options = WorldTextOptions::default();
        let width = {
            let mut draw = WorldGeometry::default();
            GameWorldDrawContext::new(game, &mut draw).text_width(&text)
        };
        assert!(width > 0.0);
        let (ok, [flat, through]) = texts(game, ORIGIN, rows(name_tag_scale()), &text, options);
        assert_eq!(ok, Ok(width.ceil()));
        assert!(!flat.is_empty() && through.is_empty());
        // Unturned, the line lies in the pose's x-y plane through the origin, left end first.
        let anchor = anchor();
        assert!(flat.iter().all(|v| (v.pos[2] - anchor.z).abs() < 1e-5));
        let min_x = flat.iter().map(|v| v.pos[0]).fold(f32::INFINITY, f32::min);
        assert!((min_x - anchor.x).abs() < 0.025 + 1e-5, "{min_x}");
        // A 90 degree yaw, a roll and both: every vertex moves about the origin as the matrix
        // turns it from the unturned line.
        for turn in [
            Mat4::from_rotation_y(90f32.to_radians()),
            Mat4::from_rotation_z(30f32.to_radians()),
            Mat4::from_rotation_y(90f32.to_radians()) * Mat4::from_rotation_z(30f32.to_radians()),
        ] {
            let pose = rows(turn * name_tag_scale());
            let (ok, [turned, _]) = texts(game, ORIGIN, pose, &text, options);
            assert_eq!(ok, Ok(width.ceil()));
            assert_eq!(turned.len(), flat.len());
            for (a, b) in flat.iter().zip(&turned) {
                let (a, b) = (Vec3::from(a.pos) - anchor, Vec3::from(b.pos) - anchor);
                let expected = turn.transform_vector3(a);
                assert!((b - expected).length() < 1e-5, "{a} -> {b}, not {expected}");
            }
        }
        // Yawed 90 degrees, the line runs along -z: its x is the origin's.
        let yawed = rows(Mat4::from_rotation_y(90f32.to_radians()) * name_tag_scale());
        let (_, [turned, _]) = texts(game, ORIGIN, yawed, &text, options);
        assert!(turned.iter().all(|v| (v.pos[0] - anchor.x).abs() < 1e-5));
        // Non-finite origins, poses and starts are refused, with nothing drawn.
        let mut nan = rows(name_tag_scale());
        nan[5] = f32::NAN;
        let mut infinite = rows(name_tag_scale());
        infinite[3] = f32::INFINITY;
        for (origin, pose, options) in [
            ([f64::NAN, 70.5, 0.5], rows(name_tag_scale()), options),
            ([0.5, f64::INFINITY, 0.5], rows(name_tag_scale()), options),
            (ORIGIN, nan, options),
            (ORIGIN, infinite, options),
            (
                ORIGIN,
                rows(name_tag_scale()),
                options.with_at([f32::NAN, 0.0]),
            ),
        ] {
            let (refused, drawn) = texts(game, origin, pose, &text, options);
            assert_eq!(refused, Err(DrawError::NonFinite));
            assert!(drawn.iter().all(Vec::is_empty));
        }
    });
}

#[test]
#[ignore = "daily"]
fn glowing_text_is_full_bright_and_see_through_text_is_unlit_over_everything() {
    with_client_game(|game| {
        let text = Text::literal("Glow");
        let pose = rows(name_tag_scale());
        let dark = WorldTextOptions::default().with_light(0x00);
        let (_, [lit, _]) = texts(game, ORIGIN, pose, &text, dark);
        assert!(!lit.is_empty());
        assert!(
            lit.iter()
                .all(|v| v.light == vertex_light(0x00) && v.layer == 0)
        );
        let (_, [sky, _]) = texts(game, ORIGIN, pose, &text, WorldTextOptions::default());
        assert!(sky.iter().all(|v| v.light == vertex_light(0xF0)));
        // Glowing sign text: LightCoordsUtil.FULL_BRIGHT on every vertex, whatever the light.
        let (_, [glowing, through]) = texts(game, ORIGIN, pose, &text, dark.with_glowing());
        assert!(through.is_empty());
        assert!(glowing.iter().all(|v| v.light == vertex_light(0xFF)));
        assert_eq!(
            glowing.iter().map(|v| (v.pos, v.color)).collect::<Vec<_>>(),
            lit.iter().map(|v| (v.pos, v.color)).collect::<Vec<_>>(),
            "glowing changes only the light"
        );
        // Font.DisplayMode.SEE_THROUGH: the same glyphs, drawn over everything and unlit.
        let (_, [world, see_through]) = texts(game, ORIGIN, pose, &text, dark.with_see_through());
        assert!(world.is_empty());
        assert_eq!(see_through.len(), lit.len());
        assert!(see_through.iter().all(|v| v.light == 0 && v.layer == 2));
    });
}

#[test]
#[ignore = "daily"]
fn an_outline_rings_each_glyph_in_its_colour_and_a_backdrop_goes_behind() {
    with_client_game(|game| {
        let text = Text::literal("A");
        let pose = rows(name_tag_scale());
        let (plain_width, [plain, _]) =
            texts(game, ORIGIN, pose, &text, WorldTextOptions::default());
        assert_eq!(plain.len(), 4, "one glyph quad");
        assert!(plain.iter().all(|v| v.color == u32::MAX));
        let outlined = WorldTextOptions::default().with_outline(0xFF00_0000);
        let (width, [ringed, _]) = texts(game, ORIGIN, pose, &text, outlined);
        assert_eq!(width, plain_width);
        // Font.drawInBatch8xOutline: eight copies in the outline colour, then the text.
        assert_eq!(ringed.len(), 9 * 4);
        let (rings, front) = ringed.split_at(8 * 4);
        assert!(rings.iter().all(|v| v.color == 0xFF00_0000));
        assert!(front.iter().all(|v| v.color == u32::MAX));
        // Each copy is the glyph one font pixel round it (x right, y down), x outer, y inner.
        let offsets = [-1, 0, 1]
            .into_iter()
            .flat_map(|dx| [-1, 0, 1].map(|dy| (dx, dy)))
            .filter(|&offset| offset != (0, 0));
        for (ring, (dx, dy)) in rings.chunks(4).zip(offsets) {
            let shift = Vec3::new(dx as f32 * 0.025, -dy as f32 * 0.025, 0.0);
            for (r, p) in ring.iter().zip(&plain) {
                let moved = Vec3::from(r.pos) - Vec3::from(p.pos);
                assert!((moved - shift).length() < 1e-5, "({dx}, {dy}): {moved}");
            }
        }
        // The text itself sits where it would unringed, a hair in front of its rings (+z).
        for (f, p) in front.iter().zip(&plain) {
            assert_eq!([f.pos[0], f.pos[1]], [p.pos[0], p.pos[1]]);
            assert!(f.pos[2] > p.pos[2]);
        }
        // Font.drawInBatch's backgroundColor: one quad, first, just behind the glyphs.
        let backed = WorldTextOptions::default().with_background(0x4000_0000);
        let (_, [backed, _]) = texts(game, ORIGIN, pose, &text, backed);
        assert_eq!(backed.len(), plain.len() + 4);
        let (backdrop, glyphs) = backed.split_at(4);
        assert!(backdrop.iter().all(|v| v.color == 0x4000_0000));
        assert!(backdrop.iter().all(|v| v.pos[2] < plain[0].pos[2]));
        assert_eq!(
            glyphs.iter().map(|v| v.pos).collect::<Vec<_>>(),
            plain.iter().map(|v| v.pos).collect::<Vec<_>>()
        );
    });
}
