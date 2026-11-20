//! The block outline drawn by a shader pack's `gbuffers_line` (or `gbuffers_basic`, as in Iris),
//! so packs that style it (Complementary Reimagined, Super Duper Vanilla) draw it their way, and
//! packs that render the world into part of their buffers (SEUS PTGI HRR) move it with the rest.
//! The edges are the outline buffer Sunrise draws itself, one instance per edge, widened to a
//! strip as `outline_clip` in terrain.wgsl does.

/// What `gbuffers_line` reads from an outline edge: its camera-relative ends, ARGB colour and
/// width in pixels, and the width of the stroke drawn over it (`over_layout`). The strip's
/// corners 0 and 1 are at the start, 2 and 3 at the end (six vertices per edge: pack pipelines
/// draw triangle lists), and a vertex's `gl_VertexID % 2` is its corner's side.
///
/// The width is applied in screen space after the pack's transform, unless the pack widened
/// the line itself, as vanilla's `rendertype_lines.vsh` does (Complementary Reimagined's
/// `gbuffers_line` too): a vertex the pack moved off its line, across it, by more than
/// [`OWN_WIDTH_PIXELS`]. Then the stroke keeps the pack's width, and a stroke with one drawn
/// over it (the high contrast outline's black one) is widened by how much wider than that one
/// the game draws it, so it still shows around it.
pub fn inputs() -> sunrise_shaders::GbuffersInputs {
    let declarations =
        "layout(location = 0) in vec3 hz_l_start;\nlayout(location = 1) in vec3 hz_l_end;\n\
         layout(location = 2) in vec4 hz_l_argb;\nlayout(location = 3) in float hz_l_width;\n\
         layout(location = 4) in float hz_l_over;\nint hz_l_c;\n"
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
    // The edge's ends where the game puts them, through the transform the pack's programs get,
    // with the y flip `gl_Position` had before `after` (a gbuffers program draws into the pack's
    // buffers); `hz_l_n`, the unit perpendicular of the edge on screen. `hz_l_s` is how far, in
    // pixels, the pack put this vertex across the line through them (from either end, both on
    // it, as an unwidened vertex is): more than `OWN_WIDTH_PIXELS`, and the
    // pack widened the line, to the side of `hz_l_s`; the game then adds only what this stroke
    // has over the one drawn on it. Otherwise the game widens it to the side of its corner.
    let after = format!(
        "    {{\n        \
         vec4 hz_l_a = hz_ModelViewProjectionMatrix * vec4(hz_l_start, 1.0);\n        \
         vec4 hz_l_b = hz_ModelViewProjectionMatrix * vec4(hz_l_end, 1.0);\n        \
         vec2 hz_l_pa = vec2(hz_l_a.x, -hz_l_a.y) / hz_l_a.w;\n        \
         vec2 hz_l_pb = vec2(hz_l_b.x, -hz_l_b.y) / hz_l_b.w;\n        \
         vec2 hz_l_d = (hz_l_pb - hz_l_pa) * hz_target.xy;\n        \
         hz_l_d = dot(hz_l_d, hz_l_d) > 1e-12 ? normalize(hz_l_d) : vec2(1.0, 0.0);\n        \
         vec2 hz_l_n = vec2(-hz_l_d.y, hz_l_d.x);\n        \
         vec2 hz_l_moved = gl_Position.xy / gl_Position.w - hz_l_pa;\n        \
         float hz_l_s = dot(hz_l_moved * hz_target.xy * 0.5, hz_l_n);\n        \
         bool hz_l_own = abs(hz_l_s) > {OWN_WIDTH_PIXELS:?};\n        \
         float hz_l_side = hz_l_own ? sign(hz_l_s) : (hz_l_c & 1) != 0 ? -1.0 : 1.0;\n        \
         float hz_l_w = hz_l_own ? hz_l_width - hz_l_over : hz_l_width;\n        \
         gl_Position.xy += hz_l_n * (hz_l_side * hz_l_w) / hz_target.xy * gl_Position.w;\n    }}\n"
    );
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

/// How far across its line, in pixels, a vertex of the pack's line program must be for the
/// game to take it that the pack widened the line itself. A pack that only transforms the line
/// leaves its vertices on it: a uniform scale of the view (vanilla's `VIEW_SCALE` in
/// `rendertype_lines.vsh`) keeps x/w and y/w, and a temporal anti-aliasing jitter moves the
/// whole frame by at most half a pixel on each axis, so by at most half a pixel across a
/// horizontal or vertical edge. A pack that widens the line by vanilla's measure moves each
/// vertex by half the line's width: 1.25 pixels for vanilla's 2.5. Measured in game (936×1018,
/// each move drawn 21 times wider), Complementary Reimagined r5.9.3 moves its vertices between
/// 0.9 and 1 pixel across all four edges of a block's top, the same in every frame, and BSL
/// 10.1.8 doesn't move them. 0.75 is between half a pixel and 0.9.
pub const OWN_WIDTH_PIXELS: f32 = 0.75;

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

/// The width of the stroke drawn over each edge (`hz_l_over`): the outline buffer bound a
/// second time, from the next stroke's first edge for a high contrast outline's black stroke,
/// from its own otherwise, so that an edge with no stroke over it reads its own width.
pub fn over_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 1] = [wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 28,
        shader_location: 4,
    }];
    wgpu::VertexBufferLayout {
        array_stride: 32,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &ATTRIBUTES,
    }
}
