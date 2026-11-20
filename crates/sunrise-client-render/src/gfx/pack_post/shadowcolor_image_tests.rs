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

/// A pack of exactly `files`, its terrain programs given `inputs`.
fn pack_of(files: &[(&str, &str)], inputs: &sunrise_shaders::GameInputs) -> Post {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, source) in files {
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
    sunrise_shaders::load_post(&pack, "", &properties, &macros, inputs)
}

/// A pack of `files` and `final`, whose fragment shader starts with `directives`.
fn pack_with(files: &[(&str, &str)], directives: &str) -> Post {
    let fragment = FINAL.replacen('\n', &format!("\n{directives}"), 1);
    let last = [("final.vsh", VERTEX), ("final.fsh", fragment.as_str())];
    let all: Vec<_> = files.iter().chain(&last).copied().collect();
    pack_of(&all, &Default::default())
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
fn a_compute_stage_attached_to_a_pass_is_enough_to_make_its_shadowcolor_buffer() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::new() else { return };
    // composite.csh runs before composite.fsh, in its pass, and is the only program that uses
    // shadowcolor1 (R32F): it loads shadowcolorimg1 (cleared to 0.2), stores twice that back and
    // into colorimg0. composite.fsh draws colortex1 only. final samples only shadowcolor0, which
    // makes the shadow pass.
    let composite = format!(
        "{COMPUTE}layout(r32f) uniform image2D shadowcolorimg1;\n\
         layout(rgba8) writeonly uniform image2D colorimg0;\n\
         void main() {{\n    ivec2 p = ivec2(gl_GlobalInvocationID.xy);\n\
             float v = imageLoad(shadowcolorimg1, p).r * 2.0;\n\
             imageStore(shadowcolorimg1, p, vec4(v));\n\
             imageStore(colorimg0, p, vec4(v, v, v, 1.0));\n}}\n"
    );
    let mut post = gpu.build(pack_of(
        &[
            ("composite.csh", &composite),
            ("composite.vsh", VERTEX),
            (
                "composite.fsh",
                "#version 120\n/* RENDERTARGETS: 1 */\nvoid main() { gl_FragData[0] = vec4(1.0); }\n",
            ),
            ("final.vsh", VERTEX),
            (
                "final.fsh",
                "#version 330 compatibility\nconst int shadowMapResolution = 8;\n\
                 /* const int shadowcolor1Format = R32F; */\n\
                 const vec4 shadowcolor1ClearColor = vec4(0.2, 0.0, 0.0, 1.0);\n\
                 uniform sampler2D colortex0;\nuniform sampler2D shadowcolor0;\n\
                 void main() {\n    ivec2 p = ivec2(gl_FragCoord.xy);\n\
                     gl_FragColor = p.x % 2 == 0 ? texelFetch(colortex0, p, 0) : texelFetch(shadowcolor0, p, 0);\n}\n",
            ),
        ],
        &Default::default(),
    ));
    // Without shadowcolor1 the image would bind a sampled-only stand-in as storage, which wgpu
    // refuses. shadowcolor1 is cleared each frame, so both frames read 0.2 and store 0.4.
    for f in frames(&gpu, &mut post) {
        assert_eq!(f, [[102, 102, 102, 255], [255; 4]]);
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

/// The start of a compute program over the 8×8 shadow map, one invocation a texel.
const COMPUTE: &str = "#version 430\nlayout(local_size_x = 1, local_size_y = 1) in;\n\
     const ivec3 workGroups = ivec3(8, 8, 1);\n";

#[test]
fn the_shadow_cache_redraws_buffers_a_later_program_stores_into() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::new() else { return };
    // composite doubles shadowcolor0 in place: from its clear colour, 0.2, to 0.4 each frame.
    let composite = format!(
        "{COMPUTE}layout(r32f) uniform image2D shadowcolorimg0;\n\
         void main() {{\n    ivec2 p = ivec2(gl_GlobalInvocationID.xy);\n\
             imageStore(shadowcolorimg0, p, imageLoad(shadowcolorimg0, p) * 2.0);\n}}\n"
    );
    let mut post = gpu.build(pack_with(
        &[("composite.csh", &composite)],
        "/* const int shadowcolor0Format = R32F; */\n\
         const vec4 shadowcolor0ClearColor = vec4(0.2, 0.0, 0.0, 1.0);\n",
    ));
    // Nothing else changes between the frames: a kept map would read 0.8 on the second.
    for frame in 0..2 {
        let pixels = gpu.render_cached(&mut post, [8, 8]);
        assert_eq!(
            [pixels[0], pixels[1]],
            [[102, 0, 0, 255], [255; 4]],
            "frame {frame}"
        );
    }
}

#[test]
fn a_shadowcomp_program_samples_a_buffer_it_stores_into_from_a_copy() {
    let _gpu = crate::gfx::test_gpu::lock();
    // In shadowcomp `shadowcolor0` is the pass's own buffer 0; bound both ways in one dispatch,
    // wgpu would refuse the frame.
    let shadowcomp = format!(
        "{COMPUTE}uniform sampler2D shadowcolor0;\n\
         layout(rgba8) writeonly uniform image2D shadowcolorimg0;\n\
         void main() {{\n    ivec2 p = ivec2(gl_GlobalInvocationID.xy);\n\
             imageStore(shadowcolorimg0, p, vec4(texelFetch(shadowcolor0, p, 0).rgb * 2.0, 1.0));\n}}\n"
    );
    let Some(frames) = columns_with(
        &[("shadowcomp.csh", &shadowcomp)],
        "const vec4 shadowcolor0ClearColor = vec4(0.2, 0.2, 0.2, 1.0);\n",
    ) else {
        return;
    };
    for f in frames {
        assert_eq!(f, [[102, 102, 102, 255], [255; 4]]);
    }
}

#[test]
fn an_image_binds_the_side_a_shadowcomp_program_flipped_to() {
    let _gpu = crate::gfx::test_gpu::lock();
    // shadowcomp draws blue into shadowcolor0's "alt" side and flips it, so its "main" is the B
    // texture on the first frame (A again on the second); composite then stores over "main".
    let composite = format!(
        "{COMPUTE}layout(rgba8) writeonly uniform image2D shadowcolorimg0;\n\
         void main() {{ imageStore(shadowcolorimg0, ivec2(gl_GlobalInvocationID.xy), vec4(0.2, 0.4, 0.6, 1.0)); }}\n"
    );
    let Some(frames) = columns(&[
        ("shadowcomp.vsh", VERTEX),
        (
            "shadowcomp.fsh",
            "#version 120\n/* RENDERTARGETS: 0 */\nvoid main() { gl_FragData[0] = vec4(0.0, 0.0, 1.0, 1.0); }\n",
        ),
        ("composite.csh", &composite),
    ]) else {
        return;
    };
    for f in frames {
        assert_eq!(f, [[51, 102, 153, 255], [255; 4]]);
    }
}

#[test]
fn a_shadowcolor_image_checks_that_its_own_buffer_can_be_stored() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::new() else { return };
    // colortex0 RGB10_A2 can't be an image on a device without adapter-specific formats, while
    // shadowcolor0 RGBA8 can: the check must look at the shadow buffer's own bit.
    let storable = |f: wgpu::TextureFormat| {
        f.guaranteed_format_features(gpu.device.features())
            .allowed_usages
            .contains(wgpu::TextureUsages::STORAGE_BINDING)
    };
    if storable(wgpu::TextureFormat::Rgb10a2Unorm) {
        eprintln!("Skipped: this device stores RGB10_A2 as well as RGBA8");
        return;
    }
    let composite = format!(
        "{COMPUTE}layout(rgba8) writeonly uniform image2D shadowcolorimg0;\n\
         void main() {{ imageStore(shadowcolorimg0, ivec2(gl_GlobalInvocationID.xy), vec4(0.2, 0.4, 0.6, 1.0)); }}\n"
    );
    let mut post = gpu.build(pack_with(
        &[("composite.csh", &composite)],
        // A colortex format in a block comment counts only at the start of its line.
        "/*\nconst int colortex0Format = RGB10_A2;\n*/\n",
    ));
    assert_eq!(post.formats[0], wgpu::TextureFormat::Rgb10a2Unorm);
    for f in frames(&gpu, &mut post) {
        assert_eq!(f, [[51, 102, 153, 255], [255; 4]]);
    }
}

#[test]
fn shadowcolorimg2_stays_a_stand_in_with_higher_shadowcolor_buffers() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::new() else { return };
    // shadowcolor2 exists (HIGHER_SHADOWCOLOR) but spec 5.2 names only shadowcolorimg0 and 1.
    let composite = format!(
        "{COMPUTE}layout(rgba8) writeonly uniform image2D shadowcolorimg2;\n\
         void main() {{ imageStore(shadowcolorimg2, ivec2(gl_GlobalInvocationID.xy), vec4(0.2)); }}\n"
    );
    let post = gpu
        .try_build(pack_with(
            &[
                ("composite.csh", &composite),
                (
                    "shaders.properties",
                    "iris.features.required = HIGHER_SHADOWCOLOR\n",
                ),
            ],
            "uniform sampler2D shadowcolor2;\n",
        ))
        .unwrap();
    assert!(
        post.notes
            .iter()
            .any(|n| n.contains("image `shadowcolorimg2` has no `image.shadowcolorimg2`")),
        "{:?}",
        post.notes
    );
}

#[test]
fn a_pack_with_no_shadow_pass_keeps_the_stand_in() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::new() else { return };
    // Nothing samples a shadow buffer, so there is no shadow pass and no buffer to bind.
    let composite = format!(
        "{COMPUTE}layout(rgba8) writeonly uniform image2D shadowcolorimg0;\n\
         void main() {{ imageStore(shadowcolorimg0, ivec2(gl_GlobalInvocationID.xy), vec4(0.2)); }}\n"
    );
    let post = gpu
        .try_build(pack_of(
            &[
                ("composite.csh", &composite),
                ("final.vsh", VERTEX),
                (
                    "final.fsh",
                    "#version 120\nvoid main() { gl_FragColor = vec4(1.0); }\n",
                ),
            ],
            &Default::default(),
        ))
        .unwrap();
    assert!(post.shadow_targets.is_none());
    assert!(
        post.notes
            .iter()
            .any(|n| n.contains("image `shadowcolorimg0` has no `image.shadowcolorimg0`")),
        "{:?}",
        post.notes
    );
}

#[test]
fn atomics_reach_an_r32ui_shadowcolor_buffer() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::with_features(wgpu::Features::TEXTURE_ATOMIC) else {
        return;
    };
    // composite adds 51 to shadowcolor0 (R32UI, cleared to 0); composite1 shows it as
    // shadowcolor1's red, which final shows. `shadow.enabled` runs the shadow pass.
    let add = format!(
        "{COMPUTE}layout(r32ui) uniform uimage2D shadowcolorimg0;\n\
         void main() {{ imageAtomicAdd(shadowcolorimg0, ivec2(gl_GlobalInvocationID.xy), 51u); }}\n"
    );
    let show = format!(
        "{COMPUTE}layout(r32ui) readonly uniform uimage2D shadowcolorimg0;\n\
         layout(rgba8) writeonly uniform image2D shadowcolorimg1;\n\
         void main() {{\n    ivec2 p = ivec2(gl_GlobalInvocationID.xy);\n\
             imageStore(shadowcolorimg1, p, vec4(float(imageLoad(shadowcolorimg0, p).r) / 255.0, 0.0, 0.0, 1.0));\n}}\n"
    );
    let mut post = gpu.build(pack_of(
        &[
            ("composite.csh", &add),
            ("composite1.csh", &show),
            ("final.vsh", VERTEX),
            (
                "final.fsh",
                "#version 330 compatibility\nconst int shadowMapResolution = 8;\n\
                 /* const int shadowcolor0Format = R32UI; */\n\
                 const vec4 shadowcolor0ClearColor = vec4(0.0);\n\
                 uniform sampler2D shadowcolor1;\n\
                 void main() { gl_FragColor = texelFetch(shadowcolor1, ivec2(gl_FragCoord.xy), 0); }\n",
            ),
            ("shaders.properties", "shadow.enabled=true\n"),
        ],
        &Default::default(),
    ));
    // Made for atomics, which Metal needs (wgpu checks only STORAGE_BINDING when binding).
    let buffer = post
        .shadow_targets
        .as_ref()
        .and_then(|t| t.color[0].as_ref());
    let usage = buffer.map_or(wgpu::TextureUsages::empty(), |c| c[0].texture().usage());
    assert!(
        usage.contains(wgpu::TextureUsages::STORAGE_ATOMIC),
        "{usage:?}"
    );
    for frame in 0..2 {
        let pixels = gpu.render(&mut post, [8, 8]);
        assert_eq!(pixels[0], [51, 0, 0, 255], "frame {frame}");
    }
}

/// A pack whose terrain `program` (`shadow` or `gbuffers_terrain`) draws the terrain
/// (clip-space positions) with `fragment`, and whose `final` shows shadowcolor0 and 1 as
/// [`FINAL`] does.
fn terrain_pack(program: &str, fragment: &str) -> Post {
    let inputs = sunrise_shaders::GameInputs {
        terrain: Some(sunrise_shaders::GbuffersInputs {
            parts: None,
            item_id_declared: false,
            far_depth: false,
            paged_maps: false,
            water_mask: false,
            water_patches: false,
            declarations: "layout(location=0)in vec3 fixture_position;\n".into(),
            body: "hz_Vertex=vec4(fixture_position,1.0);hz_Color=vec4(1.0);\
                   hz_MultiTexCoord0=vec4(0.0);hz_MultiTexCoord1=vec4(240.0);\
                   hz_MultiTexCoord2=vec4(240.0);hz_Normal=vec3(0.0,0.0,1.0);\n"
                .into(),
            attributes: vec![],
            after: String::new(),
            alpha_test: None,
        }),
        ..Default::default()
    };
    let (vertex, fragment_name) = (format!("{program}.vsh"), format!("{program}.fsh"));
    pack_of(
        &[
            (
                &vertex,
                "#version 120\nvoid main() { gl_Position = vec4(gl_Vertex.xy, 0.5, 1.0); }\n",
            ),
            (&fragment_name, fragment),
            ("final.vsh", VERTEX),
            ("final.fsh", FINAL),
        ],
        &inputs,
    )
}

/// Builds `post` with the terrain's vertex layout and renders two frames of it, the terrain a
/// quad over the whole shadow map and screen: the build's notes, and pixels (0, 0) and (1, 0) of each
/// frame.
fn shadow_frames(gpu: &Fixture, post: Post) -> (Vec<String>, Vec<[[u8; 4]; 2]>) {
    use wgpu::util::DeviceExt;
    const ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0=>Float32x3];
    let layouts = [Some(wgpu::VertexBufferLayout {
        array_stride: 12,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRS,
    })];
    let mut post = gpu.build_with_terrain(
        post,
        Some(Geometry {
            material_maps: false,
            buffers: &layouts,
            table: None,
            depth_write: true,
            atlas: None,
            ids: None,
            cull: false,
        }),
    );
    let positions: [[f32; 3]; 6] = [
        [-1., -1., 0.],
        [1., -1., 0.],
        [1., 1., 0.],
        [-1., -1., 0.],
        [1., 1., 0.],
        [-1., 1., 0.],
    ];
    let vertices = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Original shadowcolor image quad"),
            contents: bytemuck::cast_slice(&positions),
            usage: wgpu::BufferUsages::VERTEX,
        });
    let frames = (0..2)
        .map(|_| {
            let pixels = gpu.render_with_terrain(&mut post, [8, 8], &mut |rp, list, _, _| {
                if matches!(list, TerrainList::ShadowSolid | TerrainList::Solid) {
                    rp.set_vertex_buffer(0, vertices.slice(..));
                    rp.draw(0..6, 0..1);
                }
            });
            [pixels[0], pixels[1]]
        })
        .collect();
    (std::mem::take(&mut post.notes), frames)
}

#[test]
fn a_shadow_program_binding_a_buffer_it_draws_into_is_skipped() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::new() else { return };
    // The shadow pass draws into shadowcolor0 itself, which the image would bind too: wgpu
    // binds a texture one way per pass.
    let (notes, frames) = shadow_frames(
        &gpu,
        terrain_pack(
            "shadow",
            "#version 430 compatibility\nlayout(rgba8) writeonly uniform image2D shadowcolorimg0;\n\
             void main() {\n    imageStore(shadowcolorimg0, ivec2(gl_FragCoord.xy), vec4(0.0, 1.0, 0.0, 1.0));\n\
                 gl_FragData[0] = vec4(1.0, 0.0, 0.0, 1.0);\n}\n",
        ),
    );
    assert!(
        notes
            .iter()
            .any(|n| n
                .ends_with(": draws into shadowcolor0 and binds it as `shadowcolorimg0`; skipped")),
        "{notes:?}"
    );
    // Nothing draws: both buffers keep their clear colour.
    for f in frames {
        assert_eq!(f, [[255; 4]; 2]);
    }
}

#[test]
fn a_shadow_program_stores_into_a_buffer_it_does_not_draw_into() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::new() else { return };
    let (notes, frames) = shadow_frames(
        &gpu,
        terrain_pack(
            "shadow",
            "#version 430 compatibility\nlayout(rgba8) writeonly uniform image2D shadowcolorimg1;\n\
             void main() {\n    imageStore(shadowcolorimg1, ivec2(gl_FragCoord.xy), vec4(0.0, 0.6, 0.2, 1.0));\n\
                 gl_FragData[0] = vec4(1.0, 0.0, 0.0, 1.0);\n}\n",
        ),
    );
    // The terrain notes are the fixture's (no gbuffers programs); none is about the image.
    assert!(
        !notes.iter().any(|n| n.contains("shadowcolorimg")),
        "{notes:?}"
    );
    for f in frames {
        assert_eq!(f, [[255, 0, 0, 255], [0, 153, 51, 255]]);
    }
}

#[test]
fn a_gbuffers_program_stores_into_a_shadowcolor_buffer_while_drawing_its_own() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::new() else { return };
    // gbuffers_terrain draws into colortex0, a buffer of the other kind than shadowcolor0.
    let (notes, frames) = shadow_frames(
        &gpu,
        terrain_pack(
            "gbuffers_terrain",
            "#version 430 compatibility\nlayout(rgba8) writeonly uniform image2D shadowcolorimg0;\n\
             void main() {\n    imageStore(shadowcolorimg0, ivec2(gl_FragCoord.xy), vec4(0.2, 0.4, 0.6, 1.0));\n\
                 gl_FragData[0] = vec4(1.0, 0.0, 0.0, 1.0);\n}\n",
        ),
    );
    assert!(
        !notes.iter().any(|n| n.contains("shadowcolorimg")),
        "{notes:?}"
    );
    for f in frames {
        assert_eq!(f, [[51, 102, 153, 255], [255; 4]]);
    }
}

#[test]
fn a_shadowcomp_program_binds_the_buffer_it_draws_into_from_the_other_side() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Fixture::new() else { return };
    // shadowcomp draws into shadowcolor0's "alt" side and binds "main" as the image: two
    // textures, so it runs. It loads "main" (its clear colour, 0.2) and draws twice that, which
    // becomes "main" when it flips.
    let mut post = gpu.build(pack_with(
        &[
            ("shadowcomp.vsh", VERTEX),
            (
                "shadowcomp.fsh",
                "#version 430 compatibility\n/* RENDERTARGETS: 0 */\n\
                 layout(r32f) readonly uniform image2D shadowcolorimg0;\n\
                 void main() { gl_FragData[0] = imageLoad(shadowcolorimg0, ivec2(gl_FragCoord.xy)) * 2.0; }\n",
            ),
        ],
        "/* const int shadowcolor0Format = R32F; */\n\
         const vec4 shadowcolor0ClearColor = vec4(0.2, 0.0, 0.0, 1.0);\n",
    ));
    // Only shadow programs bind it: the Shadow Cache may keep the map.
    assert_eq!(post.shadow_images_outside, 0);
    for f in frames(&gpu, &mut post) {
        assert_eq!(f, [[102, 0, 0, 255], [255; 4]]);
    }
}
