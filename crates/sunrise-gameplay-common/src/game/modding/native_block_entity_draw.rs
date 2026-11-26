

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

use sunrise_level::containers::{ModBlockEntity, ModBlockEntityFields};

pub fn sprite() -> Rect {
        Rect {
            u0: 0.5,
            v0: 0.25,
            u1: 0.75,
            v1: 0.5,
            w: 16,
            h: 16,
        }
    }
pub fn marker(pos: f32) -> Vertex {
        Vertex {
            pos: [pos, 0.0, 0.0],
            uv: [0.0, 0.0],
            layer: 0,
            color: 0,
            light: 0,
            normal: [0; 4],
            glint_uv: [0.0, 0.0],
        }
    }
pub fn rows(m: Mat4) -> [f32; 12] {
        let c = m.to_cols_array();
        [
            c[0], c[4], c[8], c[12], c[1], c[5], c[9], c[13], c[2], c[6], c[10], c[14],
        ]
    }
#[cfg(test)]
/// A frustum that sees nothing.
    struct Blind;
#[cfg(test)]
impl Cull for Blind {
        fn intersects(&self, _: Vec3, _: Vec3) -> bool {
            false
        }
        fn contains(&self, _: Vec3, _: Vec3) -> bool {
            false
        }
    }
pub fn machine() -> ModRevision {
        ModBlockEntity::new("mymod:machine".into(), Default::default()).revision()
    }
pub fn vertex(x: f32) -> Vertex {
        Vertex {
            pos: [x, 0.0, 0.0],
            ..bytemuck::Zeroable::zeroed()
        }
    }
pub const LIGHT_QUAD: [[f32; 3]; 4] = [
        [0.0, 1.0, 0.5],
        [0.0, 0.0, 0.5],
        [1.0, 0.0, 0.5],
        [1.0, 1.0, 0.5],
    ];
}
