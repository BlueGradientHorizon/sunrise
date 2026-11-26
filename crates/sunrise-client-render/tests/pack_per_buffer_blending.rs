//! A pack's `PER_BUFFER_BLENDING` (spec 3.2) answered by the real `Renderer` (#343): the macros
//! `Renderer::define_pack_macros` gives a pack decide whether `read_properties` notes a required
//! `PER_BUFFER_BLENDING`. On a device whose Vulkan adapters all draw colour targets that differ
//! in blend (wgpu's `INDEPENDENT_BLEND`) the pack loads without the note, and its composite,
//! one buffer unblended and one additive, builds and runs; where none does, the note stays.
//! Asset-free; run with throwaway SUNRISE_HOME, SUNRISE_LAUNCHER_HOME, XDG_DATA_HOME and
//! XDG_CONFIG_HOME (AGENTS.md):
//! `cargo test -p sunrise-client-render --test pack_per_buffer_blending -- --ignored`
//! (under `python3 tools/ci/with_xvfb.py` without a display) gives each its own. On a Wayland
//! desktop, unset `WAYLAND_DISPLAY` as `with_xvfb.py` does: the child's own `XDG_RUNTIME_DIR`
//! has no Wayland socket.
use client::{gfx::Renderer, graphics::GraphicsApi};
use std::{path::Path, sync::Arc};
use sunrise_client_render as client;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

const NOTE: &str = "the pack requires `PER_BUFFER_BLENDING`, which is not supported yet";

#[test]
#[ignore = "daily"]
fn the_renderers_macros_answer_per_buffer_blending() {
    const CHILD: &str = "SUNRISE_PER_BUFFER_BLENDING_FIXTURE_CHILD";
    if std::env::var_os(CHILD).is_some() {
        run();
        return;
    }
    // Keep the event loop and every writable home in a fresh process.
    let scratch = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "the_renderers_macros_answer_per_buffer_blending",
            "--ignored",
            "--nocapture",
        ])
        .current_dir(scratch.path())
        .env(CHILD, "1")
        .env("SUNRISE_LAUNCHER_NO_SELF_UPDATE", "1")
        .env("SUNRISE_NO_AUDIO", "1")
        .env("SUNRISE_TELEMETRY_URL", "off");
    for name in [
        "SUNRISE_HOME",
        "SUNRISE_LAUNCHER_HOME",
        "XDG_DATA_HOME",
        "XDG_CONFIG_HOME",
        "XDG_RUNTIME_DIR",
        "SUNRISE_LAUNCHER_TRASH",
        "SUNRISE_LAUNCHER_CLIPBOARD",
        "SUNRISE_LAUNCHER_DESKTOP",
    ] {
        let home = scratch.path().join(name);
        std::fs::create_dir(&home).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        command.env(name, home);
    }
    assert!(
        command.status().unwrap().success(),
        "per-buffer blending fixture failed"
    );
}

/// Whether the renderer's device draws per-buffer blends, as wgpu reports it for every Vulkan
/// adapter of this machine; `None` when they disagree, as the renderer's pick is then not known
/// here.
fn independent_blend() -> Option<bool> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(descriptor);
    let answers: Vec<bool> =
        pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN))
            .iter()
            .map(|a| {
                eprintln!("PER_BUFFER_BLENDING_FIXTURE_ADAPTER {:?}", a.get_info());
                a.get_downlevel_capabilities()
                    .flags
                    .contains(wgpu::DownlevelFlags::INDEPENDENT_BLEND)
            })
            .collect();
    assert!(!answers.is_empty(), "refused: no Vulkan adapter");
    answers
        .iter()
        .all(|&a| a == answers[0])
        .then_some(answers[0])
}

/// A pack whose `composite` writes colortex0 unblended and colortex1 additively, and that
/// requires `PER_BUFFER_BLENDING`.
fn pack(folder: &Path) -> sunrise_shaders::Pack {
    let shaders = folder.join("shaders");
    std::fs::create_dir_all(&shaders).unwrap();
    let vertex = "#version 120\nvarying vec2 uv;\n\
                  void main() { uv = gl_MultiTexCoord0.xy; gl_Position = ftransform(); }\n";
    for (name, source) in [
        (
            "shaders.properties",
            "iris.features.required = PER_BUFFER_BLENDING\n\
             blend.composite.colortex1 = ONE ONE ONE ONE\n",
        ),
        ("composite.vsh", vertex),
        (
            "composite.fsh",
            "#version 120\n/* RENDERTARGETS: 0,1 */\nuniform sampler2D colortex0;\nvarying vec2 uv;\n\
             void main() {\n    gl_FragData[0] = vec4(texture2D(colortex0, uv).rgb, 1.0);\n\
             \x20   gl_FragData[1] = vec4(0.25, 0.5, 0.125, 1.0);\n}\n",
        ),
        ("final.vsh", vertex),
        (
            "final.fsh",
            "#version 120\nuniform sampler2D colortex1;\nvarying vec2 uv;\n\
             void main() { gl_FragColor = vec4(texture2D(colortex1, uv).rgb, 1.0); }\n",
        ),
    ] {
        std::fs::write(shaders.join(name), source).unwrap();
    }
    sunrise_shaders::Pack::open(folder).unwrap()
}

fn fixture(window: Arc<Window>) -> Result<(), Box<dyn std::error::Error>> {
    let able = independent_blend().expect("the Vulkan adapters disagree on INDEPENDENT_BLEND");
    let white = [255; 16 * 16 * 4];
    let zero = [0; 4];
    let mut renderer = pollster::block_on(Renderer::new(
        window,
        true,
        &white,
        16,
        16,
        1,
        &[],
        &[],
        (&white, 16, 16),
        None,
        (&zero, 1, 1),
        (&zero, 1, 1),
        (&zero, 1),
        Arc::new(client::gfx::panorama::Faces::colour([0, 0, 0, 255])),
        None,
        client::gfx::RendererDevice::New,
        GraphicsApi::Vulkan,
        true,
        (true, false),
        &client::graphics::Graphics::default(),
        Default::default(),
        Default::default(),
    ))?;
    renderer.resize(64, 64);
    let folder = tempfile::tempdir()?;
    let pack = pack(folder.path());
    // The game's load: the standard macros, then the renderer's.
    let mut macros = sunrise_shaders::Macros::standard();
    renderer.define_pack_macros(&mut macros);
    let (properties, notes) = sunrise_shaders::read_properties(&pack, &mut macros);
    let expected: &[&str] = if able { &[] } else { &[NOTE] };
    assert_eq!(notes, expected, "INDEPENDENT_BLEND {able}");
    eprintln!("PER_BUFFER_BLENDING_FIXTURE answer {able}, notes {notes:?}");
    if able {
        let post =
            sunrise_shaders::load_post(&pack, "", &properties, &macros, &renderer.pack_inputs());
        assert_eq!(post.programs.len(), 2, "{:?}", post.notes);
        renderer
            .set_shader_pack(0, Some(post), &[])
            .unwrap()
            .expect("the pack runs");
    }
    Ok(())
}

struct App;
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_visible(false))
                .unwrap(),
        );
        if let Err(error) = fixture(window) {
            eprintln!("fixture failed: {error}");
            std::process::exit(1);
        }
        event_loop.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

fn run() {
    for key in [
        "SUNRISE_HOME",
        "SUNRISE_LAUNCHER_HOME",
        "XDG_DATA_HOME",
        "XDG_CONFIG_HOME",
    ] {
        assert!(
            std::env::var_os(key).is_some(),
            "set throwaway {key} (AGENTS.md)"
        );
    }
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_any_thread(true);
    }
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    builder.build().unwrap().run_app(&mut App).unwrap();
}
