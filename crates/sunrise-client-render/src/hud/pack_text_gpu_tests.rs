//! A pack's GUI text reading an expression uniform (`shader_modifiers/core/text.json`): its
//! value, evaluated from the file, reaches the GUI text pipeline's block through
//! `HudRenderer::prepare`.
use super::*;
use crate::hud::coverage_tests::{Gpu, quad};
use crate::hud::{Blend, GuiScale, rgba};

/// GUI pixels 0 to 4 across and 0 to 2 down fill the target; the colour is `Tint` in red.
const VSH: &str = "#version 330\nlayout(location = 0) in vec3 Position;\n\
    void main() {\n    gl_Position = vec4(Position.x / 2.0 - 1.0, 1.0 - Position.y, 0.0, 1.0);\n}\n";
const FSH: &str = "#version 330\nlayout(std140) uniform Tint {\n    float value;\n};\n\
    layout(location = 0) out vec4 fragColor;\n\
    void main() {\n    fragColor = vec4(value, 0.0, 0.0, 1.0);\n}\n";

/// The GUI text reads `Tint` from the text shader's file (`0.1 + 0.1`, 51 in a unorm byte) in a
/// block after vanilla's four: 0 until the frame's context evaluates it.
#[test]
#[ignore = "daily"]
fn a_pack_gui_text_reads_its_expression_uniform() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let pack = ResourceSource::indexed(
        [(
            "assets/minecraft/polytone/shader_modifiers/core/text.json".to_owned(),
            sunrise_assets::resource_source::Resource::bytes(br#"{"Tint": "0.1 + 0.1"}"#.to_vec()),
        )]
        .into_iter()
        .collect(),
    );
    let modifiers = crate::polytone_post::ShaderModifiers::read(&pack, Default::default());
    let shader = crate::gfx::expression_blocks::TEXT[0];
    let names = modifiers.names(&[shader]);
    let program =
        crate::core_shader::translate_core_with(VSH, FSH, &["IS_GUI"], &|_| None, &GLUE, &|b| {
            names.iter().any(|n| n == b)
        })
        .expect("the shaders translate");
    let expressions = modifiers.resolve((shader, shader), &program, &[]).unwrap();
    gpu.hud
        .set_text_shader(&gpu.device, Some((&program, &expressions)));
    let mut batch = HudBatch::new(
        [4, 2],
        GuiScale {
            requested: 1,
            unicode: false,
        },
    );
    for x in [0.0, 2.0] {
        quad(&mut batch, Blend::Alpha, rgba(255, 255, 255, 255), x, 0.25);
    }
    for v in &mut batch.vertices {
        v.text = 1;
    }
    let mut red = |evaluate: bool| {
        if evaluate {
            let blocks = gpu
                .hud
                .text_expression_blocks()
                .expect("the pack's GUI text");
            blocks.evaluate(&Default::default());
        }
        let mut uploads = crate::gfx::upload::Uploads::new(&gpu.device);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        gpu.hud
            .prepare(&gpu.device, &mut uploads, &mut encoder, &batch, [4, 2]);
        uploads.finish(&encoder);
        gpu.queue.submit([encoder.finish()]);
        let (pixels, _) = gpu.draw(&batch, 0.0, 0.0, false);
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[0])
            .collect::<Vec<_>>()
    };
    assert_eq!(red(false), [0; 8]);
    assert_eq!(red(true), [51; 8]);
}
