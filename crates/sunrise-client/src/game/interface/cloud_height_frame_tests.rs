//! The cloud layer's height through a game's frames: `EnvironmentAttributes.CLOUD_HEIGHT` as the
//! dimension type sets it (`minecraft:visual/cloud_height`), probed at the camera each frame as
//! vanilla's `LevelExtractor` does, places the layer the renderer draws. A pack's `core/clouds`
//! shows it: its `CloudInfo.CloudOffset.y` is the layer's bottom less the camera's height. Each
//! case runs in a process of its own (one event loop, its own resources).
use super::native_entity_refs::tests::{run_fresh_cases, with_client_game};
use crate::game::native_hooks::particle_count_tests::frame_scene;
use sunrise_assets::resource_source::{Resource, ResourceSource};
use sunrise_client::gfx::ScreenshotRequest;

const NAME: &str = "game::cloud_height_frame_tests::the_cloud_layer_follows_the_dimensions_height";
const CASE: &str = "SUNRISE_CLOUD_HEIGHT_FRAME_CASE";

/// A pack whose clouds cover the whole frame (each face's quad placed by `gl_VertexIndex`,
/// nearest of all), white where the layer's bottom is more than 150 blocks above the camera
/// (`CloudOffset.y`), black where it isn't: the camera stands at y 64, 186 below a layer at 250
/// and 128 below one at 192.33.
fn pack() -> ResourceSource {
    let files = [
        (
            "assets/minecraft/shaders/core/clouds.vsh",
            "#version 330\nlayout(std140) uniform CloudInfo {\n    vec4 CloudColor;\n    \
             vec3 CloudOffset;\n    vec3 CellSize;\n};\nlayout(location = 0) out float above;\n\
             void main() {\n    int corner = gl_VertexIndex % 4;\n    \
             vec2 xy = vec2(corner == 1 || corner == 2 ? 1.0 : -1.0, corner >= 2 ? 1.0 : -1.0);\n    \
             above = CloudOffset.y > 150.0 ? 1.0 : 0.0;\n    \
             gl_Position = vec4(xy, 1.0, 1.0);\n}\n",
        ),
        (
            "assets/minecraft/shaders/core/clouds.fsh",
            "#version 330\nlayout(location = 0) in float above;\n\
             layout(location = 0) out vec4 fragColor;\n\
             void main() {\n    fragColor = vec4(vec3(above), 1.0);\n}\n",
        ),
    ];
    ResourceSource::indexed(
        files
            .into_iter()
            .map(|(path, text)| (path.to_owned(), Resource::bytes(text.as_bytes().to_vec())))
            .collect(),
    )
}

/// The overworld's dimension type as the prepared resources have it, with its
/// `minecraft:visual/cloud_height` set to `height`, loaded as the game loads one.
fn overworld(height: f64) -> sunrise_client::environment::Environment {
    let resources = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
    let path = "data/minecraft/dimension_type/overworld.json";
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(resources.join(path)).unwrap()).unwrap();
    json["attributes"]["minecraft:visual/cloud_height"] = serde_json::json!(height);
    let source = ResourceSource::indexed(
        [(
            path.to_owned(),
            Resource::bytes(json.to_string().into_bytes()),
        )]
        .into_iter()
        .collect(),
    );
    sunrise_client::environment::Environment::load_dimension(&source, "overworld").unwrap()
}

/// A pixel left of the hotbar and the crosshair.
fn drawn_pixel(game: &mut super::Game<'_>) -> [u8; 4] {
    game.renderer.as_mut().unwrap().screenshot = Some(ScreenshotRequest {
        path: "unused.png".into(),
        size: None,
        scaled: false,
        background: false,
    });
    game.render_with(|renderer, frame| renderer.render(frame))
        .unwrap();
    let result = game.renderer.as_mut().unwrap().screenshot_result.take();
    let mut pending = result.unwrap().unwrap();
    let image = pending.image_mut();
    let at = (40 * image.width as usize + 20) * 4;
    image.rgba[at..at + 4].try_into().unwrap()
}

/// The overworld's own height (192.33) leaves the layer 128 above the camera; a dimension type
/// setting 250 raises it to 186 above, the frame it is set in.
#[test]
#[ignore = "daily"]
fn the_cloud_layer_follows_the_dimensions_height() {
    if run_fresh_cases(NAME, CASE, &["default", "raised"]) {
        return;
    }
    let raised = std::env::var(CASE).unwrap() == "raised";
    with_client_game(|game| {
        game.gui = crate::game::UiResources::load(&pack(), std::path::Path::new("")).unwrap();
        assert!(game.gui.atlas.clouds_shader.is_some());
        let _event_loop = frame_scene(game);
        game.screen = super::Screen::Playing;
        game.screen_fx.vignette_brightness = 0.0;
        if raised {
            game.pack.environments = Some(std::sync::Arc::new([
                overworld(250.0),
                overworld(250.0),
                overworld(250.0),
            ]));
        }
        let expected = if raised { 255 } else { 0 };
        assert_eq!(drawn_pixel(game), [expected, expected, expected, 255]);
    });
}
