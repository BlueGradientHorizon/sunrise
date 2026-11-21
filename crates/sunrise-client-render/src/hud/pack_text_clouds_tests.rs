//! A pack's `core/clouds` as the atlas loads it (`load_clouds`).
use super::*;
use sunrise_assets::resource_source::Resource;

const VSH: &str = "assets/minecraft/shaders/core/clouds.vsh";
const FSH: &str = "assets/minecraft/shaders/core/clouds.fsh";

/// `clouds.vsh` in shape: `CloudInfo` and the `CloudFaces` texel buffer.
const CLOUDS_VSH: &str = "#version 330\n\
    layout(std140) uniform CloudInfo {\n    vec4 CloudColor;\n    vec3 CloudOffset;\n    vec3 CellSize;\n};\n\
    uniform isamplerBuffer CloudFaces;\nlayout(location = 1) out vec4 vertexColor;\n\
    void main() {\n    int x = texelFetch(CloudFaces, gl_VertexIndex / 4 * 3).r;\n\
        gl_Position = vec4(vec3(x) * CellSize + CloudOffset, 1.0);\n    vertexColor = CloudColor;\n}\n";
const CLOUDS_FSH: &str = "#version 330\nlayout(location = 1) in vec4 vertexColor;\n\
    layout(location = 0) out vec4 fragColor;\nvoid main() {\n    fragColor = vertexColor;\n}\n";

fn source(files: &[(&str, &str)]) -> ResourceSource {
    ResourceSource::indexed(
        files
            .iter()
            .map(|(p, t)| ((*p).to_owned(), Resource::bytes(t.as_bytes().to_vec())))
            .collect(),
    )
}

/// The game's two stages, and a pack's fragment stage over them: the pack's clouds load with
/// `CloudInfo` and `CloudFaces` given; with only the game's stages there is no pack shader; a
/// pack stage reading another block is refused.
#[test]
fn a_pack_replacing_a_clouds_stage_is_loaded_with_the_faces_and_cloud_info() {
    let defaults = source(&[(VSH, CLOUDS_VSH), (FSH, CLOUDS_FSH)]);
    let pink = CLOUDS_FSH.replace(
        "vertexColor;\n}",
        "vertexColor * vec4(1.0, 0.6, 0.8, 1.0);\n}",
    );
    let root = source(&[(VSH, CLOUDS_VSH), (FSH, &pink)]);
    let programs = load_clouds(&root, &defaults).expect("the pack's clouds");
    let classic = programs.classic.as_ref().expect("CLOUDS");
    assert_eq!(classic.other, ["CloudInfo", "CloudFaces"]);
    for program in programs.oit.as_deref().expect("OIT_CLOUDS") {
        assert_eq!(program.other, ["CloudInfo", "CloudFaces"]);
    }
    assert!(
        load_clouds(&defaults, &defaults).is_none(),
        "the game's own"
    );
    let other = pink.replace(
        "void main()",
        "layout(std140) uniform Other {\n    vec4 Tint;\n};\nvoid main()",
    );
    let other = other.replace("vec4(1.0, 0.6, 0.8, 1.0)", "Tint");
    let root = source(&[(VSH, CLOUDS_VSH), (FSH, &other)]);
    assert!(
        load_clouds(&root, &defaults).is_none(),
        "Other is not given"
    );
}

/// The pack's fragment stage with `declarations` before its `main` and `body` in it.
fn fragment(declarations: &str, body: &str) -> String {
    format!(
        "#version 330\nlayout(location = 1) in vec4 vertexColor;\n{declarations}\n\
         void main() {{\n{body}\n}}\n"
    )
}

/// The packs' clouds as the atlas loads them, the game's stages under a pack's `fsh`.
fn clouds(fsh: &str) -> Option<Arc<crate::clouds::pack::Programs>> {
    let defaults = source(&[(VSH, CLOUDS_VSH), (FSH, CLOUDS_FSH)]);
    let root = source(&[(VSH, CLOUDS_VSH), (FSH, fsh)]);
    load_clouds(&root, &defaults)
}

/// The shader is translated once outside Improved Transparency and once for each of its stages
/// with that stage's defines (`OIT_SNIPPET`'s and the stage's): here the transmittance stage
/// alone writes its two coefficient targets (`OIT_COEFF_ATTACHMENT_COUNT` is 2), and each
/// stage's `#ifdef` picks its own colour.
#[test]
fn a_clouds_shader_is_translated_for_each_improved_transparency_stage() {
    let fsh = fragment(
        "#ifdef OIT_TRANSMITTANCE\nlayout(location = 0) out vec4 coeff[OIT_COEFF_ATTACHMENT_COUNT];\n\
         #else\nlayout(location = 0) out vec4 fragColor;\n#endif",
        "#if defined(OIT_TRANSMITTANCE) && defined(OIT_ALPHA_ONLY) && defined(OIT) \
         && OIT_COEFF_COUNT == 8 && OIT_WAVELET_RANK == 2 && defined(RENDERPEARL_DEPTH_IS_ZERO_TO_ONE)\n\
         coeff[0] = vertexColor;\n    coeff[1] = vertexColor;\n\
         #elif defined(OIT_DEPTH_BOUNDS) && defined(OIT_ALPHA_ONLY)\n    fragColor = vec4(1.0);\n\
         #elif defined(OIT_ACCUMULATE) && !defined(OIT_ALPHA_ONLY)\n    fragColor = vec4(2.0);\n\
         #elif !defined(OIT)\n    fragColor = vertexColor;\n#endif",
    );
    let programs = clouds(&fsh).expect("the pack's clouds");
    assert_eq!(programs.classic.as_ref().unwrap().fragment_writes(), [0]);
    let oit = programs.oit.as_deref().expect("OIT_CLOUDS");
    let writes: Vec<Vec<u32>> = oit.iter().map(|p| p.fragment_writes()).collect();
    assert_eq!(writes, [vec![0], vec![0, 1], vec![0]]);
}

/// Each stage is given only the samplers its pass binds (`OitRenderPassProvider`): the depth
/// bounds to the transmittance and accumulate stages, the coefficients to the accumulate
/// stage. A stage reading one its pass doesn't bind refuses the Improved Transparency
/// programs, leaving the native clouds there, while the classic one still loads.
#[test]
fn each_improved_transparency_stage_is_given_its_passs_samplers() {
    let samplers = "uniform sampler2D DepthBoundsSampler;\nuniform sampler2D Coeff0;\n\
        uniform sampler2D Coeff1;\nuniform sampler2D Coeff2;\nlayout(location = 0) out vec4 fragColor;";
    let read = |stage: &str, sampler: &str| {
        fragment(
            samplers,
            &format!(
                "fragColor = vertexColor;\n#ifdef {stage}\n\
                 fragColor += texelFetch({sampler}, ivec2(gl_FragCoord.xy), 0);\n#endif"
            ),
        )
    };
    let given = clouds(&fragment(
        samplers,
        "fragColor = vertexColor;\n#if defined(OIT_TRANSMITTANCE) || defined(OIT_ACCUMULATE)\n\
         fragColor += texelFetch(DepthBoundsSampler, ivec2(0), 0);\n#endif\n\
         #ifdef OIT_ACCUMULATE\n\
         fragColor += texelFetch(Coeff0, ivec2(0), 0) + texelFetch(Coeff1, ivec2(0), 0);\n#endif",
    ))
    .expect("the pack's clouds");
    let oit = given.oit.as_deref().expect("every sampler is its stage's");
    let at = |name: &str| {
        let index = oit[0].other.iter().position(|n| n == name).unwrap();
        crate::core_shader::other_binding(index)
    };
    let fragment_reads = |program: &CoreProgram, name: &str| {
        program.stages_reading(at(name)) == wgpu::ShaderStages::FRAGMENT
    };
    assert!(!fragment_reads(&oit[0], "DepthBoundsSampler"));
    assert!(fragment_reads(&oit[1], "DepthBoundsSampler"));
    assert!(!fragment_reads(&oit[1], "Coeff0"));
    assert!(fragment_reads(&oit[2], "DepthBoundsSampler"));
    assert!(fragment_reads(&oit[2], "Coeff0") && fragment_reads(&oit[2], "Coeff1"));
    for (stage, sampler) in [
        ("OIT_DEPTH_BOUNDS", "DepthBoundsSampler"),
        ("OIT_TRANSMITTANCE", "Coeff0"),
        ("OIT_ACCUMULATE", "Coeff2"),
    ] {
        let refused = clouds(&read(stage, sampler)).expect("the classic program");
        assert!(refused.classic.is_some(), "{stage} {sampler}");
        assert!(refused.oit.is_none(), "{stage} {sampler}");
    }
}

/// The classic program and the Improved Transparency ones load on their own: a shader whose
/// classic variant reads a block no pipeline gives still draws under Improved Transparency.
#[test]
fn a_refused_classic_clouds_program_keeps_the_improved_transparency_ones() {
    let fsh = fragment(
        "layout(std140) uniform Other {\n    vec4 Tint;\n};\nlayout(location = 0) out vec4 fragColor;",
        "fragColor = vertexColor;\n#ifndef OIT\n    fragColor *= Tint;\n#endif",
    );
    let programs = clouds(&fsh).expect("the Improved Transparency programs");
    assert!(programs.classic.is_none());
    assert!(programs.oit.is_some());
}
