//! The colour targets `build_pass` gives composite-style programs for their `blend.*` keys, as
//! `docs/cleanroom/decisions.md` D-13 records them: `shadowcomp`'s per-buffer lines, `final`
//! never blending, and only `BLENDABLE` formats blending. The keys themselves are pinned in
//! `sunrise-shaders`' `post::blend_key_tests`. The pipelines are recorded, not made
//! (`Resources::jobs`), so each test reads the targets the pass would be made with.

use sunrise_shaders::{BUFFERS, Group};
use wgpu::TextureFormat as T;

use super::*;

const VSH: &str = "#version 120\nvoid main() { gl_Position = ftransform(); }\n";

/// The program `name` of a pack of `files`, loaded with `properties` (the shadow pass forced on).
fn program(files: &[(&str, &str)], properties: &str, name: &str) -> sunrise_shaders::PostProgram {
    let dir = tempfile::tempdir().unwrap();
    let shaders = dir.path().join("shaders");
    std::fs::create_dir_all(&shaders).unwrap();
    for (file, text) in files {
        std::fs::write(shaders.join(file), text).unwrap();
    }
    let pack = sunrise_shaders::Pack::open(dir.path()).unwrap();
    let properties = sunrise_shaders::Properties::parse(
        &format!("shadow.enabled = true\n{properties}"),
        &sunrise_shaders::Macros::default(),
    );
    let post = sunrise_shaders::load_post(
        &pack,
        "",
        &properties,
        &sunrise_shaders::Macros::standard(),
        &Default::default(),
    );
    (post.programs.into_iter().chain(post.shadow_comp))
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("{name} loads: {:?}", post.notes))
}

/// The colour targets of the one render pipeline `build_pass` records for `p`, its buffers in
/// `formats` and the screen in `Rgba8Unorm`, a format that blends.
fn recorded_targets(
    p: sunrise_shaders::PostProgram,
    formats: &[wgpu::TextureFormat; BUFFERS],
) -> Option<Vec<Option<wgpu::ColorTargetState>>> {
    let _gpu = crate::test_gpu::lock();
    let adapter = crate::test_gpu::adapter()?;
    // A pass's pipeline layout has six bind group slots.
    let desc = wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    };
    let (device, _queue) = crate::test_gpu::optional_device_with(&adapter, &desc)?;
    eprintln!("adapter: {:?}", adapter.get_info().name);
    let buffers = crate::pack_images::Buffers::new(
        &device,
        Default::default(),
        Default::default(),
        wgpu::ShaderStages::FRAGMENT,
    );
    let stand_ins = std::cell::RefCell::new(Vec::new());
    let mismatches = std::cell::RefCell::new(std::collections::HashSet::new());
    let pack = Resources {
        textures: &[],
        images: &[],
        left_out: &[],
        buffers: &buffers,
        indices: None,
        gpu: Gpu::of(&adapter, device.features()),
        storable: 0,
        rgb: 0,
        packed: [None; BUFFERS],
        released: std::cell::Cell::new(("", 0)),
        stand_ins: &stand_ins,
        shadow_linear: 0,
        jobs: Some(std::cell::RefCell::new(Vec::new())),
        mod_depth_targets: &[],
        mod_uniforms: &[],
        display_uniforms: false,
        mod_uniform_mismatches: &mismatches,
        mod_depth_target: std::cell::Cell::new(None),
        depth_compare: std::cell::Cell::new(wgpu::CompareFunction::LessEqual),
        mod_depth_bias: Default::default(),
        mod_vertex: Default::default(),
    };
    let draws = crate::pack_draw::DrawUniforms::new(&device);
    let mut notes = Vec::new();
    build_pass(
        &device,
        p,
        formats,
        None,
        T::Rgba8Unorm,
        &mut notes,
        None,
        &[],
        &draws.layout,
        &pack,
        false,
        false,
    )
    .unwrap_or_else(|| panic!("the pass builds: {notes:?}"));
    let jobs = pack.jobs.as_ref().unwrap().take();
    let targets: Vec<_> = jobs
        .iter()
        .filter_map(|j| match j {
            Job::Render(_, job) => Some(job.targets.clone()),
            Job::Compute(..) => None,
        })
        .collect();
    assert_eq!(targets.len(), 1, "one render pipeline");
    targets.into_iter().next()
}

/// The blend of each target, in location order.
fn blends(targets: &[Option<wgpu::ColorTargetState>]) -> Vec<Option<wgpu::BlendState>> {
    targets.iter().map(|t| t.as_ref().unwrap().blend).collect()
}

fn add(
    src: wgpu::BlendFactor,
    dst: wgpu::BlendFactor,
    src_alpha: wgpu::BlendFactor,
    dst_alpha: wgpu::BlendFactor,
) -> Option<wgpu::BlendState> {
    let component = |src_factor, dst_factor| wgpu::BlendComponent {
        src_factor,
        dst_factor,
        operation: wgpu::BlendOperation::Add,
    };
    Some(wgpu::BlendState {
        color: component(src, dst),
        alpha: component(src_alpha, dst_alpha),
    })
}

/// D-13: `blend.shadowcomp.colortex0` blends shadowcolor0, the program's first output;
/// `blend.shadowcomp` is not read and `blend.shadowcomp.shadowcolor1` names no buffer, so
/// shadowcolor1 is written without blending.
#[test]
#[ignore = "daily"]
fn shadowcomp_blends_the_shadow_buffer_its_colortex_line_numbers() {
    use wgpu::BlendFactor as F;
    let p = program(
        &[
            ("shadowcomp.vsh", VSH),
            (
                "shadowcomp.fsh",
                "#version 120\nvoid main() {\n\
                 gl_FragData[0] = vec4(0.75);\ngl_FragData[1] = vec4(0.875);\n}\n",
            ),
        ],
        "blend.shadowcomp = ONE ONE ONE ONE\n\
         blend.shadowcomp.colortex0 = SRC_ALPHA ONE_MINUS_SRC_ALPHA ONE ZERO\n\
         blend.shadowcomp.shadowcolor1 = ONE ZERO ONE ZERO\n",
        "shadowcomp",
    );
    assert_eq!((p.group, &p.buffers[..]), (Group::ShadowComp, &[0, 1][..]));
    let Some(targets) = recorded_targets(p, &[T::Rgba8Unorm; BUFFERS]) else {
        return;
    };
    assert_eq!(
        blends(&targets),
        [add(F::SrcAlpha, F::OneMinusSrcAlpha, F::One, F::Zero), None]
    );
}

/// D-13: `final` writes the screen without blending, though the loader keeps `blend.final`'s
/// factors on the program and the screen's format blends.
#[test]
#[ignore = "daily"]
fn final_writes_the_screen_without_blending() {
    let p = program(
        &[
            ("final.vsh", VSH),
            (
                "final.fsh",
                "#version 120\nvoid main() { gl_FragData[0] = vec4(0.5); }\n",
            ),
        ],
        "blend.final = SRC_ALPHA ONE_MINUS_SRC_ALPHA ONE ZERO\n",
        "final",
    );
    assert!(p.group == Group::Final && p.blend.is_some());
    let Some(targets) = recorded_targets(p, &[T::Rgba8Unorm; BUFFERS]) else {
        return;
    };
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].as_ref().unwrap().format, T::Rgba8Unorm);
    assert_eq!(blends(&targets), [None]);
}

/// D-13: factors apply only to an output whose buffer format wgpu reports as `BLENDABLE`. An
/// `R32Uint` buffer (integer formats never blend in wgpu) is written without blending, both
/// under `blend.<program>` (composite) and under a per-buffer line (shadowcomp), while the
/// `Rgba8Unorm` buffer beside it blends.
#[test]
#[ignore = "daily"]
fn only_blendable_formats_blend() {
    use wgpu::BlendFactor as F;
    let two = "#version 120\n/* DRAWBUFFERS:34 */\nvoid main() {\n\
               gl_FragData[0] = vec4(0.25);\ngl_FragData[1] = vec4(0.5);\n}\n";
    let composite = program(
        &[("composite.vsh", VSH), ("composite.fsh", two)],
        "blend.composite = ONE ONE ZERO ONE\n",
        "composite",
    );
    assert_eq!(&composite.buffers[..], &[3, 4]);
    let mut formats = [T::Rgba8Unorm; BUFFERS];
    formats[3] = T::R32Uint;
    let Some(targets) = recorded_targets(composite, &formats) else {
        return;
    };
    let target = |l: usize| targets[l].as_ref().unwrap().format;
    assert_eq!((target(0), target(1)), (T::R32Uint, T::Rgba8Unorm));
    assert_eq!(
        blends(&targets),
        [None, add(F::One, F::One, F::Zero, F::One)]
    );

    let shadowcomp = program(
        &[
            ("shadowcomp.vsh", VSH),
            (
                "shadowcomp.fsh",
                "#version 120\nvoid main() {\n\
                 gl_FragData[0] = vec4(0.75);\ngl_FragData[1] = vec4(0.875);\n}\n",
            ),
        ],
        "blend.shadowcomp.colortex0 = DST_COLOR ZERO ZERO ONE\n\
         blend.shadowcomp.colortex1 = DST_COLOR ZERO ZERO ONE\n",
        "shadowcomp",
    );
    let mut formats = [T::Rgba8Unorm; BUFFERS];
    formats[0] = T::R32Uint;
    let Some(targets) = recorded_targets(shadowcomp, &formats) else {
        return;
    };
    assert_eq!(
        blends(&targets),
        [None, add(F::Dst, F::Zero, F::Zero, F::One)]
    );
}
