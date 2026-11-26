//! Macros as a GLSL preprocessor keeps them (GLSL 4.60 §3.3): object-like and function-like
//! definitions, and the expansion of a line with them, as C expands it: a macro is not expanded
//! again inside its own expansion, a function-like one takes its arguments (expanded first, raw
//! beside `##`, which pastes) when a `(` follows its name. The caller says what each line is and
//! which macros it starts with: this module knows no game.

use std::cell::{Cell, RefCell};

use crate::condition::resolve_defined;
use crate::located::Lines;
use std::collections::BTreeMap;

/// How many lines one macro call may span: its arguments join the line the call starts on.
pub const MAX_CALL_LINES: usize = 64;

#[derive(Clone, Debug, PartialEq)]
struct Macro {
    /// `None` for an object-like macro; the parameter names of a function-like one.
    params: Option<Vec<String>>,
    body: String,
}

/// How deep a call may sit in another call's arguments, in either mode: each level expands its
/// arguments in a frame of its own. A depth whose frames fit in a 2 MiB thread (what Rust gives a
/// spawned thread) in a debug build with half of it to spare, which
/// `directives_tests::calls_nested_to_the_bound_fit_a_small_thread` checks, and `sunrise-shaders`'
/// tests of its preprocessor and properties for the shader packs' mode.
pub const MAX_ARGUMENT_DEPTH: usize = 128;

/// Why an expansion with a call deeper than [`MAX_ARGUMENT_DEPTH`] is refused ([`Stop::Depth`]).
pub fn calls_too_deep() -> String {
    format!("a macro call sits more than {MAX_ARGUMENT_DEPTH} calls deep in arguments")
}

/// Why an expansion stopped, its result unfinished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// The work ran out.
    Work,
    /// A call sits deeper than [`MAX_ARGUMENT_DEPTH`] in other calls' arguments.
    Depth,
    /// A call of the function-like macro of this name gives it a number of arguments it doesn't
    /// take, which naga's preprocessor (pp-rs 0.2.1) refused.
    Arguments(String),
}

/// The macros defined at a point of a stage, by name.
#[derive(Clone, Debug)]
pub struct Macros {
    map: BTreeMap<String, Macro>,
    /// The expansion work left ([`Macros::checked`]), in tokens: each expansion costs the
    /// tokens a call reads, each copy of an argument (charged before it is made) and the tokens
    /// its replacement makes; telling whether a call goes on past its line costs its expansion.
    work: Cell<usize>,
    /// [`Macros::checked`]: calls must give a function-like macro as many arguments as it takes,
    /// and a stop lasts.
    arity: bool,
    /// How deep the expansion is in calls' arguments.
    depth: Cell<usize>,
    /// The expansion is a condition's ([`Macros::expand_condition`]).
    condition: Cell<bool>,
    stopped: RefCell<Option<Stop>>,
}

/// No macros, no bound on the work, and a call with too many or too few arguments takes what it
/// has (a missing argument is empty). Calls nest at most [`MAX_ARGUMENT_DEPTH`] deep, and a call
/// past it stops that one expansion only ([`Macros::try_expand`]): the next starts afresh.
impl Default for Macros {
    fn default() -> Self {
        Self {
            map: BTreeMap::new(),
            work: Cell::new(usize::MAX),
            arity: false,
            depth: Cell::new(0),
            condition: Cell::new(false),
            stopped: RefCell::new(None),
        }
    }
}

/// Two sets of macros are equal when they define the same names the same way.
impl PartialEq for Macros {
    fn eq(&self, other: &Self) -> bool {
        self.map == other.map
    }
}

impl Macros {
    /// No macros, expanded as naga's preprocessor (pp-rs 0.2.1) expanded them: a call with the
    /// wrong number of arguments refused, and a `defined` that a condition's expansion makes read
    /// ([`Macros::expand_condition`]). With `work` tokens to spend, and calls nested at most
    /// [`MAX_ARGUMENT_DEPTH`] deep. Each stops the expansion ([`Macros::stopped`]).
    pub fn checked(work: usize) -> Self {
        Self {
            work: Cell::new(work),
            arity: true,
            ..Self::default()
        }
    }

    /// Why an expansion stopped, its result unfinished. With [`Macros::checked`], from then on
    /// every expansion stops; otherwise, the last expansion's.
    pub fn stopped(&self) -> Option<Stop> {
        self.stopped.borrow().clone()
    }

    /// An expansion starts: without [`Macros::checked`], what stopped the last one is forgotten.
    fn begin(&self) {
        if !self.arity {
            self.stopped.replace(None);
        }
    }

    /// One level deeper in calls' arguments; false (and stopped) past [`MAX_ARGUMENT_DEPTH`].
    fn deeper(&self) -> bool {
        if self.depth.get() >= MAX_ARGUMENT_DEPTH {
            self.stop(Stop::Depth);
            return false;
        }
        self.depth.set(self.depth.get() + 1);
        true
    }

    fn stop(&self, why: Stop) {
        self.stopped.borrow_mut().get_or_insert(why);
    }

    /// Spends `cost`; false (and stopped from then on) when not that much is left or an
    /// expansion stopped already.
    fn charge(&self, cost: usize) -> bool {
        if self.stopped.borrow().is_some() {
            return false;
        }
        match self.work.get().checked_sub(cost) {
            Some(left) => {
                self.work.set(left);
                true
            }
            None => {
                self.stop(Stop::Work);
                false
            }
        }
    }

    /// Whether `name` is defined with the same parameters and the same tokens, white space
    /// aside, as the definition `rest` (a `#define` line's): a definition that may be given
    /// again. `None` when `rest` defines nothing.
    pub fn defines_alike(&self, rest: &str) -> Option<bool> {
        let mut fresh = Macros::default();
        fresh.define_line(rest).ok()?;
        let (name, new) = fresh.map.into_iter().next()?;
        let tokens = |m: &Macro| -> Vec<String> {
            lex(&m.body)
                .into_iter()
                .filter(|t| !t.trim().is_empty())
                .map(str::to_owned)
                .collect()
        };
        Some(
            self.map
                .get(&name)
                .is_none_or(|old| old.params == new.params && tokens(old) == tokens(&new)),
        )
    }

    /// The names defined, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }

    /// Whether `text`, or a definition one of its names reaches through others, has the
    /// identifier `name`.
    pub fn mentions(&self, text: &str, name: &str) -> bool {
        let mut pending = vec![text];
        let mut seen = std::collections::HashSet::new();
        while let Some(source) = pending.pop() {
            for word in source.split(|c| !is_ident(c)).filter(|s| !s.is_empty()) {
                if word == name {
                    return true;
                }
                if let Some((key, m)) = self.map.get_key_value(word)
                    && seen.insert(key.as_str())
                {
                    pending.push(&m.body);
                }
            }
        }
        false
    }

    /// Joins to `line` the lines of `upcoming` a macro call on it still needs, while none of
    /// them is a directive and the call spans at most [`MAX_CALL_LINES`] lines, counting the
    /// `joined` lines `line` is already made of beyond its first; returns how many it joined.
    /// Telling whether a call is unfinished expands the line so far, which is work like any
    /// other expansion.
    pub fn join_call(&self, line: &mut String, upcoming: &[&str], joined: usize) -> usize {
        let mut taken = 0;
        while taken < upcoming.len()
            && joined + 1 + taken < MAX_CALL_LINES
            && !upcoming[taken].trim_start().starts_with('#')
            && self.unfinished_call(line, &upcoming[taken..])
        {
            line.push(' ');
            line.push_str(upcoming[taken]);
            taken += 1;
        }
        taken
    }

    pub fn define(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let body = value.into();
        self.map.insert(name.into(), Macro { params: None, body });
    }

    /// The rest of a `#define` line: `NAME body`, or `NAME(a, b) body` (no space before the `(`)
    /// for a function-like macro.
    pub fn define_line(&mut self, rest: &str) -> Result<(), String> {
        let len = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let (name, tail) = rest.split_at(len);
        if name.is_empty() {
            return Err("`#define` without a name".into());
        }
        if let Some(params) = tail.strip_prefix('(') {
            let Some((params, body)) = params.split_once(')') else {
                return Err(format!("`#define {name}(` without `)`"));
            };
            let params = params
                .split(',')
                .map(|p| p.trim().to_owned())
                .filter(|p| !p.is_empty())
                .collect();
            self.define_function(name, params, body.trim());
        } else {
            self.define(name, tail.trim());
        }
        Ok(())
    }

    pub fn define_function(&mut self, name: &str, params: Vec<String>, body: &str) {
        let body = body.to_owned();
        self.map.insert(
            name.to_owned(),
            Macro {
                params: Some(params),
                body,
            },
        );
    }

    pub fn undef(&mut self, name: &str) {
        self.map.remove(name);
    }

    pub fn is_defined(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// The value of an object-like macro.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.map
            .get(name)
            .filter(|m| m.params.is_none())
            .map(|m| m.body.as_str())
    }

    /// Whether upcoming source lines continue a call, including one whose expanded name
    /// ends `text` and whose opening parenthesis is on the next nonempty line.
    pub fn unfinished_call(&self, text: &str, upcoming: &[&str]) -> bool {
        self.begin();
        let (tokens, unfinished) = self.expand_tokens(expansion_tokens(text, false));
        unfinished
            || (tokens.iter().rfind(|t| !t.blank()).is_some_and(|t| {
                self.map.get_key_value(&t.text).is_some_and(|(name, m)| {
                    m.params.is_some() && !t.hidden.contains(&name.as_str())
                })
            }) && upcoming
                .iter()
                .map(|line| line.trim_start())
                .find(|line| !line.is_empty())
                .is_some_and(|line| line.starts_with('(')))
    }

    /// `text` with every macro expanded, repeatedly; a macro is not expanded again inside its own
    /// expansion, so `A` defined as `A + 1` stops. Function-like macros take their arguments
    /// (expanded first, raw beside `##`, which pastes) when a `(` follows their name.
    pub fn expand(&self, text: &str) -> String {
        self.begin();
        self.expand_tokens(expansion_tokens(text, false))
            .0
            .iter()
            .map(|t| t.text.as_str())
            .collect()
    }

    /// Append the same expansion as [`Self::expand`], with source positions for diagnostics.
    pub(crate) fn expand_located(&self, input: &Lines, out: &mut Lines) {
        self.begin();
        for token in self.expand_tokens(expansion_tokens(input.text(), true)).0 {
            match token.source {
                TokenSource::Copy(at) => {
                    out.push_spanned(&token.text, None, input, at..at + token.text.len());
                }
                TokenSource::Call(at) => out.push_replacement(&token.text, None, input, at),
                TokenSource::None => out.push(&token.text, None),
            }
        }
    }

    /// [`Macros::expand`], or why it stopped ([`Stop`]) when it did.
    pub fn try_expand(&self, text: &str) -> Result<String, String> {
        let expanded = self.expand(text);
        match self.stopped() {
            Some(stop) => Err(crate::directives::stopped(stop)),
            None => Ok(expanded),
        }
    }

    /// [`Macros::expand`] for the condition of an `#if`: with [`Macros::checked`], the name a
    /// `defined` reads is not expanded, and a `defined` the expansion makes is read
    /// ([`resolve_defined`]), as naga's preprocessor (pp-rs 0.2.1) read it. Otherwise
    /// [`Macros::try_expand`].
    pub fn expand_condition(&self, text: &str) -> Result<String, String> {
        if !self.arity {
            return self.try_expand(text);
        }
        self.condition.set(true);
        let expanded = self.expand(text);
        self.condition.set(false);
        resolve_defined(&expanded, |name| self.is_defined(name))
    }

    fn expand_tokens<'a>(
        &'a self,
        tokens: Vec<ExpansionToken<'a>>,
    ) -> (Vec<ExpansionToken<'a>>, bool) {
        // The tokens still to read, the next one last: a replacement goes back on top of them,
        // to be read again with the rest of the source.
        let mut pending = tokens;
        pending.reverse();
        let mut out = Vec::with_capacity(pending.len());
        let mut unfinished = false;
        while let Some(token) = pending.pop() {
            if token.text == "defined" && self.condition.get() {
                // The name `defined` reads is not expanded (`defined X`, `defined ( X )`).
                out.push(token);
                let mut open = false;
                while let Some(next) = pending.pop() {
                    let (blank, text) = (next.blank(), next.text.clone());
                    out.push(next);
                    if blank {
                        continue;
                    }
                    if text == "(" && !open {
                        open = true;
                        continue;
                    }
                    if open && text != ")" {
                        continue;
                    }
                    break;
                }
                continue;
            }
            let Some((name, m)) = self.map.get_key_value(token.text.as_str()) else {
                out.push(token);
                continue;
            };
            if token.hidden.contains(&name.as_str()) {
                out.push(token);
                continue;
            }
            let mut hidden = token.hidden.clone();
            let mut body = match &m.params {
                None => expansion_tokens(&m.body, false),
                Some(params) => {
                    let Some((args, used)) = arguments(&pending) else {
                        unfinished |= (pending.iter().rev())
                            .find(|t| !t.blank())
                            .is_some_and(|t| t.text == "(");
                        out.push(token);
                        continue;
                    };
                    // What the call reads is work, whether or not it is used.
                    if !self.charge(used) {
                        pending.push(token);
                        break;
                    }
                    let empty = args.len() == 1 && args[0].iter().all(ExpansionToken::blank);
                    if self.arity && args.len() != params.len() && !(empty && params.is_empty()) {
                        self.stop(Stop::Arguments(name.clone()));
                        pending.push(token);
                        break;
                    }
                    // A call can cross a replacement boundary. Only disabling shared by
                    // its name and closing parenthesis carries into the new replacement.
                    let close = &pending[pending.len() - used];
                    hidden.retain(|h| close.hidden.contains(h));
                    pending.truncate(pending.len() - used);
                    self.substitute(params, &args, &m.body)
                }
            };
            // The tokens it makes are work: they are read again.
            if !self.charge(body.len()) {
                break;
            }
            hidden.push(name);
            let call = match token.source {
                TokenSource::Copy(at) | TokenSource::Call(at) => TokenSource::Call(at),
                TokenSource::None => TokenSource::None,
            };
            for token in &mut body {
                if token.source == TokenSource::None {
                    token.source = call;
                }
                if token.hidden.is_empty() {
                    token.hidden.clone_from(&hidden);
                    continue;
                }
                for h in &hidden {
                    if !token.hidden.contains(h) {
                        token.hidden.push(h);
                    }
                }
            }
            // Rescan the replacement together with the remaining source. Its final name
            // may consume a call there; source tokens keep their own disabling context.
            pending.extend(body.into_iter().rev());
        }
        pending.reverse();
        out.append(&mut pending);
        (out, unfinished)
    }

    /// A function-like macro's body with its parameters replaced.
    fn substitute<'a>(
        &'a self,
        params: &[String],
        args: &[Vec<ExpansionToken<'a>>],
        body: &str,
    ) -> Vec<ExpansionToken<'a>> {
        let tokens = lex(body);
        let pasted = |at: usize, step: isize| {
            let mut j = at as isize + step;
            while (0..tokens.len() as isize).contains(&j) && tokens[j as usize].trim().is_empty() {
                j += step;
            }
            (0..tokens.len() as isize).contains(&j) && tokens[j as usize] == "##"
        };
        let mut out = Vec::new();
        for (at, token) in tokens.iter().enumerate() {
            match params.iter().position(|p| p == token) {
                Some(k) => {
                    let arg = args.get(k).map_or(&[][..], |a| a.as_slice());
                    let from = arg.iter().position(|t| !t.blank()).unwrap_or(arg.len());
                    let end = arg.iter().rposition(|t| !t.blank()).map_or(from, |n| n + 1);
                    // Each copy of an argument is work, charged before it is made: a parameter
                    // used many times can't build its copies past the work first.
                    if !self.charge(end - from) {
                        break;
                    }
                    let arg = arg[from..end].to_vec();
                    if pasted(at, -1) || pasted(at, 1) {
                        if arg.is_empty() {
                            // An empty paste operand must not capture the preceding token.
                            out.push(ExpansionToken::new(""));
                        } else {
                            out.extend(arg);
                        }
                    } else if self.deeper() {
                        out.extend(self.expand_tokens(arg).0);
                        self.depth.set(self.depth.get() - 1);
                    } else {
                        break;
                    }
                }
                None => out.push(ExpansionToken::new(token)),
            }
        }
        // `a ## b` → `ab`.
        let mut joined: Vec<ExpansionToken<'a>> = Vec::new();
        let mut paste = false;
        for token in out {
            if token.text == "##" {
                while joined.last().is_some_and(|t| t.blank()) {
                    joined.pop();
                }
                paste = true;
            } else if paste && token.blank() {
                continue;
            } else if paste {
                if let Some(mut left) = joined.pop() {
                    // An empty operand leaves the other token, including its hiding, intact.
                    if left.text.is_empty() {
                        left = token;
                    } else if !token.text.is_empty() {
                        left.text.push_str(&token.text);
                        left.source = TokenSource::None;
                        left.hidden.retain(|h| token.hidden.contains(h));
                    }
                    joined.push(left);
                } else {
                    joined.push(token);
                }
                paste = false;
            } else {
                joined.push(token);
            }
        }
        joined.retain(|t| !t.text.is_empty());
        // A paste that makes no single token (`b ## (`, `name_ ## a ## _to_ ## b ##(x)`) stays
        // the tokens it is made of: the name before the `(` is then rescanned and can call a
        // macro.
        joined
            .into_iter()
            .flat_map(|token| {
                let parts = lex(&token.text);
                if parts.len() < 2 {
                    return vec![token];
                }
                parts
                    .into_iter()
                    .map(|text| ExpansionToken {
                        text: text.into(),
                        hidden: token.hidden.clone(),
                        source: token.source,
                    })
                    .collect()
            })
            .collect()
    }
}

/// Disabling belongs to each token: rescanning a boundary must neither revive a recursive
/// replacement token nor hide an independent invocation in the surrounding source.
#[derive(Clone)]
struct ExpansionToken<'a> {
    text: String,
    hidden: Vec<&'a str>,
    source: TokenSource,
}

/// Byte offset in the macro expansion's input, as a copy or anchored at the macro call.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenSource {
    Copy(usize),
    Call(usize),
    None,
}

impl ExpansionToken<'_> {
    fn new(text: &str) -> Self {
        Self {
            text: text.into(),
            hidden: Vec::new(),
            source: TokenSource::None,
        }
    }

    fn blank(&self) -> bool {
        !self.text.is_empty() && self.text.trim().is_empty()
    }
}

/// Tokenize `text`, retaining offsets only when this is the diagnostic path's input, not
/// replacement text (which will take the macro call's position).
fn expansion_tokens(text: &str, located: bool) -> Vec<ExpansionToken<'_>> {
    let mut at = 0;
    lex(text)
        .into_iter()
        .map(|text| {
            let mut token = ExpansionToken::new(text);
            if located {
                token.source = TokenSource::Copy(at);
                at += text.len();
            }
            token
        })
        .collect()
}

/// The arguments of a call whose `(` is the next non-blank token of `pending` (read from its
/// end), split at top-level commas, and how many tokens of `pending` the call takes, its `)`
/// the last. `None` when no `(` follows or it never closes.
fn arguments<'a>(pending: &[ExpansionToken<'a>]) -> Option<(Vec<Vec<ExpansionToken<'a>>>, usize)> {
    let mut next = pending.iter().rev().enumerate();
    loop {
        let (_, t) = next.next()?;
        if !t.blank() {
            if t.text != "(" {
                return None;
            }
            break;
        }
    }
    let (mut depth, mut args, mut arg) = (0, Vec::new(), Vec::new());
    for (n, t) in next {
        match t.text.as_str() {
            "(" => depth += 1,
            ")" if depth == 0 => {
                args.push(arg);
                return Some((args, n + 1));
            }
            ")" => depth -= 1,
            "," if depth == 0 => {
                args.push(std::mem::take(&mut arg));
                continue;
            }
            _ => {}
        }
        arg.push(t.clone());
    }
    None
}

/// Preprocessing tokens: identifiers, numbers (with their suffixes and exponents), runs of
/// white space, `##`, and single characters. Concatenated they give back `text`.
pub fn lex(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        let start = i;
        i += 1;
        if is_ident_start(c) {
            while i < bytes.len() && is_ident(bytes[i] as char) {
                i += 1;
            }
        } else if c.is_ascii_digit() || (c == '.' && bytes.get(i).is_some_and(u8::is_ascii_digit)) {
            while i < bytes.len() {
                let d = bytes[i] as char;
                let exp_sign = (d == '+' || d == '-') && matches!(bytes[i - 1], b'e' | b'E');
                if !(is_ident(d) || d == '.' || exp_sign) {
                    break;
                }
                i += 1;
            }
        } else if c.is_whitespace() {
            while i < bytes.len() && (bytes[i] as char).is_whitespace() {
                i += 1;
            }
        } else if c == '#' && bytes.get(i) == Some(&b'#') {
            i += 1;
        } else {
            while !text.is_char_boundary(i) {
                i += 1;
            }
        }
        out.push(&text[start..i]);
    }
    out
}

pub fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

pub fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion_replaces_whole_words_and_stops_on_recursion() {
        let mut m = Macros::default();
        m.define("RES", "SIZE");
        m.define("SIZE", "512");
        m.define("LOOP", "LOOP + 1");
        assert_eq!(m.expand("RES RES 0.5"), "512 512 0.5");
        assert_eq!(
            m.expand("RESOLUTION colortex4 LOOP"),
            "RESOLUTION colortex4 LOOP + 1"
        );
        m.define("e5", "x");
        assert_eq!(m.expand("1e5"), "1e5");
    }

    #[test]
    fn function_like_macros_take_arguments_and_paste() {
        let mut m = Macros::default();
        m.define("N", "4");
        m.define_function("SQ", vec!["x".into()], "((x) * (x))");
        m.define_function("CAT", vec!["a".into(), "b".into()], "a ## b");
        m.define_function("PI", vec![], "3.14");
        assert_eq!(m.expand("SQ(N + 1)"), "((4 + 1) * (4 + 1))");
        assert_eq!(m.expand("SQ(min(N, 2))"), "((min(4, 2)) * (min(4, 2)))");
        assert_eq!(m.expand("CAT(N, 2) CAT(col, ortex4)"), "N2 colortex4");
        assert_eq!(m.expand("PI() SQ SQ(1"), "3.14 SQ SQ(1");
        assert_eq!(m.get("SQ"), None);
    }

    /// The work is what the expansion makes: macros that double at every level stop once they
    /// have made `work` tokens, and a long line whose macros make a token each fits in a work
    /// as long as it.
    #[test]
    fn the_work_is_what_the_expansion_makes() {
        let mut m = Macros::checked(2050);
        m.define("W", "1.0");
        let line = vec!["W"; 2050].join(" + ");
        assert_eq!(m.expand(&line), vec!["1.0"; 2050].join(" + "));
        assert_eq!(m.stopped(), None);
        let mut m = Macros::checked(10_000);
        m.define("M0", "x");
        for n in 1..40 {
            m.define(format!("M{n}"), format!("M{} M{}", n - 1, n - 1));
        }
        m.expand("M39");
        assert_eq!(m.stopped(), Some(Stop::Work));
    }

    /// A parameter used many times, given a long argument, stops at the work before its copies
    /// are made: the 20,000 copies of 20,000 tokens would be 4e8 tokens.
    #[test]
    fn argument_copies_are_charged_before_they_are_made() {
        let mut m = Macros::checked(100_000);
        m.define_function("F", vec!["x".into()], &vec!["x"; 20_000].join(" "));
        let argument = vec!["a"; 20_000].join(" ");
        m.expand(&format!("F({argument})"));
        assert_eq!(m.stopped(), Some(Stop::Work));
    }

    /// Telling whether a call goes on past its line is work: calls whose argument expands to many
    /// tokens, each closing on the next line, stop at the work.
    #[test]
    fn probing_a_call_over_lines_is_work() {
        let mut m = Macros::checked(100_000);
        m.define_function("G", vec!["x".into()], "");
        m.define("M0", "y");
        for n in 1..=14 {
            m.define(format!("M{n}"), format!("M{} M{}", n - 1, n - 1));
        }
        let lines: Vec<&str> = ["G(M14", ")"].repeat(100);
        let mut n = 0;
        while n < lines.len() && m.stopped().is_none() {
            let mut line = lines[n].to_owned();
            n += 1 + m.join_call(&mut line, &lines[n + 1..], 0);
        }
        assert_eq!(m.stopped(), Some(Stop::Work));
    }

    /// The default expansion, as `Macros::default` reads: a call made of a replacement's last
    /// name and the source's parenthesis, and an empty operand of `##`.
    #[test]
    fn calls_across_a_replacement_and_empty_pastes() {
        let mut m = Macros::default();
        m.define("F", "G");
        m.define_function("G", vec!["x".into()], "x + 1");
        m.define_function("CAT", vec!["a".into(), "b".into()], "a ## b");
        assert_eq!(m.expand("F(2)"), "2 + 1");
        assert_eq!(m.expand("CAT(, x) CAT(y, )"), "x y");
    }

    /// Without [`Macros::checked`] (the shader packs' mode), calls nest to `MAX_ARGUMENT_DEPTH`
    /// on a thread of half the 2 MiB Rust gives a spawned thread, in this (debug) build; one
    /// level more stops that expansion, by name, and the next expansion starts afresh.
    #[test]
    fn the_default_mode_stops_one_expansion_past_the_depth() {
        let nested = |depth: usize| format!("{}5{}", "F(".repeat(depth), ")".repeat(depth));
        let (deepest, past, condition, after) = std::thread::Builder::new()
            .stack_size(1 << 20)
            .spawn(move || {
                let mut m = Macros::default();
                m.define_function("F", vec!["x".into()], "x");
                (
                    m.try_expand(&nested(MAX_ARGUMENT_DEPTH)),
                    m.try_expand(&nested(400)),
                    m.expand_condition(&nested(MAX_ARGUMENT_DEPTH + 1)),
                    m.try_expand("F(2)"),
                )
            })
            .unwrap()
            .join()
            .unwrap();
        let why = crate::directives::stopped(Stop::Depth);
        assert_eq!(deepest, Ok("5".into()));
        assert_eq!(past, Err(why.clone()));
        assert_eq!(condition, Err(why));
        assert_eq!(after, Ok("2".into()));
    }
}
