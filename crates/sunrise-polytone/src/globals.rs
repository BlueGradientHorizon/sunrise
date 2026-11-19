//! Polytone's globals (wiki "Extra Shader Uniforms"): the `PolyGlobals` block a pack's post and
//! core shaders may read, imported as `#moj_import <polytone_globals.glsl>`, and the built-in
//! file for a pack that imports it without shipping it. Written from the wiki's member names
//! and their meaning.

/// The block's name.
pub const BLOCK: &str = "PolyGlobals";
/// Its std140 size: two `mat4`, three floats, an `ivec3` and a `vec3`.
pub const BLOCK_SIZE: usize = 176;
/// The include's file name; `<polytone_globals.glsl>` reads it from `shaders/include/`.
pub const INCLUDE_FILE: &str = "polytone_globals.glsl";

/// `polytone_globals.glsl`: the block and its two helpers. 26.3's projection keeps depth from 0
/// to 1, so a depth read from the depth buffer is already the clip-space z.
pub const GLSL: &str = "#ifndef POLYTONE_GLOBALS_GLSL
#define POLYTONE_GLOBALS_GLSL
layout(std140) uniform PolyGlobals {
    mat4 PolyProjMat;
    mat4 PolyModelViewMat;
    float PolySunAngle;
    float PolyDayTime;
    float PolyDeltaTime;
    ivec3 PolyPlayerBlockPos;
    vec3 PolyPlayerOffset;
};
vec3 polyPlayerPos() {
    return vec3(PolyPlayerBlockPos) - PolyPlayerOffset;
}
vec3 polyPositionFromDepth(vec2 uv, float depth) {
    vec4 view = inverse(PolyProjMat) * vec4(uv * 2.0 - 1.0, depth, 1.0);
    return (inverse(PolyModelViewMat) * vec4(view.xyz / view.w, 1.0)).xyz;
}
#endif
";

/// Whether an include id (`ns:shaders/include/<path>`) names the built-in file.
pub fn is_include(id: &str) -> bool {
    id.strip_suffix(INCLUDE_FILE)
        .is_some_and(|rest| rest.ends_with(":shaders/include/"))
}

/// What `PolyGlobals` holds this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Globals {
    /// The world's projection and model-view (the camera's turn), column major.
    pub projection: [f32; 16],
    pub model_view: [f32; 16],
    /// The sun's angle in radians, 0 at the horizon.
    pub sun_angle: f32,
    /// The time of day in ticks (0 to 24000), and the game ticks this frame advanced.
    pub day_time: f32,
    pub delta_time: f32,
    /// The player's feet: `PolyPlayerBlockPos`, and `PolyPlayerOffset` as vanilla's
    /// `CameraOffset` is (the block position less the position, so `polyPlayerPos()` is the
    /// block position less the offset).
    pub player: [f64; 3],
}

impl Globals {
    /// The std140 block.
    pub fn std140(&self) -> [u8; BLOCK_SIZE] {
        let mut out = [0u8; BLOCK_SIZE];
        let mut put = |at: usize, bytes: [u8; 4]| out[at..at + 4].copy_from_slice(&bytes);
        for (m, matrix) in [self.projection, self.model_view].iter().enumerate() {
            for (i, v) in matrix.iter().enumerate() {
                put(64 * m + 4 * i, v.to_le_bytes());
            }
        }
        for (i, v) in [self.sun_angle, self.day_time, self.delta_time]
            .iter()
            .enumerate()
        {
            put(128 + 4 * i, v.to_le_bytes());
        }
        // ivec3 and vec3 each start on 16 bytes: 144 and 160.
        let block = self.player.map(f64::floor);
        for i in 0..3 {
            // A float saturates into the int, NaN at 0.
            put(144 + 4 * i, (block[i] as i32).to_le_bytes());
            put(
                160 + 4 * i,
                ((block[i] - self.player[i]) as f32).to_le_bytes(),
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The block's std140 layout: matrices at 0 and 64, the three floats from 128, the block
    /// position at 144 and the offset at 160 (the block position less the position).
    #[test]
    fn the_globals_follow_std140() {
        let mut projection = [0.0; 16];
        projection[0] = 2.0;
        let g = Globals {
            projection,
            sun_angle: 1.5,
            day_time: 6000.0,
            delta_time: 0.5,
            player: [10.25, -3.5, 7.0],
            ..Default::default()
        };
        let b = g.std140();
        let f = |at: usize| f32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let i = |at: usize| i32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        assert_eq!(f(0), 2.0);
        assert_eq!((f(128), f(132), f(136), f(140)), (1.5, 6000.0, 0.5, 0.0));
        assert_eq!((i(144), i(148), i(152)), (10, -4, 7));
        assert_eq!((f(160), f(164), f(168)), (-0.25, -0.5, 0.0));
        // A position past an int's range saturates.
        let far = Globals {
            player: [1e300, f64::NAN, -1e300],
            ..Default::default()
        }
        .std140();
        let i = |at: usize| i32::from_le_bytes(far[at..at + 4].try_into().unwrap());
        assert_eq!((i(144), i(148), i(152)), (i32::MAX, 0, i32::MIN));
    }

    #[test]
    fn the_include_is_named_in_any_namespace() {
        assert!(is_include(
            "minecraft:shaders/include/polytone_globals.glsl"
        ));
        assert!(is_include("polytone:shaders/include/polytone_globals.glsl"));
        assert!(!is_include("minecraft:shaders/include/fog.glsl"));
        assert!(!is_include("minecraft:shaders/core/polytone_globals.glsl"));
        assert!(GLSL.contains("uniform PolyGlobals"));
    }
}
