//! A pack's `gbuffers_line` drawing the block outline. One widens its line itself, as vanilla's
//! `rendertype_lines.vsh` does: it reads `vaPosition` and `vaNormal` without declaring them (as
//! Complementary Reimagined r5.9.3's `#version 130` line program does) and offsets each vertex
//! to one side of the line's direction by `gl_VertexID % 2`. The other only places the line,
//! and the game widens it. The programs here are original and cut down: the line is placed in
//! clip space directly, so the drawn band is exact.
use super::destination_tests::Fixture;
use std::io::{Cursor, Write};
use wgpu::util::DeviceExt;

const SIZE: u32 = 64;
/// Half the band's height in clip space when the pack widens the line: 8 pixels of 64.
const HALF: f32 = 0.25;

/// A pack whose line program is `line_vsh` and draws white; its final copies colortex0 to the
/// screen.
fn pack(line_vsh: &str) -> sunrise_shaders::Pack {
    let files = [
        ("shaders/gbuffers_line.vsh", line_vsh),
        (
            "shaders/gbuffers_line.fsh",
            "#version 130\n/* DRAWBUFFERS:0 */\nvoid main() { gl_FragData[0] = vec4(1.0); }\n",
        ),
        (
            "shaders/final.vsh",
            "#version 120\nvarying vec2 uv;\n\
             void main() { gl_Position = ftransform(); uv = gl_MultiTexCoord0.xy; }\n",
        ),
        (
            "shaders/final.fsh",
            "#version 120\nuniform sampler2D colortex0;\nvarying vec2 uv;\n\
             void main() { gl_FragColor = texture2D(colortex0, uv); }\n",
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

/// One edge from (-0.5, 0, 2) to (0.5, 0, 2), camera-relative, 6 pixels wide by the game's
/// measure. The programs place it at its x and y in clip space.
fn edge(gpu: &Fixture) -> wgpu::Buffer {
    let mut bytes = Vec::new();
    for v in [-0.5f32, 0.0, 2.0, 0.5, 0.0, 2.0] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes.extend_from_slice(&0xFF00_0000u32.to_le_bytes());
    bytes.extend_from_slice(&6.0f32.to_le_bytes());
    gpu.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("line side edge"),
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        })
}

/// The pixels that aren't as they should be when the outline's edge is drawn by the pack whose
/// line program is `line_vsh`: white inside columns 16..48 and `rows`, black elsewhere (`None`
/// without a GPU). Pixel centres in clip space are at x = (x + 0.5) / 32 - 1, so the band's x
/// from -0.5 to 0.5 covers columns 16..48.
fn wrong_pixels(line_vsh: &str, rows: std::ops::Range<u32>) -> Option<Vec<(u32, u32)>> {
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
    assert!(post.lines.is_some(), "{:?}", post.notes);
    let mut post = gpu.try_build(post).expect("the pack runs");
    // The line program, not a fallback, draws the outline.
    assert_eq!(post.notes, ["the block outline draws with gbuffers_line"]);
    assert!(post.has_lines());
    post.set_lines(Some(super::Lines {
        edges: edge(&gpu),
        count: 1,
    }));
    let pixels = gpu.render(&mut post, [SIZE, SIZE]);
    let white = |x: u32, y: u32| pixels[(y * SIZE + x) as usize][..3] == [255; 3];
    let mut wrong = Vec::new();
    for y in 0..SIZE {
        for x in 0..SIZE {
            let inside = (16..48).contains(&x) && rows.contains(&y);
            if white(x, y) != inside {
                wrong.push((x, y));
            }
        }
    }
    Some(wrong)
}

/// A line program that widens its line by vertex index along the perpendicular of `vaNormal`
/// (the edge's direction, (1, 0, 0)): every pixel of the band y in [-0.25, 0.25] (rows 24..40)
/// is white, and none outside it, so both triangles of the strip have their sides (a vertex's
/// `gl_VertexID % 2` is its corner's side) and the game adds no widening of its own.
#[test]
fn a_line_program_that_widens_by_vertex_index_draws_its_band() {
    let line_vsh = format!(
        "#version 130\nvoid main() {{\n    \
         vec2 d = normalize(vaNormal.xy);\n    \
         vec2 side = vec2(-d.y, d.x) * {HALF:?};\n    \
         gl_Position = vec4(vaPosition.xy + (gl_VertexID % 2 == 0 ? side : -side), 0.5, 1.0);\n}}\n"
    );
    let Some(wrong) = wrong_pixels(&line_vsh, 24..40) else {
        return;
    };
    assert!(
        wrong.is_empty(),
        "{} pixels wrong, first {:?}",
        wrong.len(),
        &wrong[..wrong.len().min(8)]
    );
}

/// A line program that only places the line (as BSL's `gbuffers_basic` only transforms it): the
/// game widens it by its width, 6 pixels, across its direction on screen. The edge is
/// horizontal, so the offset is (0, 6 / 64) in clip space to each side
/// (`hz_l_width / hz_target`), 3 pixels at 32 pixels per clip unit: y in [-0.09375, 0.09375],
/// rows 29..35.
#[test]
fn a_line_program_that_only_places_its_line_is_widened_by_the_game() {
    let line_vsh = "#version 130\nvoid main() {\n    \
                    gl_Position = vec4(vaPosition.xy, 0.5, 1.0);\n}\n";
    let Some(wrong) = wrong_pixels(line_vsh, 29..35) else {
        return;
    };
    assert!(
        wrong.is_empty(),
        "{} pixels wrong, first {:?}",
        wrong.len(),
        &wrong[..wrong.len().min(8)]
    );
}
