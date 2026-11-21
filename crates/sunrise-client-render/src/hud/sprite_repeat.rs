//! Draw a subpixel repeat as one quad, resolving texels at device pixel centers. Separable
//! repeats upload the two axes' sample strips; affine repeats upload only exact coefficients,
//! so overlapping draws never need separate render-target-sized maps or staging slices.

use super::{HudBatch, HudVertex};
use crate::{gfx::upload::Uploads, hud_layout::Pose};
use num_bigint::{BigInt, Sign};
use std::ops::Range;

pub(crate) struct SpriteRepeat {
    pub(crate) pose: Pose,
    pub(crate) origin: [f32; 2],
    pub(crate) period: [f32; 2],
    pub(crate) source: [SpriteTexelRegion; 2],
}

#[derive(Clone, Copy)]
pub(crate) enum SpriteTexelEdge {
    Packed(u32),
    // Polytone can explicitly replace a packed edge with an arbitrary atlas UV.
    Normalized(f32),
}

#[derive(Clone, Copy)]
pub(crate) struct SpriteTexelRegion {
    pub(crate) edges: [SpriteTexelEdge; 2],
    pub(crate) offset: f32,
    pub(crate) size: f32,
}

// Every finite f32 is an integer multiple of 2^-149. Keep those integers until after
// modular reduction: dividing first, even in f64, can round away the whole tile phase.
const SHIFT: usize = 149;
fn units(value: f32) -> BigInt {
    let bits = value.to_bits();
    let exponent = (bits >> 23) & 0xff;
    let fraction = bits & 0x7f_ffff;
    let mantissa = fraction | (u32::from(exponent != 0) << 23);
    let magnitude = BigInt::from(mantissa) << exponent.saturating_sub(1) as usize;
    if bits >> 31 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

struct TexelAxis {
    start: BigInt,
    dx: BigInt,
    dy: BigInt,
    period: BigInt,
    uv_start: BigInt,
    uv_step: BigInt,
    uv_divisor: BigInt,
    extent: u32,
}

impl TexelAxis {
    fn sample(&self, phase: &BigInt) -> u16 {
        let numerator = &self.uv_start + phase * &self.uv_step;
        if numerator.sign() != Sign::Plus {
            return 0;
        }
        // Nearest sampling floors the texel-space coordinate. Divide only here, after
        // both the exact tile wrap and the atlas subrect transform, so neither a tile
        // seam nor an interior texel edge can be crossed by quantizing coefficients.
        let index = numerator / &self.uv_divisor;
        if index.bits() > u64::from(u32::BITS) {
            (self.extent - 1) as u16
        } else {
            index
                .iter_u32_digits()
                .next()
                .unwrap_or(0)
                .min(self.extent - 1) as u16
        }
    }

    fn advance(&self, phase: &mut BigInt, step: &BigInt) {
        *phase += step;
        if *phase >= self.period {
            *phase -= &self.period;
        }
    }

    // One row of the affine record (hud.wgsl): [limb count, offset sign, highest quotient bit],
    // then phase origin, X step, Y step, period, offset magnitude and shifted divisor, all
    // in little-endian limbs. Multiply the phase terms by the texel slope up front so the
    // shader needs only modular additions and comparisons/subtractions.
    fn affine_words(self) -> Vec<u32> {
        if self.extent == 1
            || self.uv_step.sign() == Sign::NoSign
            || (self.dx.sign() == Sign::NoSign && self.dy.sign() == Sign::NoSign)
        {
            return vec![0, u32::from(self.sample(&self.start)), 0];
        }
        let (phase, dx, dy, offset, step) = if self.uv_step.sign() == Sign::Minus {
            // Reversing the integer phase order makes a mirrored UV slope positive.
            let last = &self.period - 1u32;
            let negate_mod = |step: BigInt| {
                if step.sign() == Sign::NoSign {
                    step
                } else {
                    &self.period - step
                }
            };
            (
                &last - self.start,
                negate_mod(self.dx),
                negate_mod(self.dy),
                self.uv_start + &last * &self.uv_step,
                -self.uv_step,
            )
        } else {
            (self.start, self.dx, self.dy, self.uv_start, self.uv_step)
        };
        let high_shift = (self.extent - 1).ilog2();
        let negative_offset = offset.sign() == Sign::Minus;
        let terms = [
            phase * &step,
            dx * &step,
            dy * &step,
            self.period * &step,
            offset,
            self.uv_divisor << high_shift,
        ];
        // One extra bit holds a sum of two reduced phases, or a phase plus the offset.
        let words = (terms.iter().map(BigInt::bits).max().unwrap_or(0) + 1).div_ceil(32) as usize;
        let mut data = Vec::with_capacity(3 + terms.len() * words);
        data.extend([words as u32, u32::from(negative_offset), 1 << high_shift]);
        for term in terms {
            let end = data.len() + words;
            data.extend(term.iter_u32_digits());
            data.resize(end, 0);
        }
        data
    }
}

struct RepeatSamples {
    // In hud.wgsl: 1 marks strips, bits 1 and 2 select a device axis per strip, 8 marks affine.
    mode: u32,
    size: [u32; 2],
    data: Vec<u32>,
}

impl SpriteRepeat {
    fn samples(
        &self,
        scale: f32,
        atlas: [u32; 2],
        [left, top]: [u32; 2],
        size: [u32; 2],
    ) -> RepeatSamples {
        let [a, b, c, d, tx, ty] = self.pose.map(units);
        let determinant = &a * &d - &b * &c;
        let scale = units(scale);
        let [ox, oy] = self.origin.map(units);
        let [w, h] = self.period.map(units);
        let axes = [
            (d, -c, ox, w, self.source[0], atlas[0]),
            (-b, a, oy, h, self.source[1], atlas[1]),
        ]
        .map(|(u, v, origin, tile, source, extent)| {
            // Inverse pose at device centers, minus the local origin, divided by the
            // tile period. Give both slopes and the offset a common denominator.
            let dx = &u << (3 * SHIFT);
            let dy = &v << (3 * SHIFT);
            let translation = ((u * &tx + v * &ty) * &scale) << (SHIFT + 1);
            let local_origin = (origin * &determinant * &scale) << 1usize;
            let start = &dx * (2 * u64::from(left) + 1) + &dy * (2 * u64::from(top) + 1)
                - translation
                - local_origin;
            let denominator = (&determinant * &tile * &scale) << 1usize;
            let sign = denominator.sign();
            let period = BigInt::from(denominator.magnitude().clone());
            let reduce = |value: BigInt| {
                let reduced = if sign == Sign::Minus { -value } else { value } % &period;
                if reduced.sign() == Sign::Minus {
                    reduced + &period
                } else {
                    reduced
                }
            };
            let dx = reduce(dx << 1usize);
            let dy = reduce(dy << 1usize);
            let start = reduce(start);
            // Cancel their common power of two before stepping over device samples.
            let shift = [&dx, &dy, &start, &period]
                .into_iter()
                .filter_map(BigInt::trailing_zeros)
                .min()
                .unwrap_or(0) as usize;
            let period = period >> shift;
            // The source offsets are in metadata units, not atlas UVs. Compute the
            // fractional texel edges from the full integer placement, then wrap and
            // interpolate using one denominator, without rounding either edge.
            let [uv0, uv1] = source.edges.map(|edge| match edge {
                SpriteTexelEdge::Packed(texel) => BigInt::from(texel) << SHIFT,
                SpriteTexelEdge::Normalized(uv) => units(uv) * extent,
            });
            let span = uv1 - &uv0;
            let size = units(source.size);
            let uv_start = (uv0 * &size + &span * units(source.offset)) * &period;
            let uv_step = span * tile;
            let uv_divisor = (&period * size) << SHIFT;
            let shift_uv = [&uv_start, &uv_step, &uv_divisor]
                .into_iter()
                .filter_map(BigInt::trailing_zeros)
                .min()
                .unwrap_or(0) as usize;
            TexelAxis {
                dx: dx >> shift,
                dy: dy >> shift,
                start: start >> shift,
                uv_start: uv_start >> shift_uv,
                uv_step: uv_step >> shift_uv,
                uv_divisor: uv_divisor >> shift_uv,
                period,
                extent,
            }
        });
        // Quarter-turns and reflections can exchange the axes. Pack the two strips into
        // the low and high halves of a row: each atlas index fits in 16 bits.
        let directions = axes.each_ref().map(|axis| {
            if axis.dy.sign() == Sign::NoSign {
                Some(0)
            } else if axis.dx.sign() == Sign::NoSign {
                Some(1)
            } else {
                None
            }
        });
        if let [Some(u), Some(v)] = directions {
            let width = size[u].max(size[v]);
            let mut data = vec![0; width as usize];
            for (i, (axis, direction)) in axes.iter().zip([u, v]).enumerate() {
                let mut phase = axis.start.clone();
                let stride = [&axis.dx, &axis.dy][direction];
                for entry in &mut data[..size[direction] as usize] {
                    *entry |= u32::from(axis.sample(&phase)) << (16 * i);
                    axis.advance(&mut phase, stride);
                }
            }
            RepeatSamples {
                mode: 1 | ((u as u32) << 1) | ((v as u32) << 2),
                size: [width, 1],
                data,
            }
        } else {
            let [mut data, v] = axes.map(TexelAxis::affine_words);
            let width = data.len().max(v.len());
            data.resize(width, 0);
            data.extend(v);
            data.resize(width * 2, 0);
            RepeatSamples {
                mode: 8,
                size: [width as u32, 2],
                data,
            }
        }
    }
}

pub(super) struct RepeatMaps {
    pub(super) layout: wgpu::BindGroupLayout,
    pub(super) ordinary: wgpu::BindGroup,
    maps: Vec<Option<Map>>,
}

struct Map {
    vertex: [u32; 3],
    group: wgpu::BindGroup,
}

// A word is either a packed pair of 16-bit atlas indices or one affine coefficient limb.
fn map_texture(device: &wgpu::Device, [width, height]: [u32; 2]) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("HUD repeat texels"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

impl RepeatMaps {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HUD repeat map layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let ordinary = Self::bind(device, &layout, &map_texture(device, [1, 1]));
        Self {
            layout,
            ordinary,
            maps: Vec::new(),
        }
    }

    fn bind(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: &wgpu::Texture,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HUD repeat map"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(
                    &texture.create_view(&Default::default()),
                ),
            }],
        })
    }

    // The renderer supplies device bounds rather than the GUI size: a zoomed screenshot
    // has a different lattice of pixel centers than the live window.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        uploads: &mut Uploads,
        encoder: &mut wgpu::CommandEncoder,
        batch: &HudBatch,
        size: [u32; 2],
        atlas: [u32; 2],
        scale: f32,
    ) {
        self.maps.clear();
        if batch.sprite_repeats.is_empty() || batch.vertices.is_empty() || size.contains(&0) {
            return;
        }
        let mut bounds = vec![
            [
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY
            ];
            batch.sprite_repeats.len()
        ];
        for vertex in &batch.vertices {
            if let Some(index) = vertex.repeat[0].checked_sub(1) {
                let b = &mut bounds[index as usize];
                for (i, coord) in vertex.pos.into_iter().enumerate() {
                    let pixel = coord * scale;
                    b[i] = b[i].min(pixel);
                    b[i + 2] = b[i + 2].max(pixel);
                }
            }
        }
        self.maps.extend(
            batch
                .sprite_repeats
                .iter()
                .zip(bounds)
                .map(|(repeat, [l, t, r, b])| {
                    let origin = [
                        (l.floor() as u32).min(size[0]),
                        (t.floor() as u32).min(size[1]),
                    ];
                    let end = [
                        (r.ceil() as u32).min(size[0]),
                        (b.ceil() as u32).min(size[1]),
                    ];
                    if end[0] <= origin[0] || end[1] <= origin[1] {
                        return None;
                    }
                    let size = [end[0] - origin[0], end[1] - origin[1]];
                    let samples = repeat.samples(scale, atlas, origin, size);
                    let texture = map_texture(device, samples.size);
                    uploads.texture(
                        encoder,
                        texture.as_image_copy(),
                        samples.size[0],
                        samples.size[1],
                        bytemuck::cast_slice(&samples.data),
                    );
                    Some(Map {
                        vertex: [samples.mode, origin[0], origin[1]],
                        group: Self::bind(device, &self.layout, &texture),
                    })
                }),
        );
    }

    pub(super) fn vertex(&self, repeat: u32) -> Option<[u32; 3]> {
        self.maps[repeat as usize - 1]
            .as_ref()
            .map(|map| map.vertex)
    }

    pub(super) fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        vertices: &[HudVertex],
        range: Range<u32>,
    ) {
        if self.maps.is_empty() {
            pass.set_bind_group(1, &self.ordinary, &[]);
            pass.draw(range, 0..1);
            return;
        }
        let mut start = range.start;
        for run in vertices[range.start as usize..range.end as usize]
            .chunk_by(|a, b| a.repeat[0] == b.repeat[0])
        {
            let end = start + run.len() as u32;
            let group = if let Some(index) = run[0].repeat[0].checked_sub(1) {
                self.maps[index as usize].as_ref().map(|map| &map.group)
            } else {
                Some(&self.ordinary)
            };
            if let Some(group) = group {
                pass.set_bind_group(1, group, &[]);
                pass.draw(start..end, 0..1);
            }
            start = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_uploads_do_not_store_a_device_pixel_grid() {
        let source = SpriteTexelRegion {
            edges: [SpriteTexelEdge::Packed(5), SpriteTexelEdge::Packed(9)],
            offset: 0.0,
            size: 1.0,
        };
        let repeat = SpriteRepeat {
            pose: [0.75, 0.0, 0.0, 2.0, 0.0, 0.0],
            origin: [0.0; 2],
            period: [1.0; 2],
            source: [source; 2],
        };
        let target = [3840, 2160];
        let atlas = [4096, 5120];
        let strips = repeat.samples(1.0, atlas, [0, 0], target);
        assert_eq!(strips.size, [3840, 1]);
        assert_eq!(strips.data.len(), 3840);

        let shear = SpriteRepeat {
            pose: [0.75, 0.25, 0.0, 2.0, 0.0, 0.0],
            ..repeat
        };
        let large = shear.samples(1.0, atlas, [0, 0], target);
        let small = shear.samples(1.0, atlas, [0, 0], [4, 6]);
        // The same affine record covers either draw, not a map of its bounding box.
        assert_eq!(large.size, small.size);
        assert_eq!(large.data, small.data);
        assert!(large.data.len() < 64);
    }
}
