//! Switch options (spec 8.1), as far as program-enable expressions (3.3) need them: which are on
//! by default. A switch is `#define NAME` (on) or `//#define NAME` (off) with nothing after the
//! name but a comment, in any stage or included file of the pack. Packs keep their options in
//! included `.glsl` files (or files of any name a source includes) and test them elsewhere
//! (OQ-13), so the test the spec asks for (`#ifdef NAME` / `#ifndef NAME`) may be in any file
//! of the pack. An include guard (`#ifndef NAME` then `#define NAME`, 1.4) is not a switch: one
//! scan decides what declares an option, for the options list and the switches alike.

use std::collections::{BTreeMap, BTreeSet};

use crate::Pack;

const SOURCES: [&str; 8] = ["vsh", "fsh", "gsh", "csh", "tcs", "tes", "glsl", "inc"];

/// The switch options that are on by default: the switches the scan [`options`] reads finds,
/// on in every file that declares them and tested with `#ifdef NAME` or `#ifndef NAME`
/// somewhere (8.1). An include guard is not one, as it is not an option. A declaration whose
/// comment touches the name (`#define NAME//comment`) does not count here, as before the scan
/// was shared.
pub fn switches_on(pack: &Pack) -> BTreeSet<String> {
    let scan = scan(pack);
    let mut on = scan.switch_on;
    // Defined both ways is ambiguous (8.1): it keeps its default only where it is on everywhere.
    on.retain(|name| scan.switch_tested.contains(name) && !scan.switch_off.contains(name));
    on
}

/// The name a line tests with `#ifdef NAME` or `#ifndef NAME` (any space after `#`), as
/// [`switches_on`] counts a test.
fn switch_test(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix('#')?.trim_start();
    let (directive, rest) = rest.split_once(|c: char| c.is_whitespace())?;
    matches!(directive, "ifdef" | "ifndef")
        .then(|| rest.split_whitespace().next())
        .flatten()
}

/// The value options and their defaults: `#define NAME VALUE // [A B C]`, a value and the list
/// of the values it may take (8.1).
pub fn value_options(pack: &Pack) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for text in option_texts(pack) {
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix('#') else {
                continue;
            };
            let Some(rest) = rest.trim_start().strip_prefix("define") else {
                continue;
            };
            let Some((code, comment)) = rest.split_once("//") else {
                continue;
            };
            let mut words = code.split_whitespace();
            let (Some(name), Some(value), None) = (words.next(), words.next(), words.next()) else {
                continue;
            };
            if comment.contains('[') && comment.contains(']') {
                out.entry(name.to_owned())
                    .or_insert_with(|| value.to_owned());
            }
        }
    }
    out
}

/// `macros` with the pack's options at their defaults: the switches that are on, and the
/// value options. `shaders.properties` and the program switches test them (3.3, 8.1).
pub fn with_options(pack: &Pack, macros: &crate::Macros) -> crate::Macros {
    let mut out = macros.clone();
    for name in switches_on(pack) {
        out.define(name, "");
    }
    for (name, value) in value_options(pack) {
        out.define(name, value);
    }
    out
}

/// The macros `shaders.properties` is read with: the pack's options over `macros`. No
/// `IRIS_FEATURE_<name>` is among them (spec OQ-2), so what a pack puts behind one is not read.
pub fn properties_macros(pack: &Pack, macros: &crate::Macros) -> crate::Macros {
    with_options(pack, macros)
}

/// A pack's `shaders.properties` as the game reads it before the programs: parsed with
/// `properties_macros` over `macros` (the standard set and the renderer's). Returns the
/// properties and a note for each required feature Sunrise lacks with `macros`' device
/// (`Macros::feature_notes`).
/// Whatever reads a pack's programs (the game, the `real_packs` test) reads them with `macros`.
pub fn read_properties(
    pack: &Pack,
    macros: &mut crate::Macros,
) -> (crate::Properties, Vec<String>) {
    let text = pack.read_text("shaders.properties").unwrap_or_default();
    let properties = crate::Properties::parse(&text, &properties_macros(pack, macros));
    let notes = macros.feature_notes(&properties);
    (properties, notes)
}

/// An option the player can change (8.1): a switch (`values` are `false` and `true`), a value
/// option or a `const` with the list of values it may take.
#[derive(Clone, Debug, PartialEq)]
pub struct PackOption {
    pub name: String,
    pub default: String,
    pub values: Vec<String>,
    pub switch: bool,
    /// A switch some file tests; value options always count as tested.
    pub tested: bool,
}

/// A line that declares an option, as [`declaration`] reads it.
struct Declaration<'a> {
    name: &'a str,
    /// The value and its byte offset in the line; `None` for a switch.
    value: Option<(&'a str, usize)>,
    /// Behind `//`: a switch that is off.
    commented: bool,
    /// The values in the comment's `[...]`.
    values: Option<Vec<&'a str>>,
    /// `const bool`: a switch written as a value.
    boolean: bool,
    /// `#define NAME//comment`: the comment starts right after the last word, with no space.
    glued: bool,
}

/// `#define NAME`, `//#define NAME`, `#define NAME VALUE // [A B]`,
/// `const TYPE NAME = VALUE; // [A B]` or `const bool NAME = true;`.
fn declaration(line: &str) -> Option<Declaration<'_>> {
    let body = line.trim_start();
    let (commented, code) = match body.strip_prefix("//") {
        Some(rest) => (true, rest.trim_start()),
        None => (false, body),
    };
    let offset = |part: &str| part.as_ptr() as usize - line.as_ptr() as usize;
    if let Some(rest) = code.strip_prefix('#') {
        let rest = rest.trim_start().strip_prefix("define")?;
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        let (code, comment, glued) = match rest.split_once("//") {
            Some((code, comment)) => (code, comment, !code.ends_with(char::is_whitespace)),
            None => (rest, "", false),
        };
        let mut words = code.split_whitespace();
        let name = words.next()?;
        let value = words.next();
        if words.next().is_some() {
            return None;
        }
        return Some(Declaration {
            name,
            value: value.map(|v| (v, offset(v))),
            commented,
            values: list_with_default(comment, value),
            boolean: false,
            glued,
        });
    }
    if commented {
        return None;
    }
    // `const float shadowDistance = 128.0; // [64.0 128.0]`, `const bool x = true;`
    let rest = code.strip_prefix("const")?;
    let (decl, comment) = rest.split_once("//").unwrap_or((rest, ""));
    let (left, right) = decl.split_once('=')?;
    let mut words = left.split_whitespace();
    let (Some(ty), Some(name), None) = (words.next(), words.next(), words.next()) else {
        return None;
    };
    let value = right.trim().strip_suffix(';')?.trim_end();
    if value.is_empty() || value.contains(char::is_whitespace) {
        return None;
    }
    let boolean = ty == "bool" && matches!(value, "true" | "false");
    Some(Declaration {
        name,
        value: Some((value, offset(value))),
        commented: false,
        values: if boolean {
            Some(vec!["false", "true"])
        } else {
            Some(list_with_default(comment, Some(value))?)
        },
        boolean,
        glued: false,
    })
}

/// Keep the declared default selectable without creating an option from an empty range.
fn list_with_default<'a>(comment: &'a str, default: Option<&'a str>) -> Option<Vec<&'a str>> {
    let mut values = list(comment)?;
    if !values.is_empty()
        && let Some(default) = default
        && !values.contains(&default)
    {
        values.insert(0, default);
    }
    Some(values)
}

/// The values in a comment's `[...]`.
fn list(comment: &str) -> Option<Vec<&str>> {
    let (_, rest) = comment.split_once('[')?;
    let (inner, _) = rest.split_once(']')?;
    Some(inner.split_whitespace().collect())
}

/// What one pass over the pack's option texts finds: the one place that decides what declares
/// an option, so [`options`] and [`switches_on`] agree on it.
struct Scan {
    /// The value and `const` options, by name.
    values: BTreeMap<String, PackOption>,
    /// The switches declared on (`#define NAME`) and off (`//#define NAME`); include guards are
    /// in neither.
    on: BTreeSet<String>,
    off: BTreeSet<String>,
    /// `on` and `off` without the declarations whose comment touches the name
    /// (`#define NAME//comment`), as [`switches_on`] counts them.
    switch_on: BTreeSet<String>,
    switch_off: BTreeSet<String>,
    /// The names an `#ifdef`, `#ifndef`, `#if` or `#elif` line tests, as [`options`] counts it.
    tested: BTreeSet<String>,
    /// The names an `#ifdef` or `#ifndef` line tests, as [`switches_on`] counts it.
    switch_tested: BTreeSet<String>,
}

fn scan(pack: &Pack) -> Scan {
    let mut scan = Scan {
        values: BTreeMap::new(),
        on: BTreeSet::new(),
        off: BTreeSet::new(),
        switch_on: BTreeSet::new(),
        switch_off: BTreeSet::new(),
        tested: BTreeSet::new(),
        switch_tested: BTreeSet::new(),
    };
    for text in option_texts(pack) {
        // `#ifndef NAME` just before `#define NAME`, with only comment lines between: an
        // include guard, not an option.
        let mut guard: Option<&str> = None;
        let mut in_comment = false;
        for line in text.lines() {
            let trimmed = line.trim();
            let comment = comment_only(trimmed, &mut in_comment);
            let opened = guard_name(trimmed);
            for directive in ["#ifdef", "#ifndef"] {
                if let Some(name) = trimmed.strip_prefix(directive).map(str::trim) {
                    let name = name.split_whitespace().next().unwrap_or("");
                    scan.tested.insert(name.to_owned());
                }
            }
            if trimmed.starts_with("#if") || trimmed.starts_with("#elif") {
                scan.tested.extend(defined_names(trimmed));
            }
            if let Some(name) = switch_test(trimmed) {
                scan.switch_tested.insert(name.to_owned());
            }
            let d = declaration(line);
            if !comment {
                let pending = std::mem::replace(&mut guard, opened);
                if let Some(d) = &d
                    && pending == Some(d.name)
                    && d.value.is_none()
                {
                    continue;
                }
            }
            let Some(d) = d else { continue };
            match (d.value, d.values) {
                (None, _) => {
                    let (all, counted) = if d.commented {
                        (&mut scan.off, &mut scan.switch_off)
                    } else {
                        (&mut scan.on, &mut scan.switch_on)
                    };
                    all.insert(d.name.to_owned());
                    if !d.glued {
                        counted.insert(d.name.to_owned());
                    }
                }
                (Some((value, _)), Some(values)) if !d.commented && !values.is_empty() => {
                    // A `const bool` is often the pack's own setting rather than the player's:
                    // it shows where a screen names it, not with `*`.
                    scan.values
                        .entry(d.name.to_owned())
                        .or_insert_with(|| PackOption {
                            name: d.name.to_owned(),
                            default: value.to_owned(),
                            values: values.iter().map(|v| (*v).to_owned()).collect(),
                            switch: d.boolean,
                            tested: !d.boolean,
                        });
                }
                _ => {}
            }
        }
    }
    scan
}

/// Every option of the pack, by name, at its default.
pub fn options(pack: &Pack) -> BTreeMap<String, PackOption> {
    let Scan {
        values: mut out,
        on,
        off,
        tested,
        ..
    } = scan(pack);
    for name in on.iter().chain(&off) {
        if out.contains_key(name) {
            continue;
        }
        // Defined both ways is ambiguous (8.1): on only where it is on everywhere.
        let default = on.contains(name) && !off.contains(name);
        out.insert(
            name.clone(),
            PackOption {
                name: name.clone(),
                default: default.to_string(),
                values: vec!["false".into(), "true".into()],
                switch: true,
                tested: tested.contains(name),
            },
        );
    }
    out
}

/// The name an `#ifndef NAME`, `#if !defined NAME` or `#if !defined(NAME)` line tests, when
/// that test is the whole condition (any space after `#`, a comment after it). Any other
/// condition (`#if !defined(A) && defined(B)`) opens no include guard.
fn guard_name(line: &str) -> Option<&str> {
    let line = line.strip_prefix('#')?.trim_start();
    let (rest, paren) = match line.strip_prefix("ifndef") {
        Some(rest) => (rest.strip_prefix(char::is_whitespace)?, false),
        None => {
            let rest = line
                .strip_prefix("if")?
                .trim_start()
                .strip_prefix('!')?
                .trim_start()
                .strip_prefix("defined")?;
            match rest.trim_start().strip_prefix('(') {
                Some(rest) => (rest, true),
                None => (rest.strip_prefix(char::is_whitespace)?, false),
            }
        }
    };
    let rest = rest.trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let (name, after) = rest.split_at(end);
    let after = after.trim_start();
    let after = if paren {
        after.strip_prefix(')')?
    } else {
        after
    };
    (!name.is_empty() && comment_only(after, &mut false)).then_some(name)
}

/// Whether `line` has no code outside comments. `in_block` carries a `/*` left open from the
/// line before, and is left set when this line opens one it does not close.
fn comment_only(line: &str, in_block: &mut bool) -> bool {
    let mut rest = line;
    let mut code = false;
    loop {
        if *in_block {
            let Some(end) = rest.find("*/") else {
                return !code;
            };
            *in_block = false;
            rest = &rest[end + 2..];
        }
        match (rest.find("//"), rest.find("/*")) {
            (Some(at), block) if block.is_none_or(|b| at < b) => {
                return !code && rest[..at].trim().is_empty();
            }
            (_, Some(at)) => {
                code |= !rest[..at].trim().is_empty();
                *in_block = true;
                rest = &rest[at + 2..];
            }
            _ => return !code && rest.trim().is_empty(),
        }
    }
}

/// The names `defined NAME` and `defined(NAME)` test in an `#if` line.
fn defined_names(line: &str) -> Vec<String> {
    line.split("defined")
        .skip(1)
        .filter_map(|rest| {
            let rest = rest.trim_start().trim_start_matches('(').trim_start();
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            (end > 0).then(|| rest[..end].to_owned())
        })
        .collect()
}

/// `text` with the options in `chosen` at their chosen values: switches commented out or in,
/// values replaced. Lines keep their place.
pub fn apply(text: &str, chosen: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        let end = &line[content.len()..];
        let replaced = declaration(content).and_then(|d| {
            let want = chosen.get(d.name)?;
            match (d.value, d.values) {
                (None, _) => {
                    let on = want == "true";
                    if on != d.commented {
                        return None;
                    }
                    let body = content.trim_start();
                    let indent = &content[..content.len() - body.len()];
                    Some(if on {
                        format!("{indent}{}", body.trim_start_matches('/').trim_start())
                    } else {
                        format!("{indent}//{body}")
                    })
                }
                (Some((value, at)), Some(_)) if !d.commented => Some(format!(
                    "{}{want}{}",
                    &content[..at],
                    &content[at + value.len()..]
                )),
                _ => None,
            }
        });
        out.push_str(replaced.as_deref().unwrap_or(content));
        out.push_str(end);
    }
    out
}

/// Sets option `name` to `value` in `chosen` as the options screen does: a value the option doesn't
/// list, or an option the pack doesn't have, is ignored (`false`); the option's default takes it
/// out of `chosen`.
pub fn choose(
    chosen: &mut BTreeMap<String, String>,
    options: &BTreeMap<String, PackOption>,
    name: &str,
    value: String,
) -> bool {
    let Some(option) = options.get(name).filter(|o| o.values.contains(&value)) else {
        return false;
    };
    if value == option.default {
        chosen.remove(name);
    } else {
        chosen.insert(name.to_owned(), value);
    }
    true
}

/// The file the player's choices for pack `name` are kept in, beside it in `shaderpacks/`.
pub fn chosen_file(name: &str) -> String {
    format!("{name}.txt")
}

/// `NAME=value` lines; blank lines and `#` comments are skipped.
pub fn parse_chosen(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect()
}

pub fn chosen_text(chosen: &BTreeMap<String, String>) -> String {
    chosen.iter().map(|(k, v)| format!("{k}={v}\n")).collect()
}

/// The texts of the pack's sources and of the files they include.
fn option_texts(pack: &Pack) -> Vec<String> {
    let files = pack.all_files();
    let source = |path: &str| SOURCES.contains(&path.rsplit_once('.').map_or("", |(_, e)| e));
    let texts: Vec<(&String, String)> = files
        .iter()
        .filter(|p| source(p))
        .filter_map(|p| Some((p, pack.read_text(p)?)))
        .collect();
    // Files of other names count when a source includes them (`#include "shaders.settings"`).
    let included: BTreeSet<&str> = texts
        .iter()
        .flat_map(|(_, t)| t.lines())
        .filter_map(|l| l.trim().strip_prefix("#include"))
        .map(|r| r.trim().trim_matches(|c| c == '"' || c == '<' || c == '>'))
        .map(|r| r.rsplit('/').next().unwrap_or(r))
        .collect();
    let extra: Vec<(&String, String)> = files
        .iter()
        .filter(|p| !source(p) && included.contains(p.rsplit('/').next().unwrap_or(p)))
        .filter_map(|p| Some((p, pack.read_text(p)?)))
        .collect();
    texts.into_iter().chain(extra).map(|(_, t)| t).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `shaders.properties` read as the game reads it: its conditionals see the pack's options
    /// and the macros given (the renderer's vendor here), and the features it turns on are then
    /// defined for the programs, with a note for a required one Sunrise lacks.
    #[test]
    fn properties_are_read_with_the_options_then_define_the_features() {
        let bytes = crate::pack::tests::zip(&[
            (
                "shaders/options.glsl",
                "#define LEVEL 2 // [1 2]
",
            ),
            (
                "shaders/shaders.properties",
                "#if LEVEL == 2
screen = x
#endif
                 #ifdef MC_GL_VENDOR_OTHER
vendor = other
#endif
                 iris.features.optional = SSBO
iris.features.required = NOT_A_FEATURE
",
            ),
        ]);
        let pack = Pack::from_zip_bytes("p.zip", bytes).unwrap();
        let mut macros = crate::Macros::standard();
        macros.define_renderer(4);
        let (properties, notes) = read_properties(&pack, &mut macros);
        assert_eq!(properties.get("screen"), Some("x"));
        assert_eq!(properties.get("vendor"), Some("other"));
        assert!(!macros.is_defined("IRIS_FEATURE_SSBO"));
        assert_eq!(
            notes,
            ["the pack requires `NOT_A_FEATURE`, which is not supported yet"]
        );
    }

    /// Neither `shaders.properties` nor the programs see an `IRIS_FEATURE_<name>` (spec OQ-2):
    /// what a pack puts behind one is skipped, for an optional, a required, an unlisted and an
    /// unknown feature alike, and a required feature Sunrise has gets no note.
    #[test]
    fn properties_and_programs_see_no_feature_macro() {
        let bytes = crate::pack::tests::zip(&[(
            "shaders/shaders.properties",
            "iris.features.required = COMPUTE_SHADERS\n\
             #ifdef IRIS_FEATURE_CUSTOM_IMAGES\nuniform.float.optional = 1.0\n#endif\n\
             #ifdef IRIS_FEATURE_COMPUTE_SHADERS\nuniform.float.required = 1.0\n#endif\n\
             #ifdef IRIS_FEATURE_SSBO\nuniform.float.unlisted = 1.0\n#endif\n\
             #ifdef IRIS_FEATURE_NOT_A_FEATURE\nuniform.float.unknown = 1.0\n#endif\n\
             iris.features.optional = CUSTOM_IMAGES\n",
        )]);
        let pack = Pack::from_zip_bytes("p.zip", bytes).unwrap();
        let mut macros = crate::Macros::standard();
        let (properties, notes) = read_properties(&pack, &mut macros);
        let read = |name: &str| properties.get(&format!("uniform.float.{name}")).is_some();
        assert!(!read("optional") && !read("required") && !read("unlisted") && !read("unknown"));
        assert!(notes.is_empty());
        assert!(!macros.is_defined("IRIS_FEATURE_CUSTOM_IMAGES"));
    }

    #[test]
    fn declared_defaults_remain_selectable_and_apply() {
        for (source, selected) in [
            (
                "#define QUALITY 0 // [1 2]\n",
                "#define QUALITY 2 // [1 2]\n",
            ),
            (
                "const int QUALITY = 0; // [1 2]\n",
                "const int QUALITY = 2; // [1 2]\n",
            ),
        ] {
            let bytes = crate::pack::tests::zip(&[("shaders/options.glsl", source)]);
            let pack = Pack::from_zip_bytes("options.zip", bytes).unwrap();
            let available = options(&pack);
            let option = &available["QUALITY"];
            assert_eq!(option.default, "0");
            assert_eq!(option.values, ["0", "1", "2"]);
            let chosen = pack.with_chosen(BTreeMap::from([("QUALITY".into(), "2".into())]));
            assert_eq!(chosen.read_text("options.glsl").unwrap(), selected);
            let reset = chosen.with_chosen(BTreeMap::from([("QUALITY".into(), "0".into())]));
            assert_eq!(reset.read_text("options.glsl").unwrap(), source);
        }
    }

    #[test]
    fn declared_defaults_keep_authored_order_and_empty_ranges_stay_hidden() {
        let bytes = crate::pack::tests::zip(&[(
            "shaders/options.glsl",
            "#define QUALITY 1 // [0 1 2]\n#define IGNORED 0 // []\n",
        )]);
        let pack = Pack::from_zip_bytes("options.zip", bytes).unwrap();
        let available = options(&pack);
        assert_eq!(available["QUALITY"].default, "1");
        assert_eq!(available["QUALITY"].values, ["0", "1", "2"]);
        assert!(!available.contains_key("IGNORED"));
    }
    use crate::pack::tests::zip;

    #[test]
    fn switches_need_a_test_and_no_value() {
        let pack = Pack::from_zip_bytes(
            "P.zip",
            zip(&[
                (
                    "shaders/lib/settings.glsl",
                    "  #define AO // ambient occlusion\n//#define BLOOM\n#define LEVEL 2 //[1 2]\n\
                     #define SPLIT\n#define UNTESTED\n",
                ),
                (
                    "shaders/world0/deferred.fsh",
                    "#ifdef AO\n#endif\n#ifndef BLOOM\n#endif\n#ifdef SPLIT\n#endif\n#ifdef LEVEL\n#endif\n",
                ),
                ("shaders/world-1/deferred.fsh", "//#define SPLIT\n"),
                (
                    "shaders/final.fsh",
                    "#include \"shaders.settings\"\n#ifdef GODRAYS\n#endif\n",
                ),
                ("shaders/shaders.settings", "    #define GODRAYS //Toggle\n"),
                ("shaders/notes.settings", "#define NOT_INCLUDED\n#ifdef NOT_INCLUDED\n"),
                ("shaders/shaders.properties", "#define NOT_SOURCE\n#ifdef NOT_SOURCE\n"),
            ]),
        )
        .unwrap();
        assert_eq!(
            switches_on(&pack),
            BTreeSet::from(["AO".to_owned(), "GODRAYS".to_owned()])
        );
    }

    #[test]
    fn options_read_switches_values_and_consts() {
        let pack = Pack::from_zip_bytes(
            "P.zip",
            zip(&[
                (
                    "shaders/lib/settings.glsl",
                    "#define AO\n//#define BLOOM // glow\n#define LEVEL 2 //[1 2 3]\n\
                     #define FIXED 4\nconst int shadowMapResolution = 2048; // [1024 2048]\n\
                     //#define OFF_VALUE 1 // [1 2]\n",
                ),
                (
                    "shaders/final.fsh",
                    "#ifdef AO\n#endif\n#if defined(BLOOM) && LEVEL > 1\n#endif\n",
                ),
                (
                    "shaders/lib/common.glsl",
                    "#ifndef COMMON_GLSL\n#define COMMON_GLSL\n#endif\n\
                     #if !defined(INCLUDE_SKY)\n\n#define INCLUDE_SKY\n#endif\n",
                ),
            ]),
        )
        .unwrap();
        let options = options(&pack);
        let names: Vec<&str> = options.keys().map(String::as_str).collect();
        assert_eq!(names, ["AO", "BLOOM", "LEVEL", "shadowMapResolution"]);
        assert_eq!(options["AO"].default, "true");
        assert!(options["BLOOM"].switch && options["BLOOM"].tested);
        assert_eq!(options["BLOOM"].default, "false");
        assert_eq!(options["LEVEL"].values, ["1", "2", "3"]);
        assert_eq!(options["shadowMapResolution"].default, "2048");
    }

    #[test]
    fn const_bools_are_switches_screens_name() {
        let pack = Pack::from_zip_bytes(
            "P.zip",
            zip(&[(
                "shaders/lib/settings.glsl",
                "const bool shadowHardwareFiltering = true;\nconst bool generateShadowMipmap = false; // mips\n\
                 const bool notABool = 1;\n",
            )]),
        )
        .unwrap();
        let options = options(&pack);
        let filtering = &options["shadowHardwareFiltering"];
        assert!(filtering.switch && !filtering.tested);
        assert_eq!(filtering.default, "true");
        assert_eq!(options["generateShadowMipmap"].default, "false");
        assert!(!options.contains_key("notABool"));
        let chosen = parse_chosen("shadowHardwareFiltering=false\n");
        assert_eq!(
            apply("const bool shadowHardwareFiltering = true;\n", &chosen),
            "const bool shadowHardwareFiltering = false;\n"
        );
    }

    #[test]
    fn chosen_values_rewrite_their_lines() {
        let chosen = parse_chosen("AO=false\nBLOOM=true\nLEVEL=3\nshadowMapResolution=1024\n");
        let text = "  #define AO // occlusion\r\n//#define BLOOM\n#define LEVEL 2 //[1 2 3]\n\
                    #define LEVEL 9\nconst int shadowMapResolution = 2048; // [1024 2048]\n";
        assert_eq!(
            apply(text, &chosen),
            "  //#define AO // occlusion\r\n#define BLOOM\n#define LEVEL 3 //[1 2 3]\n\
             #define LEVEL 9\nconst int shadowMapResolution = 1024; // [1024 2048]\n"
        );
        let pack = Pack::from_zip_bytes(
            "P.zip",
            zip(&[
                ("shaders/lib/settings.glsl", "#define LEVEL 2 //[1 2 3]\n"),
                ("shaders/shaders.properties", "#define LEVEL 2 //[1 2 3]\n"),
            ]),
        )
        .unwrap()
        .with_chosen(chosen.clone());
        assert_eq!(value_options(&pack)["LEVEL"], "3");
        assert_eq!(
            pack.read_text("shaders.properties").unwrap(),
            "#define LEVEL 2 //[1 2 3]\n"
        );
        assert_eq!(parse_chosen(&chosen_text(&chosen)), chosen);
    }

    /// An include guard (`#ifndef NAME`, `#if !defined NAME` or `# ifndef NAME`, then
    /// `#define NAME` with nothing between but blank and comment lines) is not an option (8.1,
    /// 1.4): the options list leaves it out, and `shaders.properties` and the
    /// `program.<name>.enabled` expressions read it as undefined, through the real reader and
    /// loader. A switch declared and tested the same way elsewhere still counts.
    #[test]
    fn include_guards_are_not_options_anywhere() {
        const VSH: &str = "#version 120\nvoid main() { gl_Position = ftransform(); }";
        const FSH: &str = "#version 120\n#include \"/lib/a.glsl\"\n#include \"/lib/b.glsl\"\n\
                           #include \"/lib/c.glsl\"\n#include \"/lib/settings.glsl\"\n\
                           /* DRAWBUFFERS:0 */\nvoid main() { gl_FragData[0] = vec4(1.0); }";
        let mut files = vec![
            (
                "shaders/lib/a.glsl",
                "#ifndef A_GLSL\n#define A_GLSL\n#endif\n",
            ),
            (
                "shaders/lib/b.glsl",
                "#if !defined(B_GLSL)\n\n// guard\n#define B_GLSL\n#endif\n#ifdef B_GLSL\n#endif\n",
            ),
            (
                "shaders/lib/c.glsl",
                "# ifndef C_GLSL\n# define C_GLSL\n# endif\n",
            ),
            (
                "shaders/lib/settings.glsl",
                "#define AO\n#ifdef AO\n#endif\n#ifndef DEFAULTED\nconst int x = 1;\n\
                 #define DEFAULTED\n#endif\n",
            ),
            (
                "shaders/shaders.properties",
                "program.composite.enabled = A_GLSL\nprogram.composite1.enabled = B_GLSL\n\
                 program.composite2.enabled = C_GLSL\nprogram.composite3.enabled = AO\n\
                 program.composite4.enabled = DEFAULTED\n\
                 #if defined A_GLSL || defined B_GLSL || defined C_GLSL\nscreen = guards\n#endif\n\
                 #ifdef AO\nsliders = ao\n#endif\n",
            ),
            ("shaders/final.vsh", VSH),
            ("shaders/final.fsh", FSH),
        ];
        let names = [
            "composite",
            "composite1",
            "composite2",
            "composite3",
            "composite4",
        ];
        let paths: Vec<_> = names
            .iter()
            .flat_map(|n| [format!("shaders/{n}.vsh"), format!("shaders/{n}.fsh")])
            .collect();
        for path in &paths {
            files.push((path.as_str(), if path.ends_with("vsh") { VSH } else { FSH }));
        }
        let pack = Pack::from_zip_bytes("guards.zip", zip(&files)).unwrap();
        let listed: Vec<String> = options(&pack).into_keys().collect();
        assert_eq!(listed, ["AO", "DEFAULTED"]);
        let mut macros = crate::Macros::standard();
        let (properties, _) = read_properties(&pack, &mut macros);
        assert_eq!(properties.get("screen"), None);
        assert_eq!(properties.get("sliders"), Some("ao"));
        let post = crate::load_post(&pack, "", &properties, &macros, &Default::default());
        let loaded: Vec<&str> = post.programs.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            loaded,
            ["composite3", "composite4", "final"],
            "{:?}",
            post.notes
        );
        assert_eq!(
            switches_on(&pack),
            BTreeSet::from(["AO".to_owned(), "DEFAULTED".to_owned()])
        );
    }

    /// A guard is opened only by the bare negated test (a comment may follow it), and only
    /// comment lines may stand between it and its `#define`: `//` lines, a commented switch,
    /// and block comments on one line or several. Any other condition opens no guard, so its
    /// `#define` stays an ordinary option.
    #[test]
    fn guards_are_the_bare_negated_test_past_comment_lines() {
        let pack = Pack::from_zip_bytes(
            "P.zip",
            zip(&[
                (
                    "shaders/lib/guards.glsl",
                    "#ifndef BLOCK_GLSL\n/* note */\n#define BLOCK_GLSL\n#endif\n\
                     #ifndef LINES_GLSL // guard\n//#define OTHER\n// note\n#define LINES_GLSL\n#endif\n\
                     #if !defined( OPEN_GLSL ) /* a\nb\n*/\n/* c\n */ /* d */\n#define OPEN_GLSL\n#endif\n",
                ),
                (
                    "shaders/lib/settings.glsl",
                    "#if !defined(AO) && defined(SUPPORTED)\n#define AO\n#endif\n\
                     #ifndef BLOOM_ON 1\n#define BLOOM_ON\n#endif\n\
                     #ifndef CODE\nint x; /* c */\n#define CODE\n#endif\n\
                     #ifndef SPLIT /* a\n*/ int y;\n#define SPLIT\n#endif\n",
                ),
                (
                    "shaders/final.fsh",
                    "#ifdef AO\n#endif\n#ifdef BLOOM_ON\n#endif\n#ifdef OTHER\n#endif\n\
                     #ifdef BLOCK_GLSL\n#endif\n#ifdef LINES_GLSL\n#endif\n#ifdef OPEN_GLSL\n#endif\n",
                ),
            ]),
        )
        .unwrap();
        let listed: Vec<String> = options(&pack).into_keys().collect();
        assert_eq!(listed, ["AO", "BLOOM_ON", "CODE", "OTHER", "SPLIT"]);
        assert_eq!(options(&pack)["OTHER"].default, "false");
        assert_eq!(
            switches_on(&pack),
            BTreeSet::from(["AO", "BLOOM_ON", "CODE", "SPLIT"].map(str::to_owned))
        );
    }

    /// `#define NAME//comment` (no space before the comment) is listed as a switch, as before
    /// the scan was shared, and is still not one `shaders.properties` or a program enable sees
    /// as on: the old switch reader read the name as `NAME//comment`. Declared spaced in another
    /// file, the same name counts.
    #[test]
    fn a_switch_glued_to_its_comment_is_listed_but_not_on() {
        let pack = Pack::from_zip_bytes(
            "P.zip",
            zip(&[
                (
                    "shaders/lib/settings.glsl",
                    "#define GLUED//comment\n//#define GLUED_OFF//comment\n#define BOTH//c\n",
                ),
                ("shaders/lib/more.glsl", "#define BOTH // c\n"),
                (
                    "shaders/final.fsh",
                    "#ifdef GLUED\n#endif\n#ifdef GLUED_OFF\n#endif\n#ifdef BOTH\n#endif\n",
                ),
            ]),
        )
        .unwrap();
        let options = options(&pack);
        let listed: Vec<(&str, &str)> = options
            .values()
            .map(|o| (o.name.as_str(), o.default.as_str()))
            .collect();
        assert_eq!(
            listed,
            [("BOTH", "true"), ("GLUED", "true"), ("GLUED_OFF", "false")]
        );
        assert_eq!(switches_on(&pack), BTreeSet::from(["BOTH".to_owned()]));
    }

    /// A custom uniform's or variable's line has the macros expanded: a value option, a
    /// `#define` of `shaders.properties` (function-like too), a standard macro, and a macro in
    /// the uniform's name. Our probe pack's shape; Iris 1.11.7 drew p0..p5 as 127, 191, 64, 255,
    /// 127 and 255 of 255 with the option at its default.
    #[test]
    fn custom_uniform_lines_read_the_options_and_macros() {
        let bytes = crate::pack::tests::zip(&[
            ("shaders/final.fsh", "#define MY_OPT 2 // [1 2 3]\n"),
            (
                "shaders/shaders.properties",
                "#define PROP_DEF 3\n#define HALF(x) ((x) * 0.5)\n#define KEYNAME p5\n\
                 uniform.float.p0 = MY_OPT * 0.25\n\
                 uniform.float.p1 = PROP_DEF * 0.25\n\
                 variable.float.v2 = MY_OPT * 0.125\nuniform.float.p2 = v2\n\
                 uniform.float.p3 = if(MC_VERSION > 10000, 1.0, 0.0)\n\
                 uniform.float.p4 = HALF(MY_OPT) * 0.5\n\
                 uniform.float.KEYNAME = 1.0\n",
            ),
        ]);
        for (chosen, p0) in [(None, 0.5), (Some("3"), 0.75)] {
            let mut pack = Pack::from_zip_bytes("p.zip", bytes.clone()).unwrap();
            if let Some(value) = chosen {
                pack = pack.with_chosen(BTreeMap::from([("MY_OPT".into(), value.into())]));
            }
            let (properties, _) = read_properties(&pack, &mut crate::Macros::standard());
            assert!(properties.notes.is_empty(), "{:?}", properties.notes);
            let mut custom = crate::Custom::parse(&properties);
            let values: Vec<f64> = custom
                .eval(0.016, &|_| None)
                .iter()
                .map(|v| v.v[0])
                .collect();
            let p4 = if chosen.is_some() { 0.75 } else { 0.5 };
            assert_eq!(values, [p0, 0.75, p0 / 2.0, 1.0, p4, 1.0]);
            let names: Vec<_> = custom.uniforms().map(|(name, _)| name.to_owned()).collect();
            assert_eq!(names, ["p0", "p1", "p2", "p3", "p4", "p5"]);
            assert!(custom.notes.is_empty(), "{:?}", custom.notes);
        }
    }
}
