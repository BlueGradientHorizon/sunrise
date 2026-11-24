//! A pack's `core/text` on a game's world text, through its frames: a text display's
//! translucent background and a name tag's copies, outside and under Improved Transparency.
//! Vanilla 26.3 draws a name tag's depth-tested copy with `RenderTypes.text`
//! (`RenderPipelines.TEXT`, `OIT_TEXT`; `SubmitNodeCollection.submitNameTag`), a text display's
//! background with `RenderTypes.textPolygonOffset` (`TEXT_POLYGON_OFFSET`,
//! `OIT_TEXT_POLYGON_OFFSET`), both from `core/text`. Each case runs in a process of its own (one
//! event loop, its own resources).
use super::lightmap_expression_tests::{corner_of as corner, frame};
use super::native_entity_refs::tests::{run_fresh_cases, with_client_game};
use crate::game::native_hooks::particle_count_tests::frame_scene;
use sunrise_core::nbt::Tag;
use sunrise_protocol::entity_data::Value;

const NAME: &str = "game::world_text_frame_tests::a_packs_text_draws_world_text_with_and_without_improved_transparency";
const CASE: &str = "SUNRISE_WORLD_TEXT_FRAME_CASE";

/// Text that covers the whole frame with one triangle of each glyph's quad (a vertex's index
/// picks its corner), nearer than the empty world: translucent text nearer (depth 0.75, reversed)
/// than opaque text (0.25), so a frame shows which program drew the nearest. The
/// world text paints opaque text green (0, 255, 0) and translucent text, backgrounds included,
/// red (255, 0, 0); under Improved Transparency it goes through vanilla's `oit.glsl`, opaque, and
/// accumulates magenta (255, 0, 255). The GUI's and the see-through text draw nothing.
const TEXT: [(&str, &str); 2] = [
    (
        "assets/minecraft/shaders/core/text.vsh",
        "#version 330\nlayout(location = 0) in vec3 Position;\nlayout(location = 1) in vec4 Color;\n\
         layout(location = 0) flat out vec4 vertexColor;\nvoid main() {\n    \
         int corner = gl_VertexID % 3;\n    vec2 uv = vec2((corner << 1) & 2, corner & 2);\n    \
         float depth = Color.a > 0.99 ? 0.25 : 0.75;\n    \
         gl_Position = vec4(uv * 2.0 - 1.0 + 0.0 * Position.xy, depth, 1.0);\n    \
         vertexColor = Color;\n}\n",
    ),
    (
        "assets/minecraft/shaders/core/text.fsh",
        "#version 330\n#include <minecraft:oit.glsl>\n\
         layout(location = 0) flat in vec4 vertexColor;\n\
         #ifndef OIT_ALPHA_ONLY\nlayout(location = 0) out vec4 fragColor;\n#endif\nvoid main() {\n\
         #if defined(IS_GUI) || defined(IS_SEE_THROUGH)\n    discard;\n\
         #elif defined(OIT_ALPHA_ONLY)\n    executeAlphaOnlyPhase(gl_FragCoord.z, 1.0);\n\
         #elif defined(OIT_ACCUMULATE)\n    \
         fragColor = sampleColorForAccumulation(vec4(1.0, 0.0, 1.0, 1.0));\n\
         #else\n    fragColor = vertexColor.a > 0.99 ? vec4(0.0, 1.0, 0.0, 1.0) \
         : vec4(1.0, 0.0, 0.0, 1.0);\n#endif\n}\n",
    ),
];

const GREEN: [u8; 3] = [0, 255, 0];
const RED: [u8; 3] = [255, 0, 0];
const MAGENTA: [u8; 3] = [255, 0, 255];

/// With Improved Transparency off and on, one session with the pack's text:
/// - a text display's text (opaque, the solid phase) and its default background (translucent):
///   the background's program draws last, red outside Improved Transparency, its `OIT_*` stages'
///   magenta under it;
/// - an empty display with no background draws nothing (the frame is the empty scene's);
/// - named, its name tag's depth-tested copy is opaque text: green in either mode (the solid
///   phase, as `canRenderAsSolid` puts it);
/// - crouching (`isDiscrete`), the tag is drawn once, faded on its background: translucent text,
///   red outside Improved Transparency, magenta under it.
#[test]
#[ignore = "daily"]
fn a_packs_text_draws_world_text_with_and_without_improved_transparency() {
    if run_fresh_cases(NAME, CASE, &["classic", "improved_transparency"]) {
        return;
    }
    let improved = std::env::var(CASE).unwrap() == "improved_transparency";
    with_client_game(|game| {
        let pack = super::pack_clouds_frame_tests::pack(&TEXT);
        game.gui = crate::game::UiResources::load(&pack, std::path::Path::new("")).unwrap();
        let text = &game.gui.atlas.text_shaders;
        assert!(
            text.world.is_some() && text.oit.is_some(),
            "the pack's text"
        );
        let _event_loop = frame_scene(game);
        let renderer = game.renderer.as_mut().unwrap();
        renderer.improved_transparency = improved;
        if improved
            && !renderer
                .device
                .features()
                .contains(wgpu::Features::FLOAT32_BLENDABLE)
        {
            // `oit::supported`: Improved Transparency needs blendable float targets.
            eprintln!("skipped: no Improved Transparency on this adapter");
            return;
        }
        let sy = game.world.shape.column().section_index(65).unwrap() as i32;
        game.settings.chunk_fade = 0.0;
        game.renderer
            .as_mut()
            .unwrap()
            .upload((0, sy, 0), &sunrise_client::mesher::SectionMesh::default());
        // The world alone (F1): the GUI's text draws nothing anyway.
        game.ui.hide_gui = true;
        let empty = corner(game, frame);
        let translucent = if improved { MAGENTA } else { RED };

        // A text display in front of the camera, as a server's would arrive.
        let mut display = super::text_displays::RemoteTextDisplay::new(
            super::text_displays::Shows::Text,
            [0.5, 65.5, 3.5],
            0.0,
        );
        display.metadata(&[(23, 0, Value::Text(Tag::String("W".into())))]);
        game.text_displays.insert(1, display);
        game.tick_text_displays(game.clock.runs_normally());
        assert_eq!(corner(game, frame), translucent, "a display's background");

        // No text and no background: nothing drawn.
        let display = game.text_displays.get_mut(&1).unwrap();
        display.metadata(&[
            (23, 0, Value::Text(Tag::String("".into()))),
            (25, 1, Value::Int(0)),
        ]);
        game.tick_text_displays(game.clock.runs_normally());
        assert_eq!(corner(game, frame), empty, "an empty display");

        let display = game.text_displays.get_mut(&1).unwrap();
        display.metadata(&[
            (2, 0, Value::OptText(Some(Tag::String("Tag".into())))),
            (3, 0, Value::Bool(true)),
        ]);
        game.tick_text_displays(game.clock.runs_normally());
        assert_eq!(corner(game, frame), GREEN, "a name tag");

        let display = game.text_displays.get_mut(&1).unwrap();
        display.metadata(&[(0, 0, Value::Byte(0x02))]);
        game.tick_text_displays(game.clock.runs_normally());
        assert_eq!(corner(game, frame), translucent, "a discrete name tag");
    });
}

/// `SubmitNodeCollection.canRenderAsSolid`: a name tag's `NORMAL` copy is solid only when its
/// text is opaque and its background has no alpha, whatever the background's colour.
#[test]
fn a_name_tag_copy_is_solid_only_when_opaque_on_no_background() {
    use super::names::name_tag_layer;
    use sunrise_client::gfx::{TEXT_NORMAL_OVERLAY, TEXT_NORMAL_SOLID_OVERLAY};
    assert_eq!(name_tag_layer(0xFF, 0), TEXT_NORMAL_SOLID_OVERLAY);
    assert_eq!(name_tag_layer(0xFF, 0x00FF_FFFF), TEXT_NORMAL_SOLID_OVERLAY);
    assert_eq!(name_tag_layer(0xFF, 0x4000_0000), TEXT_NORMAL_OVERLAY);
    assert_eq!(name_tag_layer(0xFE, 0), TEXT_NORMAL_OVERLAY);
    assert_eq!(name_tag_layer(0x20, 0x4000_0000), TEXT_NORMAL_OVERLAY);
}
