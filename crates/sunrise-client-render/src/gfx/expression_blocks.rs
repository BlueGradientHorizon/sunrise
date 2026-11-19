//! A pack core shader pipeline's blocks: vanilla's four bound only to the stages that read them,
//! and its Polytone blocks after the pipeline's own: `PolyGlobals` (wiki "Extra Shader
//! Uniforms", `sunrise_polytone::globals`), the frame's, and its expression uniforms
//! (`shader_modifiers`, wiki "Expression Uniforms on Any Shader"), resolved when the shader
//! loaded (`polytone_post::ShaderModifiers::resolve`), each a block of one float holding its
//! program's value this frame. The lightmap, sky, stars and block outline pipelines lay theirs out
//! with this; any other pack core shader pipeline can.

use crate::core_shader::CoreProgram;
use crate::polytone_post::ExpressionUniform;

/// The block outline's shader, as a pack's `shader_modifiers` names it.
pub const LINES: [&str; 1] = ["minecraft:core/rendertype_lines"];
/// The sky disc's and the stars' shaders.
pub const SKY: [&str; 1] = ["minecraft:core/sky"];
pub const STARS: [&str; 1] = ["minecraft:core/stars"];

/// An expression uniform's block: one float, as std140 rounds a block.
const BLOCK: u64 = 16;

/// Each vanilla uniform block's offset in a pipeline's buffer (`core_shader::BLOCKS` order).
pub(crate) const VANILLA_OFFSETS: [u64; 4] = [0, 256, 512, 768];
pub(crate) const VANILLA_SIZES: [u64; 4] = [48, 64, 160, 48];

/// Vanilla's blocks a stage of `program` reads: binding, offset, size and the stages reading it.
pub(crate) fn vanilla_blocks(program: &CoreProgram) -> Vec<(u32, u64, u64, wgpu::ShaderStages)> {
    (0..4u32)
        .map(|b| {
            let i = b as usize;
            (
                b,
                VANILLA_OFFSETS[i],
                VANILLA_SIZES[i],
                program.stages_reading(b),
            )
        })
        .filter(|(.., stages)| !stages.is_empty())
        .collect()
}

/// A layout entry for a uniform block at `binding`, visible to `stages`, at least `size` bytes.
pub(crate) fn block_entry(
    binding: u32,
    size: u64,
    stages: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: stages,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size),
        },
        count: None,
    }
}

/// `PolyGlobals`' std140 size.
const GLOBALS: u64 = sunrise_polytone::globals::BLOCK_SIZE as u64;

/// The frame's `PolyGlobals` bytes (`polytone_post::PolyGlobals::bytes`).
pub type PolyGlobalsBytes = [u8; sunrise_polytone::globals::BLOCK_SIZE];

/// A pipeline's Polytone blocks, laid out in its uniform buffer from `base`, 256 bytes apart (the
/// uniform offset alignment): `PolyGlobals` first when a stage reads it, then the expression
/// uniforms with this frame's values.
#[derive(Debug)]
pub(crate) struct ExpressionBlocks {
    /// `PolyGlobals`' binding, the stages that read it and the frame's bytes (zeros until the
    /// game gives them).
    globals: Option<(u32, wgpu::ShaderStages, PolyGlobalsBytes)>,
    uniforms: Vec<(ExpressionUniform, f32)>,
    base: u64,
}

impl ExpressionBlocks {
    /// The Polytone blocks `program`'s stages read: `PolyGlobals`, when a stage reads it, and
    /// `uniforms` (resolved when it loaded), from `base` in the pipeline's buffer.
    pub(crate) fn new(program: &CoreProgram, uniforms: &[ExpressionUniform], base: u64) -> Self {
        let globals = (program.other.iter())
            .position(|name| name == sunrise_polytone::globals::BLOCK)
            .map(crate::core_shader::other_binding)
            .map(|binding| {
                (
                    binding,
                    program.stages_reading(binding),
                    [0; GLOBALS as usize],
                )
            })
            .filter(|(_, stages, _)| !stages.is_empty());
        Self {
            globals,
            uniforms: uniforms.iter().map(|u| (u.clone(), 0.0)).collect(),
            base,
        }
    }

    /// The offset of the `i`th expression uniform's block.
    fn offset(&self, i: usize) -> u64 {
        self.base + 256 * (i as u64 + u64::from(self.globals.is_some()))
    }

    /// The uniform buffer's size with these blocks from `base`: `base` without any.
    pub(crate) fn size(&self) -> u64 {
        self.offset(self.uniforms.len())
    }

    /// Their layout entries, each visible to the stages that read it.
    pub(crate) fn layout(&self) -> impl Iterator<Item = wgpu::BindGroupLayoutEntry> + '_ {
        let globals = (self.globals.iter())
            .map(|&(binding, stages, _)| block_entry(binding, GLOBALS, stages));
        globals.chain((self.uniforms.iter()).map(|(u, _)| block_entry(u.binding, BLOCK, u.stages)))
    }

    pub(crate) fn entries<'a>(
        &'a self,
        buffer: &'a wgpu::Buffer,
    ) -> impl Iterator<Item = wgpu::BindGroupEntry<'a>> + 'a {
        let entry = move |binding, offset, size| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset,
                size: wgpu::BufferSize::new(size),
            }),
        };
        let globals =
            (self.globals.iter()).map(move |&(binding, ..)| entry(binding, self.base, GLOBALS));
        globals.chain(
            (self.uniforms.iter().enumerate())
                .map(move |(i, (u, _))| entry(u.binding, self.offset(i), BLOCK)),
        )
    }

    /// The frame's blocks into `buffer`: `PolyGlobals` when a stage reads them, then each
    /// expression uniform's value, 16 bytes each.
    pub(crate) fn upload(
        &self,
        uploads: &mut super::upload::Uploads,
        encoder: &mut wgpu::CommandEncoder,
        buffer: &wgpu::Buffer,
    ) {
        if let Some((.., bytes)) = &self.globals {
            uploads.buffer(encoder, buffer, self.base, bytes);
        }
        for (i, (_, value)) in self.uniforms.iter().enumerate() {
            let mut block = [0u8; BLOCK as usize];
            block[..4].copy_from_slice(&value.to_le_bytes());
            uploads.buffer(encoder, buffer, self.offset(i), &block);
        }
    }

    /// Whether a stage reads `PolyGlobals`.
    pub(crate) fn reads_globals(&self) -> bool {
        self.globals.is_some()
    }

    /// The frame's `PolyGlobals` (`polytone_post::PolyGlobals::bytes`) for the stages that read
    /// them; none gives zeros.
    pub(crate) fn set_globals(&mut self, globals: Option<&PolyGlobalsBytes>) {
        if let Some((.., bytes)) = &mut self.globals {
            *bytes = globals.copied().unwrap_or([0; GLOBALS as usize]);
        }
    }

    /// Each value from its program in `frame` (the packs' expression context).
    pub(crate) fn evaluate(&mut self, frame: &crate::environment::Frame) {
        for (uniform, value) in &mut self.uniforms {
            *value = uniform.evaluate(frame);
        }
    }

    /// Whether a stage reads an expression uniform.
    pub(crate) fn has_uniforms(&self) -> bool {
        !self.uniforms.is_empty()
    }

    /// The uniforms and their values, for tests to set and read.
    #[cfg(test)]
    pub(crate) fn values(&mut self) -> &mut [(ExpressionUniform, f32)] {
        &mut self.uniforms
    }
}

/// The packs' core shader pipelines' Polytone blocks, for the game to give their values.
impl super::Renderer {
    /// The installed pack pipelines with their Polytone blocks: the block outline's, the sky's and
    /// the stars', and the lightmap's except under fullbright (which keeps the texture white).
    /// Evaluation does not depend on which pipelines draw this frame.
    fn polytone_blocks(&mut self) -> impl Iterator<Item = &mut ExpressionBlocks> {
        let fullbright = self.fullbright;
        let lightmap = (self.core_lightmap.as_mut())
            .filter(|_| !fullbright)
            .map(|core| &mut core.extra);
        let lines = self.core_lines.as_mut().map(|core| &mut core.extra);
        let sky = (self.core_sky.iter_mut()).flat_map(|core| core.expression_blocks());
        lightmap.into_iter().chain(lines).chain(sky)
    }

    /// Whether the game needs an expression context for the installed pack pipelines this frame.
    pub fn reads_pack_expressions(&mut self) -> bool {
        self.polytone_blocks().any(|blocks| blocks.has_uniforms())
    }

    /// Evaluates the installed pack pipelines' expression uniforms from `frame`. The values
    /// are uploaded separately as each pipeline prepares its draw.
    pub fn evaluate_pack_expressions(&mut self, frame: &crate::environment::Frame) {
        for blocks in self.polytone_blocks() {
            blocks.evaluate(frame);
        }
    }

    /// Whether an installed pack core shader reads `PolyGlobals`: only then does the game make them
    /// for it ([`Self::set_poly_globals`]).
    pub fn reads_poly_globals(&mut self) -> bool {
        self.polytone_blocks().any(|blocks| blocks.reads_globals())
    }

    /// The frame's `PolyGlobals` (`polytone_post::PolyGlobals::bytes`) for the installed pack core
    /// shaders that read them, which their next uploads write; none gives zeros.
    pub fn set_poly_globals(&mut self, globals: Option<&PolyGlobalsBytes>) {
        for blocks in self.polytone_blocks() {
            blocks.set_globals(globals);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A resolved uniform `name` at `binding` that the stages `stages` read, giving `value`.
    fn uniform(
        name: &str,
        binding: u32,
        stages: wgpu::ShaderStages,
        value: &str,
    ) -> ExpressionUniform {
        let file = serde_json::json!({ name: value }).to_string();
        let pack = sunrise_assets::resource_source::ResourceSource::indexed(
            [(
                "assets/minecraft/polytone/shader_modifiers/core/sky.json".to_owned(),
                sunrise_assets::resource_source::Resource::bytes(file.into_bytes()),
            )]
            .into_iter()
            .collect(),
        );
        let modifiers = crate::polytone_post::ShaderModifiers::read(&pack, Default::default());
        modifiers.uniform_for_test(name, binding, stages)
    }

    /// A program whose fragment stage reads `Tint`, and `PolyGlobals` when `globals` (which it
    /// otherwise imports without reading).
    fn program(globals: bool) -> CoreProgram {
        let include = |id: &str| {
            sunrise_polytone::globals::is_include(id)
                .then(|| sunrise_polytone::globals::GLSL.to_owned())
        };
        let import = "#moj_import <polytone_globals.glsl>\n";
        let read = if globals {
            " + float(PolyPlayerBlockPos.y)"
        } else {
            ""
        };
        let fsh = format!(
            "#version 330\n{import}layout(std140) uniform Tint {{\n    float value;\n}};\n\
             layout(location = 0) out vec4 fragColor;\n\
             void main() {{\n    fragColor = vec4(value{read});\n}}\n"
        );
        crate::core_shader::translate_core_with(
            "#version 330\nlayout(location = 0) in vec3 Position;\n\
             void main() {\n    gl_Position = vec4(Position, 1.0);\n}\n",
            &fsh,
            &[],
            &include,
            &super::super::core_sky::GLUE,
            &|_| true,
        )
        .expect("the shaders translate")
    }

    /// The blocks bind after the pipeline's own, 256 bytes apart from `base`, each visible to
    /// the stages that read it: `PolyGlobals` first when a stage reads it, then the expression
    /// uniforms, which hold their programs' values.
    #[test]
    fn expression_blocks_lay_out_and_hold_their_values() {
        let (v, f) = (wgpu::ShaderStages::VERTEX, wgpu::ShaderStages::FRAGMENT);
        let uniforms = [
            uniform("Tint", 18, f, "0.25"),
            uniform("Fade", 20, v | f, "0.5 * 3"),
        ];
        let mut blocks = ExpressionBlocks::new(&program(false), &uniforms, 1024);
        assert!(!blocks.reads_globals() && blocks.has_uniforms());
        let layout: Vec<(u32, wgpu::ShaderStages, u64)> = blocks
            .layout()
            .map(|e| match e.ty {
                wgpu::BindingType::Buffer {
                    min_binding_size, ..
                } => (e.binding, e.visibility, min_binding_size.unwrap().get()),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(layout, [(18, f, 16), (20, v | f, 16)]);
        assert_eq!(blocks.size(), 1024 + 512);
        blocks.evaluate(&Default::default());
        let values: Vec<f32> = blocks.values().iter().map(|(_, v)| *v).collect();
        assert_eq!(values, [0.25, 1.5]);
        // `PolyGlobals` read: its 176 bytes at `base`, the uniforms after it.
        let reading = program(true);
        let globals = crate::core_shader::other_binding(
            reading
                .other
                .iter()
                .position(|n| n == "PolyGlobals")
                .unwrap(),
        );
        let with = ExpressionBlocks::new(&reading, &uniforms[..1], 1024);
        assert!(with.reads_globals());
        let layout: Vec<(u32, wgpu::ShaderStages)> =
            with.layout().map(|e| (e.binding, e.visibility)).collect();
        assert_eq!(layout, [(globals, f), (18, f)]);
        assert_eq!(with.size(), 1024 + 512);
        let none = ExpressionBlocks::new(&program(false), &[], 1024);
        assert!(!none.has_uniforms() && !none.reads_globals());
        assert_eq!(none.size(), 1024);
    }
}
