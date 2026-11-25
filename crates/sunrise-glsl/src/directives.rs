//! The directives of a stage, run as a GLSL preprocessor runs them (GLSL 4.60 §3.3) before naga
//! parses it, naga then reading code with no macro left: `#define` and `#undef` ([`Macros`]),
//! `#if`, `#ifdef`, `#ifndef`, `#elif`, `#else` and `#endif` ([`Conditionals`], each condition
//! by [`test`]), `#line`, and each other line expanded with the macros of its point, `__LINE__`
//! its line. Lines ending in `\` continue on the next, comments are blanked, and a macro call's
//! arguments may span lines ([`Macros::join_call`]). `#version`, `#extension`, `#pragma`,
//! `#error` and any other directive are left as they are, for naga.
//!
//! Each line of the result is the line of the code it was, with its origin: an error names the
//! file and line the author wrote.

use crate::condition::{Conditionals, eval_condition, eval_line};
use crate::located::Lines;
use crate::macros::{Macros, Stop, is_ident};
use crate::{Error, Expanded, ImportRefusal, MAX_SCANNED};

/// The macros a program starts with: its `defines` (`NAME` or `NAME value`, in order), with
/// `work` to expand ([`Macros::checked`]).
pub(crate) fn macros(defines: &[&str], work: usize) -> Macros {
    let mut macros = Macros::checked(work);
    for define in defines {
        let (name, value) = define.split_once(' ').unwrap_or((define, ""));
        macros.define(name, value);
    }
    macros
}

/// Whether the condition of `#kind rest` holds under `macros` (`kind` is `if` for an `#elif`):
/// `#ifdef` and `#ifndef` name one macro, `#if` is [`eval_condition`]'s arithmetic.
pub(crate) fn test(kind: &str, rest: &str, macros: &Macros) -> Result<bool, String> {
    match kind {
        "ifdef" | "ifndef" => {
            if rest.is_empty() || !rest.chars().all(is_ident) {
                return Err(format!("invalid #{kind} {rest}"));
            }
            Ok(macros.is_defined(rest) == (kind == "ifdef"))
        }
        _ => eval_condition(rest, macros)
            .map(|v| v != 0)
            .map_err(|e| format!("`#{kind} {rest}`: {e}")),
    }
}

/// Why a call of `name` is refused: it gives a number of arguments the macro doesn't take.
fn arguments(name: &str) -> String {
    format!("`{name}` called with a number of arguments it doesn't take")
}

/// Why `stop` stopped an expansion.
pub(crate) fn stopped(stop: Stop) -> String {
    match stop {
        Stop::Work => ImportRefusal::TooMuchScanned.to_string(),
        Stop::Depth => crate::macros::calls_too_deep(),
        Stop::Arguments(name) => arguments(&name),
    }
}

/// The parameters of a function-like definition (`NAME(a, b) body`) that name one twice, as
/// naga's preprocessor (pp-rs 0.2.1) refused: the first such name.
fn repeated_parameter(rest: &str) -> Option<&str> {
    let name_end = rest.find(|c| !is_ident(c)).unwrap_or(rest.len());
    let params = rest[name_end..].strip_prefix('(')?.split_once(')')?.0;
    let params: Vec<&str> = params.split(',').map(str::trim).collect();
    (params.iter().enumerate()).find_map(|(n, p)| params[..n].contains(p).then_some(*p))
}

/// The directive word of a line and the rest of it, trimmed; none for a line of code.
pub(crate) fn directive(line: &str) -> Option<(&str, &str)> {
    let d = line.trim_start().strip_prefix('#')?.trim_start();
    let i = d.find(|c| !is_ident(c)).unwrap_or(d.len());
    Some((&d[..i], d[i..].trim()))
}

/// The name a `#define`, `#undef`, `#ifdef` or `#ifndef` gives (the rest of its line up to
/// what is not part of a name), when it doesn't start with a letter or `_`: naga's preprocessor
/// (pp-rs 0.2.1) read a name there as an identifier token, and refused anything else.
fn bad_name(rest: &str) -> Option<&str> {
    let name = &rest[..rest.find(|c| !is_ident(c)).unwrap_or(rest.len())];
    (!name.starts_with(crate::macros::is_ident_start)).then_some(name)
}

/// `line` with each `__LINE__` written as `number`, unsigned as naga's preprocessor gave it.
fn with_line(line: &str, number: i64) -> String {
    let mut out = String::with_capacity(line.len());
    for token in crate::macros::lex(line) {
        if token == "__LINE__" {
            out.push_str(&format!("{number}u"));
        } else {
            out.push_str(token);
        }
    }
    out
}

/// `code` with its directives run under `defines` (`NAME` or `NAME value`, in order, none
/// holding a line break), as naga's preprocessor (pp-rs 0.2.1) ran them before. Its refusals
/// stay: a macro defined again with other tokens, a parameter named twice, a name that doesn't
/// start with a letter or `_`, and a call with the wrong number of arguments. `__LINE__` is the
/// line it is written on, and in a macro's replacement the line of the call (the last line of a
/// call over lines). A `#define __LINE__` is taken and changes nothing, as there. The expansion
/// spends at most [`MAX_SCANNED`] tokens of work (what each call reads and what its replacement
/// makes, [`Macros::checked`]), the bound on what the translator reads for a stage; past it the
/// stage is refused, at the line.
pub fn run_directives(code: &Expanded, defines: &[&str]) -> Result<Expanded, Error> {
    if let Some(define) = defines.iter().find(|d| d.contains('\n')) {
        // Every line of the result is a line of the code: a replacement can't add one.
        return Err(code.located(None, format!("the define `{define}` holds a line break")));
    }
    let mut macros = macros(defines, MAX_SCANNED);
    let error = |line: usize, message: String| code.located(code.lines.origin(line), message);
    // Splicing precedes comment removal: each logical line remembers its first physical one.
    let text = code.text();
    let mut joined = String::with_capacity(text.len());
    let mut first = vec![0usize];
    for (n, line) in text.split_inclusive('\n').enumerate() {
        match line
            .strip_suffix("\\\r\n")
            .or_else(|| line.strip_suffix("\\\n"))
        {
            Some(part) => joined.push_str(part),
            None => {
                joined.push_str(line);
                first.push(n + 1);
            }
        }
    }
    let clean = crate::strip_comments(&joined);
    let logical: Vec<&str> = clean.split('\n').collect();
    let physical = text.split('\n').count();
    let mut out = vec![String::new(); physical];
    let mut branches = Conditionals::default();
    // The line of each `#if` still open, the innermost last.
    let mut open = Vec::new();
    // A `#define __LINE__`'s rest, which only another one compares with.
    let mut line_macro: Option<&str> = None;
    // As naga's preprocessor numbers lines: the text's, moved by `#line` (the directive's own
    // line takes its number).
    let mut offset = 0i64;
    let number = |k: usize, offset: i64| first[k] as i64 + 1 + offset;
    let mut k = 0;
    while k < logical.len() {
        let line = logical[k].trim_end_matches('\r');
        let at = first[k];
        let here = number(k, offset);
        macros.define("__LINE__", format!("{here}u"));
        if let Some((word, rest)) = directive(line) {
            if matches!(word, "define" | "undef" | "ifdef" | "ifndef")
                && branches.active()
                && let Some(name) = bad_name(rest)
            {
                return Err(error(at, format!("`{name}` is not a macro name")));
            }
            match branches.apply_with(word, |kind| test(kind, rest, &macros)) {
                Ok(true) => {
                    match word {
                        "if" | "ifdef" | "ifndef" => open.push(at),
                        "endif" => {
                            open.pop();
                        }
                        _ => {}
                    }
                    k += 1;
                    continue;
                }
                Ok(false) if !branches.active() => {
                    k += 1;
                    continue;
                }
                Ok(false) => {}
                Err(why) => return Err(error(at, why)),
            }
            match word {
                "define" if rest.split(|c| !is_ident(c)).next() == Some("__LINE__") => {
                    let alike = |a: &str, b: &str| {
                        let tokens = |t: &str| -> Vec<String> {
                            (crate::macros::lex(t).into_iter())
                                .filter(|t| !t.trim().is_empty())
                                .map(str::to_owned)
                                .collect()
                        };
                        tokens(a) == tokens(b)
                    };
                    if line_macro.is_some_and(|before| !alike(before, rest)) {
                        return Err(error(at, format!("`#define {rest}` redefines a macro")));
                    }
                    line_macro = Some(rest);
                }
                "define" => {
                    if let Some(name) = repeated_parameter(rest) {
                        return Err(error(at, format!("parameter `{name}` named twice")));
                    }
                    // Defined again only the same way, as naga's preprocessor (pp-rs 0.2.1)
                    // took it.
                    if macros.defines_alike(rest) == Some(false) {
                        return Err(error(at, format!("`#define {rest}` redefines a macro")));
                    }
                    macros.define_line(rest).map_err(|why| error(at, why))?;
                }
                "undef" if rest == "__LINE__" => line_macro = None,
                "undef" if !rest.is_empty() && rest.chars().all(is_ident) => macros.undef(rest),
                "undef" => return Err(error(at, format!("invalid #undef {rest}"))),
                "line" => {
                    let value = eval_line(rest, &macros).map_err(|why| error(at, why))?;
                    if !(0..1 << 32).contains(&value) {
                        return Err(error(at, "line number outside GLSL range".into()));
                    }
                    offset = value - (at as i64 + 1);
                }
                _ => out[at] = line.to_owned(),
            }
        } else if branches.active() {
            let spliced = first.get(k + 1).map_or(0, |next| next - at - 1);
            let mut probe = line.to_owned();
            let taken = macros.join_call(&mut probe, &logical[k + 1..], spliced);
            // Each line's own `__LINE__` is its number; a replacement's is the call's last line.
            let mut code = with_line(line, here);
            for j in k + 1..=k + taken {
                code.push(' ');
                code.push_str(&with_line(
                    logical[j].trim_end_matches('\r'),
                    number(j, offset),
                ));
            }
            let last = number(k + taken, offset);
            if !(0..1 << 32).contains(&last) && macros.mentions(&probe, "__LINE__") {
                return Err(error(at, "line number outside GLSL range".into()));
            }
            macros.define("__LINE__", format!("{last}u"));
            out[at] = macros.expand(&code);
            k += taken;
        }
        if let Some(stop) = macros.stopped() {
            return Err(error(at, stopped(stop)));
        }
        k += 1;
    }
    if let Some(&at) = open.last() {
        return Err(error(at, "#if without #endif".into()));
    }
    Ok(Expanded {
        lines: Lines::relined(&code.lines, &out),
        files: code.files.clone(),
    })
}
