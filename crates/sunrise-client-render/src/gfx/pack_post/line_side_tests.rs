//! A pack's `gbuffers_line` drawing the block outline. One widens its line itself, as vanilla's
//! `rendertype_lines.vsh` does: it reads `vaPosition` and `vaNormal` without declaring them (as
//! Complementary Reimagined r5.9.3's `#version 130` line program does) and offsets each vertex
//! to one side of the line's direction on screen by `gl_VertexID % 2`. The others only place
//! the line, and the game widens it. The programs here are original and cut down.
//!
//! The fixture's camera looks along +z (yaw 0) with a 90° field of view, so in the square
//! 64×64 frame a point (x, y, 2) ahead of it is at column 32 − 16x, row 32 − 16y. Each case
//! draws its edges with ends chosen so that no pixel centre lies on a stroke's border, and
//! checks every pixel: the colour of the last stroke over its centre, the clear value of
//! colortex2 (transparent black, spec 6.1.5) outside them all.
use super::destination_tests::Fixture;
use std::io::{Cursor, Write};
use wgpu::util::DeviceExt;

const SIZE: u32 = 64;
const OUTSIDE: [u8; 4] = [0; 4];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [255; 4];

/// A pack whose line program is `line_vsh` (setting `color`) and draws `color` into
/// colortex2; its final copies colortex2 to the screen.
fn pack(line_vsh: &str) -> sunrise_shaders::Pack {
    let files = [
        ("shaders/gbuffers_line.vsh", line_vsh),
        (
            "shaders/gbuffers_line.fsh",
            "#version 130\nvarying vec4 color;\n/* DRAWBUFFERS:2 */\n\
             void main() { gl_FragData[0] = color; }\n",
        ),
        (
            "shaders/final.vsh",
            "#version 120\nvarying vec2 uv;\n\
             void main() { gl_Position = ftransform(); uv = gl_MultiTexCoord0.xy; }\n",
        ),
        (
            "shaders/final.fsh",
            "#version 120\nuniform sampler2D colortex2;\nvarying vec2 uv;\n\
             void main() { gl_FragColor = texture2D(colortex2, uv); }\n",
        ),
    ];
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, source) in files {
        zip.start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(source.as_bytes()).unwrap();
    }
    sunrise_shaders::Pack::from_zip_bytes("line-side", zip.finish().unwrap().into_inner()).unwrap()
}

/// A line program that widens its line as vanilla's `rendertype_lines.vsh` does, to `width`
/// pixels, after `VIEW_SCALE` scales the view (vanilla's shrinks it by 1/256), which leaves the
/// line where it was on screen.
fn widening(width: f32) -> String {
    format!(
        "#version 130\nuniform float viewWidth, viewHeight;\nvarying vec4 color;\n\
         #define VIEW_SCALE 0.75\nvec4 place(vec3 p) {{\n    \
         vec4 view = gl_ModelViewMatrix * vec4(p, 1.0);\n    \
         view.xyz *= VIEW_SCALE;\n    return gl_ProjectionMatrix * view;\n}}\n\
         void main() {{\n    color = vaColor;\n    vec2 screen = vec2(viewWidth, viewHeight);\n    \
         vec4 a = place(vaPosition);\n    vec4 b = place(vaPosition + vaNormal);\n    \
         vec2 d = normalize((b.xy / b.w - a.xy / a.w) * screen);\n    \
         vec2 offset = vec2(-d.y, d.x) * {width:?} / screen;\n    \
         if (offset.x < 0.0) offset = -offset;\n    \
         gl_Position = a + vec4((gl_VertexID % 2 == 0 ? offset : -offset) * a.w, 0.0, 0.0);\n}}\n"
    )
}

/// A line program that only transforms its line, then moves it `up` pixels up the screen (a
/// temporal anti-aliasing jitter's kind of move).
fn transforming(up: f32) -> String {
    format!(
        "#version 130\nuniform float viewHeight;\nvarying vec4 color;\nvoid main() {{\n    \
         color = vaColor;\n    gl_Position = gl_ModelViewProjectionMatrix * vec4(vaPosition, 1.0);\n    \
         gl_Position.y += {up:?} * 2.0 / viewHeight * gl_Position.w;\n}}\n"
    )
}

/// A line program that reads its vertex index and the line's direction into a varying, as a
/// widening one does, and still only transforms its line.
const READS_INDEX_AND_NORMAL: &str = "#version 130\nvarying vec4 color;\nvarying float side;\n\
     void main() {\n    color = vaColor;\n    side = float(gl_VertexID % 2) * vaNormal.x;\n    \
     gl_Position = ftransform();\n}\n";

/// An edge's ends, camera-relative, at z = 2.
type Edge = ([f32; 2], [f32; 2]);

/// From (-0.5, 0.25) to (0.5, 0.25): columns 24..40 at row 28.
const HORIZONTAL: Edge = ([-0.5, 0.25], [0.5, 0.25]);
/// From (-0.25, -0.5) to (-0.25, 0.5): rows 24..40 at column 36.
const VERTICAL: Edge = ([-0.25, -0.5], [-0.25, 0.5]);
/// From (-0.4, -0.4) to (0.4, 0.4): from (38.4, 38.4) to (25.6, 25.6), across the diagonal.
const DIAGONAL: Edge = ([-0.4, -0.4], [0.4, 0.4]);

/// One stroke of the outline: its ARGB colour and its width in pixels by the game's measure.
type Stroke = (u32, f32);

/// The outline buffer: each stroke over `edge`, in order, as the game writes it.
fn outline(gpu: &Fixture, edge: Edge, strokes: &[Stroke]) -> wgpu::Buffer {
    let mut bytes = Vec::new();
    for &(argb, width) in strokes {
        let ([x0, y0], [x1, y1]) = edge;
        for v in [x0, y0, 2.0, x1, y1, 2.0] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&argb.to_le_bytes());
        bytes.extend_from_slice(&width.to_le_bytes());
    }
    gpu.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("line side outline"),
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        })
}

/// A stroke of the expected picture: how far it reaches to each side of its edge, in pixels,
/// and its colour.
type Band = (f32, [u8; 4]);

/// The colour expected at pixel (x, y): that of the last of `bands` whose rectangle holds the
/// pixel's centre, `OUTSIDE` for none. Each band is `edge`, moved `up` pixels up the screen,
/// reaching its distance to each side.
fn expected(x: u32, y: u32, edge: Edge, up: f32, bands: &[Band]) -> [u8; 4] {
    let at = |[ex, ey]: [f32; 2]| [32.0 - 16.0 * ex, 32.0 - 16.0 * ey - up];
    let (a, b) = (at(edge.0), at(edge.1));
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = (dx * dx + dy * dy).sqrt();
    let (px, py) = (x as f32 + 0.5 - a[0], y as f32 + 0.5 - a[1]);
    let along = (px * dx + py * dy) / length;
    let across = (px * dy - py * dx).abs() / length;
    bands
        .iter()
        .rev()
        .find(|(half, _)| (0.0..length).contains(&along) && across < *half)
        .map_or(OUTSIDE, |&(_, colour)| colour)
}

/// A pixel that isn't as expected: the edge's name, x, y, actual and expected colours.
type Wrong = (&'static str, u32, u32, [u8; 4], [u8; 4]);

/// The pixels that differ from `expected` when the pack whose line program is `line_vsh` draws
/// `strokes` over each of the three edges; `None` without a GPU.
fn wrong_pixels(line_vsh: &str, strokes: &[Stroke], up: f32, bands: &[Band]) -> Option<Vec<Wrong>> {
    let _gpu = crate::gfx::test_gpu::lock();
    let gpu = Fixture::new()?;
    let pack = pack(line_vsh);
    let mut macros = sunrise_shaders::Macros::standard();
    let (properties, _) = sunrise_shaders::read_properties(&pack, &mut macros);
    let inputs = sunrise_shaders::GameInputs {
        lines: Some(crate::gfx::pack_lines::inputs()),
        ..Default::default()
    };
    let post = sunrise_shaders::load_post(&pack, "", &properties, &macros, &inputs);
    // The line program, not a fallback, draws the outline.
    let lines = post.lines.as_ref().map(|p| p.name.as_str());
    assert_eq!(lines, Some("gbuffers_line"), "{:?}", post.notes);
    let mut post = gpu.try_build(post).expect("the pack runs");
    assert!(post.has_lines());
    let mut wrong = Vec::new();
    for (name, edge) in [
        ("horizontal", HORIZONTAL),
        ("vertical", VERTICAL),
        ("diagonal", DIAGONAL),
    ] {
        post.set_lines(Some(super::Lines {
            edges: outline(&gpu, edge, strokes),
            count: 1,
            strokes: strokes.len() as u32,
        }));
        let pixels = gpu.render(&mut post, [SIZE, SIZE]);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let actual = pixels[(y * SIZE + x) as usize];
                let expected = expected(x, y, edge, up, bands);
                if actual != expected {
                    wrong.push((name, x, y, actual, expected));
                }
            }
        }
    }
    Some(wrong)
}

fn assert_none_wrong(wrong: Option<Vec<Wrong>>) {
    let Some(wrong) = wrong else {
        return;
    };
    assert!(
        wrong.is_empty(),
        "{} pixels wrong, first {:?}",
        wrong.len(),
        &wrong[..wrong.len().min(8)]
    );
}

/// A line program that widens its line by vertex index, 4 pixels wide (2 to each side), under
/// a view scale: its band is exactly the pack's, so the game added no width of its own, and
/// both triangles of each strip have their sides (a vertex's `gl_VertexID % 2` is its
/// corner's side).
#[test]
fn a_line_program_that_widens_its_line_draws_its_own_band() {
    let wrong = wrong_pixels(&widening(4.0), &[(0xFFFF_FFFF, 6.0)], 0.0, &[(2.0, WHITE)]);
    assert_none_wrong(wrong);
}

/// A line program that only places the line (as BSL's `gbuffers_basic` does) is widened by the
/// game to its 6 pixels, 3 to each side.
#[test]
fn a_line_program_that_only_places_its_line_is_widened_by_the_game() {
    let wrong = wrong_pixels(
        &transforming(0.0),
        &[(0xFFFF_FFFF, 6.0)],
        0.0,
        &[(3.0, WHITE)],
    );
    assert_none_wrong(wrong);
}

/// A line program that reads its vertex index and `vaNormal` into a varying but only
/// transforms its line is widened by the game: reading them doesn't widen the line.
#[test]
fn a_line_program_reading_its_index_and_normal_but_only_placing_its_line_is_widened() {
    let strokes = [(0xFFFF_FFFF, 6.0)];
    let wrong = wrong_pixels(READS_INDEX_AND_NORMAL, &strokes, 0.0, &[(3.0, WHITE)]);
    assert_none_wrong(wrong);
}

/// A line moved 0.7 pixels up the screen, as a jitter moves it, is still the game's to widen
/// (`OWN_WIDTH_PIXELS` is 0.75): 3 pixels to each side of where the pack put it.
#[test]
fn a_line_moved_less_than_the_threshold_is_widened_by_the_game() {
    let wrong = wrong_pixels(
        &transforming(0.7),
        &[(0xFFFF_FFFF, 6.0)],
        0.7,
        &[(3.0, WHITE)],
    );
    assert_none_wrong(wrong);
}

/// A line the pack widens by 0.8 pixels to each side keeps the pack's width: 0.8 is past
/// `OWN_WIDTH_PIXELS`.
#[test]
fn a_line_widened_past_the_threshold_keeps_the_packs_width() {
    let wrong = wrong_pixels(&widening(1.6), &[(0xFFFF_FFFF, 6.0)], 0.0, &[(0.8, WHITE)]);
    assert_none_wrong(wrong);
}

/// The high contrast outline (a black stroke 6 pixels wide, then the bright one, 2 wide, over
/// it) under a pack that widens its lines to 4 pixels: the bright stroke keeps the pack's
/// width, 2 to each side, and the black one reaches 2 more to each side, (6 - 2) / 2, as the
/// game draws it 4 pixels wider than the bright one.
#[test]
fn a_high_contrast_outline_keeps_its_black_border_under_a_pack_that_widens_its_lines() {
    let strokes = [(0xFF00_0000, 6.0), (0xFFFF_FFFF, 2.0)];
    let bands = [(4.0, BLACK), (2.0, WHITE)];
    assert_none_wrong(wrong_pixels(&widening(4.0), &strokes, 0.0, &bands));
}

/// The same high contrast outline under a pack that only places its lines: each stroke at the
/// game's width, 3 and 1 pixels to each side.
#[test]
fn a_high_contrast_outline_under_a_pack_that_places_its_lines_has_the_games_widths() {
    let strokes = [(0xFF00_0000, 6.0), (0xFFFF_FFFF, 2.0)];
    let bands = [(3.0, BLACK), (1.0, WHITE)];
    assert_none_wrong(wrong_pixels(&transforming(0.0), &strokes, 0.0, &bands));
}
