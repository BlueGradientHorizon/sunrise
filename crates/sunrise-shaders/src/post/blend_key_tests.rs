//! The `blend.*` keys of composite-style programs where the Iris observation stops, as
//! `docs/cleanroom/decisions.md` D-13 records them: `shadowcomp`'s per-buffer lines, `final`'s
//! keys, and values that are not four factors. The pipelines they give are pinned in
//! `sunrise-render-shaderpack`'s `pack_post::blend_target_tests`.

use super::*;
use Factor::*;

const VSH: &str = "#version 120\nvarying vec2 uv;\nvoid main() {\n\
                   gl_Position = ftransform();\nuv = gl_MultiTexCoord0.xy;\n}\n";
/// Writes the first two buffers of its pass: colortex0/1, or shadowcolor0/1 in `shadowcomp`.
const TWO: &str = "#version 120\nvoid main() {\n\
                   gl_FragData[0] = vec4(0.75);\ngl_FragData[1] = vec4(0.875);\n}\n";
const ONE: &str = "#version 120\nvoid main() {\ngl_FragData[0] = vec4(0.5);\n}\n";

fn load(files: &[(&str, &str)], properties: &str) -> Post {
    let pack = Pack::from_zip_bytes("t", crate::pack::tests::zip(files)).unwrap();
    let properties = Properties::parse(properties, &Macros::default());
    load_post(
        &pack,
        "",
        &properties,
        &Macros::standard(),
        &GameInputs::default(),
    )
}

/// D-13: `shadowcomp` doesn't read `blend.<program>`, but keeps its four-factor
/// `blend.<program>.<buffer>` lines, their suffix read as a colortex name: `colortex0` and the
/// alias `gdepth` give indices 0 and 1 (shadowcolor0 and shadowcolor1 of the pass), while
/// `shadowcolor1` names no buffer and an invalid value is dropped.
#[test]
fn shadowcomp_keeps_per_buffer_lines_named_as_colortex() {
    let post = load(
        &[
            ("shaders/composite.vsh", VSH),
            ("shaders/composite.fsh", ONE),
            ("shaders/shadowcomp.vsh", VSH),
            ("shaders/shadowcomp.fsh", TWO),
            ("shaders/shadowcomp1.vsh", VSH),
            ("shaders/shadowcomp1.fsh", TWO),
        ],
        "shadow.enabled = true\n\
         blend.shadowcomp = ONE ONE ONE ONE\n\
         blend.shadowcomp.colortex0 = SRC_ALPHA ONE_MINUS_SRC_ALPHA ONE ZERO\n\
         blend.shadowcomp.shadowcolor1 = ONE ZERO ONE ZERO\n\
         blend.shadowcomp.colortex1 = ONE ONE\n\
         blend.shadowcomp1.gdepth = DST_COLOR ZERO ZERO ONE\n",
    );
    assert!(post.shadow_pass, "{:?}", post.notes);
    let comp: Vec<(&str, Option<Blend>, &[(u32, Option<Blend>)])> = post
        .shadow_comp
        .iter()
        .map(|p| (p.name.as_str(), p.blend, &p.buffer_blends[..]))
        .collect();
    assert_eq!(
        comp,
        [
            (
                "shadowcomp",
                None,
                &[(0, Some([SrcAlpha, OneMinusSrcAlpha, One, Zero]))][..]
            ),
            (
                "shadowcomp1",
                None,
                &[(1, Some([DstColor, Zero, Zero, One]))][..]
            ),
        ],
        "{:?}",
        post.notes
    );
}

/// D-13: the loader reads `blend.final` and drops `final`'s per-buffer lines (that `final`
/// draws without blending is `pack_post::blend_target_tests`').
#[test]
fn final_reads_its_program_blend_and_drops_buffer_lines() {
    let post = load(
        &[("shaders/final.vsh", VSH), ("shaders/final.fsh", ONE)],
        "blend.final = ONE ONE ZERO ONE\nblend.final.colortex0 = SRC_ALPHA ONE ONE ZERO\n",
    );
    let fin: Vec<(&str, Group, Option<Blend>, usize)> = post
        .programs
        .iter()
        .map(|p| (p.name.as_str(), p.group, p.blend, p.buffer_blends.len()))
        .collect();
    assert_eq!(
        fin,
        [("final", Group::Final, Some([One, One, Zero, One]), 0)],
        "{:?}",
        post.notes
    );
}

/// D-13: a `blend.<program>` value that is not four valid factor names (three, five, lower-case,
/// an unknown name, four known ones beside an unknown, nothing) counts as no key, not as `off` and not as a partial blend; in a
/// composite program that leaves blending off. Four valid names still blend.
#[test]
fn a_value_that_is_not_four_factors_counts_as_no_key() {
    let values = [
        ("composite", "ONE ONE ONE"),
        ("composite1", "ONE ONE ONE ONE ONE"),
        ("composite2", "one one one one"),
        ("composite3", "ONE ONE ONE FOO"),
        ("composite7", "SRC_ALPHA FOO ONE ZERO ONE"),
        ("composite4", ""),
        ("composite5", "off"),
        (
            "composite6",
            "ONE_MINUS_DST_ALPHA DST_ALPHA SRC_COLOR ONE_MINUS_SRC_COLOR",
        ),
    ];
    let text: String = values
        .iter()
        .map(|(name, value)| format!("blend.{name} = {value}\n"))
        .collect();
    let properties = Properties::parse(&text, &Macros::default());
    let keys: Vec<Option<Option<Blend>>> = values
        .iter()
        .map(|(name, _)| blend(&properties, name))
        .collect();
    let four = [OneMinusDstAlpha, DstAlpha, SrcColor, OneMinusSrcColor];
    assert_eq!(
        keys,
        [
            None,
            None,
            None,
            None,
            None,
            None,
            Some(None),
            Some(Some(four))
        ]
    );
    let files: Vec<(String, &str)> = values
        .iter()
        .flat_map(|(name, _)| {
            [
                (format!("shaders/{name}.vsh"), VSH),
                (format!("shaders/{name}.fsh"), ONE),
            ]
        })
        .collect();
    let files: Vec<(&str, &str)> = files.iter().map(|(n, t)| (n.as_str(), *t)).collect();
    let post = load(&files, &text);
    let blends: Vec<(&str, Option<Blend>)> = post
        .programs
        .iter()
        .map(|p| (p.name.as_str(), p.blend))
        .collect();
    assert_eq!(
        blends,
        [
            ("composite", None),
            ("composite1", None),
            ("composite2", None),
            ("composite3", None),
            ("composite4", None),
            ("composite5", None),
            ("composite6", Some(four)),
            ("composite7", None),
        ],
        "{:?}",
        post.notes
    );
}
