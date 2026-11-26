//! A pack's `PER_BUFFER_BLENDING` (spec 3.2) is answered by the device it runs on (#343): a
//! device that draws colour targets differing in blend (wgpu's `INDEPENDENT_BLEND`) supports
//! it, so a pack that requires it loads without a note, and its `blend.<program>.<buffer>`
//! draws that buffer alone with its own state. Original fixture (`water_blend_tests`), no pack
//! files.
use super::depth_test::Gpu;
use super::water_blend_tests::{halves, picture_with};

#[test]
#[ignore = "daily"]
fn a_device_that_draws_per_buffer_blends_supports_the_required_flag() {
    let _gpu = crate::gfx::test_gpu::lock();
    let gpu = Gpu::new().expect("a Vulkan adapter for the per-buffer blending fixture");
    let flags = gpu.adapter.get_downlevel_capabilities().flags;
    // The macros the game's load starts with on this device (`Renderer::define_pack_macros`).
    let mut macros = sunrise_shaders::Macros::standard();
    macros.define_renderer(4);
    crate::gfx::shader_pack::define_device_features(&mut macros, flags);
    let properties = "iris.features.required = PER_BUFFER_BLENDING\n\
                      blend.gbuffers_water.colortex2=off\n";
    if !flags.contains(wgpu::DownlevelFlags::INDEPENDENT_BLEND) {
        // A device without it keeps the note; drawing the pack's two states is not tried.
        let mut unable = macros.clone();
        unable.set_per_buffer_blending(false);
        assert_eq!(macros, unable, "{flags:?}");
        eprintln!("PER_BUFFER_BLENDING_FIXTURE the adapter lacks INDEPENDENT_BLEND: {flags:?}");
        return;
    }
    let (pixels, notes) = picture_with(&gpu, properties, macros);
    assert!(notes.is_empty(), "{notes:?}");
    // Glass (alpha 0.25) over water in `gbuffers_water`'s default factors (spec 3.2,
    // `SRC_ALPHA ONE_MINUS_SRC_ALPHA ONE ONE_MINUS_SRC_ALPHA`) is src * 0.25 + dst * 0.75 per
    // channel in colortex0: 0.75*0.25+0.25*0.75 = 0.375, 0.125*0.25+0.5*0.75 = 0.40625,
    // 0.875*0.25+0.0625*0.75 = 0.265625, as Rgba8Unorm bytes round(c * 255).
    let color_blended = [96, 104, 68, 255];
    // colortex2's own `off`: what the glass wrote, (0.5625, 0.75, 0.3125).
    let normal_glass = [143, 191, 80, 255];
    halves(
        &pixels,
        color_blended,
        normal_glass,
        "required PER_BUFFER_BLENDING",
    );
}

/// The renderer's answer follows `INDEPENDENT_BLEND` alone: a device without it keeps the
/// required flag's note, whatever else it has.
#[test]
fn the_answer_follows_independent_blend() {
    use wgpu::DownlevelFlags as D;
    let properties = sunrise_shaders::Properties::parse(
        "iris.features.required = PER_BUFFER_BLENDING\n",
        &sunrise_shaders::Macros::standard(),
    );
    let note = "the pack requires `PER_BUFFER_BLENDING`, which is not supported yet";
    for (flags, noted) in [
        (D::all(), false),
        (D::INDEPENDENT_BLEND, false),
        (D::all() - D::INDEPENDENT_BLEND, true),
        (D::empty(), true),
    ] {
        let mut macros = sunrise_shaders::Macros::standard();
        crate::gfx::shader_pack::define_device_features(&mut macros, flags);
        let expected: &[&str] = if noted { &[note] } else { &[] };
        assert_eq!(macros.feature_notes(&properties), expected, "{flags:?}");
    }
}
