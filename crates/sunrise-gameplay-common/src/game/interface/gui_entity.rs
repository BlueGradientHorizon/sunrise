

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

fn vertex(pos: [f32; 3], w: i8, u: f32) -> Vertex {
        Vertex {
            pos,
            uv: [u, 0.0],
            layer: 0,
            color: 0xFFFF_FFFF,
            light: 0,
            normal: [0, 0, 0, w],
            glint_uv: [0.0; 2],
        }
    }
/// A face at depth `z` looking towards the viewer (or away, reversed), its
    /// vertices' normal w `w` and its UVs marked `u`.
    pub fn face(z: f32, w: i8, u: f32, away: bool) -> [Vertex; 4] {
        let mut f = [
            vertex([0.0, 0.0, z], w, u),
            vertex([0.0, 1.0, z], w, u),
            vertex([1.0, 1.0, z], w, u),
            vertex([1.0, 0.0, z], w, u),
        ];
        if away {
            f.reverse();
        }
        f
    }
}
