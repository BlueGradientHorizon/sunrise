//! A pack's world and see-through text reading expression uniforms (`shader_modifiers`): each
//! pipeline's values, from the text shader's file, reach its own blocks through
//! [`CoreText::write`], after the pipeline's own.
use super::*;

/// The quad covers the target wherever it is placed; the world text's reads `Tint` in red and
/// the see-through text's (`IS_SEE_THROUGH`) `Fade`.
const VSH: &str = "#version 330\nlayout(location = 0) in vec3 Position;\n\
    void main() {\n    gl_Position = vec4(Position, 1.0);\n}\n";
const FSH: &str = "#version 330\n\
    #ifdef IS_SEE_THROUGH\nlayout(std140) uniform Fade {\n    float value;\n};\n\
    #else\nlayout(std140) uniform Tint {\n    float value;\n};\n#endif\n\
    layout(location = 0) out vec4 fragColor;\n\
    void main() {\n    fragColor = vec4(value, 0.0, 0.0, 1.0);\n}\n";

/// The text shader's file: values a unorm byte holds exactly (0.2 is 51, 0.6 is 153), as
/// expressions the packs' context evaluates.
const MODIFIERS: &str = r#"{"Tint": "0.1 + 0.1", "Fade": "0.3 * 2"}"#;

/// One pass of the world text (`see_through` false) or the see-through text after
/// `CoreText::write` wrote the blocks: the red of the target's middle.
fn red(device: &wgpu::Device, queue: &wgpu::Queue, text: &CoreText, see_through: bool) -> u8 {
    const SIZE: u32 = 16;
    let target = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | usage,
            view_formats: &[],
        })
    };
    let color = target(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::COPY_SRC,
    );
    let depth = target(
        wgpu::TextureFormat::Depth32Float,
        wgpu::TextureUsages::empty(),
    );
    let corner = |x: f32, y: f32| crate::mesher::Vertex {
        pos: [x, y, 0.5],
        uv: [0.0; 2],
        layer: 0,
        color: u32::MAX,
        light: 0,
        normal: [0; 4],
        glint_uv: [0.0; 2],
    };
    let quad = [
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ];
    let vertices = wgpu::util::DeviceExt::create_buffer_init(
        device,
        &wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&quad),
            usage: wgpu::BufferUsages::VERTEX,
        },
    );
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(256 * SIZE),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    text.write(queue, &[0u8; 1024]);
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        let part = if see_through {
            &text.see_through
        } else {
            &text.polygon_offset
        };
        let part = part.as_ref().expect("the pipeline");
        pass.set_pipeline(&part.pipeline);
        pass.set_bind_group(0, &part.group, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.draw(0..6, 0..1);
    }
    encoder.copy_texture_to_buffer(
        color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &read,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(SIZE),
            },
        },
        color.size(),
    );
    queue.submit([encoder.finish()]);
    read.slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let pixels = read.slice(..).get_mapped_range().unwrap().to_vec();
    pixels[8 * 256 + 8 * 4]
}

/// The world text reads `Tint` and the see-through text `Fade`, both from the text shader's
/// file, each in its own pipeline's block after vanilla's four, holding its expression's value
/// once evaluated.
#[test]
#[ignore = "daily"]
fn a_pack_world_and_see_through_text_read_their_expression_uniforms() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some((_, device, queue)) = crate::gfx::test_gpu::optional_device() else {
        eprintln!("skipped: no adapter");
        return;
    };
    let pack = sunrise_assets::resource_source::ResourceSource::indexed(
        [(
            "assets/minecraft/polytone/shader_modifiers/core/text.json".to_owned(),
            sunrise_assets::resource_source::Resource::bytes(MODIFIERS.as_bytes().to_vec()),
        )]
        .into_iter()
        .collect(),
    );
    let modifiers = crate::polytone_post::ShaderModifiers::read(&pack, Default::default());
    let names = modifiers.names(&[SHADER]);
    let load = |defines: &[&str], glue| {
        let program =
            crate::core_shader::translate_core_with(VSH, FSH, defines, &|_| None, glue, &|b| {
                names.iter().any(|n| n == b)
            })
            .expect("the shaders translate");
        let expressions = modifiers.resolve((SHADER, SHADER), &program, &[]).unwrap();
        (program, expressions)
    };
    let world = load(&[], &WORLD_GLUE);
    let see_through = load(&["IS_SEE_THROUGH"], &SEE_THROUGH_GLUE);
    let texture = device
        .create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default());
    let sampler = device.create_sampler(&Default::default());
    let mut text = CoreText::new(
        &device,
        wgpu::TextureFormat::Rgba8Unorm,
        Some((&world.0, &world.1)),
        Some((&see_through.0, &see_through.1)),
        &texture,
        (&texture, &sampler),
    )
    .expect("the pipelines");
    // Before the frame's context evaluates them, the blocks hold 0.
    assert_eq!(red(&device, &queue, &text, false), 0);
    for blocks in text.expression_blocks() {
        blocks.evaluate(&Default::default());
    }
    assert_eq!(red(&device, &queue, &text, false), 51);
    assert_eq!(red(&device, &queue, &text, true), 153);
}
