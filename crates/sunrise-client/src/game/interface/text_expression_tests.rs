//! A pack's text shader reading Polytone expression uniforms (`shader_modifiers/core/text.json`)
//! through a game's frames: the atlas loads the GUI's, the world's and the see-through text's
//! programs with their uniforms, the renderer the game builds takes them, and each drawn frame
//! shows the values the frame evaluated (fullbright changes nothing), or zeros where no world
//! runs (the title screen, a loading screen). Each case runs in a process of its own (one event
//! loop, its own game resources).
use super::native_entity_refs::tests::{run_fresh_cases, with_client_game};
use crate::game::native_hooks::particle_count_tests::frame_scene;
use sunrise_assets::resource_source::{Resource, ResourceSource};

const NAME: &str = "game::text_expression_tests::a_packs_text_draws_its_expression_uniforms";
const CASE: &str = "SUNRISE_TEXT_EXPRESSION_CASE";

/// A pack whose text covers the whole frame with one triangle of each glyph's (a vertex's index
/// picks its corner) in red: the GUI text's `Gui` (0.1 + 0.1: 51 in a byte), the world text's
/// `World` (0.3 * 2: 153), the see-through text's `Fade` (0.25 * 4: 255), each read by that
/// program only, by the shader's own defines.
fn pack() -> ResourceSource {
    let vsh = "#version 330\nlayout(location = 0) in vec3 Position;\nvoid main() {\n    \
               int corner = gl_VertexID % 3;\n    \
               vec2 uv = vec2((corner << 1) & 2, corner & 2);\n    \
               gl_Position = vec4(uv * 2.0 - 1.0 + 0.0 * Position.xy, 0.5, 1.0);\n}\n";
    let fsh = "#version 330\n#ifdef IS_GUI\nlayout(std140) uniform Gui {\n    float value;\n};\n\
               #else\n#ifdef IS_SEE_THROUGH\nlayout(std140) uniform Fade {\n    float value;\n};\n\
               #else\nlayout(std140) uniform World {\n    float value;\n};\n#endif\n#endif\n\
               layout(location = 0) out vec4 fragColor;\n\
               void main() {\n    fragColor = vec4(value, 0.0, 0.0, 1.0);\n}\n";
    let files = [
        ("assets/minecraft/shaders/core/text.vsh", vsh),
        ("assets/minecraft/shaders/core/text.fsh", fsh),
        (
            "assets/minecraft/polytone/shader_modifiers/core/text.json",
            r#"{"Gui": "0.1 + 0.1", "World": "0.3 * 2", "Fade": "0.25 * 4"}"#,
        ),
    ];
    ResourceSource::indexed(
        files
            .into_iter()
            .map(|(path, text)| (path.to_owned(), Resource::bytes(text.as_bytes().to_vec())))
            .collect(),
    )
}

/// The colour of the pixel two in from the top left corner of `draw`'s frame.
fn corner(
    game: &mut super::Game<'_>,
    draw: impl FnOnce(&mut super::Game<'_>) -> sunrise_client::Result<bool>,
) -> [u8; 3] {
    game.renderer.as_mut().unwrap().screenshot = Some(sunrise_client::gfx::ScreenshotRequest {
        path: "unused.png".into(),
        size: None,
        scaled: false,
        background: false,
    });
    draw(game).unwrap();
    let result = game.renderer.as_mut().unwrap().screenshot_result.take();
    let mut pending = result.unwrap().unwrap();
    let image = pending.image_mut();
    let width = image.width as usize;
    image.rgba[(2 * width + 2) * 4..(2 * width + 2) * 4 + 3]
        .try_into()
        .unwrap()
}

/// A game frame.
fn frame(game: &mut super::Game<'_>) -> sunrise_client::Result<bool> {
    game.render_with(|renderer, frame| renderer.render(frame))
}

#[test]
#[ignore = "daily"]
fn a_packs_text_draws_its_expression_uniforms() {
    if run_fresh_cases(NAME, CASE, &["world", "loading"]) {
        return;
    }
    let loading = std::env::var(CASE).unwrap() == "loading";
    with_client_game(|game| {
        game.gui = crate::game::UiResources::load(&pack(), std::path::Path::new("")).unwrap();
        let frame_context = sunrise_client::environment::Frame::default();
        let text = &game.gui.atlas.text_shaders;
        for (expressions, value) in [
            (&text.expressions.gui, 0.2),
            (&text.expressions.world, 0.6),
            (&text.expressions.see_through, 1.0),
        ] {
            let values: Vec<f32> = expressions
                .iter()
                .map(|e| e.evaluate(&frame_context))
                .collect();
            assert_eq!(values, [value]);
        }
        let _event_loop = frame_scene(game);
        assert!(game.reads_pack_expressions());
        if loading {
            return the_loading_screen_draws_zeros(game);
        }
        // A text display in front of the camera, as a server's would arrive.
        let mut display = super::text_displays::RemoteTextDisplay::new(
            super::text_displays::Shows::Text,
            [0.5, 65.5, 3.5],
            0.0,
        );
        display.data = sunrise_client::text_display::TextDisplayData {
            text: std::sync::Arc::new(sunrise_client::text::Text::literal("W")),
            ..Default::default()
        };
        game.text_displays.insert(1, display);
        game.tick_text_displays();
        for fullbright in [false, true] {
            game.renderer.as_mut().unwrap().fullbright = fullbright;
            // The world text over the frame with the GUI hidden (F1), then the GUI's (F3's
            // text) over it.
            game.ui.hide_gui = true;
            assert_eq!(corner(game, frame), [153, 0, 0], "fullbright {fullbright}");
            game.ui.hide_gui = false;
            game.debug = true;
            assert_eq!(corner(game, frame), [51, 0, 0], "fullbright {fullbright}");
        }
        // The title screen: no world runs, the GUI text's value is 0.
        game.title = Some(game.new_title(std::path::PathBuf::from("saves")));
        assert_eq!(corner(game, frame), [0, 0, 0]);
    });
}

/// A loading screen's frame (`loading_frame`, as the level loading and file fixer screens draw)
/// after a world frame evaluated the GUI text's value: its text draws 0.
fn the_loading_screen_draws_zeros(game: &mut super::Game<'_>) {
    game.debug = true;
    assert_eq!(corner(game, frame), [51, 0, 0]);
    let renderer = game.renderer.as_ref().unwrap();
    let size = [renderer.config.width, renderer.config.height];
    let mut hud = sunrise_client::hud::HudBatch::new(size, game.settings.gui_scale_inputs());
    sunrise_client::level_loading::draw_progress(&mut hud, &game.gui.atlas, "Loading", 0.5);
    let loading = |game: &mut super::Game<'_>| {
        let frame = super::loading_frame(&hud);
        game.renderer.as_mut().unwrap().render(&frame)
    };
    assert_eq!(corner(game, loading), [0, 0, 0]);
}
