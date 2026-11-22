use super::*;
use crate::gfx::{
    Renderer, TERRAIN_ATTRIBUTES, frustum,
    gpu_culling::tests::{on_each_device, read},
    lod, world_projection,
};
use crate::mesher::{Sides, TerrainVertex};
use glam::DVec3;

/// Each of `module`'s entry points written by every backend's writer.
fn translates_everywhere(module: &naga::Module, info: &naga::valid::ModuleInfo) {
    for entry in &module.entry_points {
        let (stage, name) = (entry.stage, entry.name.clone());
        let spv = naga::back::spv::PipelineOptions {
            shader_stage: stage,
            entry_point: name.clone(),
        };
        naga::back::spv::write_vec(module, info, &Default::default(), Some(&spv))
            .unwrap_or_else(|e| panic!("{name} (SPIR-V): {e}"));
        let pipeline = naga::back::hlsl::PipelineOptions {
            entry_point: Some((stage, name.clone())),
        };
        let mut hlsl = String::new();
        naga::back::hlsl::Writer::new(&mut hlsl, &Default::default(), &pipeline)
            .write(module, info, None)
            .unwrap_or_else(|e| panic!("{name} (HLSL): {e}"));
        let options = naga::back::msl::Options {
            fake_missing_bindings: true,
            ..Default::default()
        };
        let pipeline = naga::back::msl::PipelineOptions {
            entry_point: Some((stage, name.clone())),
            ..Default::default()
        };
        naga::back::msl::write_string(module, info, &options, &pipeline)
            .unwrap_or_else(|e| panic!("{name} (MSL): {e}"));
        let options = naga::back::glsl::Options {
            version: naga::back::glsl::Version::Embedded {
                version: 310,
                is_webgl: false,
            },
            ..Default::default()
        };
        let pipeline = naga::back::glsl::PipelineOptions {
            shader_stage: stage,
            entry_point: name.clone(),
            multiview: None,
        };
        let mut glsl = String::new();
        naga::back::glsl::Writer::new(
            &mut glsl,
            module,
            info,
            &options,
            &pipeline,
            Default::default(),
        )
        .and_then(|mut w| w.write())
        .unwrap_or_else(|e| panic!("{name} (GLSL): {e}"));
    }
}

/// The pass's shader, which sourced meshes run through (second table, `NO_RANGE`), needs no
/// capability and is written by every backend.
#[test]
fn the_shader_needs_no_capability_and_translates_everywhere() {
    let _gpu = crate::gfx::test_gpu::lock();
    let module = naga::front::wgsl::parse_str(include_str!("../../gpu_culling.wgsl")).unwrap();
    let flags = naga::valid::ValidationFlags::all();
    let info = naga::valid::Validator::new(flags, naga::valid::Capabilities::empty())
        .validate(&module)
        .unwrap();
    translates_everywhere(&module, &info);
}

/// A cell's key is its corner in sections, and its box step a sixteenth of its width; narrower
/// than a section, wider than the units hold or a corner past an i32 is not taken.
#[test]
fn cells_key_on_their_corner_in_sections() {
    let cell = |x, z, size_log2| Cell { x, z, size_log2 };
    assert_eq!(cell(3, -2, 9).key(), Some((96, 0, -64)));
    assert_eq!(cell(3, -2, 9).shift(), 5);
    assert_eq!(cell(-1, 1, 4).key(), Some((-1, 0, 1)));
    assert_eq!(cell(0, 0, 3).key(), None);
    assert_eq!(cell(0, 0, MAX_SIZE_LOG2 + 1).key(), None);
    assert_eq!(cell(1 << 20, 0, 19).key(), None);
}

/// A mesh's box is its bounds rounded outward in the cell's units.
#[test]
fn boxes_round_outward_in_the_cells_units() {
    let cell = Cell {
        x: 0,
        z: 0,
        size_log2: 9,
    };
    // 32-block steps along x and z, 16 along y: 33 lies in the second, so the box ends at 64.
    let bounds = [Vec3::new(1.0, 17.0, 31.0), Vec3::new(33.0, 20.0, 40.0)];
    let wider = [Vec3::new(0.0, 16.0, 0.0), Vec3::new(64.0, 32.0, 64.0)];
    assert_eq!(cell_box(bounds, cell), cell_box(wider, cell));
}

/// A tile's colour from its vertices, white towards the fade's end, drawn by a view and
/// projection in a uniform and depth tested, as the terrain pipelines draw.
const TILES: &str = "
@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;
struct Tile {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}
@vertex fn vs(
    @location(0) position: vec3<f32>,
    @location(3) color: vec4<f32>,
    @location(5) instance: vec4<f32>,
) -> Tile {
    let fade = clamp(-instance.w / 16.0, 0.0, 1.0);
    return Tile(
        view_proj * vec4<f32>(position + instance.xyz, 1.0),
        vec4<f32>(mix(color.rgb, vec3<f32>(1.0), fade), 1.0),
    );
}
@fragment fn fs(tile: Tile) -> @location(0) vec4<f32> {
    return tile.color;
}
";

const SIZE_LOG2: u8 = 9;
const SIDE: u32 = 128;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// How a tile stands in a frame.
#[derive(Clone, Copy, PartialEq)]
enum State {
    Steady,
    /// Fading in, at this `lod::instance` code.
    Fading(u32),
    /// Its replacement is in, fading: this one stands in at its place, whole.
    StandIn,
}

struct Tile {
    cell: Cell,
    stand: State,
    /// The mesh the tile has: where its quad is in the arena, and the one a stand-in draws.
    mesh: (u32, u64),
    previous: Option<(u32, u64)>,
    origin: [f32; 3],
    id: Option<u32>,
}

impl Tile {
    /// What the CPU draws: the stand-in while there is one.
    fn drawn(&self) -> (u32, u64) {
        self.previous.unwrap_or(self.mesh)
    }
    fn instance(&self) -> [f32; 4] {
        match self.stand {
            State::Fading(code) => lod::instance(self.origin, code),
            _ => lod::instance(self.origin, 0),
        }
    }
}

fn quad(height: f32, color: u32) -> [TerrainVertex; 4] {
    let side = (1u32 << SIZE_LOG2) as f32;
    [(0.0, 0.0), (side, 0.0), (side, side), (0.0, side)].map(|(x, z)| TerrainVertex {
        pos: [x, height, z],
        color,
        ..bytemuck::Zeroable::zeroed()
    })
}

/// The same frame drawn by the CPU alone (a draw per tile) and with the GPU path (steady tiles
/// through `SourcedGpu::draw`, after the CPU draws of the fading and standing-in ones) is the
/// same image, with a fading mesh and a stand-in among the tiles; the GPU frame without the
/// CPU's draws is not.
#[test]
#[ignore = "daily"]
fn the_gpu_path_draws_the_cpu_paths_image() {
    let _gpu = crate::gfx::test_gpu::lock();
    on_each_device(images_match);
}

fn images_match(device: &wgpu::Device, queue: &Queue) {
    let stride = std::mem::size_of::<TerrainVertex>() as u64;
    let grain = crate::gfx::arena::GRAIN;
    let mut arena = Arena::new(device, stride, grain, false);
    assert!(arena.open_page(device, 256 * grain));
    let camera = DVec3::new(100.5, 3000.25, -37.75);
    let side = f64::from(1u32 << SIZE_LOG2);
    // A 7 by 7 square of cells around the camera's, the sky over them: each tile one quad at
    // a height of its own, in a colour of its own.
    let (mut tiles, mut slot) = (Vec::new(), 0u64);
    let mut put = |height: f32, color: u32| {
        let start = slot * grain;
        slot += 1;
        queue.write_buffer(
            arena.buffer(0),
            start * stride,
            bytemuck::cast_slice(&quad(height, color)),
        );
        (0, start)
    };
    for (i, (x, z)) in (-3..=3)
        .flat_map(|x| (-3..=3).map(move |z| (x, z)))
        .enumerate()
    {
        let cell = Cell {
            x,
            z,
            size_log2: SIZE_LOG2,
        };
        let color = 0xFF00_0000 | ((i as u32 * 0x0005_1B27) & 0x00FF_FFFF);
        let mesh = put(40.0 + (i % 7) as f32 * 3.0, color);
        let (stand, previous) = match i {
            // Fading in, every few steps.
            9 | 24 | 38 => (State::Fading(1 + i as u32 % 15), None),
            // Standing in for a replacement, drawn at another height.
            12 | 30 => (State::StandIn, Some(put(80.0, color ^ 0x00FF_FF00))),
            _ => (State::Steady, None),
        };
        tiles.push(Tile {
            cell,
            stand,
            mesh,
            previous,
            origin: [
                (f64::from(x) * side - camera.x) as f32,
                (0.0 - camera.y) as f32,
                (f64::from(z) * side - camera.z) as f32,
            ],
            id: None,
        });
    }
    let sides = Sides::unsorted(1);
    let bounds = [
        Vec3::new(0.0, 30.0, 0.0),
        Vec3::new(side as f32, 90.0, side as f32),
    ];
    let step = 1.0 / crate::mesher::TILED_UV_STEPS;
    let near = GpuCulling::new(device, None).expect("the pass");
    let mut sourced = SourcedGpu::new(&near, device, step).expect("the table");
    for tile in &mut tiles {
        let mesh = Mesh {
            cell: tile.cell,
            page: tile.mesh.0,
            start: tile.mesh.1,
            quads: 1,
            sides: &sides,
            bounds,
        };
        tile.id = sourced.add(&mesh, tile.stand != State::Steady);
        assert!(tile.id.is_some());
    }

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(TILES.into()),
    });
    let instance = wgpu::vertex_attr_array![5 => Float32x4];
    let buffers = [
        Some(wgpu::VertexBufferLayout {
            array_stride: stride,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &TERRAIN_ATTRIBUTES,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance,
        }),
    ];
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            buffers: &buffers,
        },
        primitive: Default::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            targets: &[Some(COLOR.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    let lens = world_projection(100.0, 1.0, 4096.0);
    let view_proj = lens * crate::camera::view_matrix(0.0, 90.0);
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(
        &uniform,
        0,
        bytemuck::cast_slice(&view_proj.to_cols_array()),
    );
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }],
    });
    let extent = wgpu::Extent3d {
        width: SIDE,
        height: SIDE,
        depth_or_array_layers: 1,
    };
    let texture = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let target = texture(
        COLOR,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = texture(DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let (color_view, depth_view) = (
        target.create_view(&Default::default()),
        depth.create_view(&Default::default()),
    );
    let pixels = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(SIDE * SIDE) * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let indices = Renderer::quad_indices(device, 1);
    let rows: Vec<[f32; 4]> = tiles.iter().map(Tile::instance).collect();
    let rows_buffer = {
        use wgpu::util::DeviceExt;
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&rows),
            usage: wgpu::BufferUsages::VERTEX,
        })
    };
    let all = Frustum::from_planes(frustum(view_proj));
    let mut uploads = Uploads::new(device);

    // `gpu`: whether sourced geometry draws the steady tiles; `cpu_lists`: whether the CPU
    // draws the rest. Neither: the CPU draws every tile.
    let mut image = |gpu: bool, cpu_lists: bool| {
        let mut encoder = device.create_command_encoder(&Default::default());
        if gpu {
            let prepared = sourced.prepare(
                device,
                queue,
                &mut uploads,
                &mut encoder,
                camera,
                -64,
                &all,
                None,
                None,
            );
            assert_eq!(prepared, Prepared::Gpu);
            sourced.dispatch(&mut encoder, None);
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_vertex_buffer(1, rows_buffer.slice(..));
        for (i, tile) in tiles.iter().enumerate() {
            if gpu && (tile.stand == State::Steady || !cpu_lists) {
                continue;
            }
            let (page, start) = tile.drawn();
            pass.set_vertex_buffer(0, arena.buffer(page).slice(..));
            pass.draw_indexed(0..6, start as i32, i as u32..i as u32 + 1);
        }
        if gpu {
            sourced.draw(&mut pass, &arena, |_| None);
        }
        drop(pass);
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &pixels,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIDE * 4),
                    rows_per_image: None,
                },
            },
            extent,
        );
        uploads.finish(&encoder);
        queue.submit([encoder.finish()]);
        read(device, queue, &pixels, pixels.size())
    };
    let cpu = image(false, true);
    let gpu = image(true, true);
    let black = |image: &[u32]| image.iter().filter(|&&p| p == 0xFF00_0000).count();
    let (covered, total) = (cpu.len() - black(&cpu), cpu.len());
    assert!(covered > total / 4, "the tiles cover {covered} of {total}");
    let colors: std::collections::BTreeSet<_> = cpu.iter().collect();
    assert!(colors.len() > 20, "{} colours", colors.len());
    let wrong = cpu.iter().zip(&gpu).filter(|(a, b)| a != b).count();
    assert_eq!(wrong, 0, "{wrong} pixels of {total} differ");
    // Without the CPU's draws the fading and standing-in tiles are missing.
    let without = image(true, false);
    assert_ne!(cpu, without, "the flagged tiles are not drawn by the GPU");
    assert!(sourced.draw_calls() > 0);
}
