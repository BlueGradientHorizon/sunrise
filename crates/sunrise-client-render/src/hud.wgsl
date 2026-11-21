struct Hud {
    size: vec4<f32>,
    // Blend::EndPortal: atlas rects (u0, v0, width, height) of end_sky.png and
    // end_portal.png, then vanilla's GameTime in x and PvP & HUD's glint colour in yzw
    // (y below 0: vanilla's).
    end_portal: array<vec4<f32>, 3>,
}
@group(0) @binding(0) var<uniform> hud: Hud;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
// enchanted_glint_item on its own repeating texture (vanilla's GlintSampler).
@group(0) @binding(3) var glint_tex: texture_2d<f32>;
@group(0) @binding(4) var glint_sampler: sampler;
@group(0) @binding(5) var linear_sampler: sampler;
// enchanted_glint_armor, the armour layers' GlintSampler (entity.wgsl's glint_armor_tex).
@group(0) @binding(6) var glint_armor_tex: texture_2d<f32>;
@group(1) @binding(5) var repeat_texels: texture_2d<u32>;
struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    // projection_from_position(gl_Position): the End portal's texProj0.
    @location(2) proj: vec4<f32>,
    @location(3) glint_uv: vec2<f32>,
    @location(4) @interpolate(flat) repeat: vec3<u32>,
}
@vertex fn vs_main(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>, @location(3) depth: f32, @location(4) glint_uv: vec2<f32>, @location(6) repeat: vec3<u32>) -> Out {
    let pixels = pos * hud.size.z;
    var out: Out;
    out.clip = vec4(pixels.x / hud.size.x * 2.0 - 1.0, 1.0 - pixels.y / hud.size.y * 2.0, depth, 1.0);
    out.uv = uv;
    out.color = color;
    out.proj = vec4(out.clip.xy * 0.5 + 0.5, out.clip.zw);
    out.glint_uv = glint_uv;
    out.repeat = repeat;
    return out;
}
// Exact affine repeat coefficients, serialized by TexelAxis::affine_words. A finite f32
// in 2^-149 units takes at most 277 bits. The phase denominator is twice a two-product
// determinant times tile size and scale (1110 bits). The texel transform adds at most
// a source coordinate, an atlas UV times the atlas extent, and carry bits (< 1728 total).
// Only the occupied limbs are loaded and used, usually one word per coefficient.
alias RepeatInt = array<u32, 54>;
fn repeat_coefficient(row: i32, column: u32, words: u32) -> RepeatInt {
    var value: RepeatInt;
    for (var i = 0u; i < words; i++) {
        value[i] = textureLoad(repeat_texels, vec2(i32(column + i), row), 0).x;
    }
    return value;
}
fn repeat_at_least(a: ptr<function, RepeatInt>, b: ptr<function, RepeatInt>, words: u32) -> bool {
    for (var i = words; i > 0u; i--) {
        let j = i - 1u;
        if (*a)[j] != (*b)[j] { return (*a)[j] > (*b)[j]; }
    }
    return true;
}
fn repeat_add(a: ptr<function, RepeatInt>, b: ptr<function, RepeatInt>, words: u32) {
    var carry = 0u;
    for (var i = 0u; i < words; i++) {
        let x = (*a)[i];
        let sum = x + (*b)[i];
        let result = sum + carry;
        (*a)[i] = result;
        carry = select(0u, 1u, sum < x || result < sum);
    }
}
fn repeat_sub(a: ptr<function, RepeatInt>, b: ptr<function, RepeatInt>, words: u32) {
    var borrow = 0u;
    for (var i = 0u; i < words; i++) {
        let x = (*a)[i];
        let difference = x - (*b)[i];
        let result = difference - borrow;
        (*a)[i] = result;
        borrow = select(0u, 1u, difference > x || result > difference);
    }
}
fn repeat_mod_add(a: ptr<function, RepeatInt>, b: ptr<function, RepeatInt>,
                  period: ptr<function, RepeatInt>, words: u32) {
    repeat_add(a, b, words);
    if repeat_at_least(a, period, words) { repeat_sub(a, period, words); }
}
fn repeat_advance(phase: ptr<function, RepeatInt>, coefficient: RepeatInt, distance: u32,
                  period: ptr<function, RepeatInt>, words: u32) {
    var step = coefficient;
    var remaining = distance;
    while remaining != 0u {
        if (remaining & 1u) != 0u { repeat_mod_add(phase, &step, period, words); }
        remaining >>= 1u;
        if remaining == 0u { break; }
        // Double modulo the period in place.
        var carry = 0u;
        for (var i = 0u; i < words; i++) {
            let value = step[i];
            step[i] = (value << 1u) | carry;
            carry = value >> 31u;
        }
        if repeat_at_least(&step, period, words) { repeat_sub(&step, period, words); }
    }
}
fn repeat_affine_texel(pixel: vec2<u32>, row: i32, limit: u32) -> u32 {
    let words = textureLoad(repeat_texels, vec2(0, row), 0).x;
    let offset_field = textureLoad(repeat_texels, vec2(1, row), 0).x;
    if words == 0u { return offset_field; } // Constant atlas index.
    var bit = textureLoad(repeat_texels, vec2(2, row), 0).x;
    var phase = repeat_coefficient(row, 3u, words);
    var period = repeat_coefficient(row, 3u + 3u * words, words);
    repeat_advance(&phase, repeat_coefficient(row, 3u + words, words), pixel.x, &period, words);
    repeat_advance(&phase, repeat_coefficient(row, 3u + 2u * words, words), pixel.y, &period, words);
    var offset = repeat_coefficient(row, 3u + 4u * words, words);
    if offset_field == 0u {
        repeat_add(&phase, &offset, words);
    } else {
        if !repeat_at_least(&phase, &offset, words) { return 0u; }
        repeat_sub(&phase, &offset, words);
    }
    // Divide exactly, testing only the quotient bits that could fit in the atlas.
    // A quotient larger than that range saturates before the final clamp.
    var divisor = repeat_coefficient(row, 3u + 5u * words, words);
    var texel = 0u;
    while bit != 0u {
        if repeat_at_least(&phase, &divisor, words) {
            repeat_sub(&phase, &divisor, words);
            texel |= bit;
        }
        bit >>= 1u;
        var carry = 0u;
        for (var i = words; i > 0u; i--) {
            let j = i - 1u;
            let low = divisor[j] & 1u;
            divisor[j] = (divisor[j] >> 1u) | carry;
            carry = low << 31u;
        }
    }
    return min(texel, limit);
}

// Nearest sampling in the selected atlas subrect, resolved at the active device scale.
fn sprite_sample(in: Out) -> vec4<f32> {
    let ordinary = textureSample(atlas, atlas_sampler, in.uv);
    if in.repeat.x == 0u {
        return ordinary;
    }
    let pixel = vec2<u32>(floor(in.clip.xy)) - in.repeat.yz;
    var texel: vec2<u32>;
    if in.repeat.x == 8u {
        let limits = textureDimensions(atlas, 0) - vec2(1u);
        texel = vec2(repeat_affine_texel(pixel, 0, limits.x),
                     repeat_affine_texel(pixel, 1, limits.y));
    } else {
        let u = select(pixel.x, pixel.y, (in.repeat.x & 2u) != 0u);
        let v = select(pixel.x, pixel.y, (in.repeat.x & 4u) != 0u);
        texel = vec2(textureLoad(repeat_texels, vec2(i32(u), 0), 0).x & 0xffffu,
                     textureLoad(repeat_texels, vec2(i32(v), 0), 0).x >> 16u);
    }
    return textureLoad(atlas, vec2<i32>(texel), 0);
}
@fragment fn fs_main(in: Out) -> @location(0) vec4<f32> {
    // u >= 2: the GLINT_TEXTURING sample over the item, masked by the item's
    // texture alpha (item.fsh's ALPHA_CUTOUT and GLINT path). u >= 4: the same
    // over a GUI model's armour, from enchanted_glint_armor (ARMOR_ENTITY_GLINT).
    if in.repeat.x == 0u && in.uv.x >= 2.0 {
        let armor = in.uv.x >= 4.0;
        let offset = select(2.0, 4.0, armor);
        let mask = textureSample(atlas, atlas_sampler, in.uv - vec2(offset, 0.0)).a;
        let item_sample = textureSample(glint_tex, glint_sampler, in.glint_uv);
        let armor_sample = textureSample(glint_armor_tex, glint_sampler, in.glint_uv);
        if mask < 0.1 { discard; }
        var glint = hud.size.w * select(item_sample, armor_sample, armor);
        let tint = hud.end_portal[2].yzw;
        if tint.x >= 0.0 {
            glint = vec4(max(glint.r, max(glint.g, glint.b)) * tint, glint.a);
        }
        return vec4(glint.rgb * glint.rgb, mask);
    }
    let c = sprite_sample(in) * in.color;
    if c.a < 0.004 { discard; }
    return c;
}
// OptiFine CIT glint layers (hud/cit_glint.rs): every rule's texture in one array, the slot in
// the vertex colour's red and the layer's intensity in its alpha. Masked by the item's texture
// as the vanilla glint is, then blended by the pipeline the rule's `blend` asks for.
struct CitSlot {
    rgba: vec4<f32>,
    // x: linear filtering (`blur`).
    blur: vec4<f32>,
}
struct CitSlots {
    slots: array<CitSlot, 256>,
}
// The glint array is bound twice, once per sampler: naga's GLSL writer pairs each image with one
// sampler, so one image sampled through both cannot run on OpenGL (#2386).
@group(1) @binding(0) var cit_tex: texture_2d_array<f32>;
@group(1) @binding(1) var cit_blur: sampler;
@group(1) @binding(2) var cit_nearest: sampler;
@group(1) @binding(3) var<uniform> cit: CitSlots;
@group(1) @binding(4) var cit_tex_sharp: texture_2d_array<f32>;
@fragment fn fs_cit_glint(in: Out) -> @location(0) vec4<f32> {
    let mask = textureSample(atlas, atlas_sampler, in.uv).a;
    let index = u32(round(in.color.r * 255.0));
    let slot = cit.slots[index];
    let blurred = textureSampleLevel(cit_tex, cit_blur, in.glint_uv, index, 0.0);
    let sharp = textureSampleLevel(cit_tex_sharp, cit_nearest, in.glint_uv, index, 0.0);
    if mask < 0.1 { discard; }
    // The layer's colour times its intensity (the glint strength option included) in alpha,
    // cut where vanilla's glint shader cuts, which CIT Resewn draws through.
    var t = select(sharp, blurred, slot.blur.x > 0.5) * slot.rgba;
    t.a *= in.color.a;
    if t.a < 0.1 { discard; }
    return t;
}
@fragment fn fs_crosshair(in: Out) -> @location(0) vec4<f32> {
    let c = sprite_sample(in) * in.color;
    if c.a == 0.0 { discard; }
    return c;
}
// RenderPipelines.VIGNETTE (WinScreen's credits vignette): bilinear sample of
// credits_vignette.png (which has blur: true in its .mcmeta and a wrapped border in the atlas).
@fragment fn fs_vignette(in: Out) -> @location(0) vec4<f32> {
    let c = textureSample(atlas, linear_sampler, in.uv) * in.color;
    return vec4(c.rgb, 1.0);
}
// rendertype_end_portal.fsh (entity.wgsl has the same for portal blocks).
const PORTAL_COLORS = array<vec3<f32>, 16>(
    vec3(0.022087, 0.098399, 0.110818),
    vec3(0.011892, 0.095924, 0.089485),
    vec3(0.027636, 0.101689, 0.100326),
    vec3(0.046564, 0.109883, 0.114838),
    vec3(0.064901, 0.117696, 0.097189),
    vec3(0.063761, 0.086895, 0.123646),
    vec3(0.084817, 0.111994, 0.166380),
    vec3(0.097489, 0.154120, 0.091064),
    vec3(0.106152, 0.131144, 0.195191),
    vec3(0.097721, 0.110188, 0.187229),
    vec3(0.133516, 0.138278, 0.148582),
    vec3(0.070006, 0.243332, 0.235792),
    vec3(0.196766, 0.142899, 0.214696),
    vec3(0.047281, 0.315338, 0.321970),
    vec3(0.204675, 0.390010, 0.302066),
    vec3(0.080955, 0.314821, 0.661491),
);
fn portal_sample(rect: vec4<f32>, p: vec4<f32>) -> vec3<f32> {
    let st = fract(p.xy / p.w);
    return textureSampleLevel(atlas, atlas_sampler, rect.xy + st * rect.zw, 0.0).rgb;
}
fn portal_layer(p: vec4<f32>, layer: f32) -> vec4<f32> {
    let a = radians((layer * layer * 4321.0 + layer * 9.0) * 2.0);
    let c = cos(a);
    let s = sin(a);
    let k = (4.5 - layer / 4.0) * 2.0;
    var v = vec4(k * (c * p.x - s * p.y), k * (s * p.x + c * p.y), p.z, p.w);
    v = vec4(v.x + 17.0 / layer * v.w, v.y + (2.0 + layer / 1.5) * (hud.end_portal[2].x * 1.5) * v.w, v.z, v.w);
    return vec4(0.5 * v.x + 0.25 * v.w, 0.5 * v.y + 0.25 * v.w, v.z, v.w);
}
// RenderPipelines.END_PORTAL (WinScreen's poem background): end_sky and 15
// layers of end_portal.png.
@fragment fn fs_end_portal(in: Out) -> @location(0) vec4<f32> {
    var c = portal_sample(hud.end_portal[0], in.proj) * PORTAL_COLORS[0];
    for (var i = 0; i < 15; i++) {
        c += portal_sample(hud.end_portal[1], portal_layer(in.proj, f32(i + 1))) * PORTAL_COLORS[i];
    }
    return vec4(c, 1.0);
}
