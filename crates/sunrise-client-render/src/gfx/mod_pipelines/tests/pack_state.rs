//! Mod pipelines hold shader-pack state only while a shader pack is selected (#6728): the pack's
//! uniform buffer, each pipeline's pack set-0 group and, under linear light, its gamma fragment
//! module are built on the selection, for pipelines built before it too, dropped when the pack
//! is turned off and built again when it is turned back on; nothing is uploaded for a pack
//! before. Draws on the world's target and on a pack's stay the same throughout.
use super::*;

impl ModPipelines {
    /// `[pack uniform buffers, pack set-0 groups, pack fragment modules]` held now.
    fn pack_objects(&self) -> [usize; 3] {
        let Some(pack) = &self.pack else {
            return [0; 3];
        };
        let sources = || pack.sources.iter().flatten();
        [
            1,
            sources().count(),
            sources().filter(|s| s.fragment_shader.is_some()).count(),
        ]
    }
}

/// The quad's colour, as stored bytes: each channel distinct.
const COLOR: [u8; 4] = [200, 40, 120, 255];
/// The frame's clear colour as `render_target` reads it back in 8 bits.
const BACKGROUND: [f32; 4] = [64.0, 64.0, 64.0, 255.0];

/// `n` through-walls pipelines of load `generation` writing their vertex colour.
fn pipelines(generation: u64, n: usize) -> Pipelines {
    let shaders = vec![(PASS, PipelineBlend::Opaque, WorldDepth::ThroughWalls); n];
    load(generation, &shaders)
}

/// The 1024-byte vanilla block uploads one `prepare` of `vertices` drawn by `runs` makes, or of
/// no runs with the frame's vanilla blocks when `runs` is empty.
fn block_uploads(
    (device, queue): &(wgpu::Device, wgpu::Queue),
    primitives: &mut WorldPrimitives,
    set: &Pipelines,
    vertices: &[PipelineVertex],
    runs: &[PipelineRun],
) -> usize {
    let atlas = device.create_texture(&wgpu::TextureDescriptor {
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
    });
    let blocks = fallback_vanilla_blocks(view_proj(), [SIZE, SIZE], 0.0);
    let mut uploads = 0;
    primitives.prepare(
        device,
        &mut |buffer, bytes| {
            if buffer.size() == 1024 && bytes.len() == 1024 {
                uploads += 1;
            }
            queue.write_buffer(buffer, 0, bytes);
        },
        &world_primitives::Batch {
            pipelines: Batch {
                set: Some(set),
                vertices,
                runs,
                ..Default::default()
            },
            ..Default::default()
        },
        view_proj(),
        [SIZE, SIZE],
        &atlas.create_view(&Default::default()),
        Some(&blocks),
    );
    uploads
}

/// The world's target shows the quad over the frame's top left (screen rows); a pack's shows
/// it over its bottom left, its rows being GL's (`pack_clip_matrix`), which only the pack's
/// blocks give.
fn assert_drawn(at: &dyn Fn(u32, u32) -> [f32; 4], pack: bool, what: &str) {
    let color = COLOR.map(f32::from);
    let (drawn, clear) = if pack {
        ((16, 48), (16, 16))
    } else {
        ((16, 16), (16, 48))
    };
    assert_eq!(at(drawn.0, drawn.1), color, "{what}: the quad");
    assert_eq!(at(clear.0, clear.1), BACKGROUND, "{what}: beside it");
}

#[test]
#[ignore = "daily"]
fn pack_state_exists_only_while_a_pack_is_selected() {
    let _gpu = crate::gfx::test_gpu::lock();
    let Some(gpu) = device() else {
        panic!("no GPU adapter");
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let vertices = quad([8.0, 8.0, 24.0, 24.0], 2.0, COLOR);
    let runs = [PipelineRun {
        slot: 1,
        stage: world_primitives::STAGE_CUSTOM_GEOMETRY as u32,
        quads: 1,
        modifier_stack: 0,
    }];
    for linear in [false, true] {
        let what = |step: &str| format!("linear light {linear}, {step}");
        let mut primitives = WorldPrimitives::new(&gpu.0, (format, linear));
        let two = pipelines(61, 2);
        let batch = |set| Batch {
            set: Some(set),
            vertices: &vertices,
            runs: &runs,
            ..Default::default()
        };
        // No pack: the world draws, and nothing of a pack is built or uploaded.
        let at = render_target(
            &gpu,
            &mut primitives,
            format,
            batch(&two),
            STAGES - 1,
            false,
        );
        if !linear {
            assert_drawn(&at, false, &what("no pack"));
        }
        assert_eq!(
            primitives.pipelines.pack_objects(),
            [0, 0, 0],
            "{}",
            what("no pack")
        );
        assert_eq!(
            block_uploads(&gpu, &mut primitives, &two, &vertices, &runs),
            1
        );
        assert_eq!(block_uploads(&gpu, &mut primitives, &two, &[], &[]), 1);

        // The first selection builds them for the two pipelines built before it.
        let gamma = usize::from(linear) * 2;
        let at = render_target(&gpu, &mut primitives, format, batch(&two), STAGES - 1, true);
        assert_drawn(&at, true, &what("pack on"));
        assert_eq!(
            primitives.pipelines.pack_objects(),
            [1, 2, gamma],
            "{}",
            what("pack on")
        );
        assert_eq!(
            block_uploads(&gpu, &mut primitives, &two, &vertices, &runs),
            2
        );
        assert_eq!(block_uploads(&gpu, &mut primitives, &two, &[], &[]), 2);

        // Off: all of it goes, the world still draws.
        primitives.pipelines.deselect_pack();
        assert_eq!(
            primitives.pipelines.pack_objects(),
            [0, 0, 0],
            "{}",
            what("pack off")
        );
        let at = render_target(
            &gpu,
            &mut primitives,
            format,
            batch(&two),
            STAGES - 1,
            false,
        );
        if !linear {
            assert_drawn(&at, false, &what("pack off"));
        }
        assert_eq!(
            block_uploads(&gpu, &mut primitives, &two, &vertices, &runs),
            1
        );

        // On again: built again, and the pack's target draws as before.
        let at = render_target(&gpu, &mut primitives, format, batch(&two), STAGES - 1, true);
        assert_drawn(&at, true, &what("pack on again"));
        assert_eq!(
            primitives.pipelines.pack_objects(),
            [1, 2, gamma],
            "{}",
            what("pack on again")
        );

        // A load built while the pack is on gets its own.
        let three = pipelines(62, 3);
        primitives.pipelines.build(&gpu.0, &three, None).unwrap();
        let gamma = usize::from(linear) * 3;
        assert_eq!(
            primitives.pipelines.pack_objects(),
            [1, 3, gamma],
            "{}",
            what("reload")
        );
        let at = render_target(
            &gpu,
            &mut primitives,
            format,
            batch(&three),
            STAGES - 1,
            true,
        );
        assert_drawn(&at, true, &what("reload"));
    }
}

/// Through the real `Renderer`: a frame's shader pack selection builds the mod pipelines' pack
/// state, turning the pack off drops it and turning it on again builds it again. A child
/// process of the test binary (a window and its event loop), with throwaway homes.
#[test]
#[ignore = "daily"]
fn a_renderer_builds_mod_pipeline_pack_state_only_with_a_pack() {
    const CHILD: &str = "SUNRISE_MOD_PIPELINE_PACK_STATE_CHILD";
    if std::env::var_os(CHILD).is_some() {
        renderer_child();
        return;
    }
    let _gpu = crate::gfx::test_gpu::lock();
    let scratch = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "gfx::mod_pipelines::tests::pack_state::a_renderer_builds_mod_pipeline_pack_state_only_with_a_pack",
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
    let output = command.output().unwrap();
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("{stdout}");
    assert!(output.status.success(), "the renderer fixture failed");
    // The child ran the fixture: a filtered-out run would pass without it.
    assert!(
        stdout.contains("1 passed"),
        "the child ran no test: {stdout}"
    );
}

/// A pack of only a `final` showing `colortex0`.
fn final_pack(folder: &std::path::Path) -> sunrise_shaders::Pack {
    let shaders = folder.join("shaders");
    std::fs::create_dir_all(&shaders).unwrap();
    for (name, source) in [
        (
            "final.vsh",
            "#version 120\nvarying vec2 uv;\n\
             void main() { uv = gl_MultiTexCoord0.xy; gl_Position = ftransform(); }\n",
        ),
        (
            "final.fsh",
            "#version 120\nuniform sampler2D colortex0;\nvarying vec2 uv;\n\
             void main() { gl_FragColor = vec4(texture2D(colortex0, uv).rgb, 1.0); }\n",
        ),
    ] {
        std::fs::write(shaders.join(name), source).unwrap();
    }
    sunrise_shaders::Pack::open(folder).unwrap()
}

fn renderer_fixture(window: std::sync::Arc<winit::window::Window>) {
    use crate::gfx::Renderer;
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
        std::sync::Arc::new(crate::gfx::panorama::Faces::colour([0, 0, 0, 255])),
        None,
        crate::gfx::RendererDevice::New,
        crate::graphics::GraphicsApi::Vulkan,
        true,
        (true, false),
        &crate::graphics::Graphics::default(),
        Default::default(),
        Default::default(),
    ))
    .expect("a renderer");
    renderer.resize(64, 64);
    eprintln!(
        "MOD_PIPELINE_PACK_STATE_ADAPTER {:?}",
        renderer.adapter.get_info()
    );
    let linear = renderer.output.frame.linear_light;
    let objects = |renderer: &Renderer| {
        renderer
            .world_primitives
            .as_ref()
            .expect("world primitives")
            .pipelines
            .pack_objects()
    };
    let vertices = quad([8.0, 8.0, 24.0, 24.0], 2.0, COLOR);
    let runs = [PipelineRun {
        slot: 0,
        stage: world_primitives::STAGE_CUSTOM_GEOMETRY as u32,
        quads: 1,
        modifier_stack: 0,
    }];
    let hud = crate::hud::HudBatch::scratch([4, 4]);
    let render = |renderer: &mut Renderer, set: &Pipelines| {
        let mut frame = crate::gfx::frame_tests::frame(&hud);
        frame.world_primitives = world_primitives::Batch {
            pipelines: Batch {
                set: Some(set),
                vertices: &vertices,
                runs: &runs,
                ..Default::default()
            },
            ..Default::default()
        };
        renderer.render(&frame, None).unwrap();
    };
    let folder = tempfile::tempdir().unwrap();
    let pack = final_pack(folder.path());
    let set_pack = |renderer: &mut Renderer| {
        let mut macros = sunrise_shaders::Macros::standard();
        renderer.define_pack_macros(&mut macros);
        let (properties, _) = sunrise_shaders::read_properties(&pack, &mut macros);
        let post =
            sunrise_shaders::load_post(&pack, "", &properties, &macros, &renderer.pack_inputs());
        renderer
            .set_shader_pack(0, Some(post), &[])
            .unwrap_or_else(|_| panic!("the pack is refused"))
            .expect("the pack runs");
    };

    let one = pipelines(71, 1);
    renderer.build_mod_pipelines(&one, &|_| None).unwrap();
    render(&mut renderer, &one);
    assert_eq!(objects(&renderer), [0, 0, 0], "no pack");

    set_pack(&mut renderer);
    // The selection alone builds nothing: the frame does.
    assert_eq!(objects(&renderer), [0, 0, 0], "selected, no frame yet");
    render(&mut renderer, &one);
    let gamma = usize::from(linear);
    assert_eq!(
        objects(&renderer),
        [1, 1, gamma],
        "pack on (linear light {linear})"
    );

    renderer.set_shader_pack(0, None, &[]).unwrap();
    render(&mut renderer, &one);
    assert_eq!(objects(&renderer), [0, 0, 0], "pack off");

    set_pack(&mut renderer);
    render(&mut renderer, &one);
    assert_eq!(objects(&renderer), [1, 1, gamma], "pack on again");

    let two = pipelines(72, 2);
    renderer.build_mod_pipelines(&two, &|_| None).unwrap();
    render(&mut renderer, &two);
    assert_eq!(
        objects(&renderer),
        [1, 2, 2 * gamma],
        "a reload with the pack on"
    );
    eprintln!("MOD_PIPELINE_PACK_STATE_FIXTURE linear light {linear}: passed");
}

fn renderer_child() {
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop},
        window::{Window, WindowId},
    };
    struct App;
    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let window = std::sync::Arc::new(
                event_loop
                    .create_window(Window::default_attributes().with_visible(false))
                    .unwrap(),
            );
            renderer_fixture(window);
            event_loop.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
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
