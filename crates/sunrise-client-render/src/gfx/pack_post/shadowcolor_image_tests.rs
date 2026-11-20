//! `shadowcolorimg0` and `1` (spec 5.2: images in all programs) bind the shadowcolor buffers
//! themselves, as `colorimgN` binds colortex N: what a program stores there is what
//! `shadowcolor0`/`1` read afterwards, and a declared format is matched against the buffer's.
//! Original constant-colour programs, no pack files.
use super::destination_tests::Fixture;
use super::*;
use std::io::{Cursor, Write};

const VERTEX: &str = "#version 120\nvarying vec2 uv;\nvoid main() { uv=gl_MultiTexCoord0.xy; gl_Position=ftransform(); }\n";

/// Even columns show `shadowcolor0`, odd ones `shadowcolor1`, texel for pixel.
const FINAL: &str = "#version 330 compatibility\nconst int shadowMapResolution = 8;\n\
     uniform sampler2D shadowcolor0;\nuniform sampler2D shadowcolor1;\n\
     void main() {\n    ivec2 p = ivec2(gl_FragCoord.xy);\n\
         gl_FragColor = p.x % 2 == 0 ? texelFetch(shadowcolor0, p, 0) : texelFetch(shadowcolor1, p, 0);\n}\n";

/// A pack of `files` and `final`, whose fragment shader starts with `directives`.
fn pack_with(files: &[(&str, &str)], directives: &str) -> Post {
    let fragment = FINAL.replacen('\n', &format!("\n{directives}"), 1);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let last = [("final.vsh", VERTEX), ("final.fsh", fragment.as_str())];
    for (name, source) in files.iter().chain(&last) {
        zip.start_file(
            format!("shaders/{name}"),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(source.as_bytes()).unwrap();
    }
    let pack = sunrise_shaders::Pack::from_zip_bytes(
        "original-shadowcolor-images",
        zip.finish().unwrap().into_inner(),
    )
    .unwrap();
    let mut macros = sunrise_shaders::Macros::standard();
    let (properties, _) = sunrise_shaders::read_properties(&pack, &mut macros);
    sunrise_shaders::load_post(&pack, "", &properties, &macros, &Default::default())
}

fn pack(files: &[(&str, &str)]) -> Post {
    pack_with(files, "")
}

/// Two frames of `post` on an 8×8 screen (the shadow map's size): pixels (0, 0) and (1, 0),
/// shadowcolor0 and shadowcolor1, of each frame.
fn frames(gpu: &Fixture, post: &mut PackPost) -> Vec<[[u8; 4]; 2]> {
    (0..2)
        .map(|_| {
            let pixels = gpu.render(post, [8, 8]);
            assert!(
                pixels
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .all(|p| p[..] == pixels[..2]),
                "{pixels:?}"
            );
            [pixels[0], pixels[1]]
        })
        .collect()
}

/// [`frames`] of `files` and `final` with `directives`, which run without notes. `None`
/// without a GPU.
fn columns_with(files: &[(&str, &str)], directives: &str) -> Option<Vec<[[u8; 4]; 2]>> {
    let gpu = Fixture::new()?;
    let mut post = gpu.build(pack_with(files, directives));
    Some(frames(&gpu, &mut post))
}

fn columns(files: &[(&str, &str)]) -> Option<Vec<[[u8; 4]; 2]>> {
    columns_with(files, "")
}

#[test]
fn a_compute_program_stores_into_the_shadowcolor_buffers() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(frames) = columns(&[(
        "composite.csh",
        "#version 430\nlayout(local_size_x = 1, local_size_y = 1) in;\n\
         const ivec3 workGroups = ivec3(8, 8, 1);\n\
         layout(rgba8) writeonly uniform image2D shadowcolorimg0;\n\
         layout(rgba8) writeonly uniform image2D shadowcolorimg1;\n\
         void main() {\n    ivec2 p = ivec2(gl_GlobalInvocationID.xy);\n\
             imageStore(shadowcolorimg0, p, vec4(0.2, 0.4, 0.6, 1.0));\n\
             imageStore(shadowcolorimg1, p, vec4(1.0, 0.2, 0.0, 1.0));\n}\n",
    )]) else {
        return;
    };
    // Stand-ins would leave both buffers at their clear colour, white.
    for f in frames {
        assert_eq!(f, [[51, 102, 153, 255], [255, 51, 0, 255]]);
    }
}

#[test]
fn a_fragment_program_stores_into_a_shadowcolor_buffer() {
    let _gpu = crate::gfx::test_gpu::lock();
    // A composite program with no output draws into the sink; its only effect is the image.
    let Some(frames) = columns(&[
        ("composite.vsh", VERTEX),
        (
            "composite.fsh",
            "#version 430 compatibility\n\
             layout(rgba8) writeonly uniform image2D shadowcolorimg1;\n\
             void main() { imageStore(shadowcolorimg1, ivec2(gl_FragCoord.xy), vec4(0.0, 0.6, 0.2, 1.0)); }\n",
        ),
    ]) else {
        return;
    };
    for f in frames {
        assert_eq!(f, [[255; 4], [0, 153, 51, 255]]);
    }
}

#[test]
fn loads_and_samples_of_a_stored_shadowcolor_buffer_read_it_as_it_was() {
    let _gpu = crate::gfx::test_gpu::lock();
    // composite1 loads shadowcolorimg0 before storing it (its loads read a copy taken first), and
    // samples shadowcolor1 while storing into it (sampled from a copy, as wgpu binds a texture
    // one way per dispatch). Formats are read from fragment shaders: `final` makes shadowcolor0
    // R32F.
    let Some(frames) = columns_with(
        &[
            (
                "composite.csh",
                "#version 430\nlayout(local_size_x = 1, local_size_y = 1) in;\n\
             const ivec3 workGroups = ivec3(8, 8, 1);\n\
             layout(r32f) writeonly uniform image2D shadowcolorimg0;\n\
             layout(rgba8) writeonly uniform image2D shadowcolorimg1;\n\
             void main() {\n    ivec2 p = ivec2(gl_GlobalInvocationID.xy);\n\
                 imageStore(shadowcolorimg0, p, vec4(0.2));\n\
                 imageStore(shadowcolorimg1, p, vec4(0.2, 0.2, 0.2, 1.0));\n}\n",
            ),
            (
                "composite1.csh",
                "#version 430\nlayout(local_size_x = 1, local_size_y = 1) in;\n\
             const ivec3 workGroups = ivec3(8, 8, 1);\n\
             layout(r32f) uniform image2D shadowcolorimg0;\n\
             layout(rgba8) writeonly uniform image2D shadowcolorimg1;\n\
             uniform sampler2D shadowcolor1;\n\
             void main() {\n    ivec2 p = ivec2(gl_GlobalInvocationID.xy);\n\
                 float v = imageLoad(shadowcolorimg0, p).r;\n\
                 imageStore(shadowcolorimg0, p, vec4(v * 2.0));\n\
                 imageStore(shadowcolorimg1, p, vec4(texelFetch(shadowcolor1, p, 0).rgb * 3.0, 1.0));\n}\n",
            ),
        ],
        "/* const int shadowcolor0Format = R32F; */\n",
    ) else {
        return;
    };
    // An R32F buffer reads (r, 0, 0, 1).
    for f in frames {
        assert_eq!(f, [[102, 0, 0, 255], [153, 153, 153, 255]]);
    }
}

#[test]
fn a_shadowcolor_image_of_another_format_skips_its_program() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::new() else { return };
    let compute = |format: &str| {
        format!(
            "#version 430\nlayout(local_size_x = 1, local_size_y = 1) in;\n\
             const ivec3 workGroups = ivec3(8, 8, 1);\n\
             layout({format}) writeonly uniform image2D shadowcolorimg0;\n\
             void main() {{ imageStore(shadowcolorimg0, ivec2(gl_GlobalInvocationID.xy), vec4(0.2)); }}\n"
        )
    };
    // shadowcolor0 is RGBA8: an `rgba8` image is it, an `r32f` one skips its program, which
    // leaves the buffer at its clear colour.
    let right = compute("rgba8");
    let mut post = gpu.build(pack(&[("composite.csh", &right)]));
    assert_eq!(frames(&gpu, &mut post)[1][0], [51; 4]);
    let wrong = compute("r32f");
    let mut post = gpu.try_build(pack(&[("composite.csh", &wrong)])).unwrap();
    assert!(
        post.notes
            .iter()
            .any(|n| n.contains("`shadowcolorimg0` is r32f")),
        "{:?}",
        post.notes
    );
    assert_eq!(frames(&gpu, &mut post)[1][0], [255; 4]);
}
