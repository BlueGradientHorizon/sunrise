//! A resource pack's `core/text` on world text, as vanilla 26.3's `TEXT_POLYGON_OFFSET` and
//! `TEXT_SEE_THROUGH` draw text displays, name tags and their backgrounds.

use crate::core_shader::{CoreProgram, Glue};
use std::sync::Arc;

/// The entity vertices' light bytes are sky then block; vanilla's `UV2` is block then sky.
pub(crate) const WORLD_GLUE: Glue = Glue {
    inputs: &[
        ("Position", "vec3"),
        ("Color", "vec4"),
        ("UV0", "vec2"),
        ("UV2", "ivec2"),
    ],
    prelude: "layout(location = 0) in vec3 hz_pos;\n\
              layout(location = 1) in vec2 hz_uv;\n\
              layout(location = 3) in vec4 hz_color;\n\
              layout(location = 4) in uvec4 hz_light;",
    setup: "Position = hz_pos; Color = hz_color; UV0 = hz_uv; UV2 = ivec2(hz_light.y, hz_light.x);",
    finish: "",
    vertex_id: "int(gl_VertexIndex)",
};

pub(crate) const SEE_THROUGH_GLUE: Glue = Glue {
    inputs: &[("Position", "vec3"), ("Color", "vec4"), ("UV0", "vec2")],
    prelude: "layout(location = 0) in vec3 hz_pos;\n\
              layout(location = 1) in vec2 hz_uv;\n\
              layout(location = 3) in vec4 hz_color;",
    setup: "Position = hz_pos; Color = hz_color; UV0 = hz_uv;",
    finish: "",
    vertex_id: "int(gl_VertexIndex)",
};

/// The text shader's id, which a pack's `shader_modifiers` file names it by
/// (`polytone/shader_modifiers/core/text.json`): one shader, `text.vsh` and `text.fsh`, for
/// the GUI's, the world's and the see-through text.
pub const SHADER: &str = "minecraft:core/text";

#[derive(Default)]
pub struct TextShaders {
    pub gui: Option<Arc<CoreProgram>>,
    pub world: Option<Arc<CoreProgram>>,
    pub see_through: Option<Arc<CoreProgram>>,
    /// Each program's Polytone expression uniforms (`shader_modifiers`), resolved when it
    /// loaded: the blocks its stages read, with their programs, bound after the pipeline's own
    /// (`expression_blocks`).
    pub expressions: TextExpressions,
}

/// [`TextShaders`]' expression uniforms, program by program.
#[derive(Debug, Default)]
pub struct TextExpressions {
    pub gui: Vec<crate::polytone_post::ExpressionUniform>,
    pub world: Vec<crate::polytone_post::ExpressionUniform>,
    pub see_through: Vec<crate::polytone_post::ExpressionUniform>,
}

#[derive(Clone, Copy)]
pub(super) struct PackBlocks {
    pub(super) globals: [u8; 48],
    pub(super) fog: [f32; 6],
}

impl super::Renderer {
    pub fn set_world_text_shaders(&mut self, shaders: &TextShaders) {
        let expressions = &shaders.expressions;
        self.core_text = CoreText::new(
            &self.device,
            self.output.frame.format,
            (shaders.world.as_deref()).map(|p| (p, &expressions.world[..])),
            (shaders.see_through.as_deref()).map(|p| (p, &expressions.see_through[..])),
            &self.entity_atlas,
            (&self.lightmap_view, &self.lightmap_sampler),
        );
    }

    /// The vanilla `Globals` block a pack's core shaders read this frame.
    pub(super) fn pack_globals(&self, frame: &super::Frame) -> [u8; 48] {
        let globals = crate::post_chain::Globals {
            glint_alpha: frame.glint_alpha,
            ..frame.post_effects.globals
        };
        let size = [self.config.width, self.config.height];
        globals.bytes(size, frame.post_effects.menu_blur_radius as i32)
    }

    pub(super) fn pack_blocks(
        &self,
        frame: &super::Frame,
        fog: [f32; 4],
        ends: [f32; 4],
    ) -> PackBlocks {
        PackBlocks {
            globals: self.pack_globals(frame),
            fog: [fog[0], fog[1], fog[2], fog[3], ends[0], ends[1]],
        }
    }

    pub(super) fn write_core_text(
        &self,
        pack: &PackBlocks,
        view: &super::View,
        pose: glam::Mat4,
        jitter: glam::Mat4,
        fog_color: [f32; 4],
    ) {
        let Some(core) = &self.core_text else {
            return;
        };
        let data =
            super::core_lines::uniforms(pack.globals, view, pose, jitter, 1.0, fog_color, pack.fog);
        core.write(&self.queue, &data);
    }

    pub(super) fn bind_world_text(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        allowed: bool,
        see_through: bool,
    ) {
        let pack = self
            .core_text
            .as_ref()
            .filter(|_| allowed)
            .and_then(|core| {
                let part = if see_through {
                    core.see_through.as_ref()
                } else {
                    core.polygon_offset.as_ref()
                };
                part.map(|part| (&part.pipeline, &part.group))
            });
        match pack {
            Some((pipeline, group)) => {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, group, &[]);
            }
            None => pass.set_bind_group(0, &self.entity_group, &[]),
        }
    }
}

/// One of the pack's world text pipelines, with its own blocks: the world's and the
/// see-through text are the text shader with different defines, so each may read different
/// expression uniforms.
pub(super) struct Part {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    /// The packs' expression uniforms its stages read (`shader_modifiers`).
    extra: super::expression_blocks::ExpressionBlocks,
}

pub(super) struct CoreText {
    pub(super) polygon_offset: Option<Part>,
    pub(super) see_through: Option<Part>,
}

impl CoreText {
    pub(super) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        world: Option<(&CoreProgram, &[crate::polytone_post::ExpressionUniform])>,
        see_through: Option<(&CoreProgram, &[crate::polytone_post::ExpressionUniform])>,
        atlas: &wgpu::TextureView,
        lightmap: (&wgpu::TextureView, &wgpu::Sampler),
    ) -> Option<Self> {
        if world.is_none() && see_through.is_none() {
            return None;
        }
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let glyphs = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Resource pack world text glyphs"),
            ..Default::default()
        });
        let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Uint32,3=>Unorm8x4,4=>Uint8x4,5=>Snorm8x4,6=>Float32x2];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::mesher::Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        let part =
            |label,
             (program, expressions): (&CoreProgram, &[crate::polytone_post::ExpressionUniform]),
             depth: wgpu::DepthStencilState| {
                // Each block a stage reads, bound to the stages reading it: vanilla's four, then
                // the expression uniforms; then the glyphs and the lightmap.
                let extra = super::expression_blocks::ExpressionBlocks::new(expressions, 1024);
                let blocks = super::expression_blocks::vanilla_blocks(program);
                let entries: Vec<wgpu::BindGroupLayoutEntry> = (blocks.iter())
                    .map(|&(binding, _, size, stages)| {
                        super::expression_blocks::block_entry(binding, size, stages)
                    })
                    .chain(extra.layout())
                    .chain([texture(4), sampler(5), texture(8), sampler(9)])
                    .collect();
                let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some(label),
                    entries: &entries,
                });
                let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: extra.size(),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let resources: Vec<wgpu::BindGroupEntry> = (blocks.iter())
                    .map(|&(binding, offset, size, _)| wgpu::BindGroupEntry {
                        binding,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &uniform,
                            offset,
                            size: wgpu::BufferSize::new(size),
                        }),
                    })
                    .chain(extra.entries(&uniform))
                    .chain([
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(atlas),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::Sampler(&glyphs),
                        },
                        wgpu::BindGroupEntry {
                            binding: 8,
                            resource: wgpu::BindingResource::TextureView(lightmap.0),
                        },
                        wgpu::BindGroupEntry {
                            binding: 9,
                            resource: wgpu::BindingResource::Sampler(lightmap.1),
                        },
                    ])
                    .collect();
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(label),
                    layout: &layout,
                    entries: &resources,
                });
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(label),
                        bind_group_layouts: &[Some(&layout)],
                        ..Default::default()
                    });
                let module = |module: &naga::Module| {
                    device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some(label),
                        source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module.clone())),
                    })
                };
                let (vertex, fragment) = (module(&program.vertex), module(&program.fragment));
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vertex,
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        buffers: &buffers,
                    },
                    primitive: wgpu::PrimitiveState {
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: Some(depth),
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &fragment,
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
                Part {
                    pipeline,
                    group,
                    uniform,
                    extra,
                }
            };
        let depth = |write, compare, bias| wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(write),
            depth_compare: Some(compare),
            stencil: Default::default(),
            bias,
        };
        let offset = wgpu::DepthBiasState {
            constant: 10,
            slope_scale: 1.0,
            clamp: 0.0,
        };
        let polygon_offset = world.map(|program| {
            part(
                "Resource pack polygon offset text",
                program,
                depth(true, wgpu::CompareFunction::GreaterEqual, offset),
            )
        });
        let see_through = see_through.map(|program| {
            part(
                "Resource pack see-through text",
                program,
                depth(false, wgpu::CompareFunction::Always, Default::default()),
            )
        });
        let internal = pollster::block_on(internal.pop());
        if let Some(e) = internal.or(pollster::block_on(validation.pop())) {
            log::warn!("the resource pack's world text shader is not used: {e}");
            return None;
        }
        Some(Self {
            polygon_offset,
            see_through,
        })
    }

    /// Vanilla's blocks (`data`, as `core_lines::uniforms` lays them out) into each pipeline's
    /// buffer, then its expression uniforms' values.
    pub(super) fn write(&self, queue: &wgpu::Queue, data: &[u8]) {
        for part in [&self.polygon_offset, &self.see_through]
            .into_iter()
            .flatten()
        {
            queue.write_buffer(&part.uniform, 0, data);
            part.extra.write(queue, &part.uniform);
        }
    }

    /// The world's and the see-through text's expression uniforms.
    pub(super) fn expression_blocks(
        &mut self,
    ) -> impl Iterator<Item = &mut super::expression_blocks::ExpressionBlocks> {
        [self.polygon_offset.as_mut(), self.see_through.as_mut()]
            .into_iter()
            .flatten()
            .map(|part| &mut part.extra)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "core_text_expression_tests.rs"]
mod expression_tests;
