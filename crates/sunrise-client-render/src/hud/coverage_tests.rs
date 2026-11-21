use super::*;
use crate::{
    gfx::hud_coverage,
    hud_layout::Blend as SpriteBlend,
    ui::{GuiSpriteBlit, GuiSpriteScaling, NineSlice, blit_gui_sprite},
};

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];

#[test]
fn standalone_picture_draw_order_and_screen_exit_release() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(mut gpu) = Gpu::new() else { return };
    let image = Arc::new(pictures::Picture {
        id: u64::MAX,
        width: 1,
        height: 1,
        pixels: vec![255, 0, 0, 255],
        flags: TextureFlags::default(),
    });
    let weak = Arc::downgrade(&image);
    let mut batch = HudBatch::scratch([4, 2]);
    batch.pictures.push(image.clone());
    let rect = Rect {
        u0: 0.0,
        v0: 0.0,
        u1: 1.0,
        v1: 1.0,
        w: 1,
        h: 1,
    };
    batch.rect_uv(0.0, 0.0, 4.0, 2.0, rect, u32::MAX);
    batch.blend_since(0, Blend::Picture(image.id));
    batch.rect_uv(
        2.0,
        0.0,
        2.0,
        2.0,
        Rect {
            u0: 0.0,
            u1: 0.5,
            ..rect
        },
        rgba(0, 255, 0, 255),
    );
    let mut uploads = crate::gfx::upload::Uploads::new(&gpu.device);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    gpu.hud
        .prepare_pictures(&gpu.device, &mut uploads, &mut encoder, &batch);
    uploads.finish(&encoder);
    gpu.queue.submit([encoder.finish()]);
    let (pixels, mask) = gpu.draw(&batch, 0.0, 0.0, true);
    for row in pixels.as_chunks::<16>().0 {
        assert_eq!(&row[..8], &[255, 0, 0, 255, 255, 0, 0, 255]);
        assert_eq!(&row[8..], &[0, 255, 0, 255, 0, 255, 0, 255]);
    }
    assert!(mask.iter().all(|v| *v == 255));
    drop(image);
    drop(batch);
    assert!(weak.upgrade().is_some());
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    gpu.hud.prepare_pictures(
        &gpu.device,
        &mut uploads,
        &mut encoder,
        &HudBatch::scratch([4, 2]),
    );
    assert!(gpu.hud.pictures.is_empty());
    assert!(
        weak.upgrade().is_none(),
        "closing the screen releases pixels and the GPU bind group"
    );
    eprintln!("standalone picture GPU rendering and release exercised");
}

#[test]
#[ignore = "daily"]
fn tiled_sprites_sample_the_same_atlas_phase_at_device_pixel_centers() {
    let _gpu = crate::gfx::test_gpu::lock();
    let mut atlas = HudAtlas::boot();
    (atlas.width, atlas.height) = (1, 5120);
    atlas.pixels = vec![0; 5120 * 4];
    atlas.pixels[5 * 4..9 * 4].copy_from_slice(&[RED, GREEN, BLUE, YELLOW].concat());
    let Some(mut gpu) = Gpu::with_atlas([atlas.width, atlas.height], &atlas.pixels) else {
        return;
    };
    let atlas_rect = Rect {
        u0: 0.0,
        v0: 5.0 / 5120.0,
        u1: 1.0,
        v1: 9.0 / 5120.0,
        w: 1,
        h: 4,
    };
    let extreme = 3.0 * f32::from_bits(67 << 23); // 3 * 2^-60
    for (pose, dst, zoom, size, expected) in [
        // y=0.5 is phase 1/4, exactly row 6 of the offset [5, 9) sprite.
        // Interpolating the rounded atlas UVs in exact arithmetic falls below that edge.
        (
            [0.5, 0.0, 0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 8.0, 3.0],
            1.0,
            [4, 6],
            [GREEN, YELLOW, GREEN, YELLOW, GREEN, YELLOW],
        ),
        // Extreme scale: the first centers hit phases 2/3, 0 and 1/3.
        (
            [1.0, 0.0, 0.0, extreme, 0.0, 0.0],
            [0.0, 0.0, 4.0, 6.0 / extreme],
            1.0,
            [4, 6],
            [BLUE, RED, GREEN, BLUE, RED, GREEN],
        ),
        // Ordinary subpixel pose, captured at 2x: 1/3, 0 and 2/3 instead.
        (
            [1.0, 0.0, 0.0, 0.75, 0.0, 0.0],
            [0.0, 0.0, 4.0, 8.0],
            2.0,
            [8, 12],
            [GREEN, RED, BLUE, GREEN, RED, BLUE],
        ),
        // X is subpixel, while Y is huge. The first sample (y=0.5) is just before
        // the seam at y=0.625, with exact phase 1 - 2^-35. The next one is after it.
        (
            [0.5, 0.0, 0.0, 4_294_967_296.0, 0.0, 0.625],
            [0.0, -1.0, 8.0, 2.0],
            1.0,
            [4, 6],
            [YELLOW, RED, RED, RED, RED, RED],
        ),
    ] {
        let mut batch = HudBatch::scratch([4, 6]);
        assert!(blit_gui_sprite(
            &mut batch,
            &atlas,
            "test:phase",
            GuiSpriteBlit {
                rect: atlas_rect,
                scaling: Some(GuiSpriteScaling::Tile([1, 1])),
                dst,
                color: u32::MAX,
                region: None,
                blend: SpriteBlend::Textured,
            },
            pose,
            None,
        ));
        gpu.hud.zoom = zoom;
        let pixels = gpu.draw_prepared(&batch, size);
        let rows = pixels.chunks_exact(size[0] as usize * 4);
        for (row, color) in rows.zip(expected) {
            assert!(
                row.as_chunks::<4>().0.iter().all(|pixel| *pixel == color),
                "pose={pose:?}, zoom={zoom}: {row:?}"
            );
        }
    }
}

#[test]
#[ignore = "daily"]
fn overlapping_affine_repeats_sample_both_device_axes_in_draw_order() {
    let _gpu = crate::gfx::test_gpu::lock();
    let mut atlas = HudAtlas::boot();
    (atlas.width, atlas.height) = (1, 8);
    atlas.pixels = [BLUE, BLUE, BLUE, BLUE, [0; 4], [0; 4], [0; 4], RED].concat();
    let Some(mut gpu) = Gpu::with_atlas([atlas.width, atlas.height], &atlas.pixels) else {
        return;
    };
    let size = [4, 3];
    let mut batch = HudBatch::scratch(size);
    let tall = f32::from_bits(167 << 23); // 2^40 needs more than one coefficient word.
    for (row, pose, dst) in [
        (
            0.0,
            [0.75, 0.25, 0.0, 0.75, 0.0, -1.0],
            [0.0, 0.0, 6.0, 10.0],
        ),
        (
            4.0,
            [0.5, 0.25, 0.0, tall, 0.0, 0.625],
            [0.0, -1.0, 8.0, 2.0],
        ),
    ] {
        assert!(blit_gui_sprite(
            &mut batch,
            &atlas,
            "test:phase",
            GuiSpriteBlit {
                rect: Rect {
                    u0: 0.0,
                    v0: row / 8.0,
                    u1: 1.0,
                    v1: (row + 4.0) / 8.0,
                    w: 1,
                    h: 4,
                },
                scaling: Some(GuiSpriteScaling::Tile([1, 1])),
                dst,
                color: u32::MAX,
                region: None,
                blend: SpriteBlend::Textured,
            },
            pose,
            None,
        ));
    }
    let pixels = gpu.draw_prepared(&batch, size);
    // The top repeat's local Y is (Y - X/2 - 5/8) / 2^40. Just below the
    // seam it hits the last (red) texel; above it the transparent first texel
    // lets the underlying blue repeat show. The rows straddle a diagonal.
    assert_eq!(
        pixels,
        [
            RED, RED, RED, RED, BLUE, BLUE, RED, RED, BLUE, BLUE, BLUE, BLUE
        ]
        .concat()
    );
}

#[test]
#[ignore = "daily"]
fn bounded_nine_slice_repeats_the_inner_subregion_in_metadata_units() {
    let _gpu = crate::gfx::test_gpu::lock();
    let mut atlas = HudAtlas::boot();
    (atlas.width, atlas.height) = (8, 8);
    let mut texels = [[[0; 4]; 8]; 8];
    texels[3][3..5].copy_from_slice(&[RED, GREEN]);
    texels[4][3..5].copy_from_slice(&[BLUE, YELLOW]);
    atlas.pixels = texels.as_flattened().concat();
    let Some(mut gpu) = Gpu::with_atlas([atlas.width, atlas.height], &atlas.pixels) else {
        return;
    };
    let size = [6, 6];
    let mut batch = HudBatch::scratch(size);
    assert!(blit_gui_sprite(
        &mut batch,
        &atlas,
        "test:panel",
        GuiSpriteBlit {
            rect: Rect {
                u0: 1.0 / 8.0,
                v0: 1.0 / 8.0,
                u1: 7.0 / 8.0,
                v1: 7.0 / 8.0,
                w: 6,
                h: 6,
            },
            scaling: Some(GuiSpriteScaling::NineSlice(NineSlice {
                width: 3,
                height: 3,
                border: [1; 4],
                stretch_inner: false,
            })),
            dst: [0.0, 0.0, 8.0, 8.0],
            color: u32::MAX,
            region: None,
            blend: SpriteBlend::Textured,
        },
        [0.75, 0.0, 0.0, 0.75, 0.375, 0.375],
        None,
    ));
    let pixels = gpu.draw_prepared(&batch, size);
    // As in blitNineSliceInnerSegment, the center uses source [1, 2) on each metadata
    // axis: atlas [3, 5), not the whole [1, 7) sprite. At device centers 1.5, 2.5, 3.5,
    // the phases are 1/2, 5/6, 1/6. The first sample is on the edge of texel 4.
    let rows = pixels.as_chunks::<{ 6 * 4 }>().0;
    for (row, expected) in rows[1..4].iter().zip([
        [YELLOW, YELLOW, BLUE],
        [YELLOW, YELLOW, BLUE],
        [GREEN, GREEN, RED],
    ]) {
        assert_eq!(&row[4..16], expected.as_flattened());
    }
}

pub(super) struct Gpu {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) hud: HudRenderer,
}

impl Gpu {
    pub(super) fn new() -> Option<Self> {
        Self::with_atlas([2, 1], &[255, 255, 255, 255, 255, 255, 255, 0])
    }

    fn with_atlas([width, height]: [u32; 2], pixels: &[u8]) -> Option<Self> {
        let instance = crate::gfx::test_gpu::instance();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
        if !hud_coverage::supported(
            adapter.limits().max_color_attachments,
            adapter.get_downlevel_capabilities().flags,
            adapter.get_texture_format_features(hud_coverage::FORMAT),
        ) {
            eprintln!("skipping HUD coverage: adapter lacks independent blending or R8 rendering");
            return None;
        }
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).ok()?;
        let atlas = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            pixels,
        );
        let view = atlas.create_view(&Default::default());
        let hud = HudRenderer::new(
            &device,
            hud_coverage::HUD_FORMAT,
            &view,
            &view,
            &view,
            &device.create_sampler(&Default::default()),
            true,
        )
        .unwrap();
        Some(Self { device, queue, hud })
    }

    fn texture(&self, [width, height]: [u32; 2], format: wgpu::TextureFormat) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// `batch` drawn on the HUD's target cleared to `backdrop`, with coverage cleared to
    /// `initial`: the target's and the coverage's 8-bit texels.
    pub(super) fn draw(
        &self,
        batch: &HudBatch,
        backdrop: f64,
        initial: f64,
        coverage: bool,
    ) -> (Vec<u8>, Vec<u8>) {
        self.queue
            .write_buffer(&self.hud.buffer, 0, bytemuck::cast_slice(&batch.vertices));
        self.queue.write_buffer(
            &self.hud.uniform,
            0,
            bytemuck::bytes_of(&HudUniform::new([4.0, 2.0, 1.0, 0.75], batch.end_portal, 0)),
        );
        let encoder = self.device.create_command_encoder(&Default::default());
        self.read_draw(encoder, batch, [4, 2], [backdrop, initial], coverage)
    }

    fn draw_prepared(&mut self, batch: &HudBatch, size: [u32; 2]) -> Vec<u8> {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut uploads = crate::gfx::upload::Uploads::new(&self.device);
        self.hud
            .prepare(&self.device, &mut uploads, &mut encoder, batch, size);
        uploads.finish(&encoder);
        self.read_draw(encoder, batch, size, [0.0, 0.0], false).0
    }

    fn read_draw(
        &self,
        mut encoder: wgpu::CommandEncoder,
        batch: &HudBatch,
        size: [u32; 2],
        [backdrop, initial]: [f64; 2],
        coverage: bool,
    ) -> (Vec<u8>, Vec<u8>) {
        let frame = self.texture(size, hud_coverage::HUD_FORMAT);
        let mask = self.texture(size, hud_coverage::FORMAT);
        let depth = self.texture(size, wgpu::TextureFormat::Depth32Float);
        let [frame_view, mask_view, depth_view] =
            [&frame, &mask, &depth].map(|t| t.create_view(&Default::default()));
        for (view, value) in [(&frame_view, backdrop), (&mask_view, initial)] {
            let mut attachment = hud_coverage::attachment(view);
            attachment.ops.load = wgpu::LoadOp::Clear(wgpu::Color {
                r: value,
                g: value,
                b: value,
                a: value,
            });
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(attachment)],
                ..Default::default()
            });
        }
        self.hud.draw_range(
            &mut encoder,
            &frame_view,
            coverage.then_some(&mask_view),
            &depth_view,
            batch,
            size,
            0..batch.vertices.len() as u32,
            None,
        );
        let stride = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffers = [&frame, &mask].map(|t| {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: u64::from(stride) * u64::from(size[1]),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                t.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: None,
                    },
                },
                t.size(),
            );
            buffer
        });
        self.queue.submit([encoder.finish()]);
        let [color, mask] = buffers.map(|buffer| {
            buffer.map_async(wgpu::MapMode::Read, .., |r| r.unwrap());
            self.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            buffer.get_mapped_range(..).unwrap().to_vec()
        });
        let width = size[0] as usize;
        (
            color
                .chunks(stride as usize)
                .flat_map(|r| r[..width * 4].iter().copied())
                .collect(),
            mask.chunks(stride as usize)
                .flat_map(|r| r[..width].iter().copied())
                .collect(),
        )
    }

    fn check(&self, batch: &HudBatch, initial: f64, expected: [u8; 4]) {
        let (plain, _) = self.draw(batch, 0.3, initial, false);
        let (color, mask) = self.draw(batch, 0.3, initial, true);
        assert_eq!(color, plain, "coverage changed the primary colour or alpha");
        for (got, want) in mask.iter().zip(expected.repeat(2)) {
            assert!(
                got.abs_diff(want) <= 1,
                "coverage {mask:?}, expected {expected:?}"
            );
        }
    }
}

pub(super) fn quad(batch: &mut HudBatch, blend: Blend, color: u32, x: f32, uv: f32) {
    let start = batch.vertices.len();
    batch.quad(
        [[x, 0.0], [x + 2.0, 0.0], [x + 2.0, 2.0], [x, 2.0]],
        [[uv, 0.5]; 4],
        color,
    );
    batch.blend_since(start, blend);
}

#[test]
fn coverage_follows_hud_blends_discard_scissor_and_depth_without_changing_colour() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Gpu::new() else {
        return;
    };
    for (blend, color, initial, count, expected) in [
        (Blend::Alpha, rgba(255, 255, 255, 128), 0.0, 2, 192),
        (Blend::Additive, rgba(255, 255, 255, 128), 0.3, 1, 77),
        (Blend::Invert, rgba(255, 255, 255, 32), 0.0, 1, 255),
        (Blend::EndPortal, rgba(255, 255, 255, 255), 0.0, 1, 255),
        (Blend::Vignette, rgba(102, 102, 102, 255), 0.0, 1, 102),
    ] {
        let mut batch = HudBatch::new(
            [4, 2],
            crate::hud::GuiScale {
                requested: 1,
                unicode: false,
            },
        );
        for _ in 0..count {
            quad(&mut batch, blend, color, 0.0, 0.25);
        }
        // Crosshair fragments with zero texture alpha discard; EndPortal has no discard.
        if blend == Blend::Invert {
            quad(&mut batch, blend, color, 2.0, 0.75);
        }
        gpu.check(
            &batch,
            initial,
            [
                expected,
                expected,
                (initial * 255.0).round() as u8,
                (initial * 255.0).round() as u8,
            ],
        );
    }
    let mut batch = HudBatch::new(
        [4, 2],
        crate::hud::GuiScale {
            requested: 1,
            unicode: false,
        },
    );
    for depth in [0.25, 0.75] {
        quad(
            &mut batch,
            Blend::Alpha,
            rgba(255, 255, 255, 128),
            0.0,
            0.25,
        );
        let start = batch.vertices.len() - 6;
        for vertex in &mut batch.vertices[start..] {
            vertex.depth = depth;
        }
    }
    batch.blends.push((0..12, Blend::Depth([1, 0, 2, 2])));
    gpu.check(&batch, 0.0, [0, 128, 0, 0]);
}

/// The HUD's target is 8-bit as vanilla's frame is, so each draw clips at white before the
/// next blends over it: a text highlight added over a light backdrop, then a 50% black panel.
#[test]
fn each_hud_draw_clips_at_white_as_on_an_8_bit_frame() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = Gpu::new() else {
        return;
    };
    let mut batch = HudBatch::scratch([4, 2]);
    quad(
        &mut batch,
        Blend::Additive,
        rgba(128, 128, 128, 255),
        0.0,
        0.25,
    );
    quad(&mut batch, Blend::Alpha, rgba(0, 0, 0, 128), 0.0, 0.25);
    // Vanilla's frame in integers: 204 + 128 clips at 255, then the panel keeps 127/255 of it.
    let (backdrop, highlight, panel) = (204u32, 128u32, 128u32);
    let clipped = (backdrop + highlight).min(255);
    let want = ((clipped * (255 - panel)) as f64 / 255.0).round() as u8;
    assert_eq!(want, 127);
    // The HUD layer's target, and the 8-bit frame a 0.2 pack finishes on in SDR, where the
    // HUD draws without coverage.
    for coverage in [true, false] {
        let (color, mask) = gpu.draw(&batch, f64::from(backdrop) / 255.0, 0.0, coverage);
        // Unclipped, the panel would have kept 127/255 of 332: 165.
        for pixel in [0, 1, 4, 5] {
            assert_eq!(
                color[pixel * 4..pixel * 4 + 3],
                [want; 3],
                "{coverage}: {color:?}"
            );
        }
        // The highlight hides nothing; the panel hides half of what is under it.
        if coverage {
            assert_eq!(mask[0], 128, "{mask:?}");
        }
    }
}

#[test]
fn coverage_preserves_each_cit_blend_and_its_destination_alpha() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    use sunrise_optifine::cit::parse_blend;
    for (name, expected) in [
        ("alpha", 128),
        ("add", 0),
        ("dodge", 0),
        ("glint", 0),
        ("burn", 102),
        ("multiply", 26),
        ("overlay", 51),
        ("screen", 255),
        ("subtract", 255),
        ("replace", 255),
        ("0 768", 153),
        ("774 0", 153),
        ("770 771 772 773", 128),
    ] {
        let textures = cit_glint::GlintTextures {
            width: 1,
            height: 1,
            pixels: vec![102, 102, 102, 128],
            slots: vec![cit_glint::Slot {
                rgba: [1.0; 4],
                blur: false,
                blend: 0,
            }],
            blends: vec![parse_blend(name).unwrap()],
            ..Default::default()
        };
        gpu.hud
            .set_cit_glints(&gpu.device, &gpu.queue, Some(&textures));
        let mut batch = HudBatch::new(
            [4, 2],
            crate::hud::GuiScale {
                requested: 1,
                unicode: false,
            },
        );
        quad(
            &mut batch,
            Blend::CitGlint(0),
            rgba(0, 0, 0, 255),
            0.0,
            0.25,
        );
        quad(
            &mut batch,
            Blend::CitGlint(0),
            rgba(0, 0, 0, 255),
            2.0,
            0.75,
        );
        gpu.check(&batch, 0.0, [expected, expected, 0, 0]);
    }
}

#[test]
fn coverage_uses_pack_texts_final_alpha_and_discard() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    // A translated-style struct result and early return; no dependency on game resources.
    let program = crate::core_shader::CoreProgram::validated(
        naga::front::wgsl::parse_str("@vertex fn main(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> { return vec4(p.x / 2.0 - 1.0, 1.0 - p.y, 0.0, 1.0); }").unwrap(),
        naga::front::wgsl::parse_str("struct Result { @location(0) color: vec4<f32> } @fragment fn main(@builtin(position) p: vec4<f32>) -> Result { if p.x > 2.0 { discard; } return Result(vec4(0.4, 0.2, 0.1, 0.5)); }").unwrap(),
        Vec::new(),
    );
    gpu.hud.set_text_shader(&gpu.device, Some((&program, &[])));
    assert!(gpu.hud.pack_text.is_some());
    let mut batch = HudBatch::new(
        [4, 2],
        crate::hud::GuiScale {
            requested: 1,
            unicode: false,
        },
    );
    for x in [0.0, 2.0, 0.0, 2.0] {
        quad(&mut batch, Blend::Alpha, rgba(255, 255, 255, 255), x, 0.25);
    }
    for v in &mut batch.vertices {
        v.text = 1;
    }
    gpu.check(&batch, 0.0, [191, 191, 0, 0]);
}

#[test]
fn a_pack_text_shader_without_a_colour_output_is_dropped_not_wrapped() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let program = crate::core_shader::CoreProgram::validated(
        naga::front::wgsl::parse_str("@vertex fn main(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> { return vec4(p, 0.0, 1.0); }").unwrap(),
        naga::front::wgsl::parse_str("@fragment fn main() -> @location(1) vec4<f32> { return vec4(1.0); }").unwrap(),
        Vec::new(),
    );
    gpu.hud.set_text_shader(&gpu.device, Some((&program, &[])));
    assert!(gpu.hud.pack_text.is_none());
}
