//! Errors at the file and line the author wrote, through includes nested in includes and the
//! rewrites that add, drop and move lines.
use super::tests::{FSH_FILE, LIMITS, VSH_FILE, translate_core_at};
use super::*;

const GLUE: Glue = Glue {
    inputs: &[("Position", "vec3")],
    prelude: "layout(location = 0) in vec3 hz_position;\nint hz_unused() {\n    return 0;\n}",
    setup: "Position = hz_position;",
    finish: "",
    vertex_id: "int(gl_VertexIndex)",
    interface: LIMITS,
};

const VSH: &str = "#version 330\nin vec3 Position;\nout vec4 vertexColor;\n\
    void main() { vertexColor = vec4(1.0); gl_Position = vec4(Position, 1.0); }\n";

/// The files these tests include, by id.
const FILES: [(&str, &str); 5] = [
    // An inactive branch, then a quoted include from this file's folder.
    (
        "minecraft:shaders/include/outer.glsl",
        "#ifdef MISSING\nfloat dropped;\n#endif\n#include \"inner/deep.glsl\"\nfloat outer_kept;\n",
    ),
    // A comment over two lines, a continued line and a `#line` before the error on line 6.
    (
        "minecraft:shaders/include/inner/deep.glsl",
        "/* a comment\n   over lines */\nfloat spliced = \\\n    1.0;\n#line 100\nfloat broken = undefined_thing;\n",
    ),
    // A helper taking the sampler it reads, above the sampler's declaration: copied below it.
    (
        "minecraft:shaders/include/tap.glsl",
        "vec4 tap(sampler2D s, vec2 uv) {\n    vec4 c = texture(s, uv);\n    return c * missing_scale;\n}\n",
    ),
    // An include of a file nobody has, on line 2.
    (
        "minecraft:shaders/include/asks.glsl",
        "float asks_first;\n#include <minecraft:absent.glsl>\n",
    ),
    // Comments kept (an expansion without defines keeps every line as written).
    (
        "minecraft:shaders/include/plain.glsl",
        "float a;\n/* two\n   lines */ float b;\n",
    ),
];

fn read(id: &str) -> Result<Option<String>, String> {
    Ok(FILES
        .iter()
        .find(|(file, _)| *file == id)
        .map(|(_, text)| (*text).to_owned()))
}

fn translate(vertex: &str, fragment: &str) -> Result<Translated, Vec<Error>> {
    let stages = Stages {
        dir: "minecraft:shaders/core/",
        vertex,
        fragment,
        vertex_file: VSH_FILE,
        fragment_file: FSH_FILE,
    };
    translate_core_at(&stages, &[], &read, &GLUE, &|_| false)
}

/// The file and line of each error.
fn located(errors: &[Error]) -> Vec<(&str, u32)> {
    errors.iter().map(|e| (e.file.as_str(), e.line)).collect()
}

/// An error in a file an included file includes names that file and the line it is written
/// on, past the lines an inactive branch drops, a comment, a continued line and a `#line`.
#[test]
fn an_error_in_a_nested_include_names_that_file_and_line() {
    let fsh = "#version 330\n// the stage\n#include <minecraft:outer.glsl>\n\
        in vec4 vertexColor;\nout vec4 fragColor;\n\
        void main() { fragColor = vertexColor * broken; }\n";
    let errors = translate(VSH, fsh).unwrap_err();
    assert_eq!(
        located(&errors),
        [("minecraft:shaders/include/inner/deep.glsl", 6)]
    );
    assert!(
        errors[0].message.starts_with("fragment shader: "),
        "{errors:?}"
    );
}

/// The stage's own lines keep their numbers past the rewrites that add lines: the glue before
/// the vertex stage's code, a sampler split into three lines, two declarations on one line.
#[test]
fn an_error_in_the_stage_keeps_its_line_past_the_rewrites() {
    let vsh = "#version 330\nuniform sampler2D Sampler0;\nin vec3 Position;\n\
        out vec4 vertexColor;out vec2 uv;\nvoid main() {\n\
            vertexColor = texture(Sampler0, vec2(0.0));\n\
            uv = vec2(1.0); gl_Position = vec4(Position, nope);\n}\n";
    let fsh = "#version 330\nin vec4 vertexColor;\nin vec2 uv;\nout vec4 fragColor;\n\
        void main() { fragColor = vertexColor * uv.x; }\n";
    let errors = translate(vsh, fsh).unwrap_err();
    assert_eq!(located(&errors), [(VSH_FILE, 7)]);
}

/// A helper taking a sampler is copied per sampler passed, below the sampler's declaration:
/// an error in it names the line of the helper's body it is on.
#[test]
fn an_error_in_a_copied_helper_names_its_bodys_line() {
    let fsh = "#version 330\n#moj_import <minecraft:tap.glsl>\nuniform sampler2D Sampler0;\n\
        in vec4 vertexColor;\nout vec4 fragColor;\n\
        void main() { fragColor = tap(Sampler0, vec2(0.5)) * vertexColor; }\n";
    let errors = translate(VSH, fsh).unwrap_err();
    assert_eq!(
        located(&errors),
        [("minecraft:shaders/include/tap.glsl", 3)]
    );
}

/// An include refused (a file nobody has) is named at the directive asking for it, in the
/// included file that has it.
#[test]
fn a_refused_include_names_the_directive_in_the_file_that_asks() {
    let fsh = "#version 330\n#include <minecraft:asks.glsl>\nin vec4 vertexColor;\n\
        out vec4 fragColor;\nvoid main() { fragColor = vertexColor; }\n";
    let errors = translate(VSH, fsh).unwrap_err();
    assert_eq!(
        errors,
        [Error {
            file: "minecraft:shaders/include/asks.glsl".into(),
            line: 2,
            message: "minecraft:shaders/include/absent.glsl not found".into(),
        }]
    );
}

/// A location the fragment stage reads and the vertex stage doesn't write is named at the
/// input that reads it.
#[test]
fn a_location_no_stage_writes_names_the_input() {
    let fsh = "#version 330\nin vec4 vertexColor;\nlayout(location = 5) in vec4 extra;\n\
        out vec4 fragColor;\nvoid main() { fragColor = vertexColor * extra; }\n";
    let errors = translate(VSH, fsh).unwrap_err();
    assert_eq!(
        errors,
        [Error {
            file: FSH_FILE.into(),
            line: 3,
            message: "the fragment shader reads location 5, which the vertex shader doesn't write"
                .into(),
        }]
    );
}

/// Expanded without defines (a post shader's way), each line is where the author wrote it, and
/// text wrapped around and edited into it keeps that; Sunrise's own lines name the stage file.
#[test]
fn an_expansion_without_defines_locates_each_line() {
    let includes = Includes {
        read: &read,
        namespace: "minecraft",
        folder: "shaders/include/",
        once: &["moj_import"],
    };
    let mut expanded = expand_includes(
        "#version 330\n#include <minecraft:plain.glsl>\nfloat after;\n",
        "minecraft:shaders/post/blur.fsh",
        "minecraft:shaders/post/",
        &includes,
        None,
    )
    .unwrap();
    let at_a = expanded.text().find("float a;").unwrap();
    expanded.edit(vec![(at_a..at_a + 8, "float a;\nfloat added;".into())]);
    expanded.wrap("#version 450\n", "\nvoid main() {}\n");
    let at = |needle: &str| {
        let error = expanded.error_at(expanded.text().find(needle), String::new());
        (error.file, error.line)
    };
    let plain = "minecraft:shaders/include/plain.glsl";
    let stage = "minecraft:shaders/post/blur.fsh";
    assert_eq!(at("float a;"), (plain.into(), 1));
    assert_eq!(at("float added;"), (plain.into(), 1));
    assert_eq!(at("float b;"), (plain.into(), 3));
    assert_eq!(at("float after;"), (stage.into(), 3));
    assert_eq!(at("#version 450"), (stage.into(), 0));
    assert_eq!(at("void main()"), (stage.into(), 0));
}
