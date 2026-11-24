#!/usr/bin/env python3
"""Rewrite and check field accesses for fields moved out of `struct Game<'a>`.

Reads `tools/game_field_owners.tsv` next to this script (or `--table PATH`).
Fields with owner `server` live on `Game.server` (`ServerState`); fields with
owner `client` live on `Game.client` (`ClientState`).

Usage:
    python3 tools/game_field_owners.py --fix [paths...]
    python3 tools/game_field_owners.py --check [paths...]
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

Token = tuple[str, str, int, int]
ClosureArgMap = dict[int, dict[int, str]]


@dataclass
class WorkspaceIndex:
    # Struct name -> field names (or tuple indices "0", "1", ...) holding a `Game`
    struct_game_fields: dict[str, set[str]] = field(default_factory=dict)
    # Method names on wrapper structs returning `&Game` / `&mut Game` / `Game`
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


@dataclass
class FileLocalIndex:
    defined_structs: set[str] = field(default_factory=set)
    struct_game_fields: dict[str, set[str]] = field(default_factory=dict)
    defined_free_fns: set[str] = field(default_factory=set)
    game_returning_fns: dict[str, set[int | None]] = field(default_factory=dict)
    fn_closure_params: dict[str, ClosureArgMap] = field(default_factory=dict)


def load_moved_fields(table_path: Path) -> dict[str, str]:
    moved: dict[str, str] = {}
    with table_path.open(newline="", encoding="utf-8") as fp:
        for row in csv.DictReader(fp, delimiter="\t"):
            owner = row["owner"].strip()
            if owner in ("server", "client"):
                moved[row["field"].strip()] = owner
    return moved


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

        if i + 1 < n and src[i : i + 2] in ("::", "->", "=>", "..", "&&", "||"):
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
            # Possible `pub(in ...)` / `pub(crate)` / `pub(super)`
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
                if arg_closure_map:
                    if impl_target == "Game":
                        merge_closure_map(index.game_method_closure_params.setdefault(fname, {}), arg_closure_map)
                    elif impl_target and impl_target in wrapper_structs:
                        wmap = index.wrapper_method_closure_params.setdefault(impl_target, {})
                        merge_closure_map(wmap.setdefault(fname, {}), arg_closure_map)
            else:
                # Associated functions on `impl Game` (e.g., `Game::new`) are handled via `Game::...` syntax
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

    # Prune private free fns whose name collides with any non-Game free fn elsewhere.
    for colliding in index.non_game_return_free_fns:
        index.priv_game_returning_fns.pop(colliding, None)
    for colliding in index.non_game_closure_free_fns:
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
    """Given index of `fn`, return `(dict[param_name -> "Game" | WrapperStruct | ""], open_brace_idx)`."""
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
        colon = next((idx_p for idx_p, tok in enumerate(p_slice) if tok[1] == ":"), None)
        if colon is None:
            continue
        pat = p_slice[:colon]
        ty = p_slice[colon + 1 :]
        pname = None
        if len(pat) == 1 and pat[0][0] == "IDENT" and pat[0][1] != "self":
            pname = pat[0][1]
        elif len(pat) == 2 and pat[0][1] == "mut" and pat[1][0] == "IDENT" and pat[1][1] != "self":
            pname = pat[1][1]
        if not pname:
            continue
        kind = resolve_type_kind(ty, wrapper_structs, self_type)
        typed_params[pname] = kind if kind is not None else ""
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
            while q >= 0 and b - q <= 60:
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
    if len(toks) == 1 and toks[0][0] == "IDENT" and toks[0][1] != "_":
        return toks[0][1]
    if len(toks) == 2 and toks[0][1] == "mut" and toks[1][0] == "IDENT" and toks[1][1] != "_":
        return toks[1][1]
    if len(toks) == 2 and toks[0][1] == "ref" and toks[1][0] == "IDENT" and toks[1][1] != "_":
        return toks[1][1]
    if len(toks) == 3 and toks[0][1] == "ref" and toks[1][1] == "mut" and toks[2][0] == "IDENT" and toks[2][1] != "_":
        return toks[2][1]
    return None


def find_edits(
    src: str,
    rel_posix: str,
    moved: dict[str, str],
    index: WorkspaceIndex,
) -> list[tuple[int, str, str, str]]:
    """Return sorted list of `(insert_offset, insert_text, field, reason)` for one file."""
    tokens = tokenize_rust(src)
    if file_defines_local_mock_game(tokens, rel_posix):
        return []

    local = build_file_local_index(tokens, set(index.struct_game_fields))
    struct_game_fields: dict[str, set[str]] = dict(index.struct_game_fields)
    for sname in local.defined_structs:
        if sname in local.struct_game_fields:
            struct_game_fields[sname] = local.struct_game_fields[sname]
        else:
            struct_game_fields.pop(sname, None)
    wrapper_structs = set(struct_game_fields)

    def lookup_free_fn_returns(fname: str) -> set[int | None] | None:
        if fname in local.defined_free_fns:
            return local.game_returning_fns.get(fname)
        return index.pub_game_returning_fns.get(fname) or index.priv_game_returning_fns.get(fname)

    def lookup_free_fn_closures(fname: str) -> ClosureArgMap | None:
        if fname in local.defined_free_fns:
            return local.fn_closure_params.get(fname)
        return index.pub_fn_closure_params.get(fname) or index.priv_fn_closure_params.get(fname)

    line_starts = compute_line_starts(src)
    n = len(tokens)
    edits: dict[int, tuple[str, str, str]] = {}

    impl_opens: dict[int, str | None] = {}
    fn_opens: dict[int, dict[str, str]] = {}
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
                    if tparams:
                        fn_opens.setdefault(brace_idx, {}).update(tparams)

    brace_depth = 0
    impl_stack: list[tuple[int, str | None]] = []
    # var_name (or "$var") -> "Game" | WrapperStructName | "" (explicitly shadowed non-Game)
    var_scopes: list[tuple[int, dict[str, str]]] = [(0, {})]
    expr_closure_scopes: list[tuple[int, int, int, dict[str, str]]] = []
    macro_stack: list[tuple[int, set[str]]] = []

    paren_depth = 0
    bracket_depth = 0

    def active_impl_target() -> str | None:
        return impl_stack[-1][1] if impl_stack else None

    def var_type(name: str) -> str | None:
        for _, _, _, cvars in reversed(expr_closure_scopes):
            if name in cvars:
                return cvars[name] or None
        for _, scope in reversed(var_scopes):
            if name in scope:
                return scope[name] or None
        return None

    def is_game_macro_var(name: str) -> bool:
        return any(name in mvars for _, mvars in macro_stack)

    def maybe_record(field_idx: int, reason: str) -> None:
        if field_idx >= n or tokens[field_idx][0] != "IDENT":
            return
        fname = tokens[field_idx][1]
        owner = moved.get(fname)
        if owner is None:
            return
        nxt = tokens[field_idx + 1][1] if field_idx + 1 < n else None
        if nxt in ("(", "::"):
            return
        ins_off = tokens[field_idx][2]
        edits.setdefault(ins_off, (f"{owner}.", fname, reason))

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

        # `(*self).<...>`
        if (
            prev_text == ")"
            and dot_idx >= 4
            and tokens[dot_idx - 2][1] == "self"
            and tokens[dot_idx - 3][1] == "*"
            and tokens[dot_idx - 4][1] == "("
        ):
            t = active_impl_target()
            if t == "Game":
                return "Game", "impl-game-deref-self"
            if t and t in wrapper_structs:
                return t, None
            return None, None

        # `self.<game_method>().<...>`
        if (
            prev_text == ")"
            and dot_idx >= 5
            and tokens[dot_idx - 2][1] == "("
            and tokens[dot_idx - 3][0] == "IDENT"
            and tokens[dot_idx - 3][1] in index.game_returning_methods
            and tokens[dot_idx - 4][1] == "."
            and tokens[dot_idx - 5][1] == "self"
        ):
            return "Game", "wrapper-self-method"

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
            if wt and wt != "Game" and prev_text in struct_game_fields.get(wt, ()):
                return "Game", "wrapper-var-field"
            return None, None

        # `<game_var>.<...>` or `<wrapper_var>.<...>`
        if prev_kind in ("IDENT", "MACRO_VAR") and prev_text != "self" and prev2 not in (".", "::"):
            vt = var_type(prev_text)
            if vt == "Game":
                return "Game", "typed-game-var"
            if vt:
                return vt, None
            if prev_kind == "MACRO_VAR" and is_game_macro_var(prev_text):
                return "Game", "game-macro"

        return None, None

    def classify_let_rhs(rhs_idx: int) -> tuple[str | None, dict[int, str]]:
        """Classify a `let` RHS starting at `rhs_idx`.

        Returns `(whole_type, tuple_element_types)`.
        """
        r = rhs_idx
        while r < n and tokens[r][1] in ("&", "mut", "*"):
            r += 1
        if r >= n:
            return None, {}
        tk, tv, _, _ = tokens[r]
        if tk != "IDENT":
            return None, {}

        # Case A: `self` or `self.<game_field>` or `self.<game_method>()`
        if tv == "self":
            if active_impl_target() == "Game" and (r + 1 >= n or tokens[r + 1][1] in (";", ",", ")", "else")):
                return "Game", {}
            if r + 2 < n and tokens[r + 1][1] == "." and tokens[r + 2][0] in ("IDENT", "LIT"):
                impl_t = active_impl_target()
                if impl_t and tokens[r + 2][1] in struct_game_fields.get(impl_t, ()):
                    after = r + 3
                    if after >= n or tokens[after][1] in (";", ",", ")", "else"):
                        return "Game", {}
                if tokens[r + 2][1] in index.game_returning_methods and r + 3 < n and tokens[r + 3][1] == "(":
                    if rhs_preserves_game_after_call(tokens, r + 3):
                        return "Game", {}
            return None, {}

        # Case B: `<var>` or `<wrapper_var>.<game_field>`
        vt = var_type(tv)
        if vt == "Game":
            if r + 1 >= n or tokens[r + 1][1] in (";", ",", ")", "else"):
                return "Game", {}
        elif vt and vt in struct_game_fields:
            if r + 2 < n and tokens[r + 1][1] == "." and tokens[r + 2][1] in struct_game_fields[vt]:
                after = r + 3
                if after >= n or tokens[after][1] in (";", ",", ")", "else"):
                    return "Game", {}

        # Case C: `<wrapper_struct> { ... }`
        if tv in wrapper_structs and r + 1 < n and tokens[r + 1][1] == "{":
            return tv, {}

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
            return "Game", {}
        if last_seg in wrapper_structs and q_call < n and tokens[q_call][1] == "{":
            return last_seg, {}
        if len(segments) >= 2 and segments[-2] == "Game" and q_call < n and tokens[q_call][1] == "(":
            if (last_seg in ("new", "construct") or last_seg.startswith(("new_", "test", "fixture"))) and rhs_preserves_game_after_call(
                tokens, q_call
            ):
                return "Game", {}
        if len(segments) >= 2 and segments[-2] in wrapper_structs and q_call < n and tokens[q_call][1] == "(":
            if last_seg in ("new", "of", "default") and rhs_preserves_game_after_call(tokens, q_call):
                return segments[-2], {}
        if q_call < n and tokens[q_call][1] == "(" and rhs_preserves_game_after_call(tokens, q_call):
            pos_set = lookup_free_fn_returns(last_seg)
            if pos_set:
                whole = "Game" if None in pos_set else None
                tup = {p: "Game" for p in pos_set if isinstance(p, int)}
                return whole, tup
        return None, {}

    def bind_let_pattern(pat_toks: list[Token], whole: str | None, tup_types: dict[int, str], target_scope: dict[str, str]) -> None:
        if not pat_toks:
            return
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
            return
        if len(p) >= 2 and p[0][1] == "(" and p[-1][1] == ")" and skip_balanced(p, 0, "(", ")") == len(p):
            for tidx, elem in enumerate(split_top_level(p[1:-1], ",")):
                vname = parse_simple_pat_ident(elem)
                if vname:
                    target_scope[vname] = tup_types.get(tidx, "")
            return
        vname = parse_simple_pat_ident(p)
        if vname:
            target_scope[vname] = whole if whole is not None else ""

    i = 0
    while i < n:
        kind, text, start, end = tokens[i]

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

        if text == "{":
            brace_depth += 1
            scope_vars: dict[str, str] = {}
            if i in impl_opens:
                impl_stack.append((brace_depth, impl_opens[i]))
            if i in fn_opens:
                scope_vars.update(fn_opens[i])
            if i in macro_opens:
                macro_stack.append((brace_depth, macro_opens[i]))
            var_scopes.append((brace_depth, scope_vars))
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

        # Closure parameters: `|game: &mut Game<'_>|`, `|g: &Self|`, `self.as_guest(1, |g| ...)`, etc.
        if (
            text == "|"
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
            da = dp = db = 0
            while j < n and j < i + 50:
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
                elif tv in ("{", "}", ";", "."):
                    break
                elif tv == "|" and da == 0 and dp == 0 and db == 0:
                    close_bar = j
                    break
                j += 1
            if close_bar is not None:
                c_slice = tokens[i + 1 : close_bar]
                cvars: dict[str, str] = {}
                expected_params: dict[int, str] = {}

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
                        # Special-case `Mutate::new(|tick| ...)` where `new` is noisy globally:
                        if (
                            call_name == "new"
                            and i >= 5
                            and tokens[i - 2][1] == "new"
                            and tokens[i - 3][1] == "::"
                            and tokens[i - 4][1] == "Mutate"
                        ):
                            expected_params = {0: "BlockEntityTick"}
                        else:
                            fmap = lookup_free_fn_closures(call_name)
                            if fmap:
                                expected_params = fmap.get(arg_idx, {})

                cur_self = active_impl_target()
                for pidx, part in enumerate(split_top_level(c_slice, ",")):
                    colon = next((idx_p for idx_p, tk in enumerate(part) if tk[1] == ":"), None)
                    pname = None
                    ptype: str | None = None
                    if colon is not None:
                        pat = part[:colon]
                        ty = part[colon + 1 :]
                        if len(pat) == 1 and pat[0][0] in ("IDENT", "MACRO_VAR") and pat[0][1] != "_":
                            pname = pat[0][1]
                        elif len(pat) == 2 and pat[0][1] == "mut" and pat[1][0] in ("IDENT", "MACRO_VAR") and pat[1][1] != "_":
                            pname = pat[1][1]
                        ptype = resolve_type_kind(ty, wrapper_structs, cur_self)
                    else:
                        if len(part) == 1 and part[0][0] in ("IDENT", "MACRO_VAR") and part[0][1] != "_":
                            pname = part[0][1]
                        elif len(part) == 2 and part[0][1] == "mut" and part[1][0] in ("IDENT", "MACRO_VAR") and part[1][1] != "_":
                            pname = part[1][1]
                        if pname and pidx in expected_params:
                            ptype = expected_params[pidx]
                    if pname:
                        # Always record (with "" for non-Game params) so inner closures shadow outer Game vars.
                        cvars[pname] = ptype if ptype is not None else ""

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

        # `let` bindings (`let x = ...`, `let (a, b) = ...`, `let Some(x) = ... else`, `if let ...`)
        if kind == "IDENT" and text == "let" and i + 1 < n:
            is_cond_let = i > 0 and tokens[i - 1][1] in ("if", "while", "&&")
            # Scan pattern up to top-level `:` or `=` or `;`
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
                whole_ty, tup_tys = classify_let_rhs(k + 1)

            if not is_cond_let:
                bind_let_pattern(pat_toks, whole_ty, tup_tys, var_scopes[-1][1])
            elif whole_ty is not None or tup_tys:
                # For `if let` / `while let` binding a `Game`, attach to its body `{`.
                bscan = k + 1
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
                        bind_let_pattern(pat_toks, whole_ty, tup_tys, fn_opens.setdefault(bscan, {}))
                        break
                    elif tv == ";" and dp2 == 0 and db2 == 0:
                        break
                    bscan += 1

        # Match field access `.field`
        if text == "." and i > 0 and i + 1 < n and tokens[i + 1][0] == "IDENT":
            recv_kind, reason = receiver_kind_before_dot(i)
            if recv_kind == "Game" and reason is not None:
                maybe_record(i + 1, reason)
            elif EXTRA_SITES:
                fname = tokens[i + 1][1]
                if fname in moved:
                    lineno = offset_to_line(line_starts, tokens[i + 1][2])
                    if (rel_posix, lineno, fname) in EXTRA_SITES:
                        maybe_record(i + 1, "explicit-site")

        i += 1

    return [(off, ins, fname, reason) for off, (ins, fname, reason) in sorted(edits.items())]


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


def apply_edits(src: str, edits: list[tuple[int, str, str, str]]) -> str:
    if not edits:
        return src
    parts: list[str] = []
    last = 0
    for off, ins, _, _ in edits:
        parts.append(src[last:off])
        parts.append(ins)
        last = off
    parts.append(src[last:])
    return "".join(parts)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Rewrite or check moved `Game` field accesses.")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--fix", action="store_true", help="rewrite accesses in place")
    mode.add_argument("--check", action="store_true", help="exit 1 if any unrewritten access remains")
    parser.add_argument("--table", type=Path, default=DEFAULT_TABLE, help="path to game_field_owners.tsv")
    parser.add_argument("--stats", action="store_true", help="print breakdown by category")
    parser.add_argument("--no-fmt", action="store_true", help="skip running rustfmt on modified files")
    parser.add_argument("paths", nargs="*", type=Path, help="files or directories to scan")
    args = parser.parse_args(argv)

    moved = load_moved_fields(args.table)
    paths = args.paths if args.paths else list(DEFAULT_PATHS)
    files = collect_files(paths)
    index = build_workspace_index(paths)

    total_edits = 0
    files_changed = 0
    changed_paths: list[Path] = []
    by_reason: dict[str, int] = {}

    for path in files:
        try:
            rel_posix = path.resolve().relative_to(ROOT.resolve()).as_posix()
        except ValueError:
            rel_posix = path.as_posix()
        src = path.read_text(encoding="utf-8")
        edits = find_edits(src, rel_posix, moved, index)
        if not edits:
            continue
        files_changed += 1
        total_edits += len(edits)
        for _, _, _, reason in edits:
            by_reason[reason] = by_reason.get(reason, 0) + 1
        if args.fix:
            path.write_text(apply_edits(src, edits), encoding="utf-8")
            changed_paths.append(path)
        else:
            line_starts = compute_line_starts(src)
            for off, ins, fname, reason in edits:
                lineno = offset_to_line(line_starts, off)
                print(f"{rel_posix}:{lineno}: moved Game field `{fname}` -> `{ins}{fname}` ({reason})")

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
        print(f"Found {total_edits} unrewritten sites across {files_changed} files.")

    if args.stats and by_reason:
        for reason, count in sorted(by_reason.items()):
            print(f"  {reason}: {count}")

    if args.check and total_edits > 0:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
