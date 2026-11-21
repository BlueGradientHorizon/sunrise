//! A subpixel repeat needs only the texels hit by device pixel centers, not a quad per tile.
//! Resolve those texels with the active render target's scale, then let the HUD rasterize the
//! original geometry (including its scissor and blend) against that map.

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
}

impl SpriteRepeat {
    fn texels(
        &self,
        scale: f32,
        atlas: [u32; 2],
        [left, top]: [u32; 2],
        [width, height]: [u32; 2],
    ) -> Vec<[u16; 2]> {
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
        // Axis-aligned (including quarter-turned) sprites only need exact arithmetic
        // once per column or row. A shear/rotation can depend on both device axes.
        let columns = std::array::from_fn::<_, 2, _>(|i| {
            let axis = &axes[i];
            (axis.dy.sign() == Sign::NoSign).then(|| {
                let mut phase = axis.start.clone();
                (0..width)
                    .map(|_| {
                        let texel = axis.sample(&phase);
                        axis.advance(&mut phase, &axis.dx);
                        texel
                    })
                    .collect::<Vec<_>>()
            })
        });
        let mut row = axes.each_ref().map(|axis| axis.start.clone());
        let mut texels = Vec::with_capacity(width as usize * height as usize);
        for _ in 0..height {
            let fixed = std::array::from_fn::<_, 2, _>(|i| {
                (axes[i].dx.sign() == Sign::NoSign).then(|| axes[i].sample(&row[i]))
            });
            let mut phase = row.clone();
            for x in 0..width as usize {
                texels.push(std::array::from_fn(|i| {
                    if let Some(col) = &columns[i] {
                        col[x]
                    } else if let Some(texel) = fixed[i] {
                        texel
                    } else {
                        let axis = &axes[i];
                        let texel = axis.sample(&phase[i]);
                        axis.advance(&mut phase[i], &axis.dx);
                        texel
                    }
                }));
            }
            for (phase, axis) in row.iter_mut().zip(&axes) {
                axis.advance(phase, &axis.dy);
            }
        }
        texels
    }
}

pub(super) struct RepeatMaps {
    pub(super) layout: wgpu::BindGroupLayout,
    pub(super) ordinary: wgpu::BindGroup,
    maps: Vec<Option<Map>>,
}

struct Map {
    origin: [u32; 2],
    group: wgpu::BindGroup,
}

// Use Rg16Uint: one 4-byte upload texel contains two indices into the HUD atlas
// (each dimension of the atlas is at most MAX_ATLAS_SIZE).
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
        format: wgpu::TextureFormat::Rg16Uint,
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
                    let texels = repeat.texels(scale, atlas, origin, size);
                    let texture = map_texture(device, size);
                    uploads.texture(
                        encoder,
                        texture.as_image_copy(),
                        size[0],
                        size[1],
                        bytemuck::cast_slice(&texels),
                    );
                    Some(Map {
                        origin,
                        group: Self::bind(device, &self.layout, &texture),
                    })
                }),
        );
    }

    pub(super) fn origin(&self, repeat: u32) -> Option<[u32; 2]> {
        self.maps[repeat as usize - 1]
            .as_ref()
            .map(|map| map.origin)
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
