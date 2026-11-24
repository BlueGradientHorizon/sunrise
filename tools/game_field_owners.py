#!/usr/bin/env python3
"""Rewrite and check field accesses for fields moved out of `struct Game<'a>`.

Reads `tools/game_field_owners.tsv` next to this script (or `--table PATH`).
Fields with owner `server` live on `Game.server` (`ServerState`); fields with
owner `client` live on `Game.client` (`ClientState`); fields with owner `game`
live directly on `Game`. Normalises accesses in both directions.

Usage:
    python3 tools/game_field_owners.py --fix [--allow-ambiguous] [paths...]
    python3 tools/game_field_owners.py --check [--allow-ambiguous] [paths...]
"""

from __future__ import annotations

import argparse
import csv
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_TABLE = Path(__file__).resolve().with_name("game_field_owners.tsv")
DEFAULT_PATHS = (
    Path("crates/sunrise-client/src/game.rs"),
    Path("crates/sunrise-client/src/game"),
)

# `macro_rules!` in `game.rs` / `game/` whose `$var` parameter is a `Game` receiver.
GAME_MACROS: dict[str, set[str]] = {
    "world_items": {"$s"},
    "world_recipes": {"$s"},
    "world_stations": {"$s"},
    "world_biomes": {"$s"},
    "acting_equipment": {"$s"},
    "game_side": {"$s"},
    "section_scene": {"$game"},
    "hit_reads_of": {"$game"},
    "damage_level_of": {"$game"},
    "hit_effects_of": {"$game"},
    "player_resolver": {"$game"},
    "server_ctx": {"$s"},
}

# Macros whose arguments are token trees or non-expressions (never rewrite inside).
TOKEN_ONLY_MACROS: set[str] = {
    "stringify",
    "concat",
    "concat_idents",
    "include",
    "include_str",
    "include_bytes",
    "env",
    "option_env",
    "cfg",
    "compile_error",
    "file",
    "line",
    "column",
    "module_path",
    "asm",
    "global_asm",
    "naked_asm",
    "quote",
    "quote_spanned",
    "parse_quote",
    "parse_quote_spanned",
}

# Macros known to evaluate arguments as ordinary Rust expressions.
EXPR_MACROS: set[str] = set(GAME_MACROS) | {
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "matches",
    "format",
    "format_args",
    "print",
    "println",
    "eprint",
    "eprintln",
    "write",
    "writeln",
    "panic",
    "todo",
    "unimplemented",
    "unreachable",
    "dbg",
    "vec",
    "try",
    "pin",
    "select",
    "join",
    "trace",
    "debug",
    "info",
    "warn",
    "error",
    "event",
    "span",
    "json",
    "bail",
    "ensure",
    "anyhow",
    "assign_resident_id",
    "assign_resident_ids",
    "with_entity",
}

# Keywords that can precede `!` as logical negation rather than a macro call.
NON_MACRO_KEYWORDS: set[str] = {
    "if",
    "while",
    "match",
    "return",
    "else",
    "in",
    "break",
    "as",
    "let",
    "mut",
    "ref",
    "where",
    "yield",
    "async",
    "move",
    "unsafe",
    "loop",
    "const",
    "static",
    "pub",
}

# Explicit (relpath_posix, line_1based, field_name) fallback sites, if any.
EXTRA_SITES: set[tuple[str, int, str]] = set()

# Method/fn names too generic to infer across unrelated files without receiver proof.
NOISY_GLOBAL_FNS: set[str] = {
    "new",
    "default",
    "from",
    "into",
    "clone",
    "as_ref",
    "as_mut",
    "get",
    "get_mut",
    "take",
    "map",
    "and_then",
    "find",
    "filter",
    "ok_or",
}

# Suffix methods on a `let` RHS that preserve the underlying `Game` value.
PASSTHROUGH_METHODS: set[str] = {
    "unwrap",
    "expect",
    "ok",
    "as_ref",
    "as_mut",
    "as_deref",
    "as_deref_mut",
}

# Sentinel variable type strings: "" = known non-Game, "?" = ambiguous receiver.
NON_GAME = ""
AMBIGUOUS = "?"

Token = tuple[str, str, int, int]
ClosureArgMap = dict[int, dict[int, str]]


@dataclass(frozen=True)
class Edit:
    start: int
    end: int
    replacement: str
    field: str
    reason: str
    old_expr: str = ""
    new_expr: str = ""


@dataclass(frozen=True)
class AmbiguousSite:
    offset: int
    field: str
    detail: str


@dataclass
class WorkspaceIndex:
    # Struct name -> field names (or tuple indices "0", "1", ...) holding a `Game`
    struct_game_fields: dict[str, set[str]] = field(default_factory=dict)
    # Struct name -> set of 0-arg method names returning `&Game` / `&mut Game` / `Game`
    wrapper_game_returning_methods: dict[str, set[str]] = field(default_factory=dict)
    # Flat union of all 0-arg method names returning `Game` on any wrapper struct
    game_returning_methods: set[str] = field(default_factory=set)
    # Methods on `impl Game` taking closures: method_name -> arg_idx -> {closure_param_idx: kind}
    game_method_closure_params: dict[str, ClosureArgMap] = field(default_factory=dict)
    # Methods on `impl WrapperStruct` taking closures: wrapper -> method -> arg_idx -> {param_idx: kind}
    wrapper_method_closure_params: dict[str, dict[str, ClosureArgMap]] = field(default_factory=dict)
    # Public free fns returning Game: fn_name -> set of return positions (None = whole, int = tuple idx)
    pub_game_returning_fns: dict[str, set[int | None]] = field(default_factory=dict)
    # Private free fns returning Game (pruned of names that collide with non-Game fns)
    priv_game_returning_fns: dict[str, set[int | None]] = field(default_factory=dict)
    # Public free fns taking closures: fn_name -> arg_idx -> {closure_param_idx: kind}
    pub_fn_closure_params: dict[str, ClosureArgMap] = field(default_factory=dict)
    # Private free fns taking closures (pruned of names that collide with non-Game fns)
    priv_fn_closure_params: dict[str, ClosureArgMap] = field(default_factory=dict)
    # Free fn names that appear anywhere returning non-Game / taking non-Game closures
    non_game_return_free_fns: set[str] = field(default_factory=set)
    non_game_closure_free_fns: set[str] = field(default_factory=set)
    # Free fn names that have BOTH Game and non-Game signatures in the workspace
    ambiguous_return_fns: set[str] = field(default_factory=set)
    ambiguous_closure_fns: set[str] = field(default_factory=set)


@dataclass
class FileLocalIndex:
    defined_structs: set[str] = field(default_factory=set)
    struct_game_fields: dict[str, set[str]] = field(default_factory=dict)
    defined_free_fns: set[str] = field(default_factory=set)
    game_returning_fns: dict[str, set[int | None]] = field(default_factory=dict)
    fn_closure_params: dict[str, ClosureArgMap] = field(default_factory=dict)


def load_field_owners(table_path: Path) -> dict[str, str]:
    """Load `field -> owner` (`game` | `server` | `client`) from the TSV table."""
    owners: dict[str, str] = {}
    with table_path.open(newline="", encoding="utf-8") as fp:
        for row in csv.DictReader(fp, delimiter="\t"):
            owner = row["owner"].strip()
            if owner in ("game", "server", "client"):
                owners[row["field"].strip()] = owner
    return owners


def load_moved_fields(table_path: Path) -> dict[str, str]:
    return {f: o for f, o in load_field_owners(table_path).items() if o in ("server", "client")}


def tokenize_rust(src: str) -> list[Token]:
    """Tokenize Rust source into non-trivia tokens `(kind, text, start, end)`.

    Properly skips line/block comments (including nested block comments),
    string/byte/c-string/raw-string literals, and char/byte literals while
    preserving lifetimes and `$ident` macro metavariables.
    """
    tokens: list[Token] = []
    n = len(src)
    i = 0

    while i < n:
        ch = src[i]

        if ch.isspace():
            i += 1
            continue

        if ch == "/" and i + 1 < n and src[i + 1] == "/":
            nl = src.find("\n", i + 2)
            i = n if nl == -1 else nl + 1
            continue

        if ch == "/" and i + 1 < n and src[i + 1] == "*":
            depth = 1
            i += 2
            while i < n and depth > 0:
                if src[i] == "/" and i + 1 < n and src[i + 1] == "*":
                    depth += 1
                    i += 2
                elif src[i] == "*" and i + 1 < n and src[i + 1] == "/":
                    depth -= 1
                    i += 2
                else:
                    i += 1
            continue

        raw_prefix = 0
        if ch == "r" and i + 1 < n and src[i + 1] in ('"', "#"):
            raw_prefix = 1
        elif ch in ("b", "c") and i + 2 < n and src[i + 1] == "r" and src[i + 2] in ('"', "#"):
            raw_prefix = 2
        if raw_prefix:
            j = i + raw_prefix
            hashes = 0
            while j < n and src[j] == "#":
                hashes += 1
                j += 1
            if j < n and src[j] == '"':
                start = i
                j += 1
                closing = '"' + "#" * hashes
                end_pos = src.find(closing, j)
                i = n if end_pos == -1 else end_pos + len(closing)
                tokens.append(("LIT", src[start:i], start, i))
                continue

        if ch == "r" and i + 2 < n and src[i + 1] == "#" and (src[i + 2].isalpha() or src[i + 2] == "_"):
            start = i
            j = i + 2
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(("IDENT", src[start:j], start, j))
            i = j
            continue

        if ch in ("b", "c") and i + 1 < n and src[i + 1] == '"':
            start = i
            j = i + 2
            while j < n:
                if src[j] == "\\":
                    j += 2
                elif src[j] == '"':
                    j += 1
                    break
                else:
                    j += 1
            tokens.append(("LIT", src[start:j], start, j))
            i = j
            continue

        if ch == "b" and i + 1 < n and src[i + 1] == "'":
            start = i
            j = i + 2
            if j < n and src[j] == "\\":
                j += 2
                while j < n and src[j] != "'":
                    j += 1
            elif j < n:
                j += 1
            if j < n and src[j] == "'":
                j += 1
            tokens.append(("LIT", src[start:j], start, j))
            i = j
            continue

        if ch == '"':
            start = i
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                elif src[j] == '"':
                    j += 1
                    break
                else:
                    j += 1
            tokens.append(("LIT", src[start:j], start, j))
            i = j
            continue

        if ch == "'":
            start = i
            if i + 2 < n and src[i + 1] == "\\":
                j = i + 3
                while j < n and src[j] != "'":
                    j += 1
                if j < n:
                    j += 1
                tokens.append(("LIT", src[start:j], start, j))
                i = j
                continue
            if i + 2 < n and src[i + 2] == "'" and src[i + 1] != "'":
                tokens.append(("LIT", src[start : i + 3], start, i + 3))
                i += 3
                continue
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            if j > i + 1:
                tokens.append(("LIFETIME", src[start:j], start, j))
                i = j
                continue
            tokens.append(("PUNCT", "'", start, i + 1))
            i += 1
            continue

        if ch == "$" and i + 1 < n and (src[i + 1].isalpha() or src[i + 1] == "_"):
            start = i
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(("MACRO_VAR", src[start:j], start, j))
            i = j
            continue

        if ch.isalpha() or ch == "_":
            start = i
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(("IDENT", src[start:j], start, j))
            i = j
            continue

        if ch.isdigit():
            start = i
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(("LIT", src[start:j], start, j))
            i = j
            continue

        if i + 2 < n and src[i : i + 3] == "..=":
            tokens.append(("PUNCT", "..=", i, i + 3))
            i += 3
            continue

        if i + 1 < n and src[i : i + 2] in ("::", "->", "=>", "..", "&&", "||", "==", "!=", "<=", ">="):
            tokens.append(("PUNCT", src[i : i + 2], i, i + 2))
            i += 2
            continue

        tokens.append(("PUNCT", ch, i, i + 1))
        i += 1

    return tokens


def skip_balanced(tokens: list[Token], idx: int, open_ch: str, close_ch: str) -> int:
    depth = 0
    n = len(tokens)
    while idx < n:
        t = tokens[idx][1]
        if t == open_ch:
            depth += 1
        elif t == close_ch:
            depth -= 1
            if depth == 0:
                return idx + 1
        idx += 1
    return n


def skip_angle_generics(tokens: list[Token], idx: int) -> int:
    if idx >= len(tokens) or tokens[idx][1] != "<":
        return idx
    depth = 0
    n = len(tokens)
    while idx < n:
        t = tokens[idx][1]
        if t == "<":
            depth += 1
        elif t == ">":
            depth -= 1
            if depth == 0:
                return idx + 1
        elif t == "(":
            idx = skip_balanced(tokens, idx, "(", ")")
            continue
        elif t == "[":
            idx = skip_balanced(tokens, idx, "[", "]")
            continue
        elif t in ("{", ";"):
            return idx
        idx += 1
    return n


def split_top_level(tokens: list[Token], sep: str = ",") -> list[list[Token]]:
    parts: list[list[Token]] = []
    cur: list[Token] = []
    da = dp = db = dc = 0
    for tok in tokens:
        v = tok[1]
        if v == "<":
            da += 1
        elif v == ">":
            da = max(0, da - 1)
        elif v == "(":
            dp += 1
        elif v == ")":
            dp = max(0, dp - 1)
        elif v == "[":
            db += 1
        elif v == "]":
            db = max(0, db - 1)
        elif v == "{":
            dc += 1
        elif v == "}":
            dc = max(0, dc - 1)
        elif v == sep and da == 0 and dp == 0 and db == 0 and dc == 0:
            parts.append(cur)
            cur = []
            continue
        cur.append(tok)
    if cur:
        parts.append(cur)
    return parts


def split_top_level_no_angles(tokens: list[Token], sep: str) -> list[list[Token]]:
    """Split tokens by `sep` tracking `()`, `[]`, `{}` without treating `<`/`>` as delimiters."""
    parts: list[list[Token]] = []
    cur: list[Token] = []
    dp = db = dc = 0
    for tok in tokens:
        v = tok[1]
        if v == "(":
            dp += 1
        elif v == ")":
            dp = max(0, dp - 1)
        elif v == "[":
            db += 1
        elif v == "]":
            db = max(0, db - 1)
        elif v == "{":
            dc += 1
        elif v == "}":
            dc = max(0, dc - 1)
        elif v == sep and dp == 0 and db == 0 and dc == 0:
            parts.append(cur)
            cur = []
            continue
        cur.append(tok)
    if cur:
        parts.append(cur)
    return parts


def find_top_level_token(tokens: list[Token], target: str, track_angles: bool = False) -> int | None:
    da = dp = db = dc = 0
    for idx, tok in enumerate(tokens):
        v = tok[1]
        if track_angles and v == "<":
            da += 1
        elif track_angles and v == ">":
            da = max(0, da - 1)
        elif v == "(":
            dp += 1
        elif v == ")":
            dp = max(0, dp - 1)
        elif v == "[":
            db += 1
        elif v == "]":
            db = max(0, db - 1)
        elif v == "{":
            dc += 1
        elif v == "}":
            dc = max(0, dc - 1)
        elif v == target and da == 0 and dp == 0 and db == 0 and dc == 0:
            return idx
    return None


def extract_pattern_bound_names(pat_toks: list[Token]) -> set[str]:
    """Return every local variable name introduced by a Rust pattern."""
    if not pat_toks:
        return set()

    # Or-patterns `p1 | p2` (including leading `|`)
    or_parts = split_top_level_no_angles(pat_toks, "|")
    if len(or_parts) > 1:
        names: set[str] = set()
        for part in or_parts:
            names.update(extract_pattern_bound_names(part))
        return names
    if pat_toks[0][1] == "|":
        return extract_pattern_bound_names(pat_toks[1:])

    # Strip leading `&`, `mut`, `box`, `ref`
    p = list(pat_toks)
    has_binding_mod = False
    while p:
        v = p[0][1]
        if v == "&":
            p.pop(0)
            if p and p[0][1] == "mut":
                p.pop(0)
        elif v == "box":
            p.pop(0)
        elif v in ("ref", "mut"):
            has_binding_mod = True
            p.pop(0)
        else:
            break
    if not p:
        return set()

    # `@` subpattern: `[ref] [mut] ident @ subpat`
    at_idx = find_top_level_token(p, "@")
    if at_idx is not None:
        names = set()
        left = p[:at_idx]
        if len(left) == 1 and left[0][0] == "IDENT" and left[0][1] != "_":
            names.add(left[0][1])
        names.update(extract_pattern_bound_names(p[at_idx + 1 :]))
        return names

    # Range pattern `a..=b`, `a..b`, `..=b`, `..`
    if find_top_level_token(p, "..") is not None or find_top_level_token(p, "..=") is not None:
        return set()

    # Tuple `(p1, p2, ...)` or parenthesised pattern
    if p[0][1] == "(" and p[-1][1] == ")" and skip_balanced(p, 0, "(", ")") == len(p):
        names = set()
        for elem in split_top_level_no_angles(p[1:-1], ","):
            names.update(extract_pattern_bound_names(elem))
        return names

    # Slice `[p1, p2, ...]`
    if p[0][1] == "[" and p[-1][1] == "]" and skip_balanced(p, 0, "[", "]") == len(p):
        names = set()
        for elem in split_top_level_no_angles(p[1:-1], ","):
            names.update(extract_pattern_bound_names(elem))
        return names

    # Struct pattern `Path { f1: p1, f2, .. }`
    if p[-1][1] == "}":
        # Find matching opening `{` for the final `}`
        dc = 0
        open_brace = None
        for idx in range(len(p) - 1, -1, -1):
            if p[idx][1] == "}":
                dc += 1
            elif p[idx][1] == "{":
                dc -= 1
                if dc == 0:
                    open_brace = idx
                    break
        if open_brace is not None:
            names = set()
            for field_pat in split_top_level_no_angles(p[open_brace + 1 : -1], ","):
                if not field_pat or (len(field_pat) == 1 and field_pat[0][1] == ".."):
                    continue
                colon = find_top_level_token(field_pat, ":")
                if colon is not None:
                    names.update(extract_pattern_bound_names(field_pat[colon + 1 :]))
                else:
                    # Shorthand `[ref] [mut] field`
                    q = 0
                    while q < len(field_pat) and field_pat[q][1] in ("ref", "mut"):
                        q += 1
                    if q == len(field_pat) - 1 and field_pat[q][0] == "IDENT" and field_pat[q][1] != "_":
                        names.add(field_pat[q][1])
            return names

    # Tuple-struct / enum-variant pattern `Path ( p1, p2, ... )`
    if p[-1][1] == ")":
        dp = 0
        open_paren = None
        for idx in range(len(p) - 1, -1, -1):
            if p[idx][1] == ")":
                dp += 1
            elif p[idx][1] == "(":
                dp -= 1
                if dp == 0:
                    open_paren = idx
                    break
        if open_paren is not None and open_paren > 0:
            names = set()
            for arg in split_top_level_no_angles(p[open_paren + 1 : -1], ","):
                names.update(extract_pattern_bound_names(arg))
            return names

    # Leaf identifier
    if len(p) == 1 and p[0][0] == "IDENT":
        ident = p[0][1]
        if ident in ("_", "true", "false", "self", "Self", "crate", "super"):
            return set()
        # Bare UpperCamelCase/ALL_CAPS without `mut`/`ref` is a unit variant or constant.
        raw_name = ident[2:] if ident.startswith("r#") else ident
        if not has_binding_mod and raw_name and raw_name[0].isupper():
            return set()
        return {ident}

    return set()


def extract_type_base_name(toks: list[Token]) -> str | None:
    if not toks:
        return None
    j = 0
    n = len(toks)
    while j < n and (toks[j][1] in ("&", "mut", "*", "const", "dyn") or toks[j][0] == "LIFETIME"):
        j += 1
    last_ident = None
    while j < n:
        if toks[j][0] == "IDENT":
            last_ident = toks[j][1]
            j += 1
            if j < n and toks[j][1] == "::":
                j += 1
                continue
            break
        elif toks[j][1] == "::":
            j += 1
        else:
            return None
    return last_ident


def is_exact_type_name(toks: list[Token], target_name: str) -> bool:
    """Return True if `toks` is `[& ['a] [mut]] [path::]TargetName[<...>]`."""
    j = 0
    n = len(toks)
    while j < n and (toks[j][1] in ("&", "mut", "*", "const") or toks[j][0] == "LIFETIME"):
        j += 1
    if j >= n:
        return False
    last_ident = None
    while j < n:
        k, v, _, _ = toks[j]
        if k == "IDENT":
            last_ident = v
            j += 1
            if j < n and toks[j][1] == "::":
                j += 1
                continue
            break
        elif v == "::":
            j += 1
        else:
            return False
    if last_ident != target_name:
        return False
    if j < n and toks[j][1] == "<":
        j = skip_angle_generics(toks, j)
    return j == n


def is_game_type_tokens(toks: list[Token], self_type: str | None = None) -> bool:
    if is_exact_type_name(toks, "Game"):
        return True
    if self_type == "Game" and is_exact_type_name(toks, "Self"):
        return True
    return False


def resolve_type_kind(
    toks: list[Token],
    wrapper_structs: set[str],
    self_type: str | None = None,
) -> str | None:
    if is_exact_type_name(toks, "Game"):
        return "Game"
    if self_type is not None and is_exact_type_name(toks, "Self"):
        if self_type == "Game":
            return "Game"
        if self_type in wrapper_structs:
            return self_type
    base = extract_type_base_name(toks)
    if base and base in wrapper_structs and is_exact_type_name(toks, base):
        return base
    return None


def unwrap_result_or_option(toks: list[Token]) -> list[Token]:
    """Peel nested `Result<T, ...>`, `Option<T>`, `Box<T>` wrappers."""
    cur = toks
    while True:
        j = 0
        n = len(cur)
        last_ident = None
        while j < n:
            if cur[j][0] == "IDENT":
                last_ident = cur[j][1]
                j += 1
                if j < n and cur[j][1] == "::":
                    j += 1
                    continue
                break
            elif cur[j][1] == "::":
                j += 1
            else:
                return cur
        if last_ident in ("Result", "Option", "Box") and j < n and cur[j][1] == "<":
            end = skip_angle_generics(cur, j)
            if end == n:
                args = split_top_level(cur[j + 1 : end - 1], ",")
                if args:
                    cur = args[0]
                    continue
        return cur


def extract_game_return_positions(ret_toks: list[Token], self_type: str | None = None) -> set[int | None]:
    """Return positions where a function's return type carries `Game`."""
    inner = unwrap_result_or_option(ret_toks)
    if is_game_type_tokens(inner, self_type):
        return {None}
    if len(inner) >= 2 and inner[0][1] == "(" and inner[-1][1] == ")":
        elems = split_top_level(inner[1:-1], ",")
        out: set[int | None] = set()
        for idx, elem in enumerate(elems):
            if is_game_type_tokens(unwrap_result_or_option(elem), self_type):
                out.add(idx)
        return out
    return set()


def parse_impl_target(tokens: list[Token], impl_idx: int) -> tuple[str | None, int] | None:
    n = len(tokens)
    j = impl_idx + 1
    if j < n and tokens[j][1] == "const":
        j += 1
    if j < n and tokens[j][1] == "<":
        j = skip_angle_generics(tokens, j)
    if j < n and tokens[j][1] == "!":
        j += 1
    start = j
    for_idx = None
    where_idx = None
    angle = paren = bracket = 0
    while j < n:
        v = tokens[j][1]
        if v == "<":
            angle += 1
        elif v == ">":
            angle = max(0, angle - 1)
        elif v == "(":
            paren += 1
        elif v == ")":
            paren = max(0, paren - 1)
        elif v == "[":
            bracket += 1
        elif v == "]":
            bracket = max(0, bracket - 1)
        elif angle == 0 and paren == 0 and bracket == 0:
            if v == "for" and for_idx is None:
                for_idx = j
            elif v == "where" and where_idx is None:
                where_idx = j
            elif v == "{":
                type_start = (for_idx + 1) if for_idx is not None else start
                type_end = where_idx if where_idx is not None else j
                target = extract_type_base_name(tokens[type_start:type_end])
                return target, j
            elif v == ";":
                return None
        j += 1
    return None


def is_pub_before_fn(tokens: list[Token], fn_idx: int) -> bool:
    """Return True if `fn` at `fn_idx` has a `pub` or `pub(...)` visibility prefix."""
    k = fn_idx - 1
    while k >= 0:
        v = tokens[k][1]
        if v in ("async", "unsafe", "const", "default"):
            k -= 1
            continue
        if tokens[k][0] == "LIT" and k > 0 and tokens[k - 1][1] == "extern":
            k -= 2
            continue
        if v == "extern":
            k -= 1
            continue
        if v == ")":
            depth = 1
            k -= 1
            while k >= 0 and depth > 0:
                if tokens[k][1] == ")":
                    depth += 1
                elif tokens[k][1] == "(":
                    depth -= 1
                k -= 1
            return k >= 0 and tokens[k][1] == "pub"
        return v == "pub"
    return False


def file_defines_local_mock_game(tokens: list[Token], rel_posix: str) -> bool:
    if rel_posix.endswith("crates/sunrise-client/src/game.rs"):
        return False
    n = len(tokens)
    for i in range(n - 1):
        if tokens[i][0] == "IDENT" and tokens[i][1] == "struct" and tokens[i + 1][0] == "IDENT" and tokens[i + 1][1] == "Game":
            return True
    return False


def scan_structs(tokens: list[Token]) -> tuple[set[str], dict[str, set[str]]]:
    """Return `(all_defined_structs, struct_name -> set of fields holding Game)`."""
    defined: set[str] = set()
    game_fields: dict[str, set[str]] = {}
    n = len(tokens)
    i = 0
    while i < n:
        if tokens[i][0] == "IDENT" and tokens[i][1] == "struct" and i + 1 < n and tokens[i + 1][0] == "IDENT":
            sname = tokens[i + 1][1]
            defined.add(sname)
            j = i + 2
            if j < n and tokens[j][1] == "<":
                j = skip_angle_generics(tokens, j)
            while j < n and tokens[j][1] not in ("{", ";", "("):
                j += 1
            if j < n and tokens[j][1] == "{":
                end = skip_balanced(tokens, j, "{", "}")
                for part in split_top_level(tokens[j + 1 : end - 1], ","):
                    k = 0
                    while k < len(part):
                        if part[k][1] == "#" and k + 1 < len(part) and part[k + 1][1] == "[":
                            k = skip_balanced(part, k + 1, "[", "]")
                        elif part[k][1] == "pub":
                            k += 1
                            if k < len(part) and part[k][1] == "(":
                                k = skip_balanced(part, k, "(", ")")
                        else:
                            break
                    if k + 1 < len(part) and part[k][0] == "IDENT" and part[k + 1][1] == ":":
                        fname = part[k][1]
                        if is_game_type_tokens(part[k + 2 :]):
                            game_fields.setdefault(sname, set()).add(fname)
                i = end
                continue
            elif j < n and tokens[j][1] == "(":
                end = skip_balanced(tokens, j, "(", ")")
                for tidx, part in enumerate(split_top_level(tokens[j + 1 : end - 1], ",")):
                    k = 0
                    while k < len(part):
                        if part[k][1] == "#" and k + 1 < len(part) and part[k + 1][1] == "[":
                            k = skip_balanced(part, k + 1, "[", "]")
                        elif part[k][1] == "pub":
                            k += 1
                            if k < len(part) and part[k][1] == "(":
                                k = skip_balanced(part, k, "(", ")")
                        else:
                            break
                    if is_game_type_tokens(part[k:]):
                        game_fields.setdefault(sname, set()).add(str(tidx))
                i = end
                continue
        i += 1
    return defined, game_fields


def extract_fn_trait_params(
    ty_toks: list[Token],
    wrapper_structs: set[str],
    self_type: str | None,
) -> dict[int, str]:
    """Extract `{closure_param_idx: kind}` from `FnOnce(...)` / `FnMut(...)` / `Fn(...)` / `fn(...)`."""
    found: dict[int, str] = {}
    m = len(ty_toks)
    for idx, tok in enumerate(ty_toks):
        if tok[0] == "IDENT" and tok[1] in ("FnOnce", "FnMut", "Fn", "fn") and idx + 1 < m and ty_toks[idx + 1][1] == "(":
            cend = skip_balanced(ty_toks, idx + 1, "(", ")")
            cargs = split_top_level(ty_toks[idx + 2 : cend - 1], ",")
            for pidx, arg in enumerate(cargs):
                kind = resolve_type_kind(arg, wrapper_structs, self_type)
                if kind:
                    found[pidx] = kind
    return found


def scan_fn_signatures(
    tokens: list[Token],
    wrapper_structs: set[str],
):
    """Yield `(fname, is_pub, impl_target, has_self, non_self_param_count, ret_positions, arg_closure_map)`."""
    n = len(tokens)
    impl_opens: dict[int, str | None] = {}
    for i in range(n):
        if tokens[i][0] == "IDENT" and tokens[i][1] == "impl":
            prev = tokens[i - 1][1] if i > 0 else None
            if prev in (None, ";", "{", "}", "]", "unsafe", "default"):
                parsed = parse_impl_target(tokens, i)
                if parsed is not None:
                    target, brace_idx = parsed
                    impl_opens[brace_idx] = target

    brace_depth = 0
    impl_stack: list[tuple[int, str | None]] = []

    i = 0
    while i < n:
        tv = tokens[i][1]
        if tv == "{":
            brace_depth += 1
            if i in impl_opens:
                impl_stack.append((brace_depth, impl_opens[i]))
            i += 1
            continue
        if tv == "}":
            while impl_stack and impl_stack[-1][0] >= brace_depth:
                impl_stack.pop()
            brace_depth = max(0, brace_depth - 1)
            i += 1
            continue

        if tokens[i][0] == "IDENT" and tv == "fn" and i + 1 < n and tokens[i + 1][0] == "IDENT":
            prev = tokens[i - 1][1] if i > 0 else None
            if prev in (":", "->", "<", ",", "(", "dyn", "impl", "+"):
                i += 1
                continue
            fname = tokens[i + 1][1]
            is_pub = is_pub_before_fn(tokens, i)
            impl_target = impl_stack[-1][1] if impl_stack else None

            j = i + 2
            gen_toks: list[Token] = []
            if j < n and tokens[j][1] == "<":
                gend = skip_angle_generics(tokens, j)
                gen_toks = tokens[j + 1 : gend - 1]
                j = gend
            if j >= n or tokens[j][1] != "(":
                i += 1
                continue
            pend = skip_balanced(tokens, j, "(", ")")
            param_toks = tokens[j + 1 : pend - 1]

            k = pend
            ret_toks: list[Token] = []
            where_toks: list[Token] = []
            if k < n and tokens[k][1] == "->":
                k += 1
                rs = k
                da = dp = db = 0
                while k < n:
                    tkv = tokens[k][1]
                    if tkv == "<":
                        da += 1
                    elif tkv == ">":
                        da = max(0, da - 1)
                    elif tkv == "(":
                        dp += 1
                    elif tkv == ")":
                        dp = max(0, dp - 1)
                    elif tkv == "[":
                        db += 1
                    elif tkv == "]":
                        db = max(0, db - 1)
                    elif tkv in ("{", ";", "where") and da == 0 and dp == 0 and db == 0:
                        break
                    k += 1
                ret_toks = tokens[rs:k]
            if k < n and tokens[k][1] == "where":
                ws = k + 1
                while k < n and tokens[k][1] not in ("{", ";"):
                    k += 1
                where_toks = tokens[ws:k]

            params = split_top_level(param_toks, ",")
            has_self = bool(params) and any(t[1] == "self" for t in params[0])
            non_self_params = params[1:] if has_self else params

            ret_positions = extract_game_return_positions(ret_toks, impl_target) if ret_toks else set()

            # Build generic type parameter -> closure param types from `<...>` and `where ...`
            generic_bounds: dict[str, dict[int, str]] = {}
            for gpart in split_top_level(gen_toks, ","):
                colon = next((idx_p for idx_p, tk in enumerate(gpart) if tk[1] == ":"), None)
                if colon is not None and colon >= 1 and gpart[0][0] == "IDENT":
                    cparams = extract_fn_trait_params(gpart[colon + 1 :], wrapper_structs, impl_target)
                    if cparams:
                        generic_bounds.setdefault(gpart[0][1], {}).update(cparams)
            for wpart in split_top_level(where_toks, ","):
                colon = next((idx_p for idx_p, tk in enumerate(wpart) if tk[1] == ":"), None)
                if colon is not None:
                    gname = extract_type_base_name(wpart[:colon])
                    if gname:
                        cparams = extract_fn_trait_params(wpart[colon + 1 :], wrapper_structs, impl_target)
                        if cparams:
                            generic_bounds.setdefault(gname, {}).update(cparams)

            arg_closure_map: ClosureArgMap = {}
            for arg_idx, p_slice in enumerate(non_self_params):
                colon = next((idx_p for idx_p, tk in enumerate(p_slice) if tk[1] == ":"), None)
                if colon is None:
                    continue
                ty = p_slice[colon + 1 :]
                cparams = extract_fn_trait_params(ty, wrapper_structs, impl_target)
                if not cparams:
                    gbase = extract_type_base_name(ty)
                    if gbase and gbase in generic_bounds:
                        cparams = dict(generic_bounds[gbase])
                if cparams:
                    arg_closure_map[arg_idx] = cparams

            yield fname, is_pub, impl_target, has_self, len(non_self_params), ret_positions, arg_closure_map
            i = k
            continue
        i += 1


def merge_closure_map(dst: ClosureArgMap, src: ClosureArgMap) -> None:
    for arg_idx, pmap in src.items():
        entry = dst.setdefault(arg_idx, {})
        for pidx, kind in pmap.items():
            entry.setdefault(pidx, kind)


def build_workspace_index(extra_files: list[Path]) -> WorkspaceIndex:
    """Scan game sources once to discover structs, helpers, and closures carrying `Game`."""
    index = WorkspaceIndex()
    scan_paths: list[Path] = []
    seen_paths: set[Path] = set()
    for p in list(DEFAULT_PATHS) + extra_files:
        full = p if p.is_absolute() else (ROOT / p)
        if full.is_file() and full.suffix == ".rs":
            rp = full.resolve()
            if rp not in seen_paths:
                seen_paths.add(rp)
                scan_paths.append(rp)
        elif full.is_dir():
            for child in sorted(full.rglob("*.rs")):
                rp = child.resolve()
                if rp not in seen_paths:
                    seen_paths.add(rp)
                    scan_paths.append(rp)

    tokenized: list[list[Token]] = []
    for path in scan_paths:
        try:
            rel_posix = path.relative_to(ROOT.resolve()).as_posix()
        except ValueError:
            rel_posix = path.as_posix()
        src = path.read_text(encoding="utf-8")
        if "Game" not in src:
            continue
        toks = tokenize_rust(src)
        if file_defines_local_mock_game(toks, rel_posix):
            continue
        _, sfields = scan_structs(toks)
        for sname, fset in sfields.items():
            index.struct_game_fields.setdefault(sname, set()).update(fset)
        tokenized.append(toks)

    wrapper_structs = set(index.struct_game_fields)

    for toks in tokenized:
        for fname, is_pub, impl_target, has_self, non_self_count, ret_positions, arg_closure_map in scan_fn_signatures(
            toks, wrapper_structs
        ):
            if has_self:
                if ret_positions and None in ret_positions and non_self_count == 0 and impl_target != "Game":
                    index.game_returning_methods.add(fname)
                    if impl_target:
                        index.wrapper_game_returning_methods.setdefault(impl_target, set()).add(fname)
                if arg_closure_map:
                    if impl_target == "Game":
                        merge_closure_map(index.game_method_closure_params.setdefault(fname, {}), arg_closure_map)
                    elif impl_target and impl_target in wrapper_structs:
                        wmap = index.wrapper_method_closure_params.setdefault(impl_target, {})
                        merge_closure_map(wmap.setdefault(fname, {}), arg_closure_map)
            else:
                if impl_target is not None:
                    continue
                if ret_positions:
                    target_ret = index.pub_game_returning_fns if is_pub else index.priv_game_returning_fns
                    target_ret.setdefault(fname, set()).update(ret_positions)
                else:
                    index.non_game_return_free_fns.add(fname)
                if arg_closure_map:
                    target_cl = index.pub_fn_closure_params if is_pub else index.priv_fn_closure_params
                    merge_closure_map(target_cl.setdefault(fname, {}), arg_closure_map)
                else:
                    index.non_game_closure_free_fns.add(fname)

    for noisy in NOISY_GLOBAL_FNS:
        index.pub_game_returning_fns.pop(noisy, None)
        index.priv_game_returning_fns.pop(noisy, None)
        index.pub_fn_closure_params.pop(noisy, None)
        index.priv_fn_closure_params.pop(noisy, None)

    # Record collisions between Game-typed and non-Game free fns before pruning privates.
    for colliding in index.non_game_return_free_fns:
        if colliding in index.priv_game_returning_fns:
            index.ambiguous_return_fns.add(colliding)
        index.priv_game_returning_fns.pop(colliding, None)
    for colliding in index.non_game_closure_free_fns:
        if colliding in index.priv_fn_closure_params:
            index.ambiguous_closure_fns.add(colliding)
        index.priv_fn_closure_params.pop(colliding, None)

    return index


def build_file_local_index(tokens: list[Token], global_wrapper_structs: set[str]) -> FileLocalIndex:
    local = FileLocalIndex()
    local.defined_structs, local.struct_game_fields = scan_structs(tokens)
    wrapper_structs = (global_wrapper_structs - local.defined_structs) | set(local.struct_game_fields)

    for fname, _is_pub, impl_target, has_self, _non_self_count, ret_positions, arg_closure_map in scan_fn_signatures(
        tokens, wrapper_structs
    ):
        if has_self or impl_target is not None:
            continue
        local.defined_free_fns.add(fname)
        if ret_positions:
            local.game_returning_fns.setdefault(fname, set()).update(ret_positions)
        if arg_closure_map:
            merge_closure_map(local.fn_closure_params.setdefault(fname, {}), arg_closure_map)
    return local


def parse_fn_typed_params(
    tokens: list[Token],
    fn_idx: int,
    wrapper_structs: set[str],
    self_type: str | None,
) -> tuple[dict[str, str], int] | None:
    """Given index of `fn`, return `(dict[param_name -> kind | ''], open_brace_idx)`."""
    n = len(tokens)
    j = fn_idx + 1
    if j >= n or tokens[j][0] != "IDENT":
        return None
    j += 1
    if j < n and tokens[j][1] == "<":
        j = skip_angle_generics(tokens, j)
    if j >= n or tokens[j][1] != "(":
        return None
    params_end = skip_balanced(tokens, j, "(", ")")
    typed_params: dict[str, str] = {}
    for p_slice in split_top_level(tokens[j + 1 : params_end - 1], ","):
        colon = find_top_level_token(p_slice, ":")
        if colon is None:
            continue
        pat = p_slice[:colon]
        ty = p_slice[colon + 1 :]
        simple_name = parse_simple_pat_ident(pat)
        if simple_name and simple_name != "self":
            kind = resolve_type_kind(ty, wrapper_structs, self_type)
            typed_params[simple_name] = kind if kind is not None else NON_GAME
        else:
            # Destructured parameters shadow any outer tracked names.
            for bname in extract_pattern_bound_names(pat):
                typed_params[bname] = NON_GAME
    m = params_end
    while m < n:
        tv = tokens[m][1]
        if tv == "{":
            return typed_params, m
        if tv == ";":
            return None
        if tv == "(":
            m = skip_balanced(tokens, m, "(", ")")
            continue
        if tv == "[":
            m = skip_balanced(tokens, m, "[", "]")
            continue
        m += 1
    return None


def compute_line_starts(src: str) -> list[int]:
    starts = [0]
    pos = src.find("\n")
    while pos != -1:
        starts.append(pos + 1)
        pos = src.find("\n", pos + 1)
    return starts


def offset_to_line(line_starts: list[int], offset: int) -> int:
    lo, hi = 0, len(line_starts) - 1
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if line_starts[mid] <= offset:
            lo = mid
        else:
            hi = mid - 1
    return lo + 1


def find_enclosing_call_and_arg(tokens: list[Token], bar_idx: int) -> tuple[str, int, bool, int | None] | None:
    """Walk backwards from closure `|` to find `(call_name, arg_idx, is_method, dot_idx)`."""
    b = bar_idx - 1
    while b >= 0 and tokens[b][1] in ("move", "mut", "&", "async", "static"):
        b -= 1
    if b < 0 or tokens[b][1] not in ("(", ","):
        return None
    dp = db = dc = 0
    arg_idx = 0
    while b >= 1:
        tv = tokens[b][1]
        if tv == ")":
            dp += 1
        elif tv == "]":
            db += 1
        elif tv == "}":
            dc += 1
        elif tv == "[":
            if db == 0:
                return None
            db -= 1
        elif tv == "{":
            if dc == 0:
                return None
            dc -= 1
        elif tv == ";" and dp == 0 and db == 0 and dc == 0:
            return None
        elif tv == "," and dp == 0 and db == 0 and dc == 0:
            arg_idx += 1
        elif tv == "|" and dp == 0 and db == 0 and dc == 0:
            # Skip backwards over a prior closure's `|a, b|` parameter list so its commas don't inflate `arg_idx`.
            q = b - 1
            qdp = qdb = qda = 0
            matched_open = None
            while q >= 0 and b - q <= 80:
                qv = tokens[q][1]
                if qv == ")":
                    qdp += 1
                elif qv == "(":
                    if qdp == 0:
                        break
                    qdp -= 1
                elif qv == "]":
                    qdb += 1
                elif qv == "[":
                    if qdb == 0:
                        break
                    qdb -= 1
                elif qv == ">":
                    qda += 1
                elif qv == "<":
                    qda = max(0, qda - 1)
                elif qv in ("{", "}", ";", "=", "=>") and qdp == 0 and qdb == 0 and qda == 0:
                    break
                elif qv == "|" and qdp == 0 and qdb == 0 and qda == 0:
                    matched_open = q
                    break
                q -= 1
            if matched_open is not None:
                b = matched_open
        elif tv == "(":
            if dp > 0:
                dp -= 1
            elif db == 0 and dc == 0:
                name_idx = None
                if tokens[b - 1][0] == "IDENT":
                    name_idx = b - 1
                elif tokens[b - 1][1] == ">":
                    # Turbofish `foo::<T>(|g| ...)`
                    q = b - 1
                    ad = 0
                    while q >= 0:
                        if tokens[q][1] == ">":
                            ad += 1
                        elif tokens[q][1] == "<":
                            ad -= 1
                            if ad == 0:
                                if q >= 2 and tokens[q - 1][1] == "::" and tokens[q - 2][0] == "IDENT":
                                    name_idx = q - 2
                                break
                        q -= 1
                if name_idx is None:
                    return None
                call_name = tokens[name_idx][1]
                if name_idx > 0 and tokens[name_idx - 1][1] == ".":
                    return call_name, arg_idx, True, name_idx - 1
                return call_name, arg_idx, False, None
            else:
                return None
        b -= 1
    return None


def rhs_preserves_game_after_call(tokens: list[Token], call_open_paren: int) -> bool:
    """Return True if the tokens after `func(...)` only unwrap/borrow the returned `Game`."""
    n = len(tokens)
    k = skip_balanced(tokens, call_open_paren, "(", ")")
    while k < n:
        v = tokens[k][1]
        if v == "?":
            k += 1
            continue
        if v == ".":
            if k + 2 < n and tokens[k + 1][0] == "IDENT" and tokens[k + 1][1] in PASSTHROUGH_METHODS and tokens[k + 2][1] == "(":
                k = skip_balanced(tokens, k + 2, "(", ")")
                continue
            return False
        if v in (";", ",", ")", "}", "{", "else", "&&", "||"):
            return True
        return False
    return True


def parse_simple_pat_ident(toks: list[Token]) -> str | None:
    if len(toks) == 1 and toks[0][0] in ("IDENT", "MACRO_VAR") and toks[0][1] != "_":
        return toks[0][1]
    if len(toks) == 2 and toks[0][1] == "mut" and toks[1][0] in ("IDENT", "MACRO_VAR") and toks[1][1] != "_":
        return toks[1][1]
    if len(toks) == 2 and toks[0][1] == "ref" and toks[1][0] in ("IDENT", "MACRO_VAR") and toks[1][1] != "_":
        return toks[1][1]
    if (
        len(toks) == 3
        and toks[0][1] == "ref"
        and toks[1][1] == "mut"
        and toks[2][0] in ("IDENT", "MACRO_VAR")
        and toks[2][1] != "_"
    ):
        return toks[2][1]
    return None


def find_match_arms(
    tokens: list[Token],
    match_brace: int,
    match_end: int,
) -> list[tuple[int, int, int | None, int, int, list[Token]]]:
    """Parse arms inside `match ... { ... }`.

    Returns `(pat_start, pat_end, guard_start_or_None, arrow_idx, arm_end, pat_toks)`.
    """
    arms: list[tuple[int, int, int | None, int, int, list[Token]]] = []
    pos = match_brace + 1
    limit = match_end - 1
    while pos < limit:
        # Skip leading attributes `#[...]` on the arm.
        while pos + 1 < limit and tokens[pos][1] == "#" and tokens[pos + 1][1] == "[":
            pos = skip_balanced(tokens, pos + 1, "[", "]")
        if pos >= limit:
            break
        pat_start = pos
        guard_idx: int | None = None
        arrow_idx: int | None = None
        dp = db = dc = 0
        k = pos
        while k < limit:
            tv = tokens[k][1]
            if tv == "(":
                dp += 1
            elif tv == ")":
                dp = max(0, dp - 1)
            elif tv == "[":
                db += 1
            elif tv == "]":
                db = max(0, db - 1)
            elif tv == "{":
                dc += 1
            elif tv == "}":
                dc = max(0, dc - 1)
            elif dp == 0 and db == 0 and dc == 0:
                if tv == "if" and guard_idx is None:
                    guard_idx = k
                elif tv == "=>":
                    arrow_idx = k
                    break
            k += 1
        if arrow_idx is None:
            break
        pat_end = guard_idx if guard_idx is not None else arrow_idx
        pat_toks = tokens[pat_start:pat_end]
        guard_start = (guard_idx + 1) if guard_idx is not None else None

        # Find end of arm body after `=>`.
        body_start = arrow_idx + 1
        if body_start >= limit:
            break
        if tokens[body_start][1] == "{":
            blk_end = skip_balanced(tokens, body_start, "{", "}")
            # A block-bodied arm ends immediately unless chained with `.`/`?`/operators.
            if blk_end >= limit or tokens[blk_end][1] == "," or tokens[blk_end][1] not in (".", "?", "+", "-", "*", "/", "%", "&", "|", "^", "&&", "||", "==", "!=", "<", ">", "<=", ">=", "as", "[", "("):
                arm_end = blk_end
                next_pos = blk_end + 1 if (blk_end < limit and tokens[blk_end][1] == ",") else blk_end
            else:
                k = blk_end
                dp = db = dc = 0
                while k < limit:
                    tv = tokens[k][1]
                    if tv == "(":
                        dp += 1
                    elif tv == ")":
                        dp = max(0, dp - 1)
                    elif tv == "[":
                        db += 1
                    elif tv == "]":
                        db = max(0, db - 1)
                    elif tv == "{":
                        dc += 1
                    elif tv == "}":
                        dc = max(0, dc - 1)
                    elif tv == "," and dp == 0 and db == 0 and dc == 0:
                        break
                    k += 1
                arm_end = k
                next_pos = k + 1 if (k < limit and tokens[k][1] == ",") else k
        else:
            k = body_start
            dp = db = dc = 0
            while k < limit:
                tv = tokens[k][1]
                if tv == "(":
                    dp += 1
                elif tv == ")":
                    dp = max(0, dp - 1)
                elif tv == "[":
                    db += 1
                elif tv == "]":
                    db = max(0, db - 1)
                elif tv == "{":
                    dc += 1
                elif tv == "}":
                    dc = max(0, dc - 1)
                elif tv == "," and dp == 0 and db == 0 and dc == 0:
                    break
                k += 1
            arm_end = k
            next_pos = k + 1 if (k < limit and tokens[k][1] == ",") else k

        arms.append((pat_start, pat_end, guard_start, arrow_idx, arm_end, pat_toks))
        pos = next_pos
    return arms


def scan_file(
    src: str,
    rel_posix: str,
    owners: dict[str, str],
    index: WorkspaceIndex,
) -> tuple[list[Edit], list[AmbiguousSite]]:
    """Scan one Rust file and return `(sorted_edits, sorted_ambiguous_sites)`."""
    tokens = tokenize_rust(src)
    if file_defines_local_mock_game(tokens, rel_posix):
        return [], []

    local = build_file_local_index(tokens, set(index.struct_game_fields))
    struct_game_fields: dict[str, set[str]] = dict(index.struct_game_fields)
    for sname in local.defined_structs:
        if sname in local.struct_game_fields:
            struct_game_fields[sname] = local.struct_game_fields[sname]
        else:
            struct_game_fields.pop(sname, None)
    wrapper_structs = set(struct_game_fields)

    def lookup_free_fn_returns(fname: str) -> tuple[set[int | None] | None, bool]:
        if fname in local.defined_free_fns:
            return local.game_returning_fns.get(fname), False
        if fname in index.ambiguous_return_fns and fname not in index.pub_game_returning_fns:
            return None, True
        res = index.pub_game_returning_fns.get(fname) or index.priv_game_returning_fns.get(fname)
        return res, False

    def lookup_free_fn_closures(fname: str) -> tuple[ClosureArgMap | None, bool]:
        if fname in local.defined_free_fns:
            return local.fn_closure_params.get(fname), False
        if fname in index.ambiguous_closure_fns and fname not in index.pub_fn_closure_params:
            return None, True
        res = index.pub_fn_closure_params.get(fname) or index.priv_fn_closure_params.get(fname)
        return res, False

    line_starts = compute_line_starts(src)
    n = len(tokens)
    edits: dict[tuple[int, int], Edit] = {}
    ambiguous: dict[int, AmbiguousSite] = {}

    impl_opens: dict[int, str | None] = {}
    impl_header_ranges: list[tuple[int, int]] = []
    fn_opens: dict[int, dict[str, str]] = {}
    fn_item_braces: set[int] = set()
    macro_opens: dict[int, set[str]] = {}

    # Pre-scan `impl` and `macro_rules!` blocks so `fn` scanning knows `impl_target` (for `Self`).
    for i in range(n):
        kind, text, _, _ = tokens[i]
        if kind != "IDENT":
            continue
        if text == "impl":
            prev = tokens[i - 1][1] if i > 0 else None
            if prev in (None, ";", "{", "}", "]", "unsafe", "default"):
                parsed = parse_impl_target(tokens, i)
                if parsed is not None:
                    target, brace_idx = parsed
                    impl_opens[brace_idx] = target
                    impl_header_ranges.append((i, brace_idx))
        elif text == "macro_rules" and i + 3 < n and tokens[i + 1][1] == "!" and tokens[i + 2][0] == "IDENT":
            mname = tokens[i + 2][1]
            if mname in GAME_MACROS and tokens[i + 3][1] == "{":
                macro_opens[i + 3] = GAME_MACROS[mname]

    # Pre-scan `fn` blocks with active `impl` target so `g: &mut Self` inside `impl Game` works.
    pre_depth = 0
    pre_impl: list[tuple[int, str | None]] = []
    for i in range(n):
        tv = tokens[i][1]
        if tv == "{":
            pre_depth += 1
            if i in impl_opens:
                pre_impl.append((pre_depth, impl_opens[i]))
        elif tv == "}":
            while pre_impl and pre_impl[-1][0] >= pre_depth:
                pre_impl.pop()
            pre_depth = max(0, pre_depth - 1)
        elif tokens[i][0] == "IDENT" and tv == "fn":
            prev = tokens[i - 1][1] if i > 0 else None
            if prev not in (":", "->", "<", ",", "(", "dyn", "impl", "+"):
                cur_self = pre_impl[-1][1] if pre_impl else None
                parsed = parse_fn_typed_params(tokens, i, wrapper_structs, cur_self)
                if parsed is not None:
                    tparams, brace_idx = parsed
                    fn_item_braces.add(brace_idx)
                    if tparams:
                        fn_opens.setdefault(brace_idx, {}).update(tparams)

    def in_impl_header(idx: int) -> bool:
        return any(s <= idx < e for s, e in impl_header_ranges)

    brace_depth = 0
    impl_stack: list[tuple[int, str | None]] = []
    # var_name -> "Game" | WrapperStructName | "" (shadowed non-Game) | "?" (ambiguous)
    # `is_fn_barrier` stops outer local variable capture across nested `fn` items.
    var_scopes: list[tuple[int, dict[str, str], bool]] = [(0, {}, False)]
    expr_closure_scopes: list[tuple[int, int, int, dict[str, str]]] = []
    # Range-scoped bindings for `match` arm guards/bodies and `if let ... && ...` chains: `(end_idx, vars)`
    range_scopes: list[tuple[int, dict[str, str]]] = []
    # Scheduled range scope activations keyed by token index: `start_idx -> list[(end_idx, vars)]`
    scheduled_ranges: dict[int, list[tuple[int, dict[str, str]]]] = {}
    # Deferred `let` statement bindings applied at the statement's terminating `;`
    deferred_let_bindings: dict[int, dict[str, str]] = {}
    # Token index ranges inside `match` arm patterns so `|` is not parsed as a closure
    match_pat_ranges: list[tuple[int, int]] = []
    macro_stack: list[tuple[int, set[str]]] = []
    # Active unknown macro invocations `(end_idx, macro_name)`
    unknown_macro_stack: list[tuple[int, str]] = []

    paren_depth = 0
    bracket_depth = 0

    def active_impl_target() -> str | None:
        return impl_stack[-1][1] if impl_stack else None

    def var_type(name: str) -> str | None:
        for _, rvars in reversed(range_scopes):
            if name in rvars:
                return rvars[name] or None
        for _, _, _, cvars in reversed(expr_closure_scopes):
            if name in cvars:
                return cvars[name] or None
        for _, scope, is_barrier in reversed(var_scopes):
            if name in scope:
                return scope[name] or None
            if is_barrier:
                break
        return None

    def is_game_macro_var(name: str) -> bool:
        return any(name in mvars for _, mvars in macro_stack)

    def in_match_pattern(idx: int) -> bool:
        return any(s <= idx < e for s, e in match_pat_ranges)

    def record_access(dot_idx: int, recv_kind: str, reason: str) -> None:
        """Check `.field` or `.server.field` / `.client.field` after a Game or ambiguous receiver."""
        if dot_idx + 1 >= n or tokens[dot_idx + 1][0] != "IDENT":
            return
        first_idx = dot_idx + 1
        first_name = tokens[first_idx][1]

        # Case 1: already tail-qualified `<recv>.server.<field>` or `<recv>.client.<field>`
        if (
            first_name in ("server", "client")
            and first_idx + 2 < n
            and tokens[first_idx + 1][1] == "."
            and tokens[first_idx + 2][0] == "IDENT"
        ):
            field_idx = first_idx + 2
            fname = tokens[field_idx][1]
            expected = owners.get(fname)
            if expected is None:
                return
            nxt = tokens[field_idx + 1][1] if field_idx + 1 < n else None
            if nxt in ("(", "::"):
                return
            if expected == first_name:
                return
            if recv_kind == AMBIGUOUS:
                off = tokens[first_idx][2]
                ambiguous.setdefault(off, AmbiguousSite(off, fname, reason))
                return
            if unknown_macro_stack:
                off = tokens[first_idx][2]
                mac_name = unknown_macro_stack[-1][1]
                ambiguous.setdefault(off, AmbiguousSite(off, fname, f"inside unknown macro `{mac_name}!`"))
                return
            if expected == "game":
                span = (tokens[first_idx][2], tokens[field_idx][2])
                edits.setdefault(
                    span,
                    Edit(span[0], span[1], "", fname, f"{reason}:unmove", f"{first_name}.{fname}", fname),
                )
            else:
                span = (tokens[first_idx][2], tokens[first_idx][3])
                edits.setdefault(
                    span,
                    Edit(span[0], span[1], expected, fname, f"{reason}:retail", f"{first_name}.{fname}", f"{expected}.{fname}"),
                )
            return

        # Case 2: direct `<recv>.<field>`
        expected = owners.get(first_name)
        if expected not in ("server", "client"):
            return
        nxt = tokens[first_idx + 1][1] if first_idx + 1 < n else None
        if nxt in ("(", "::"):
            return
        if recv_kind == AMBIGUOUS:
            off = tokens[first_idx][2]
            ambiguous.setdefault(off, AmbiguousSite(off, first_name, reason))
            return
        if unknown_macro_stack:
            off = tokens[first_idx][2]
            mac_name = unknown_macro_stack[-1][1]
            ambiguous.setdefault(off, AmbiguousSite(off, first_name, f"inside unknown macro `{mac_name}!`"))
            return
        off = tokens[first_idx][2]
        span = (off, off)
        edits.setdefault(
            span,
            Edit(off, off, f"{expected}.", first_name, reason, first_name, f"{expected}.{first_name}"),
        )

    def receiver_kind_before_dot(dot_idx: int) -> tuple[str | None, str | None]:
        """Classify the receiver ending at `dot_idx - 1`. Returns `(kind, edit_reason)`."""
        if dot_idx <= 0:
            return None, None
        prev_kind, prev_text, _, _ = tokens[dot_idx - 1]
        prev2 = tokens[dot_idx - 2][1] if dot_idx >= 2 else None

        # `self.<...>`
        if prev_text == "self" and prev2 not in (".", "::"):
            t = active_impl_target()
            if t == "Game":
                return "Game", "impl-game-self"
            if t and t in wrapper_structs:
                return t, None
            return None, None

        # `(*self).<...>` or `(*var).<...>`
        if (
            prev_text == ")"
            and dot_idx >= 4
            and tokens[dot_idx - 3][1] == "*"
            and tokens[dot_idx - 4][1] == "("
        ):
            inner_tok = tokens[dot_idx - 2]
            if inner_tok[1] == "self":
                t = active_impl_target()
                if t == "Game":
                    return "Game", "impl-game-deref-self"
                if t and t in wrapper_structs:
                    return t, None
                return None, None
            if inner_tok[0] in ("IDENT", "MACRO_VAR"):
                vt = var_type(inner_tok[1])
                if vt == "Game":
                    return "Game", "typed-game-deref-var"
                if vt == AMBIGUOUS:
                    return AMBIGUOUS, f"ambiguous dereferenced variable `{inner_tok[1]}`"
                if vt:
                    return vt, None
                return None, None

        # `self.<game_method>().<...>`
        if (
            prev_text == ")"
            and dot_idx >= 5
            and tokens[dot_idx - 2][1] == "("
            and tokens[dot_idx - 3][0] == "IDENT"
            and tokens[dot_idx - 4][1] == "."
            and tokens[dot_idx - 5][1] == "self"
        ):
            mname = tokens[dot_idx - 3][1]
            impl_t = active_impl_target()
            if impl_t and mname in index.wrapper_game_returning_methods.get(impl_t, ()):
                return "Game", "wrapper-self-method"
            if impl_t is None and mname in index.game_returning_methods:
                return AMBIGUOUS, f"unresolved self method `{mname}()`"
            return None, None

        # `self.<game_field>.<...>` or `self.0.<...>`
        if (
            prev_kind in ("IDENT", "LIT")
            and prev2 == "."
            and dot_idx >= 3
            and tokens[dot_idx - 3][1] == "self"
            and (dot_idx < 4 or tokens[dot_idx - 4][1] not in (".", "::"))
        ):
            impl_t = active_impl_target()
            if impl_t and prev_text in struct_game_fields.get(impl_t, ()):
                return "Game", "wrapper-self-field"
            return None, None

        # `<wrapper_var>.<game_field>.<...>`
        if (
            prev_kind in ("IDENT", "LIT")
            and prev2 == "."
            and dot_idx >= 3
            and tokens[dot_idx - 3][0] in ("IDENT", "MACRO_VAR")
            and tokens[dot_idx - 3][1] != "self"
            and (dot_idx < 4 or tokens[dot_idx - 4][1] not in (".", "::"))
        ):
            wt = var_type(tokens[dot_idx - 3][1])
            if wt and wt not in ("Game", AMBIGUOUS) and prev_text in struct_game_fields.get(wt, ()):
                return "Game", "wrapper-var-field"
            return None, None

        # `<game_var>.<...>` or `<wrapper_var>.<...>` or ambiguous var
        if prev_kind in ("IDENT", "MACRO_VAR") and prev_text != "self" and prev2 not in (".", "::"):
            vt = var_type(prev_text)
            if vt == "Game":
                return "Game", "typed-game-var"
            if vt == AMBIGUOUS:
                return AMBIGUOUS, f"ambiguous variable `{prev_text}`"
            if vt:
                return vt, None
            if prev_kind == "MACRO_VAR" and is_game_macro_var(prev_text):
                return "Game", "game-macro"

        return None, None

    def classify_let_rhs(rhs_idx: int) -> tuple[str | None, dict[int, str], bool]:
        """Classify a `let` RHS starting at `rhs_idx`.

        Returns `(whole_type, tuple_element_types, is_ambiguous)`.
        """
        r = rhs_idx
        while r < n and tokens[r][1] in ("&", "mut", "*"):
            r += 1
        if r >= n:
            return None, {}, False
        tk, tv, _, _ = tokens[r]
        if tk != "IDENT":
            return None, {}, False

        # Case A: `self` or `self.<game_field>` or `self.<game_method>()`
        if tv == "self":
            if active_impl_target() == "Game" and (r + 1 >= n or tokens[r + 1][1] in (";", ",", ")", "else")):
                return "Game", {}, False
            if r + 2 < n and tokens[r + 1][1] == "." and tokens[r + 2][0] in ("IDENT", "LIT"):
                impl_t = active_impl_target()
                if impl_t and tokens[r + 2][1] in struct_game_fields.get(impl_t, ()):
                    after = r + 3
                    if after >= n or tokens[after][1] in (";", ",", ")", "else"):
                        return "Game", {}, False
                if (
                    impl_t
                    and tokens[r + 2][1] in index.wrapper_game_returning_methods.get(impl_t, ())
                    and r + 3 < n
                    and tokens[r + 3][1] == "("
                ):
                    if rhs_preserves_game_after_call(tokens, r + 3):
                        return "Game", {}, False
            return None, {}, False

        # Case B: `<var>` or `<wrapper_var>.<game_field>`
        vt = var_type(tv)
        if vt == "Game":
            if r + 1 >= n or tokens[r + 1][1] in (";", ",", ")", "else"):
                return "Game", {}, False
        elif vt == AMBIGUOUS:
            if r + 1 >= n or tokens[r + 1][1] in (";", ",", ")", "else"):
                return None, {}, True
        elif vt and vt in struct_game_fields:
            if r + 2 < n and tokens[r + 1][1] == "." and tokens[r + 2][1] in struct_game_fields[vt]:
                after = r + 3
                if after >= n or tokens[after][1] in (";", ",", ")", "else"):
                    return "Game", {}, False

        # Case C: `<wrapper_struct> { ... }`
        if tv in wrapper_structs and r + 1 < n and tokens[r + 1][1] == "{":
            return tv, {}, False

        # Case D: path call `a::b::func(...)` or `Game::new(...)` or `Game { ... }`
        q = r
        segments = [tv]
        while q + 2 < n and tokens[q + 1][1] == "::" and tokens[q + 2][0] == "IDENT":
            q += 2
            segments.append(tokens[q][1])
        q_call = q + 1
        if q_call + 1 < n and tokens[q_call][1] == "::" and tokens[q_call + 1][1] == "<":
            q_call = skip_angle_generics(tokens, q_call + 1)

        last_seg = segments[-1]
        if last_seg == "Game" and q_call < n and tokens[q_call][1] == "{":
            return "Game", {}, False
        if last_seg in wrapper_structs and q_call < n and tokens[q_call][1] == "{":
            return last_seg, {}, False
        if len(segments) >= 2 and segments[-2] == "Game" and q_call < n and tokens[q_call][1] == "(":
            if (last_seg in ("new", "construct") or last_seg.startswith(("new_", "test", "fixture"))) and rhs_preserves_game_after_call(
                tokens, q_call
            ):
                return "Game", {}, False
        if len(segments) >= 2 and segments[-2] in wrapper_structs and q_call < n and tokens[q_call][1] == "(":
            if last_seg in ("new", "of", "default") and rhs_preserves_game_after_call(tokens, q_call):
                return segments[-2], {}, False
        if q_call < n and tokens[q_call][1] == "(" and rhs_preserves_game_after_call(tokens, q_call):
            pos_set, is_ambig = lookup_free_fn_returns(last_seg)
            if is_ambig:
                return None, {}, True
            if pos_set:
                whole = "Game" if None in pos_set else None
                tup = {p: "Game" for p in pos_set if isinstance(p, int)}
                return whole, tup, False
        return None, {}, False

    def build_pattern_bindings(
        pat_toks: list[Token],
        whole: str | None,
        tup_types: dict[int, str],
        is_ambig: bool = False,
    ) -> dict[str, str]:
        """Compute `name -> kind | "" | "?"` for every variable bound by `pat_toks`."""
        bindings: dict[str, str] = {}
        default_kind = AMBIGUOUS if is_ambig else NON_GAME
        # Every bound identifier shadows any outer tracked name by default.
        for bname in extract_pattern_bound_names(pat_toks):
            bindings[bname] = default_kind

        if not pat_toks or (whole is None and not tup_types):
            return bindings

        # Unwrap `Some(inner)` / `Ok(inner)` / `Option::Some(inner)` / `Result::Ok(inner)`
        p = pat_toks
        if len(p) >= 3 and p[-1][1] == ")":
            open_idx = None
            if len(p) >= 3 and p[0][1] in ("Some", "Ok") and p[1][1] == "(":
                open_idx = 1
            elif len(p) >= 5 and p[1][1] == "::" and p[2][1] in ("Some", "Ok") and p[3][1] == "(":
                open_idx = 3
            if open_idx is not None and skip_balanced(p, open_idx, "(", ")") == len(p):
                p = p[open_idx + 1 : -1]
        if not p:
            return bindings
        if len(p) >= 2 and p[0][1] == "(" and p[-1][1] == ")" and skip_balanced(p, 0, "(", ")") == len(p):
            for tidx, elem in enumerate(split_top_level_no_angles(p[1:-1], ",")):
                vname = parse_simple_pat_ident(elem)
                if vname and tidx in tup_types:
                    bindings[vname] = tup_types[tidx]
            return bindings
        vname = parse_simple_pat_ident(p)
        if vname and whole is not None:
            bindings[vname] = whole
        return bindings

    i = 0
    while i < n:
        # Expire range scopes and unknown macro frames that ended at or before `i`.
        while range_scopes and range_scopes[-1][0] <= i:
            range_scopes.pop()
        while unknown_macro_stack and unknown_macro_stack[-1][0] <= i:
            unknown_macro_stack.pop()
        if i in scheduled_ranges:
            for end_idx, rvars in scheduled_ranges.pop(i):
                range_scopes.append((end_idx, rvars))

        kind, text, start, end = tokens[i]

        # Skip token-only macros and track unknown macros.
        if (
            kind == "IDENT"
            and text not in NON_MACRO_KEYWORDS
            and text != "macro_rules"
            and i + 2 < n
            and tokens[i + 1][1] == "!"
            and tokens[i + 2][1] in ("(", "[", "{")
            and (i == 0 or tokens[i - 1][1] != "#")
        ):
            open_ch = tokens[i + 2][1]
            close_ch = {")": "(", "]": "[", "}": "{"}
            close_for = {"(": ")", "[": "]", "{": "}"}[open_ch]
            mac_end = skip_balanced(tokens, i + 2, open_ch, close_for)
            if text in TOKEN_ONLY_MACROS:
                i = mac_end
                continue
            if text not in EXPR_MACROS:
                unknown_macro_stack.append((mac_end, text))

        if text == "(":
            paren_depth += 1
        elif text == ")":
            while expr_closure_scopes and (
                expr_closure_scopes[-1][0] == brace_depth
                and expr_closure_scopes[-1][1] == paren_depth
                and expr_closure_scopes[-1][2] == bracket_depth
            ):
                expr_closure_scopes.pop()
            paren_depth = max(0, paren_depth - 1)
        elif text == "[":
            bracket_depth += 1
        elif text == "]":
            while expr_closure_scopes and (
                expr_closure_scopes[-1][0] == brace_depth
                and expr_closure_scopes[-1][1] == paren_depth
                and expr_closure_scopes[-1][2] == bracket_depth
            ):
                expr_closure_scopes.pop()
            bracket_depth = max(0, bracket_depth - 1)
        elif text in (",", ";"):
            while expr_closure_scopes and (
                expr_closure_scopes[-1][0] == brace_depth
                and expr_closure_scopes[-1][1] == paren_depth
                and expr_closure_scopes[-1][2] == bracket_depth
            ):
                expr_closure_scopes.pop()

        if text == ";" and i in deferred_let_bindings:
            var_scopes[-1][1].update(deferred_let_bindings.pop(i))

        if text == "{":
            brace_depth += 1
            scope_vars: dict[str, str] = {}
            is_fn_barrier = i in fn_item_braces
            if i in impl_opens:
                impl_stack.append((brace_depth, impl_opens[i]))
            if i in fn_opens:
                scope_vars.update(fn_opens[i])
            if i in macro_opens:
                macro_stack.append((brace_depth, macro_opens[i]))
            var_scopes.append((brace_depth, scope_vars, is_fn_barrier))
            i += 1
            continue

        if text == "}":
            while expr_closure_scopes and expr_closure_scopes[-1][0] >= brace_depth:
                expr_closure_scopes.pop()
            while var_scopes and var_scopes[-1][0] >= brace_depth:
                var_scopes.pop()
            while impl_stack and impl_stack[-1][0] >= brace_depth:
                impl_stack.pop()
            while macro_stack and macro_stack[-1][0] >= brace_depth:
                macro_stack.pop()
            brace_depth = max(0, brace_depth - 1)
            i += 1
            continue

        # `match scrutinee { pat [if guard] => body, ... }`
        if kind == "IDENT" and text == "match" and (i == 0 or tokens[i - 1][1] not in (".", "::")):
            whole_scrut, tup_scrut, scrut_ambig = classify_let_rhs(i + 1)
            k = i + 1
            dp2 = db2 = dc2 = 0
            match_brace: int | None = None
            while k < n:
                tv = tokens[k][1]
                if tv == "(":
                    dp2 += 1
                elif tv == ")":
                    dp2 = max(0, dp2 - 1)
                elif tv == "[":
                    db2 += 1
                elif tv == "]":
                    db2 = max(0, db2 - 1)
                elif tv == "{" and dp2 == 0 and db2 == 0 and dc2 == 0:
                    match_brace = k
                    break
                elif tv == "{":
                    dc2 += 1
                elif tv == "}":
                    dc2 = max(0, dc2 - 1)
                elif tv == ";" and dp2 == 0 and db2 == 0 and dc2 == 0:
                    break
                k += 1
            if match_brace is not None:
                match_end = skip_balanced(tokens, match_brace, "{", "}")
                for pat_start, pat_end, guard_start, arrow_idx, arm_end, pat_toks in find_match_arms(
                    tokens, match_brace, match_end
                ):
                    match_pat_ranges.append((pat_start, pat_end))
                    arm_vars = build_pattern_bindings(pat_toks, whole_scrut, tup_scrut, scrut_ambig)
                    if arm_vars:
                        if guard_start is not None:
                            scheduled_ranges.setdefault(guard_start, []).append((arrow_idx, arm_vars))
                        scheduled_ranges.setdefault(arrow_idx + 1, []).append((arm_end, arm_vars))

        # `for pat in iter { body }`
        if (
            kind == "IDENT"
            and text == "for"
            and not in_impl_header(i)
            and i + 1 < n
            and tokens[i + 1][1] != "<"
            and (
                i == 0
                or tokens[i - 1][1]
                in ("{", "}", ";", "=", "=>", "(", ",", ":", "else", "return", "break", "in")
            )
        ):
            p_start = i + 1
            k = p_start
            dp2 = db2 = dc2 = 0
            in_idx: int | None = None
            while k < n:
                tv = tokens[k][1]
                if tv == "(":
                    dp2 += 1
                elif tv == ")":
                    dp2 = max(0, dp2 - 1)
                elif tv == "[":
                    db2 += 1
                elif tv == "]":
                    db2 = max(0, db2 - 1)
                elif tv == "{":
                    if dp2 == 0 and db2 == 0 and dc2 == 0:
                        break
                    dc2 += 1
                elif tv == "}":
                    dc2 = max(0, dc2 - 1)
                elif tv == ";" and dp2 == 0 and db2 == 0 and dc2 == 0:
                    break
                elif tv == "in" and dp2 == 0 and db2 == 0 and dc2 == 0:
                    in_idx = k
                    break
                k += 1
            if in_idx is not None:
                for_pat = tokens[p_start:in_idx]
                bscan = in_idx + 1
                dp2 = db2 = 0
                while bscan < n:
                    tv = tokens[bscan][1]
                    if tv == "(":
                        dp2 += 1
                    elif tv == ")":
                        dp2 = max(0, dp2 - 1)
                    elif tv == "[":
                        db2 += 1
                    elif tv == "]":
                        db2 = max(0, db2 - 1)
                    elif tv == "{" and dp2 == 0 and db2 == 0:
                        for_vars = build_pattern_bindings(for_pat, None, {})
                        if for_vars:
                            fn_opens.setdefault(bscan, {}).update(for_vars)
                        break
                    elif tv == ";" and dp2 == 0 and db2 == 0:
                        break
                    bscan += 1

        # Closure parameters: `|game: &mut Game<'_>|`, `|g: &Self|`, `|(game, _)|`, `self.as_guest(1, |g| ...)`, etc.
        if (
            text == "|"
            and not in_match_pattern(i)
            and (
                i == 0
                or tokens[i - 1][1]
                in (
                    "(",
                    ",",
                    "[",
                    "{",
                    ";",
                    "=",
                    "=>",
                    ":",
                    "..",
                    "&",
                    "mut",
                    "move",
                    "return",
                    "break",
                    "if",
                    "else",
                    "match",
                    "in",
                    "async",
                    "static",
                )
            )
            and (i + 1 < n and tokens[i + 1][1] not in ("|", "="))
        ):
            close_bar = None
            j = i + 1
            da = dp = db = dc = 0
            while j < n and j < i + 80:
                tv = tokens[j][1]
                if tv == "<":
                    da += 1
                elif tv == ">":
                    da = max(0, da - 1)
                elif tv == "(":
                    dp += 1
                elif tv == ")":
                    if dp == 0:
                        break
                    dp -= 1
                elif tv == "[":
                    db += 1
                elif tv == "]":
                    db = max(0, db - 1)
                elif tv == "{":
                    # Struct destructuring pattern `|Wrap { game }|` has an identifier or `>` before `{`.
                    if j > i + 1 and (tokens[j - 1][0] == "IDENT" or tokens[j - 1][1] == ">"):
                        dc += 1
                    else:
                        break
                elif tv == "}":
                    if dc == 0:
                        break
                    dc -= 1
                elif tv in (";", ".") and dc == 0:
                    break
                elif tv == "|" and da == 0 and dp == 0 and db == 0 and dc == 0:
                    close_bar = j
                    break
                j += 1
            if close_bar is not None:
                c_slice = tokens[i + 1 : close_bar]
                cvars: dict[str, str] = {}
                expected_params: dict[int, str] = {}
                call_is_ambig = False

                call_info = find_enclosing_call_and_arg(tokens, i)
                if call_info is not None:
                    call_name, arg_idx, is_method, dot_idx = call_info
                    if is_method and dot_idx is not None:
                        recv_kind, _ = receiver_kind_before_dot(dot_idx)
                        if recv_kind == "Game":
                            expected_params = index.game_method_closure_params.get(call_name, {}).get(arg_idx, {})
                        elif recv_kind and recv_kind in wrapper_structs:
                            expected_params = (
                                index.wrapper_method_closure_params.get(recv_kind, {}).get(call_name, {}).get(arg_idx, {})
                            )
                    elif not is_method:
                        # Special-case `Mutate::new(|tick| ...)` where `new` is noisy globally.
                        if (
                            call_name == "new"
                            and i >= 5
                            and tokens[i - 2][1] == "new"
                            and tokens[i - 3][1] == "::"
                            and tokens[i - 4][1] == "Mutate"
                        ):
                            expected_params = {0: "BlockEntityTick"}
                        else:
                            fmap, call_is_ambig = lookup_free_fn_closures(call_name)
                            if fmap:
                                expected_params = fmap.get(arg_idx, {})

                cur_self = active_impl_target()
                for pidx, part in enumerate(split_top_level(c_slice, ",")):
                    colon = find_top_level_token(part, ":")
                    if colon is not None:
                        pat = part[:colon]
                        ty = part[colon + 1 :]
                        simple_name = parse_simple_pat_ident(pat)
                        if simple_name:
                            ptype = resolve_type_kind(ty, wrapper_structs, cur_self)
                            cvars[simple_name] = ptype if ptype is not None else NON_GAME
                        else:
                            for bname in extract_pattern_bound_names(pat):
                                cvars[bname] = NON_GAME
                    else:
                        simple_name = parse_simple_pat_ident(part)
                        if simple_name:
                            if pidx in expected_params:
                                cvars[simple_name] = expected_params[pidx]
                            elif call_is_ambig:
                                cvars[simple_name] = AMBIGUOUS
                            else:
                                cvars[simple_name] = NON_GAME
                        else:
                            for bname in extract_pattern_bound_names(part):
                                cvars[bname] = NON_GAME

                if cvars:
                    after = close_bar + 1
                    if after < n and tokens[after][1] == "->":
                        after += 1
                        da = dp = db = 0
                        while after < n:
                            tv = tokens[after][1]
                            if tv == "<":
                                da += 1
                            elif tv == ">":
                                da = max(0, da - 1)
                            elif tv == "(":
                                dp += 1
                            elif tv == ")":
                                if dp == 0:
                                    break
                                dp -= 1
                            elif tv == "[":
                                db += 1
                            elif tv == "]":
                                db = max(0, db - 1)
                            elif tv in ("{", ",", ";") and da == 0 and dp == 0 and db == 0:
                                break
                            after += 1
                    if after < n and tokens[after][1] == "{":
                        fn_opens.setdefault(after, {}).update(cvars)
                    else:
                        expr_closure_scopes.append((brace_depth, paren_depth, bracket_depth, cvars))
                i = close_bar + 1
                continue

        # `let` bindings (`let x = ...`, `let (a, b) = ...`, `let Some(x) = ... else`, `if let ...`, `while let ...`)
        if kind == "IDENT" and text == "let" and i + 1 < n:
            is_cond_let = i > 0 and tokens[i - 1][1] in ("if", "while", "&&")
            p_start = i + 1
            k = p_start
            da = dp = db = dc = 0
            while k < n:
                tv = tokens[k][1]
                if tv == "<":
                    da += 1
                elif tv == ">":
                    da = max(0, da - 1)
                elif tv == "(":
                    dp += 1
                elif tv == ")":
                    dp = max(0, dp - 1)
                elif tv == "[":
                    db += 1
                elif tv == "]":
                    db = max(0, db - 1)
                elif tv == "{":
                    dc += 1
                elif tv == "}":
                    dc = max(0, dc - 1)
                elif tv in (":", "=", ";") and da == 0 and dp == 0 and db == 0 and dc == 0:
                    break
                k += 1
            pat_toks = tokens[p_start:k]
            whole_ty: str | None = None
            tup_tys: dict[int, str] = {}
            rhs_ambig = False
            if k < n and tokens[k][1] == ":":
                ts = k + 1
                te = ts
                da = dp = db = 0
                while te < n:
                    tv = tokens[te][1]
                    if tv == "<":
                        da += 1
                    elif tv == ">":
                        da = max(0, da - 1)
                    elif tv == "(":
                        dp += 1
                    elif tv == ")":
                        dp = max(0, dp - 1)
                    elif tv == "[":
                        db += 1
                    elif tv == "]":
                        db = max(0, db - 1)
                    elif tv in ("=", ";") and da == 0 and dp == 0 and db == 0:
                        break
                    te += 1
                whole_ty = resolve_type_kind(tokens[ts:te], wrapper_structs, active_impl_target())
                k = te
            if whole_ty is None and k < n and tokens[k][1] == "=":
                whole_ty, tup_tys, rhs_ambig = classify_let_rhs(k + 1)

            new_bindings = build_pattern_bindings(pat_toks, whole_ty, tup_tys, rhs_ambig)

            if not is_cond_let:
                if new_bindings:
                    # Defer activation to the terminating `;` so the RHS and `else` see outer bindings.
                    sscan = k
                    dp2 = db2 = dc2 = 0
                    while sscan < n:
                        tv = tokens[sscan][1]
                        if tv == "(":
                            dp2 += 1
                        elif tv == ")":
                            dp2 = max(0, dp2 - 1)
                        elif tv == "[":
                            db2 += 1
                        elif tv == "]":
                            db2 = max(0, db2 - 1)
                        elif tv == "{":
                            dc2 += 1
                        elif tv == "}":
                            dc2 = max(0, dc2 - 1)
                        elif tv == ";" and dp2 == 0 and db2 == 0 and dc2 == 0:
                            deferred_let_bindings.setdefault(sscan, {}).update(new_bindings)
                            break
                        sscan += 1
            elif new_bindings:
                # Attach `if let` / `while let` / `&& let` bindings to the condition chain and body `{`.
                bscan = k + 1
                dp2 = db2 = 0
                first_amp: int | None = None
                while bscan < n:
                    tv = tokens[bscan][1]
                    if tv == "(":
                        dp2 += 1
                    elif tv == ")":
                        dp2 = max(0, dp2 - 1)
                    elif tv == "[":
                        db2 += 1
                    elif tv == "]":
                        db2 = max(0, db2 - 1)
                    elif tv == "&&" and dp2 == 0 and db2 == 0 and first_amp is None:
                        first_amp = bscan
                    elif tv == "{" and dp2 == 0 and db2 == 0:
                        if first_amp is not None:
                            scheduled_ranges.setdefault(first_amp + 1, []).append((bscan, new_bindings))
                        fn_opens.setdefault(bscan, {}).update(new_bindings)
                        break
                    elif tv == ";" and dp2 == 0 and db2 == 0:
                        break
                    bscan += 1

        # Match field access `.field` or `.server.field` / `.client.field`
        if text == "." and i > 0 and i + 1 < n and tokens[i + 1][0] == "IDENT":
            recv_kind, reason = receiver_kind_before_dot(i)
            if recv_kind in ("Game", AMBIGUOUS) and reason is not None:
                record_access(i, recv_kind, reason)
            elif EXTRA_SITES:
                fname = tokens[i + 1][1]
                if fname in owners:
                    lineno = offset_to_line(line_starts, tokens[i + 1][2])
                    if (rel_posix, lineno, fname) in EXTRA_SITES:
                        record_access(i, "Game", "explicit-site")

        i += 1

    sorted_edits = sorted(edits.values(), key=lambda e: (e.start, e.end))
    sorted_ambig = sorted(ambiguous.values(), key=lambda a: a.offset)
    return sorted_edits, sorted_ambig


def find_edits(
    src: str,
    rel_posix: str,
    owners: dict[str, str],
    index: WorkspaceIndex,
) -> list[Edit]:
    edits, _ = scan_file(src, rel_posix, owners, index)
    return edits


def collect_files(paths: list[Path]) -> list[Path]:
    files: list[Path] = []
    for p in paths:
        full = p if p.is_absolute() else (ROOT / p)
        if full.is_file():
            if full.suffix == ".rs":
                files.append(full)
        elif full.is_dir():
            for child in sorted(full.rglob("*.rs")):
                files.append(child)
        else:
            print(f"error: path not found: {p}", file=sys.stderr)
            sys.exit(2)
    return files


def apply_edits(src: str, edits: list[Edit]) -> str:
    if not edits:
        return src
    parts: list[str] = []
    last = 0
    for e in edits:
        assert e.start >= last, f"overlapping edits: {last} > {e.start}"
        parts.append(src[last : e.start])
        parts.append(e.replacement)
        last = e.end
    parts.append(src[last:])
    return "".join(parts)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Normalise or check `Game` field accesses from the owner table.")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--fix", action="store_true", help="rewrite accesses in place")
    mode.add_argument("--check", action="store_true", help="exit 1 if any stale access remains")
    parser.add_argument("--table", type=Path, default=DEFAULT_TABLE, help="path to game_field_owners.tsv")
    parser.add_argument("--stats", action="store_true", help="print breakdown by category")
    parser.add_argument("--no-fmt", action="store_true", help="skip running rustfmt on modified files")
    parser.add_argument(
        "--allow-ambiguous",
        action="store_true",
        help="do not fail when ambiguous receivers are reported for manual review",
    )
    parser.add_argument("paths", nargs="*", type=Path, help="files or directories to scan")
    args = parser.parse_args(argv)

    owners = load_field_owners(args.table)
    paths = args.paths if args.paths else list(DEFAULT_PATHS)
    files = collect_files(paths)
    index = build_workspace_index(paths)

    total_edits = 0
    total_ambiguous = 0
    files_changed = 0
    changed_paths: list[Path] = []
    by_reason: dict[str, int] = {}

    for path in files:
        try:
            rel_posix = path.resolve().relative_to(ROOT.resolve()).as_posix()
        except ValueError:
            rel_posix = path.as_posix()
        src = path.read_text(encoding="utf-8")
        edits, ambig = scan_file(src, rel_posix, owners, index)
        line_starts = compute_line_starts(src) if (ambig or (edits and args.check)) else []
        for site in ambig:
            total_ambiguous += 1
            lineno = offset_to_line(line_starts, site.offset)
            print(
                f"{rel_posix}:{lineno}: ambiguous receiver for Game field `{site.field}` ({site.detail}) — review manually",
                file=sys.stderr,
            )
        if not edits:
            continue
        files_changed += 1
        total_edits += len(edits)
        for e in edits:
            by_reason[e.reason] = by_reason.get(e.reason, 0) + 1
        if args.fix:
            path.write_text(apply_edits(src, edits), encoding="utf-8")
            changed_paths.append(path)
        else:
            for e in edits:
                lineno = offset_to_line(line_starts, e.start)
                print(f"{rel_posix}:{lineno}: stale Game field `{e.old_expr}` -> `{e.new_expr}` ({e.reason})")

    if args.fix and changed_paths and not args.no_fmt:
        rustfmt = shutil.which("rustfmt")
        if rustfmt:
            batch = 100
            for start in range(0, len(changed_paths), batch):
                subprocess.run(
                    [rustfmt, "--edition", "2024", *(str(p) for p in changed_paths[start : start + batch])],
                    check=True,
                )

    if args.fix:
        print(f"Rewrote {total_edits} sites across {files_changed} files.")
    else:
        print(f"Found {total_edits} stale sites across {files_changed} files.")

    if total_ambiguous > 0:
        print(f"Reported {total_ambiguous} ambiguous site(s) for manual review.", file=sys.stderr)

    if args.stats and by_reason:
        for reason, count in sorted(by_reason.items()):
            print(f"  {reason}: {count}")

    if args.check and total_edits > 0:
        return 1
    if total_ambiguous > 0 and not args.allow_ambiguous:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
