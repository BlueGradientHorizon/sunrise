//! The block outline drawn by a shader pack's `gbuffers_line` (or `gbuffers_basic`, as in Iris),
//! so packs that style it (Complementary Reimagined, Super Duper Vanilla) draw it their way, and
//! packs that render the world into part of their buffers (SEUS PTGI HRR) move it with the rest.
//! The edges are the outline buffer Sunrise draws itself, one instance per edge, widened to a
//! strip as `outline_clip` in terrain.wgsl does.

/// What `gbuffers_line` reads from an outline edge: its camera-relative ends, ARGB colour and
/// width in pixels. The strip's corners 0 and 1 are at the start, 2 and 3 at the end (six
/// vertices per edge: pack pipelines draw triangle lists), and a vertex's `gl_VertexID % 2` is
/// its corner's side. The width is applied in screen space after the pack's transform, unless
/// the pack's vertex stage reads its vertex index and the line's direction
/// (`hz_places_vertices`): that one widens the line itself, as vanilla's `rendertype_lines.vsh`
/// does.
pub fn inputs() -> sunrise_shaders::GbuffersInputs {
    let declarations =
        "layout(location = 0) in vec3 hz_l_start;\nlayout(location = 1) in vec3 hz_l_end;\n\
         layout(location = 2) in vec4 hz_l_argb;\nlayout(location = 3) in float hz_l_width;\n\
         int hz_l_c;\n"
            .to_owned();
    // Pack pipelines draw triangle lists: six vertices per edge, the strip's corners 0 1 2 and
    // 3 2 1, so a vertex's `gl_VertexID % 2` is its corner's side, as in vanilla's lines
    // (`rendertype_lines.vsh` offsets a vertex by that parity; Complementary's `gbuffers_line`
    // does the same).
    let body = "    int hz_l_v = int(gl_VertexIndex);\n    \
         hz_l_c = hz_l_v < 4 ? hz_l_v : hz_l_v == 4 ? 2 : 1;\n    \
         hz_Vertex = vec4(hz_l_c >= 2 ? hz_l_end : hz_l_start, 1.0);\n    \
         hz_Color = hz_l_argb.zyxw;\n    \
         hz_Normal = normalize(hz_l_end - hz_l_start);\n    \
         hz_MultiTexCoord0 = vec4(0.0, 0.0, 0.0, 1.0);\n    \
         hz_MultiTexCoord1 = vec4(240.0, 240.0, 0.0, 1.0);\n    \
         hz_MultiTexCoord2 = hz_MultiTexCoord1;\n"
        .to_owned();
    // ARGB in a little-endian u32 reads as B, G, R, A.
    let attributes = [
        ("vaPosition", "hz_Vertex.xyz"),
        ("vaColor", "hz_Color"),
        ("vaNormal", "hz_Normal"),
    ];
    let after = "    if (!hz_places_vertices) {\n        \
         vec4 hz_l_a = hz_ModelViewProjectionMatrix * vec4(hz_l_start, 1.0);\n        \
         vec4 hz_l_b = hz_ModelViewProjectionMatrix * vec4(hz_l_end, 1.0);\n        \
         vec2 hz_l_d = (hz_l_b.xy / hz_l_b.w - hz_l_a.xy / hz_l_a.w) * hz_target.xy;\n        \
         hz_l_d = dot(hz_l_d, hz_l_d) > 1e-12 ? normalize(hz_l_d) : vec2(1.0, 0.0);\n        \
         vec2 hz_l_o = vec2(-hz_l_d.y, hz_l_d.x) * hz_l_width / hz_target.xy;\n        \
         if (hz_l_o.x < 0.0) hz_l_o = -hz_l_o;\n        \
         if ((hz_l_c & 1) != 0) hz_l_o = -hz_l_o;\n        \
         gl_Position.xy += hz_l_o * gl_Position.w;\n    }\n"
        .to_owned();
    sunrise_shaders::GbuffersInputs {
        declarations,
        body,
        attributes: attributes
            .iter()
            .map(|&(n, v)| (n.to_owned(), v.to_owned()))
            .collect(),
        after,
        alpha_test: None,
        parts: None,
        item_id_declared: false,
        far_depth: false,
        paged_maps: false,
        water_mask: false,
        water_patches: false,
    }
}

/// The outline edges as the pack's lines program reads them: one 32-byte instance per edge.
pub fn layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4, 3 => Float32];
    wgpu::VertexBufferLayout {
        array_stride: 32,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &ATTRIBUTES,
    }
}
