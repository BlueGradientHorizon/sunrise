#!/usr/bin/env python3
"""Re-parent one-sided gameplay modules under `game::client_side` / `game::server_side`.

Reads `tools/game_module_sides.tsv` and `tools/game_field_owners.tsv` and keeps the tree
in sync:
- Out-of-line side modules `crates/sunrise-client/src/game/{client,server}_side.rs` hold
  `ClientState`/`ServerState`, their field visibilities (`pub(in crate::game::<side>_side)`
  for `side`, `pub(in crate::game)` for `game`), the glob import `pub(in crate::game::<side>_side)
  use super::*;`, and the `mod` declarations for that side.
- `crates/sunrise-client/src/game.rs` declares `client_side` and `server_side` (aliasing
  `client_state` / `server_state`), keeps `game`-side `mod` declarations in place, and
  re-exports moved modules so existing `super::` / `crate::game::` imports keep resolving.
- Root-module `pub(super)` items in moved modules (and their root-level `include!`d files)
  are rewritten to `pub(in crate::game)`, while `pub(super)` inside nested `mod` blocks is
  preserved; unsupported relative visibilities, ambiguous macro token trees, unresolved
  root includes, shared includes across conflicting scopes, non-complementary multi-arm
  `cfg` guards, `cfg_attr` path attributes, raw-identifier module declarations, and
  unexpected pre-struct side-file items are refused without modifying files.
- Exact test selectors, parity harness selectors, `tools/test_areas.toml`, `fn-replace-check`
  manifests, docs, and recognized Rust test-selector literals are rewritten from the table.
- `--fix` runs `rustfmt --edition 2024` on modified Rust files so generated output matches
  the repository's formatting gate; pass `--no-fmt` to inspect raw source substitutions.

Usage:
    python3 tools/game_module_sides.py --fix [--no-fmt]
    python3 tools/game_module_sides.py --check
    python3 tools/game_module_sides.py --list
    python3 tools/game_module_sides.py --renames BEFORE_FUNCTIONS_JSON AFTER_FUNCTIONS_JSON [--out PATH]
"""

from __future__ import annotations

import argparse
from bisect import bisect_right
import csv
import json
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_TABLE = Path(__file__).resolve().with_name("game_module_sides.tsv")
DEFAULT_FIELDS_TABLE = Path(__file__).resolve().with_name("game_field_owners.tsv")

SIDE_MOD_NAME = {"client": "client_side", "server": "server_side"}
SIDE_STRUCT_NAME = {"client": "ClientState", "server": "ServerState"}
SIDE_OLD_FILE = {"client": "client_state.rs", "server": "server_state.rs"}
SIDE_NEW_FILE = {"client": "client_side.rs", "server": "server_side.rs"}
SIDE_CONTAINERS = frozenset({"client_state", "server_state", "client_side", "server_side"})

# Keywords that can precede `!` as logical negation rather than a macro call.
NON_MACRO_KEYWORDS = frozenset({
    "if", "while", "match", "return", "else", "in", "break", "as", "let", "mut",
    "ref", "where", "yield", "async", "move", "unsafe", "loop", "const", "static", "pub",
})

# Rust subprocess test helpers whose string literal argument is a libtest selector.
TEST_RUNNER_FNS = frozenset({"run_fresh_cases", "in_own_process"})

GAME_PATH_RE = re.compile(r"(?<![A-Za-z0-9_:])game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::)")
FULL_GAME_PATH_RE = re.compile(
    r"sunrise_client::game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::)"
)
STATE_TYPE_PATH_RE = re.compile(
    r"sunrise_client::game::(client_state|server_state|client_side|server_side)::(ClientState|ServerState)\b"
)
RUST_GAME_SELECTOR_RE = re.compile(
    r"^game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::.+)$"
)
TEST_AREAS_FILTER_RE = re.compile(
    r'^(\s*\{\s*package\s*=\s*"sunrise-client",\s*target\s*=\s*"lib",\s*filter\s*=\s*")'
    r'(game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::[^"]*)?)'
    r'("\s*\},?\s*)$'
)

Token = tuple[str, str, int, int]


class LineIndex:
    """Map byte/char offsets in a string to 1-based line numbers."""

    def __init__(self, text: str) -> None:
        self._newlines = [i for i, ch in enumerate(text) if ch == "\n"]

    def line_of(self, offset: int) -> int:
        return bisect_right(self._newlines, offset - 1) + 1 if offset > 0 else 1


def norm_ident(text: str) -> str:
    return text[2:] if text.startswith("r#") else text


def line_start_of(text: str, offset: int) -> int:
    pos = text.rfind("\n", 0, offset)
    return 0 if pos == -1 else pos + 1


def line_indent_at(text: str, offset: int) -> str:
    start = line_start_of(text, offset)
    i = start
    while i < offset and text[i] in (" ", "\t"):
        i += 1
    return text[start:i]


def consume_trailing_newline(text: str, offset: int) -> int:
    i = offset
    n = len(text)
    while i < n and text[i] in (" ", "\t"):
        i += 1
    if i < n and text[i] == "\r":
        i += 1
    if i < n and text[i] == "\n":
        return i + 1
    return offset


def tokenize_rust(src: str) -> list[Token]:
    """Tokenize Rust source into `(kind, text, start, end)` skipping comments and whitespace."""
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

        raw_kind: str | None = None
        raw_prefix = 0
        if ch == "r" and i + 1 < n and src[i + 1] in ('"', "#"):
            raw_kind = "STR"
            raw_prefix = 1
        elif ch in ("b", "c") and i + 2 < n and src[i + 1] == "r" and src[i + 2] in ('"', "#"):
            raw_kind = "BYTES"
            raw_prefix = 2
        if raw_prefix and raw_kind is not None:
            j = i + raw_prefix
            hashes = 0
            while j < n and src[j] == "#":
                hashes += 1
                j += 1
            if j < n and src[j] == '"':
                start = i
                j += 1
                closing = '"' + ("#" * hashes)
                end_pos = src.find(closing, j)
                i = n if end_pos == -1 else end_pos + len(closing)
                tokens.append((raw_kind, src[start:i], start, i))
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
            tokens.append(("BYTES", src[start:j], start, j))
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
            tokens.append(("CHAR", src[start:j], start, j))
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
            tokens.append(("STR", src[start:j], start, j))
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
                tokens.append(("CHAR", src[start:j], start, j))
                i = j
                continue
            if i + 2 < n and src[i + 2] == "'" and src[i + 1] != "'":
                tokens.append(("CHAR", src[start : i + 3], start, i + 3))
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
            tokens.append(("NUM", src[start:j], start, j))
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


def decode_rust_str(tok: Token) -> str | None:
    """Decode a Rust ordinary or raw string literal token (`kind == 'STR'`)."""
    kind, text, _, _ = tok
    if kind != "STR":
        return None
    if text.startswith("r"):
        q = text.find('"')
        if q == -1:
            return None
        hashes = q - 1
        end_seq = '"' + ("#" * hashes)
        if not text.endswith(end_seq) or len(text) < q + 1 + len(end_seq):
            return None
        return text[q + 1 : len(text) - len(end_seq)]
    if not (text.startswith('"') and text.endswith('"') and len(text) >= 2):
        return None
    inner = text[1:-1]
    out: list[str] = []
    i, n = 0, len(inner)
    while i < n:
        c = inner[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        i += 1
        if i >= n:
            return None
        esc = inner[i]
        if esc == "n":
            out.append("\n")
            i += 1
        elif esc == "r":
            out.append("\r")
            i += 1
        elif esc == "t":
            out.append("\t")
            i += 1
        elif esc == "0":
            out.append("\0")
            i += 1
        elif esc in ("\\", '"', "'"):
            out.append(esc)
            i += 1
        elif esc == "x" and i + 2 < n:
            try:
                out.append(chr(int(inner[i + 1 : i + 3], 16)))
            except ValueError:
                return None
            i += 3
        elif esc == "u" and i + 1 < n and inner[i + 1] == "{":
            close = inner.find("}", i + 2)
            if close == -1:
                return None
            try:
                out.append(chr(int(inner[i + 2 : close].replace("_", ""), 16)))
            except ValueError:
                return None
            i = close + 1
        elif esc in ("\n", "\r"):
            while i < n and inner[i] in (" ", "\t", "\r", "\n"):
                i += 1
        else:
            return None
    return "".join(out)


def encode_rust_str_like(orig_tok: Token, new_value: str) -> str:
    """Re-encode `new_value` keeping ordinary vs raw-string delimiter spelling."""
    text = orig_tok[1]
    if text.startswith("r"):
        q = text.find('"')
        hashes = q - 1
        return f'r{"#" * hashes}"{new_value}"{"#" * hashes}'
    escaped = new_value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def skip_balanced(tokens: list[Token], idx: int) -> int:
    """Advance past a balanced `(`, `[`, or `{` group starting at `tokens[idx]`."""
    open_ch = tokens[idx][1]
    close_ch = {"(": ")", "[": "]", "{": "}"}[open_ch]
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


def has_comment_in_span(src: str, start: int, end: int) -> bool:
    """Return True if raw slice `src[start:end]` contains a Rust comment outside strings."""
    i = start
    while i < end:
        c = src[i]
        if c == "/" and i + 1 < end and src[i + 1] in ("/", "*"):
            return True
        if c == '"':
            i += 1
            while i < end:
                if src[i] == "\\":
                    i += 2
                elif src[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
            continue
        i += 1
    return False


@dataclass(frozen=True)
class AttrInfo:
    name: str
    raw_text: str
    start: int
    end: int
    line: int
    inner_tokens: tuple[Token, ...]
    path_value: str | None = None
    path_str_tok: Token | None = None
    cfg_tokens: tuple[str, ...] | None = None
    has_cfg_attr_path: bool = False


@dataclass(frozen=True)
class VisInfo:
    norm: str
    raw: str
    start: int
    end: int
    line: int
    inner_tokens: tuple[Token, ...]
    has_comment: bool = False


@dataclass(frozen=True)
class ModArm:
    name: str
    vis: str
    attrs: tuple[AttrInfo, ...]
    indent: str
    file_rel: str
    line: int
    start: int
    end: int
    parent_dir: Path
    path_value: str | None = None


@dataclass(frozen=True)
class ReexportDecl:
    side_mod: str
    name: str
    vis: str
    attrs: tuple[AttrInfo, ...]
    indent: str
    line: int
    start: int
    end: int


@dataclass(frozen=True)
class TopItem:
    kind: str
    name: str
    start: int
    end: int
    line: int
    indent: str
    attrs: tuple[AttrInfo, ...]
    vis: VisInfo | None
    mod_arm: ModArm | None = None
    reexport: ReexportDecl | None = None
    alias_pair: tuple[str, str] | None = None


@dataclass(frozen=True)
class FieldSpec:
    field: str
    owner: str
    visibility: str


@dataclass(frozen=True)
class IncludeSite:
    includer: Path
    line: int
    mod_depth: int
    raw_arg: str
    target: Path | None
    is_literal: bool


def parse_outer_attr(tokens: list[Token], idx: int, src: str, line_index: LineIndex) -> tuple[AttrInfo, int] | None:
    """Parse an outer `#[...]` attribute starting at `tokens[idx]`."""
    if idx + 1 >= len(tokens) or tokens[idx][1] != "#" or tokens[idx + 1][1] != "[":
        return None
    end_idx = skip_balanced(tokens, idx + 1)
    start = tokens[idx][2]
    end = tokens[end_idx - 1][3]
    inner = tuple(tokens[idx + 2 : end_idx - 1])
    name = inner[0][1] if inner and inner[0][0] == "IDENT" else ""
    line = line_index.line_of(start)
    raw_text = src[start:end]

    path_value: str | None = None
    path_str_tok: Token | None = None
    cfg_tokens: tuple[str, ...] | None = None
    has_cfg_attr_path = False

    if name == "path" and len(inner) == 3 and inner[1][1] == "=" and inner[2][0] == "STR":
        path_str_tok = inner[2]
        path_value = decode_rust_str(inner[2])
    elif name == "cfg" and len(inner) >= 3 and inner[1][1] == "(" and inner[-1][1] == ")":
        cfg_tokens = tuple(t[1] for t in inner[2:-1])
    elif name == "cfg_attr" and len(inner) >= 3 and inner[1][1] == "(":
        depth = 0
        comma_seen = False
        k = 2
        while k < len(inner) - 1:
            tv = inner[k][1]
            if tv in ("(", "[", "{"):
                depth += 1
            elif tv in (")", "]", "}"):
                depth = max(0, depth - 1)
            elif tv == "," and depth == 0:
                comma_seen = True
                if k + 1 < len(inner) - 1 and inner[k + 1][0] == "IDENT" and norm_ident(inner[k + 1][1]) == "path":
                    has_cfg_attr_path = True
            elif comma_seen and depth == 0 and inner[k][0] == "IDENT" and norm_ident(tv) == "path":
                if k + 1 < len(inner) - 1 and inner[k + 1][1] == "=":
                    has_cfg_attr_path = True
            k += 1

    return (
        AttrInfo(
            name=name,
            raw_text=raw_text,
            start=start,
            end=end,
            line=line,
            inner_tokens=inner,
            path_value=path_value,
            path_str_tok=path_str_tok,
            cfg_tokens=cfg_tokens,
            has_cfg_attr_path=has_cfg_attr_path,
        ),
        end_idx,
    )


def format_norm_vis(inner_tokens: tuple[Token, ...]) -> str:
    if not inner_tokens:
        return "pub"
    parts: list[str] = []
    for k, tok in enumerate(inner_tokens):
        if k > 0 and tok[1] != "::" and inner_tokens[k - 1][1] != "::":
            parts.append(" ")
        parts.append(tok[1])
    return f"pub({''.join(parts)})"


def parse_vis(tokens: list[Token], idx: int, src: str, line_index: LineIndex) -> tuple[VisInfo | None, int]:
    """Parse optional Rust visibility starting at `tokens[idx]`."""
    if idx >= len(tokens) or tokens[idx][0] != "IDENT" or tokens[idx][1] != "pub":
        return None, idx
    start = tokens[idx][2]
    line = line_index.line_of(start)
    if idx + 1 < len(tokens) and tokens[idx + 1][1] == "(":
        end_idx = skip_balanced(tokens, idx + 1)
        end = tokens[end_idx - 1][3]
        inner = tuple(tokens[idx + 2 : end_idx - 1])
        has_comment = has_comment_in_span(src, tokens[idx + 1][2], end)
        return (
            VisInfo(
                norm=format_norm_vis(inner),
                raw=src[start:end],
                start=start,
                end=end,
                line=line,
                inner_tokens=inner,
                has_comment=has_comment,
            ),
            end_idx,
        )
    end = tokens[idx][3]
    return VisInfo(norm="pub", raw="pub", start=start, end=end, line=line, inner_tokens=()), idx + 1


def skip_item_body(tokens: list[Token], idx: int) -> int:
    """Advance past a top-level item ending at `;` or a balanced `{ ... }` block."""
    n = len(tokens)
    while idx < n:
        t = tokens[idx][1]
        if t == ";":
            return idx + 1
        if t == "{":
            return skip_balanced(tokens, idx)
        if t in ("(", "["):
            idx = skip_balanced(tokens, idx)
            continue
        idx += 1
    return n


def scan_top_level_items(src: str, file_rel: str, parent_dir: Path) -> tuple[list[TopItem], list[str]]:
    """Parse top-level Rust items from `src`, attaching outer attributes across comments/blank lines."""
    tokens = tokenize_rust(src)
    line_index = LineIndex(src)
    items: list[TopItem] = []
    errors: list[str] = []
    n = len(tokens)
    i = 0

    while i < n:
        # Record inner attributes `#![...]` so side-file preambles refuse them instead of dropping them.
        if tokens[i][1] == "#" and i + 2 < n and tokens[i + 1][1] == "!" and tokens[i + 2][1] == "[":
            end_idx = skip_balanced(tokens, i + 2)
            start_pos = tokens[i][2]
            end_pos = tokens[end_idx - 1][3]
            items.append(
                TopItem("inner_attr", "", start_pos, end_pos, line_index.line_of(start_pos), line_indent_at(src, start_pos), (), None)
            )
            i = end_idx
            continue

        attrs: list[AttrInfo] = []
        while i < n:
            parsed = parse_outer_attr(tokens, i, src, line_index)
            if parsed is None:
                break
            attr_info, i = parsed
            attrs.append(attr_info)
        if i >= n:
            break

        item_tok_start = attrs[0].start if attrs else tokens[i][2]
        item_line = line_index.line_of(tokens[i][2])
        indent = line_indent_at(src, item_tok_start)

        # `macro_rules! name { ... }`
        if tokens[i][0] == "IDENT" and tokens[i][1] == "macro_rules" and i + 2 < n and tokens[i + 1][1] == "!":
            mname = tokens[i + 2][1] if tokens[i + 2][0] == "IDENT" else ""
            j = i + 3
            if j < n and tokens[j][1] in ("(", "[", "{"):
                j = skip_balanced(tokens, j)
                if j < n and tokens[j][1] == ";":
                    j += 1
            else:
                j = skip_item_body(tokens, j)
            items.append(
                TopItem("macro_rules", mname, item_tok_start, tokens[j - 1][3], item_line, indent, tuple(attrs), None)
            )
            i = j
            continue

        vis, j = parse_vis(tokens, i, src, line_index)
        if j >= n:
            break

        # Item-level qualifiers before keyword (`unsafe`, `async`, `const`, `default`, `extern`).
        k = j
        while k < n and tokens[k][0] == "IDENT" and tokens[k][1] in ("unsafe", "async", "const", "default", "extern"):
            k += 1
            if tokens[k - 1][1] == "extern" and k < n and tokens[k][0] == "STR":
                k += 1
        if k >= n:
            break

        kw_kind, kw_text, _, _ = tokens[k]

        # `mod <name>;` or `mod <name> { ... }`
        if kw_kind == "IDENT" and kw_text == "mod" and k + 1 < n and tokens[k + 1][0] == "IDENT":
            raw_mod_name = tokens[k + 1][1]
            mod_line = line_index.line_of(tokens[k][2])
            if k + 2 < n and tokens[k + 2][1] == ";":
                end_pos = tokens[k + 2][3]
                if raw_mod_name.startswith("r#"):
                    errors.append(
                        f"{file_rel}:{mod_line}: raw-identifier module declaration `mod {raw_mod_name};` is unsupported"
                    )
                    i = k + 3
                    continue
                path_attrs = [a for a in attrs if a.name == "path"]
                cfg_attr_paths = [a for a in attrs if a.has_cfg_attr_path]
                for ca in cfg_attr_paths:
                    errors.append(
                        f"{file_rel}:{ca.line}: module `{raw_mod_name}` uses unsupported `cfg_attr` path attribute"
                    )
                if len(path_attrs) > 1:
                    errors.append(
                        f"{file_rel}:{mod_line}: module `{raw_mod_name}` has multiple `#[path]` attributes"
                    )
                path_val: str | None = None
                if len(path_attrs) == 1:
                    if path_attrs[0].path_value is None:
                        errors.append(
                            f"{file_rel}:{path_attrs[0].line}: module `{raw_mod_name}` has malformed `#[path]` attribute"
                        )
                    else:
                        path_val = path_attrs[0].path_value
                arm = ModArm(
                    name=raw_mod_name,
                    vis=vis.norm if vis else "",
                    attrs=tuple(attrs),
                    indent=indent,
                    file_rel=file_rel,
                    line=mod_line,
                    start=item_tok_start,
                    end=end_pos,
                    parent_dir=parent_dir,
                    path_value=path_val,
                )
                items.append(
                    TopItem("mod_decl", raw_mod_name, item_tok_start, end_pos, mod_line, indent, tuple(attrs), vis, mod_arm=arm)
                )
                i = k + 3
                continue
            if k + 2 < n and tokens[k + 2][1] == "{":
                end_idx = skip_balanced(tokens, k + 2)
                items.append(
                    TopItem(
                        "inline_mod",
                        raw_mod_name,
                        item_tok_start,
                        tokens[end_idx - 1][3],
                        mod_line,
                        indent,
                        tuple(attrs),
                        vis,
                    )
                )
                i = end_idx
                continue

        # `use ...;`
        if kw_kind == "IDENT" and kw_text == "use":
            end_idx = skip_item_body(tokens, k + 1)
            use_toks = tokens[k + 1 : end_idx - 1]
            end_pos = tokens[end_idx - 1][3]
            use_line = line_index.line_of(tokens[k][2])
            if (
                len(use_toks) == 3
                and use_toks[0][1] in ("client_side", "server_side")
                and use_toks[1][1] == "::"
                and use_toks[2][0] == "IDENT"
            ):
                reexp = ReexportDecl(
                    side_mod=use_toks[0][1],
                    name=use_toks[2][1],
                    vis=vis.norm if vis else "",
                    attrs=tuple(attrs),
                    indent=indent,
                    line=use_line,
                    start=item_tok_start,
                    end=end_pos,
                )
                items.append(
                    TopItem(
                        "use_reexport",
                        use_toks[2][1],
                        item_tok_start,
                        end_pos,
                        use_line,
                        indent,
                        tuple(attrs),
                        vis,
                        reexport=reexp,
                    )
                )
            elif (
                len(use_toks) == 3
                and use_toks[0][1] in ("client_side", "server_side")
                and use_toks[1][1] == "as"
                and use_toks[2][1] in ("client_state", "server_state")
            ):
                items.append(
                    TopItem(
                        "use_side_alias",
                        use_toks[2][1],
                        item_tok_start,
                        end_pos,
                        use_line,
                        indent,
                        tuple(attrs),
                        vis,
                        alias_pair=(use_toks[0][1], use_toks[2][1]),
                    )
                )
            elif len(use_toks) == 3 and use_toks[0][1] == "super" and use_toks[1][1] == "::" and use_toks[2][1] == "*":
                items.append(
                    TopItem("use_super_glob", "super::*", item_tok_start, end_pos, use_line, indent, tuple(attrs), vis)
                )
            else:
                items.append(TopItem("use", "", item_tok_start, end_pos, use_line, indent, tuple(attrs), vis))
            i = end_idx
            continue

        # `struct`, `enum`, `fn`, `trait`, `type`, `const`, `static`, `impl`
        if kw_kind == "IDENT" and kw_text in ("struct", "enum", "fn", "trait", "type", "impl"):
            name = tokens[k + 1][1] if (kw_text != "impl" and k + 1 < n and tokens[k + 1][0] == "IDENT") else ""
            end_idx = skip_item_body(tokens, k + 1)
            items.append(
                TopItem(kw_text, name, item_tok_start, tokens[end_idx - 1][3], item_line, indent, tuple(attrs), vis)
            )
            i = end_idx
            continue

        # `const` / `static` (already consumed above when followed by item kw, otherwise constant/static item).
        if k > j and tokens[k - 1][1] in ("const", "static"):
            name = tokens[k][1] if tokens[k][0] == "IDENT" else ""
            end_idx = skip_item_body(tokens, k)
            items.append(
                TopItem(
                    tokens[k - 1][1],
                    name,
                    item_tok_start,
                    tokens[end_idx - 1][3],
                    item_line,
                    indent,
                    tuple(attrs),
                    vis,
                )
            )
            i = end_idx
            continue

        # Top-level macro invocation `<path>::name!(...)`.
        m = j
        if m < n and tokens[m][1] == "::":
            m += 1
        while m + 1 < n and tokens[m][0] == "IDENT" and tokens[m + 1][1] == "::":
            m += 2
        if m + 2 < n and tokens[m][0] == "IDENT" and tokens[m + 1][1] == "!" and tokens[m + 2][1] in ("(", "[", "{"):
            mname = tokens[m][1]
            end_idx = skip_balanced(tokens, m + 2)
            if end_idx < n and tokens[end_idx][1] == ";":
                end_idx += 1
            items.append(
                TopItem("macro_call", mname, item_tok_start, tokens[end_idx - 1][3], item_line, indent, tuple(attrs), vis)
            )
            i = end_idx
            continue

        end_idx = skip_item_body(tokens, max(k, j))
        items.append(TopItem("other", "", item_tok_start, tokens[end_idx - 1][3], item_line, indent, tuple(attrs), vis))
        i = end_idx

    return items, errors


def load_module_sides(table_path: Path) -> dict[str, tuple[str, str]]:
    """Return `module -> (side, reason)` from `game_module_sides.tsv`."""
    sides: dict[str, tuple[str, str]] = {}
    with table_path.open(newline="", encoding="utf-8") as fp:
        reader = csv.DictReader(fp, delimiter="\t")
        if not reader.fieldnames or not {"module", "side", "reason"}.issubset(reader.fieldnames):
            raise SystemExit(f"{table_path}: expected columns module, side, reason")
        for lineno, row in enumerate(reader, 2):
            mod = row["module"].strip()
            side = row["side"].strip()
            reason = row["reason"].strip()
            if not mod or side not in ("client", "server", "game"):
                raise SystemExit(f"{table_path}:{lineno}: invalid row ({mod!r}, {side!r})")
            if mod in SIDE_CONTAINERS:
                raise SystemExit(f"{table_path}:{lineno}: side container `{mod}` must not appear in table")
            if mod in sides:
                raise SystemExit(f"{table_path}:{lineno}: duplicate module `{mod}`")
            sides[mod] = (side, reason)
    return sides


def load_field_specs(fields_table_path: Path) -> dict[str, FieldSpec]:
    """Return `field -> FieldSpec` from `game_field_owners.tsv`."""
    specs: dict[str, FieldSpec] = {}
    with fields_table_path.open(newline="", encoding="utf-8") as fp:
        reader = csv.DictReader(fp, delimiter="\t")
        if not reader.fieldnames or not {"field", "owner", "visibility"}.issubset(reader.fieldnames):
            raise SystemExit(f"{fields_table_path}: expected columns including field, owner, visibility")
        for lineno, row in enumerate(reader, 2):
            fld = row["field"].strip()
            owner = row["owner"].strip()
            vis = row["visibility"].strip()
            if owner in ("client", "server") and vis not in ("side", "game"):
                raise SystemExit(
                    f"{fields_table_path}:{lineno}: {owner} field `{fld}` must have visibility `side` or `game`, got {vis!r}"
                )
            if owner == "game" and vis not in ("-", "game"):
                raise SystemExit(
                    f"{fields_table_path}:{lineno}: game field `{fld}` has unexpected visibility {vis!r}"
                )
            specs[fld] = FieldSpec(field=fld, owner=owner, visibility=vis)
    return specs


def resolve_arm_entry_file(root: Path, arm: ModArm) -> tuple[Path | None, str | None]:
    """Resolve one `ModArm` to its exact entry `.rs` file relative to `arm.parent_dir`."""
    src_root = (root / "crates/sunrise-client/src").resolve()
    if arm.path_value is not None:
        if Path(arm.path_value).is_absolute():
            return None, f"{arm.file_rel}:{arm.line}: module `{arm.name}` has absolute #[path = {arm.path_value!r}]"
        cand = (arm.parent_dir / arm.path_value).resolve()
        if not cand.is_relative_to(src_root):
            return None, f"{arm.file_rel}:{arm.line}: module `{arm.name}` #[path = {arm.path_value!r}] escapes crate src"
        if not cand.is_file():
            return None, f"{arm.file_rel}:{arm.line}: module `{arm.name}` resolved file `{arm.path_value}` does not exist"
        return cand, None

    # Without `#[path]`, both `src/game.rs` and `#[path = "game/*_side.rs"]` search `src/game/`.
    search_dir = src_root / "game"
    c1 = search_dir / f"{arm.name}.rs"
    c2 = search_dir / arm.name / "mod.rs"
    if c1.is_file():
        return c1.resolve(), None
    if c2.is_file():
        return c2.resolve(), None
    return None, f"{arm.file_rel}:{arm.line}: module `{arm.name}` entry file not found under `{search_dir.relative_to(root).as_posix()}`"


def rebase_attr_path(attr: AttrInfo, from_dir: Path, to_dir: Path, src_root: Path) -> tuple[str, str | None]:
    """Rebase `#[path = "..."]` cleanly from `from_dir` to `to_dir`."""
    if attr.name != "path" or attr.path_value is None or attr.path_str_tok is None:
        return attr.raw_text + "\n", None
    target = (from_dir / attr.path_value).resolve()
    if not target.is_relative_to(src_root.resolve()):
        return "", f"line {attr.line}: #[path = {attr.path_value!r}] escapes crate src"
    new_rel = Path(os.path.relpath(target, to_dir.resolve())).as_posix()
    rel_start = attr.path_str_tok[2] - attr.start
    rel_end = attr.path_str_tok[3] - attr.start
    new_lit = encode_rust_str_like(attr.path_str_tok, new_rel)
    return attr.raw_text[:rel_start] + new_lit + attr.raw_text[rel_end:] + "\n", None


def validate_and_map_mod_vis(arm: ModArm) -> tuple[str, str, str | None]:
    """Return `(side_mod_vis, game_reexport_vis, error)` for a moved module arm."""
    v = arm.vis
    if v in ("", "pub(self)", "pub(in self)", "pub(in crate::game)"):
        return "pub(in crate::game)", "pub(in crate::game)", None
    if v == "pub":
        return "pub", "pub", None
    if v in ("pub(crate)", "pub(super)", "pub(in super)", "pub(in crate)"):
        return "pub(crate)", "pub(crate)", None
    return "", "", f"{arm.file_rel}:{arm.line}: module `{arm.name}` has unsupported visibility `{v}`"


def cfg_attrs_for_reexport(arms: list[ModArm]) -> tuple[list[str], str, str | None]:
    """Validate multi-arm cfg guards and visibility; return `(cfg_lines, reexport_vis, error)`."""
    mapped_vis: list[str] = []
    for arm in arms:
        _, rv, err = validate_and_map_mod_vis(arm)
        if err:
            return [], "", err
        mapped_vis.append(rv)
    if len(set(mapped_vis)) != 1:
        return [], "", f"{arms[0].file_rel}:{arms[0].line}: multi-arm module `{arms[0].name}` has mismatched arm visibilities"
    reexp_vis = mapped_vis[0]

    if len(arms) == 1:
        cfgs = [a.raw_text + "\n" for a in arms[0].attrs if a.name in ("cfg", "cfg_attr")]
        return cfgs, reexp_vis, None

    if len(arms) == 2:
        arm_cfgs: list[list[AttrInfo]] = []
        for arm in arms:
            if any(a.name == "cfg_attr" for a in arm.attrs):
                return [], "", f"{arm.file_rel}:{arm.line}: multi-arm module `{arm.name}` uses unsupported `cfg_attr`"
            cfgs = [a for a in arm.attrs if a.name == "cfg"]
            if len(cfgs) != 1 or cfgs[0].cfg_tokens is None:
                return [], "", f"{arm.file_rel}:{arm.line}: multi-arm module `{arm.name}` requires a single #[cfg(...)] per arm"
            arm_cfgs.append(cfgs)
        p0 = arm_cfgs[0][0].cfg_tokens
        p1 = arm_cfgs[1][0].cfg_tokens
        assert p0 is not None and p1 is not None
        is_comp = (
            (len(p1) >= 3 and p1[0] == "not" and p1[1] == "(" and p1[-1] == ")" and p1[2:-1] == p0)
            or (len(p0) >= 3 and p0[0] == "not" and p0[1] == "(" and p0[-1] == ")" and p0[2:-1] == p1)
        )
        if is_comp:
            return [], reexp_vis, None

    return (
        [],
        "",
        f"{arms[0].file_rel}:{arms[0].line}: multi-arm module `{arms[0].name}` does not have complementary #[cfg(P)] / #[cfg(not(P))] guards",
    )


def scan_includes_in_tokens(tokens: list[Token], src: str, file_path: Path) -> list[IncludeSite]:
    """Find every `include!` invocation in `tokens` with its exact inline `mod` nesting depth."""
    line_index = LineIndex(src)
    sites: list[IncludeSite] = []
    n = len(tokens)

    def walk(start: int, end: int, mod_depth: int) -> None:
        i = start
        while i < end:
            # Skip `macro_rules! name { ... }` bodies when discovering module includes.
            if tokens[i][0] == "IDENT" and tokens[i][1] == "macro_rules" and i + 2 < end and tokens[i + 1][1] == "!":
                j = i + 3
                if j < end and tokens[j][1] in ("(", "[", "{"):
                    i = skip_balanced(tokens, j)
                    continue
            if (
                tokens[i][0] == "IDENT"
                and tokens[i][1] == "mod"
                and (i == start or tokens[i - 1][1] not in ("::", "."))
                and i + 2 < end
                and tokens[i + 1][0] == "IDENT"
                and tokens[i + 2][1] == "{"
            ):
                body_end = skip_balanced(tokens, i + 2)
                walk(i + 3, body_end - 1, mod_depth + 1)
                i = body_end
                continue
            if (
                tokens[i][0] == "IDENT"
                and tokens[i][1] == "include"
                and i + 2 < end
                and tokens[i + 1][1] == "!"
                and tokens[i + 2][1] in ("(", "[", "{")
            ):
                prev_ok = i == start or tokens[i - 1][1] != "."
                if prev_ok:
                    arg_end = skip_balanced(tokens, i + 2)
                    arg_toks = tokens[i + 3 : arg_end - 1]
                    raw_arg = src[tokens[i + 2][3] : tokens[arg_end - 1][2]].strip()
                    line = line_index.line_of(tokens[i][2])
                    if len(arg_toks) == 1 and arg_toks[0][0] == "STR":
                        decoded = decode_rust_str(arg_toks[0])
                        target = (file_path.parent / decoded).resolve() if decoded is not None else None
                        sites.append(IncludeSite(file_path.resolve(), line, mod_depth, raw_arg, target, decoded is not None))
                    else:
                        sites.append(IncludeSite(file_path.resolve(), line, mod_depth, raw_arg, None, False))
                    i = arg_end
                    continue
            i += 1

    walk(0, n, 0)
    return sites


def scan_child_mod_decls(
    src: str,
    file_path: Path,
    root: Path,
    base_depth: int,
    is_path_loaded: bool,
    tokens: list[Token] | None = None,
) -> tuple[list[tuple[Path, int, bool]], list[str]]:
    """Find out-of-line child `mod <name>;` files declared in `file_path` (including inside inline mods)."""
    if tokens is None:
        tokens = tokenize_rust(src)
    line_index = LineIndex(src)
    rel = file_path.relative_to(root).as_posix()
    src_root = (root / "crates/sunrise-client/src").resolve()
    children: list[tuple[Path, int, bool]] = []
    errors: list[str] = []
    n = len(tokens)

    def resolve_child(
        mod_name: str,
        inline_path: tuple[str, ...],
        path_val: str | None,
        line: int,
        has_cfg: bool,
    ) -> None:
        if path_val is not None:
            base_dir = file_path.parent if (is_path_loaded or file_path.name == "mod.rs") else (file_path.parent / file_path.stem)
            for seg in inline_path:
                base_dir = base_dir / seg
            cand = (base_dir / path_val).resolve()
            if not cand.is_file() and base_dir != file_path.parent:
                alt = (file_path.parent / path_val).resolve()
                if alt.is_file():
                    cand = alt
            if not cand.is_relative_to(src_root):
                errors.append(f"{rel}:{line}: child module `{mod_name}` #[path = {path_val!r}] escapes crate src")
                return
            if cand.is_file():
                children.append((cand, base_depth + len(inline_path) + 1, True))
            elif not has_cfg:
                errors.append(f"{rel}:{line}: child module `{mod_name}` resolved file `{path_val}` does not exist")
            return

        if file_path.name == "mod.rs":
            search = file_path.parent
        else:
            search = file_path.parent / file_path.stem
        for seg in inline_path:
            search = search / seg
        c1 = search / f"{mod_name}.rs"
        c2 = search / mod_name / "mod.rs"
        if c1.is_file():
            children.append((c1.resolve(), base_depth + len(inline_path) + 1, False))
        elif c2.is_file():
            children.append((c2.resolve(), base_depth + len(inline_path) + 1, False))
        elif not has_cfg:
            errors.append(f"{rel}:{line}: child module `{mod_name}` file not found under `{search.relative_to(root).as_posix()}`")

    def walk(start: int, end: int, inline_path: tuple[str, ...]) -> None:
        i = start
        pending_attrs: list[AttrInfo] = []
        while i < end:
            parsed = parse_outer_attr(tokens, i, src, line_index)
            if parsed is not None:
                attr, i = parsed
                pending_attrs.append(attr)
                continue
            if tokens[i][0] == "IDENT" and tokens[i][1] == "macro_rules" and i + 2 < end and tokens[i + 1][1] == "!":
                pending_attrs.clear()
                j = i + 3
                i = skip_balanced(tokens, j) if (j < end and tokens[j][1] in ("(", "[", "{")) else j
                continue
            vis, j = parse_vis(tokens, i, src, line_index)
            if vis is not None:
                i = j
                continue
            if (
                tokens[i][0] == "IDENT"
                and tokens[i][1] == "mod"
                and (i == start or tokens[i - 1][1] not in ("::", "."))
                and i + 2 < end
                and tokens[i + 1][0] == "IDENT"
            ):
                mod_name = tokens[i + 1][1]
                mod_line = line_index.line_of(tokens[i][2])
                attrs_here = list(pending_attrs)
                pending_attrs.clear()
                if tokens[i + 2][1] == ";":
                    if mod_name.startswith("r#"):
                        errors.append(f"{rel}:{mod_line}: raw-identifier child module `mod {mod_name};` is unsupported")
                        i += 3
                        continue
                    for ca in attrs_here:
                        if ca.has_cfg_attr_path:
                            errors.append(f"{rel}:{ca.line}: child module `{mod_name}` uses unsupported `cfg_attr` path")
                    path_attrs = [a for a in attrs_here if a.name == "path"]
                    path_val = path_attrs[0].path_value if len(path_attrs) == 1 else None
                    has_cfg = any(a.name == "cfg" for a in attrs_here)
                    resolve_child(mod_name, inline_path, path_val, mod_line, has_cfg)
                    i += 3
                    continue
                if tokens[i + 2][1] == "{":
                    body_end = skip_balanced(tokens, i + 2)
                    walk(i + 3, body_end - 1, (*inline_path, mod_name))
                    i = body_end
                    continue
            pending_attrs.clear()
            if tokens[i][1] in ("(", "[", "{"):
                i = skip_balanced(tokens, i)
            else:
                i += 1

    walk(0, n, ())
    return children, errors


def check_relative_vis_at(
    vis: VisInfo, mod_depth: int, rel: str
) -> tuple[str | None, str | None]:
    """Classify a `VisInfo` at `mod_depth`: return `(replacement_or_None, refusal_error_or_None)`."""
    inner = vis.inner_tokens
    if not inner:
        return None, None
    # `pub(super)`
    if len(inner) == 1 and inner[0][0] == "IDENT" and inner[0][1] == "super":
        if mod_depth == 0:
            if vis.has_comment:
                return None, f"{rel}:{vis.line}: commented relative visibility `{vis.raw}` is unsupported"
            return "pub(in crate::game)", None
        return None, None
    # `pub(in super...)`
    if len(inner) >= 2 and inner[0][0] == "IDENT" and inner[0][1] == "in":
        k = 1
        super_count = 0
        while k < len(inner) and inner[k][0] == "IDENT" and inner[k][1] == "super":
            super_count += 1
            k += 1
            if k < len(inner) and inner[k][1] == "::":
                k += 1
            else:
                break
        if super_count > 0 and super_count > mod_depth:
            return (
                None,
                f"{rel}:{vis.line}: relative visibility `{vis.raw}` crosses moved module boundary (depth {mod_depth})",
            )
    return None, None


def find_rel_vis_in_token_range(
    tokens: list[Token], start: int, end: int, src: str, line_index: LineIndex, mod_depth: int
) -> list[VisInfo]:
    """Return any `pub(super)` or boundary-crossing `pub(in super...)` inside `tokens[start:end]`."""
    hits: list[VisInfo] = []
    i = start
    while i < end:
        if tokens[i][0] == "IDENT" and tokens[i][1] == "pub" and i + 1 < end and tokens[i + 1][1] == "(":
            vis, next_i = parse_vis(tokens, i, src, line_index)
            if vis is not None:
                rep, err = check_relative_vis_at(vis, mod_depth, "")
                if rep is not None or err is not None:
                    hits.append(vis)
            i = next_i
            continue
        i += 1
    return hits


def scan_macro_name_uses_in_game(
    root: Path, moved_root_files: set[Path], macro_name: str, def_file: Path
) -> list[str]:
    """Verify every non-definition, non-import mention of `macro_name` in `game` is at root scope of a moved file."""
    bad: list[str] = []
    game_rs = (root / "crates/sunrise-client/src/game.rs").resolve()
    game_dir = (root / "crates/sunrise-client/src/game").resolve()
    candidates = [game_rs] if game_rs.is_file() else []
    if game_dir.is_dir():
        candidates.extend(sorted(p.resolve() for p in game_dir.rglob("*.rs")))

    for path in candidates:
        txt = path.read_text(encoding="utf-8")
        if macro_name not in txt:
            continue
        tokens = tokenize_rust(txt)
        line_index = LineIndex(txt)
        rel = path.relative_to(root).as_posix()
        n = len(tokens)

        def walk(start: int, end: int, mod_depth: int) -> None:
            i = start
            while i < end:
                if tokens[i][0] == "IDENT" and tokens[i][1] == "macro_rules" and i + 2 < end and tokens[i + 1][1] == "!":
                    j = i + 3
                    if j < end and tokens[j][1] in ("(", "[", "{"):
                        i = skip_balanced(tokens, j)
                        continue
                if tokens[i][0] == "IDENT" and tokens[i][1] == "use":
                    i = skip_item_body(tokens, i + 1)
                    continue
                if (
                    tokens[i][0] == "IDENT"
                    and tokens[i][1] == "mod"
                    and (i == start or tokens[i - 1][1] not in ("::", "."))
                    and i + 2 < end
                    and tokens[i + 1][0] == "IDENT"
                    and tokens[i + 2][1] == "{"
                ):
                    body_end = skip_balanced(tokens, i + 2)
                    walk(i + 3, body_end - 1, mod_depth + 1)
                    i = body_end
                    continue
                if tokens[i][0] == "IDENT" and tokens[i][1] == macro_name:
                    if path not in moved_root_files or mod_depth != 0:
                        bad.append(f"{rel}:{line_index.line_of(tokens[i][2])}")
                i += 1

        walk(0, n, 0)
    return bad


def analyze_file_visibility(
    src: str,
    file_path: Path,
    root: Path,
    base_depth: int,
    moved_root_files: set[Path],
    tokens: list[Token] | None = None,
) -> tuple[str, int, list[int], list[str]]:
    """Analyze and rewrite root-scope `pub(super)` in `file_path` at `base_depth`, refusing unsafe forms."""
    if tokens is None:
        tokens = tokenize_rust(src)
    line_index = LineIndex(src)
    rel = file_path.relative_to(root).as_posix()
    edits: list[tuple[int, int, str]] = []
    lines_hit: list[int] = []
    errors: list[str] = []
    n = len(tokens)

    def walk_expansion(start: int, end: int, mod_depth: int, macro_name: str) -> None:
        i = start
        while i < end:
            if (
                tokens[i][0] == "IDENT"
                and tokens[i][1] == "mod"
                and (i == start or tokens[i - 1][1] not in ("::", "."))
                and i + 2 < end
                and tokens[i + 1][0] == "IDENT"
                and tokens[i + 2][1] == "{"
            ):
                body_end = skip_balanced(tokens, i + 2)
                walk_expansion(i + 3, body_end - 1, mod_depth + 1, macro_name)
                i = body_end
                continue
            if (
                tokens[i][0] == "IDENT"
                and tokens[i][1] not in NON_MACRO_KEYWORDS
                and i + 2 < end
                and tokens[i + 1][1] == "!"
                and tokens[i + 2][1] in ("(", "[", "{")
            ):
                call_end = skip_balanced(tokens, i + 2)
                for hit in find_rel_vis_in_token_range(tokens, i + 3, call_end - 1, src, line_index, mod_depth):
                    errors.append(
                        f"{rel}:{hit.line}: macro_rules! `{macro_name}` expansion passes relative visibility `{hit.raw}` to `{tokens[i][1]}!`"
                    )
                i = call_end
                continue
            if tokens[i][0] == "IDENT" and tokens[i][1] == "pub" and i + 1 < end and tokens[i + 1][1] == "(":
                vis, next_i = parse_vis(tokens, i, src, line_index)
                if vis is not None:
                    rep, err = check_relative_vis_at(vis, mod_depth, rel)
                    if err:
                        errors.append(err)
                    elif rep is not None:
                        edits.append((vis.start, vis.end, rep))
                        lines_hit.append(vis.line)
                i = next_i
                continue
            i += 1

    def handle_macro_rules(
        idx: int, end: int, mod_depth: int, pending_attrs: list[AttrInfo]
    ) -> int:
        macro_name = tokens[idx + 2][1] if idx + 2 < end and tokens[idx + 2][0] == "IDENT" else "<unnamed>"
        macro_line = line_index.line_of(tokens[idx][2])
        body_idx = idx + 3
        if body_idx >= end or tokens[body_idx][1] not in ("(", "[", "{"):
            return body_idx
        body_end = skip_balanced(tokens, body_idx)
        is_exported = any(a.name == "macro_export" for a in pending_attrs)

        # Parse `(matcher) => (expansion);` rules inside `macro_rules!`.
        r = body_idx + 1
        expansions_with_vis: list[tuple[int, int]] = []
        while r < body_end - 1:
            if tokens[r][1] not in ("(", "[", "{"):
                r += 1
                continue
            matcher_end = skip_balanced(tokens, r)
            for hit in find_rel_vis_in_token_range(tokens, r + 1, matcher_end - 1, src, line_index, mod_depth):
                errors.append(
                    f"{rel}:{hit.line}: macro_rules! `{macro_name}` matcher contains relative visibility `{hit.raw}`"
                )
            if matcher_end + 1 < body_end and tokens[matcher_end][1] == "=>" and tokens[matcher_end + 1][1] in ("(", "[", "{"):
                exp_start = matcher_end + 1
                exp_end = skip_balanced(tokens, exp_start)
                check_depth = 0 if is_exported else mod_depth
                if find_rel_vis_in_token_range(tokens, exp_start + 1, exp_end - 1, src, line_index, check_depth):
                    expansions_with_vis.append((exp_start + 1, exp_end - 1))
                r = exp_end
                if r < body_end - 1 and tokens[r][1] == ";":
                    r += 1
            else:
                r = matcher_end

        if expansions_with_vis:
            if is_exported:
                errors.append(
                    f"{rel}:{macro_line}: #[macro_export] macro_rules! `{macro_name}` emits relative visibility with unknown expansion scope"
                )
            elif mod_depth != 0:
                for es, ee in expansions_with_vis:
                    walk_expansion(es, ee, mod_depth, macro_name)
            else:
                bad_uses = scan_macro_name_uses_in_game(root, moved_root_files, macro_name, file_path)
                if bad_uses:
                    errors.append(
                        f"{rel}:{macro_line}: macro_rules! `{macro_name}` emits relative visibility and is referenced outside moved root scope ({', '.join(bad_uses[:3])})"
                    )
                else:
                    for es, ee in expansions_with_vis:
                        walk_expansion(es, ee, mod_depth, macro_name)
        return body_end

    def walk(start: int, end: int, mod_depth: int) -> None:
        i = start
        pending_attrs: list[AttrInfo] = []
        while i < end:
            parsed = parse_outer_attr(tokens, i, src, line_index)
            if parsed is not None:
                attr, i = parsed
                pending_attrs.append(attr)
                continue

            if tokens[i][0] == "IDENT" and tokens[i][1] == "macro_rules" and i + 2 < end and tokens[i + 1][1] == "!":
                attrs_here = list(pending_attrs)
                pending_attrs.clear()
                i = handle_macro_rules(i, end, mod_depth, attrs_here)
                continue

            if (
                tokens[i][0] == "IDENT"
                and tokens[i][1] == "mod"
                and (i == start or tokens[i - 1][1] not in ("::", "."))
                and i + 2 < end
                and tokens[i + 1][0] == "IDENT"
                and tokens[i + 2][1] == "{"
            ):
                pending_attrs.clear()
                body_end = skip_balanced(tokens, i + 2)
                walk(i + 3, body_end - 1, mod_depth + 1)
                i = body_end
                continue

            if (
                tokens[i][0] == "IDENT"
                and tokens[i][1] not in NON_MACRO_KEYWORDS
                and i + 2 < end
                and tokens[i + 1][1] == "!"
                and tokens[i + 2][1] in ("(", "[", "{")
            ):
                pending_attrs.clear()
                mname = tokens[i][1]
                call_end = skip_balanced(tokens, i + 2)
                if mname != "include":
                    for hit in find_rel_vis_in_token_range(tokens, i + 3, call_end - 1, src, line_index, mod_depth):
                        errors.append(
                            f"{rel}:{hit.line}: macro invocation `{mname}!` contains relative visibility `{hit.raw}`"
                        )
                i = call_end
                continue

            if tokens[i][0] == "IDENT" and tokens[i][1] == "pub" and i + 1 < end and tokens[i + 1][1] == "(":
                pending_attrs.clear()
                vis, next_i = parse_vis(tokens, i, src, line_index)
                if vis is not None:
                    rep, err = check_relative_vis_at(vis, mod_depth, rel)
                    if err:
                        errors.append(err)
                    elif rep is not None:
                        edits.append((vis.start, vis.end, rep))
                        lines_hit.append(vis.line)
                i = next_i
                continue

            if tokens[i][0] == "IDENT" and tokens[i][1] == "pub":
                i += 1
                continue

            pending_attrs.clear()
            i += 1

    walk(0, n, base_depth)
    if not edits or errors:
        return src, len(edits), lines_hit, errors

    parts: list[str] = []
    last = 0
    for s, e, rep in edits:
        parts.append(src[last:s])
        parts.append(rep)
        last = e
    parts.append(src[last:])
    return "".join(parts), len(edits), lines_hit, errors


def find_struct_span(text: str, struct_name: str) -> tuple[int, int, int, int, list[Token], int, int]:
    """Return `(decl_start, body_open_offset, body_close_offset, decl_end, tokens, open_tok_idx, close_tok_idx)`."""
    tokens = tokenize_rust(text)
    n = len(tokens)
    i = 0
    while i < n:
        if tokens[i][1] == "#" and i + 1 < n and tokens[i + 1][1] in ("[", "!"):
            if tokens[i + 1][1] == "!" and i + 2 < n and tokens[i + 2][1] == "[":
                i = skip_balanced(tokens, i + 2)
                continue
            i = skip_balanced(tokens, i + 1)
            continue
        decl_start = tokens[i][2]
        _, j = parse_vis(tokens, i, text, LineIndex(text))
        if (
            j + 1 < n
            and tokens[j][0] == "IDENT"
            and tokens[j][1] == "struct"
            and tokens[j + 1][0] == "IDENT"
            and norm_ident(tokens[j + 1][1]) == struct_name
        ):
            k = j + 2
            while k < n and tokens[k][1] not in ("{", ";"):
                if tokens[k][1] in ("(", "["):
                    k = skip_balanced(tokens, k)
                else:
                    k += 1
            if k < n and tokens[k][1] == "{":
                close_after = skip_balanced(tokens, k)
                close_tok = close_after - 1
                return (
                    decl_start,
                    tokens[k][2],
                    tokens[close_tok][2],
                    tokens[close_tok][3],
                    tokens,
                    k,
                    close_tok,
                )
        if tokens[i][1] in ("(", "[", "{"):
            i = skip_balanced(tokens, i)
        else:
            i += 1
    raise ValueError(f"struct `{struct_name}` not found")


def parse_struct_fields(text: str, struct_name: str, rel: str = "") -> list[tuple[str, str, int, int]]:
    """Return `[(canonical_field_name, norm_vis, vis_start, vis_end)]` for named fields of `struct_name`."""
    _, _, _, _, tokens, open_tok, close_tok = find_struct_span(text, struct_name)
    line_index = LineIndex(text)
    fields: list[tuple[str, str, int, int]] = []
    i = open_tok + 1

    while i < close_tok:
        while i < close_tok and tokens[i][1] == "#" and i + 1 < close_tok and tokens[i + 1][1] == "[":
            i = skip_balanced(tokens, i + 1)
        if i >= close_tok:
            break
        vis, j = parse_vis(tokens, i, text, line_index)
        if j >= close_tok or tokens[j][0] != "IDENT":
            ln = line_index.line_of(tokens[j][2] if j < close_tok else tokens[close_tok][2])
            raise ValueError(f"{rel}:{ln}: unrecognized field syntax inside `struct {struct_name}`")
        raw_fname = tokens[j][1]
        fname = norm_ident(raw_fname)
        if j + 1 >= close_tok or tokens[j + 1][1] != ":":
            ln = line_index.line_of(tokens[j][2])
            raise ValueError(f"{rel}:{ln}: expected `:` after field `{raw_fname}` in `struct {struct_name}`")
        if vis is not None:
            vis_start = vis.start
            vis_end = tokens[j][2]
            vis_norm = vis.norm
        else:
            vis_start = tokens[j][2]
            vis_end = tokens[j][2]
            vis_norm = ""
        fields.append((fname, vis_norm, vis_start, vis_end))

        # Skip the field type tokens to the next top-level comma or end of struct body.
        k = j + 2
        angle_depth = 0
        while k < close_tok:
            t = tokens[k][1]
            if t in ("(", "[", "{"):
                k = skip_balanced(tokens, k)
                continue
            if t == "<":
                angle_depth += 1
            elif t == ">":
                angle_depth = max(0, angle_depth - 1)
            elif t == "," and angle_depth == 0:
                k += 1
                break
            k += 1
        i = k

    return fields


def check_state_struct_rules(text: str, struct_name: str, rel: str) -> list[str]:
    """Check Wall Rules 1 & 2: no derives and no Deref/AsRef impls on ClientState/ServerState."""
    problems: list[str] = []
    decl_start, _, _, _, tokens, _, _ = find_struct_span(text, struct_name)
    line_index = LineIndex(text)
    items, _ = scan_top_level_items(text, rel, Path("."))
    for item in items:
        if item.kind == "struct" and norm_ident(item.name) == struct_name:
            for attr in item.attrs:
                if attr.name == "derive" or (attr.name == "cfg_attr" and "derive" in attr.raw_text):
                    problems.append(f"{rel}:{attr.line}: `{struct_name}` must not carry `#[derive(...)]`")
        elif item.kind == "impl":
            impl_toks = [t for t in tokens if item.start <= t[2] and t[3] <= item.end]
            for_idx = next((idx for idx, t in enumerate(impl_toks) if t[0] == "IDENT" and t[1] == "for"), None)
            if for_idx is not None:
                after_for = [t[1] for t in impl_toks[for_idx + 1 :]]
                if struct_name in after_for:
                    before_for = [t[1] for t in impl_toks[:for_idx]]
                    if any(tr in before_for for tr in ("Deref", "DerefMut", "AsRef", "AsMut")):
                        problems.append(
                            f"{rel}:{line_index.line_of(item.start)}: `{struct_name}` must not implement Deref/DerefMut/AsRef/AsMut"
                        )
    return problems


def rewrite_state_file(
    src_text: str,
    side: str,
    moved_arms: list[ModArm],
    field_specs: dict[str, FieldSpec],
    root: Path,
) -> tuple[str, list[str]]:
    """Build the updated `{client,server}_side.rs` text and return `(new_text, errors)`."""
    side_mod = SIDE_MOD_NAME[side]
    struct_name = SIDE_STRUCT_NAME[side]
    rel = f"crates/sunrise-client/src/game/{SIDE_NEW_FILE[side]}"
    src_root = (root / "crates/sunrise-client/src").resolve()
    side_dir = src_root / "game"
    errors: list[str] = []

    try:
        decl_start, _, _, _, _, _, _ = find_struct_span(src_text, struct_name)
    except ValueError as err:
        return src_text, [f"{rel}: {err}"]

    items, scan_errs = scan_top_level_items(src_text, rel, side_dir)
    errors.extend(scan_errs)

    # Refuse unexpected items in the preamble before `struct {Client,Server}State`.
    struct_item: TopItem | None = None
    for item in items:
        if item.kind == "struct" and norm_ident(item.name) == struct_name:
            struct_item = item
            break
        if item.kind in ("use_super_glob", "mod_decl"):
            continue
        errors.append(
            f"{rel}:{item.line}: unexpected `{item.kind}` item before `struct {struct_name}` in side-file preamble"
        )
    if errors:
        return src_text, errors

    # Preserve leading `//!` inner doc comments (refuse `#![...]` inner attributes).
    preamble_end = struct_item.start if struct_item is not None else decl_start
    preamble_lines = src_text[:preamble_end].splitlines(keepends=True)
    doc_lines: list[str] = []
    idx = 0
    while idx < len(preamble_lines) and (
        preamble_lines[idx].lstrip().startswith("//!") or not preamble_lines[idx].strip()
    ):
        if preamble_lines[idx].lstrip().startswith("//!"):
            doc_lines.append(preamble_lines[idx])
        idx += 1

    # Preserve `///` doc comments and outer attributes directly attached to the state struct.
    struct_attr_lines: list[str] = []
    if struct_item is not None:
        doc_end_offset = struct_item.start
        attr_prefix_lines = src_text[:doc_end_offset].splitlines(keepends=True)
        rev = len(attr_prefix_lines) - 1
        while rev >= idx and attr_prefix_lines[rev].lstrip().startswith("///"):
            struct_attr_lines.append(attr_prefix_lines[rev])
            rev -= 1
        struct_attr_lines.reverse()
        if struct_item.attrs:
            struct_attr_lines.append(src_text[struct_item.start : decl_start])

    mod_lines: list[str] = []
    for arm in moved_arms:
        side_vis, _, vis_err = validate_and_map_mod_vis(arm)
        if vis_err:
            errors.append(vis_err)
            continue
        for attr in arm.attrs:
            rebased, rerr = rebase_attr_path(attr, arm.parent_dir, side_dir, src_root)
            if rerr:
                errors.append(f"{arm.file_rel}: {rerr}")
                continue
            mod_lines.append(rebased)
        mod_lines.append(f"{side_vis} mod {arm.name};\n")
    if errors:
        return src_text, errors

    header = "".join(doc_lines)
    header += f"#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use super::*;\n\n"
    if mod_lines:
        header += "".join(mod_lines) + "\n"
    header += "".join(struct_attr_lines)

    rest = src_text[decl_start:]
    try:
        fields = parse_struct_fields(rest, struct_name, rel)
    except ValueError as err:
        return src_text, [str(err)]

    seen_fields: set[str] = set()
    edits: list[tuple[int, int, str]] = []
    for fname, _actual_vis, vstart, vend in fields:
        seen_fields.add(fname)
        spec = field_specs.get(fname)
        if spec is None or spec.owner != side:
            errors.append(
                f"{SIDE_NEW_FILE[side]}: field `{fname}` of `{struct_name}` missing or wrong owner in game_field_owners.tsv"
            )
            continue
        target_vis = f"pub(in crate::game::{side_mod})" if spec.visibility == "side" else "pub(in crate::game)"
        edits.append((vstart, vend, f"{target_vis} "))

    expected = {f for f, s in field_specs.items() if s.owner == side}
    for missing in sorted(expected - seen_fields):
        errors.append(f"{SIDE_NEW_FILE[side]}: expected `{side}` field `{missing}` missing from `{struct_name}`")
    if errors:
        return src_text, errors

    parts: list[str] = []
    last = 0
    for s, e, rep in edits:
        parts.append(rest[last:s])
        parts.append(rep)
        last = e
    parts.append(rest[last:])
    return header + "".join(parts), errors


def rewrite_game_rs(
    game_text: str,
    table: dict[str, tuple[str, str]],
    all_arms: dict[str, list[ModArm]],
    root: Path,
) -> tuple[str, list[str]]:
    """Regenerate `crates/sunrise-client/src/game.rs` module declarations and re-exports."""
    src_root = (root / "crates/sunrise-client/src").resolve()
    game_rel = "crates/sunrise-client/src/game.rs"
    items, errors = scan_top_level_items(game_text, game_rel, src_root)
    if errors:
        return game_text, errors

    emitted_reexports: set[str] = set()
    emitted_side_containers: set[str] = set()
    emitted_game_mods: set[str] = set()
    edits: list[tuple[int, int, str]] = []

    def side_container_text(side: str, indent: str) -> str:
        if side in emitted_side_containers:
            return ""
        emitted_side_containers.add(side)
        smod = SIDE_MOD_NAME[side]
        old_alias = "client_state" if side == "client" else "server_state"
        return (
            f'{indent}#[path = "game/{smod}.rs"]\n'
            f"{indent}pub(in crate::game) mod {smod};\n"
            f"{indent}#[allow(unused_imports)]\n"
            f"{indent}pub(in crate::game) use {smod} as {old_alias};\n"
        )

    def moved_reexport_text(mod_name: str, target_side: str, indent: str) -> str:
        if mod_name in emitted_reexports:
            return ""
        emitted_reexports.add(mod_name)
        arms = all_arms[mod_name]
        cfgs, reexp_vis, cfg_err = cfg_attrs_for_reexport(arms)
        if cfg_err:
            errors.append(cfg_err)
            return ""
        smod = SIDE_MOD_NAME[target_side]
        buf = [f"{indent}{c}" for c in cfgs]
        buf.append(f"{indent}#[allow(unused_imports)]\n")
        buf.append(f"{indent}{reexp_vis} use {smod}::{mod_name};\n")
        return "".join(buf)

    def game_mod_text(mod_name: str, indent: str) -> str:
        if mod_name in emitted_game_mods:
            return ""
        emitted_game_mods.add(mod_name)
        buf: list[str] = []
        for arm in all_arms[mod_name]:
            for attr in arm.attrs:
                rebased, rerr = rebase_attr_path(attr, arm.parent_dir, src_root, src_root)
                if rerr:
                    errors.append(f"{arm.file_rel}: {rerr}")
                    continue
                buf.append(f"{indent}{rebased}")
            vis_prefix = f"{arm.vis} " if arm.vis else ""
            buf.append(f"{indent}{vis_prefix}mod {mod_name};\n")
        return "".join(buf)

    for item in items:
        span_start = line_start_of(game_text, item.start)
        span_end = consume_trailing_newline(game_text, item.end)
        indent = item.indent

        if item.kind == "mod_decl" and item.mod_arm is not None:
            name = item.mod_arm.name
            if name in ("client_state", "client_side"):
                edits.append((span_start, span_end, side_container_text("client", indent)))
                continue
            if name in ("server_state", "server_side"):
                edits.append((span_start, span_end, side_container_text("server", indent)))
                continue
            if name not in table:
                errors.append(f"{game_rel}:{item.line}: undeclared module `{name}` missing from table")
                continue
            target_side = table[name][0]
            if target_side in ("client", "server"):
                edits.append((span_start, span_end, moved_reexport_text(name, target_side, indent)))
            else:
                emitted_game_mods.add(name)
            continue

        if item.kind == "use_side_alias" and item.alias_pair is not None:
            side = "client" if item.alias_pair[0] == "client_side" else "server"
            edits.append((span_start, span_end, side_container_text(side, indent)))
            continue

        if item.kind == "use_reexport" and item.reexport is not None:
            name = item.reexport.name
            if name not in table:
                errors.append(f"{game_rel}:{item.line}: re-exported module `{name}` missing from table")
                continue
            target_side = table[name][0]
            if target_side in ("client", "server"):
                edits.append((span_start, span_end, moved_reexport_text(name, target_side, indent)))
            else:
                edits.append((span_start, span_end, game_mod_text(name, indent)))

    if errors:
        return game_text, errors

    parts: list[str] = []
    last = 0
    for s, e, rep in edits:
        parts.append(game_text[last:s])
        parts.append(rep)
        last = e
    parts.append(game_text[last:])
    return "".join(parts), errors


def normalize_game_prefix(mod_name: str, mod_to_side: dict[str, str], full: bool = False) -> str:
    """Return canonical `game::[side::]<mod>::` (or with `sunrise_client::`)."""
    prefix = "sunrise_client::game::" if full else "game::"
    side = mod_to_side.get(mod_name)
    if side in ("client", "server"):
        return f"{prefix}{SIDE_MOD_NAME[side]}::{mod_name}::"
    return f"{prefix}{mod_name}::"


def rewrite_game_paths_in_text(text: str, mod_to_side: dict[str, str], *, full: bool = False) -> str:
    pattern = FULL_GAME_PATH_RE if full else GAME_PATH_RE

    def repl(m: re.Match[str]) -> str:
        mod_name = m.group(2)
        if mod_name not in mod_to_side:
            return m.group(0)
        return normalize_game_prefix(mod_name, mod_to_side, full=full)

    return pattern.sub(repl, text)


def rewrite_state_type_paths(text: str) -> str:
    def repl(m: re.Match[str]) -> str:
        ty = m.group(2)
        side_mod = "client_side" if ty == "ClientState" else "server_side"
        return f"sunrise_client::game::{side_mod}::{ty}"

    return STATE_TYPE_PATH_RE.sub(repl, text)


def rewrite_rust_test_literals(
    text: str, mod_to_side: dict[str, str], tokens: list[Token] | None = None
) -> str:
    """Rewrite recognized Rust test-selector string literals (ordinary and raw), preserving unrelated data."""
    if tokens is None:
        tokens = tokenize_rust(text)
    n = len(tokens)

    # Collect declared `fn` and `mod` names in this file and test-runner string argument token indices.
    declared_fns_and_mods: set[str] = set()
    runner_arg_indices: set[int] = set()
    for i, tok in enumerate(tokens):
        if tok[0] == "IDENT" and tok[1] in ("fn", "mod") and i + 1 < n and tokens[i + 1][0] == "IDENT":
            declared_fns_and_mods.add(norm_ident(tokens[i + 1][1]))
        if tok[0] == "IDENT" and tok[1] in TEST_RUNNER_FNS and i + 1 < n and tokens[i + 1][1] == "(":
            end_i = skip_balanced(tokens, i + 1)
            for k in range(i + 2, end_i - 1):
                if tokens[k][0] == "STR":
                    runner_arg_indices.add(k)
        if tok[0] == "IDENT" and tok[1] == "format" and i + 2 < n and tokens[i + 1][1] == "!" and tokens[i + 2][1] == "(":
            if i + 3 < n and tokens[i + 3][0] == "STR":
                runner_arg_indices.add(i + 3)
        if tok[0] == "STR" and decode_rust_str(tok) == "--exact":
            for k in range(i + 1, min(n, i + 6)):
                if tokens[k][0] == "STR":
                    runner_arg_indices.add(k)
                    break

    edits: list[tuple[int, int, str]] = []
    for i, tok in enumerate(tokens):
        if tok[0] != "STR":
            continue
        decoded = decode_rust_str(tok)
        if decoded is None:
            continue
        m = RUST_GAME_SELECTOR_RE.fullmatch(decoded)
        if not m:
            continue
        mod_name = m.group(2)
        tail = m.group(3)
        if mod_name not in mod_to_side:
            continue
        last_seg = tail.rsplit("::", 1)[-1]
        is_selector = (
            last_seg in declared_fns_and_mods
            or i in runner_arg_indices
            or ("{" in last_seg and i in runner_arg_indices)
        )
        if not is_selector:
            continue
        side = mod_to_side[mod_name]
        if side in ("client", "server"):
            new_val = f"game::{SIDE_MOD_NAME[side]}::{mod_name}{tail}"
        else:
            new_val = f"game::{mod_name}{tail}"
        if new_val != decoded:
            edits.append((tok[2], tok[3], encode_rust_str_like(tok, new_val)))

    if not edits:
        return text
    parts: list[str] = []
    last = 0
    for s, e, rep in edits:
        parts.append(text[last:s])
        parts.append(rep)
        last = e
    parts.append(text[last:])
    return "".join(parts)


def rewrite_ignored_tests_file(text: str, mod_to_side: dict[str, str]) -> str:
    """Rewrite column 2 of `sunrise-client` rows in `tools/ci/ignored-tests/*.txt`."""
    out: list[str] = []
    for line in text.splitlines(keepends=True):
        stripped = line.lstrip()
        if not stripped or stripped.startswith("#"):
            out.append(line)
            continue
        m = re.match(r"^(\s*sunrise-client\s+)(game::\S+)(.*)$", line, re.S)
        if not m:
            out.append(line)
            continue
        new_test = rewrite_game_paths_in_text(m.group(2), mod_to_side, full=False)
        out.append(m.group(1) + new_test + m.group(3))
    return "".join(out)


def rewrite_ignored_exceptions_json(text: str, mod_to_side: dict[str, str]) -> str:
    """Rewrite `"test"` fields of `sunrise-client` entries in `ignored-test-exceptions.json`."""
    data = json.loads(text)
    changed = False
    for entry in data:
        if entry.get("package") == "sunrise-client" and isinstance(entry.get("test"), str):
            old = entry["test"]
            new = rewrite_game_paths_in_text(old, mod_to_side, full=False)
            if new != old:
                entry["test"] = new
                changed = True
    if not changed:
        return text
    return json.dumps(data, indent=2, ensure_ascii=False) + "\n"


def rewrite_test_areas_toml(text: str, mod_to_side: dict[str, str]) -> str:
    """Rewrite `tools/test_areas.toml` `sunrise-client` `game::` filters from the module table."""
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    i = 0
    while i < len(lines):
        m = TEST_AREAS_FILTER_RE.match(lines[i])
        if not m:
            out.append(lines[i])
            i += 1
            continue
        head, _full_filter, _old_side, mod_or_prefix, suffix, tail = m.groups()
        suffix = suffix or ""
        j = i + 1
        while j < len(lines):
            mj = TEST_AREAS_FILTER_RE.match(lines[j])
            if mj and mj.group(4) == mod_or_prefix and (mj.group(5) or "") == suffix:
                j += 1
            else:
                break

        if suffix:
            if mod_or_prefix in mod_to_side:
                side = mod_to_side[mod_or_prefix]
                new_filter = (
                    f"game::{SIDE_MOD_NAME[side]}::{mod_or_prefix}{suffix}"
                    if side in ("client", "server")
                    else f"game::{mod_or_prefix}{suffix}"
                )
                out.append(f"{head}{new_filter}{tail}")
            else:
                out.extend(lines[i:j])
        else:
            matching_sides = {
                side for mod, side in mod_to_side.items() if mod.startswith(mod_or_prefix)
            }
            if not matching_sides:
                out.extend(lines[i:j])
            else:
                ordered: list[str] = []
                if "game" in matching_sides:
                    ordered.append(f"game::{mod_or_prefix}")
                if "client" in matching_sides:
                    ordered.append(f"game::client_side::{mod_or_prefix}")
                if "server" in matching_sides:
                    ordered.append(f"game::server_side::{mod_or_prefix}")
                for filt in ordered:
                    out.append(f"{head}{filt}{tail}")
        i = j
    return "".join(out)


def rewrite_doc_file(text: str, mod_to_side: dict[str, str]) -> str:
    """Rewrite exact test selectors and State struct paths in documentation."""
    new = rewrite_state_type_paths(text)
    new = rewrite_game_paths_in_text(new, mod_to_side, full=True)
    pat = re.compile(r"([\s`'\"])game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::)")

    def repl(m: re.Match[str]) -> str:
        mod_name = m.group(3)
        if mod_name not in mod_to_side:
            return m.group(0)
        return m.group(1) + normalize_game_prefix(mod_name, mod_to_side, full=False)

    return pat.sub(repl, new)


def collect_all_arms(root: Path) -> tuple[dict[str, list[ModArm]], dict[str, str], dict[str, list[Path]], list[str]]:
    """Collect out-of-line gameplay `mod <name>;` declarations across `game.rs`, `client_side.rs`, `server_side.rs`."""
    src_root = (root / "crates/sunrise-client/src").resolve()
    game_rs = src_root / "game.rs"
    game_dir = src_root / "game"
    arms: dict[str, list[ModArm]] = {}
    parents: dict[str, str] = {}
    resolved_entries: dict[str, list[Path]] = {}
    errors: list[str] = []

    for parent, path, decl_dir in [
        ("game", game_rs, src_root),
        ("client", game_dir / "client_side.rs", game_dir),
        ("server", game_dir / "server_side.rs", game_dir),
    ]:
        if not path.is_file():
            continue
        rel = path.relative_to(root).as_posix()
        items, scan_errs = scan_top_level_items(path.read_text(encoding="utf-8"), rel, decl_dir)
        errors.extend(scan_errs)
        for item in items:
            if item.kind != "mod_decl" or item.mod_arm is None:
                continue
            arm = item.mod_arm
            if arm.name in SIDE_CONTAINERS:
                continue
            if arm.name in parents and parents[arm.name] != parent:
                errors.append(
                    f"{rel}:{arm.line}: module `{arm.name}` declared in multiple parents ({parents[arm.name]} and {parent})"
                )
                continue
            arms.setdefault(arm.name, []).append(arm)
            parents[arm.name] = parent
            entry, rerr = resolve_arm_entry_file(root, arm)
            if rerr:
                errors.append(rerr)
            elif entry is not None:
                resolved_entries.setdefault(arm.name, []).append(entry)

    return arms, parents, resolved_entries, errors


def parse_game_reexports(text: str, root: Path) -> tuple[dict[str, list[ReexportDecl]], list[str]]:
    """Parse `use (client_side|server_side)::<mod>;` re-exports at top level of `game.rs`."""
    src_root = (root / "crates/sunrise-client/src").resolve()
    items, errors = scan_top_level_items(text, "crates/sunrise-client/src/game.rs", src_root)
    reexports: dict[str, list[ReexportDecl]] = {}
    for item in items:
        if item.kind == "use_reexport" and item.reexport is not None:
            reexports.setdefault(item.reexport.name, []).append(item.reexport)
    return reexports, errors


def read_tokens_cached(path: Path, cache: dict[Path, tuple[str, list[Token]]] | None) -> tuple[str, list[Token]]:
    resolved = path.resolve()
    if cache is not None and resolved in cache:
        return cache[resolved]
    txt = resolved.read_text(encoding="utf-8")
    toks = tokenize_rust(txt)
    if cache is not None:
        cache[resolved] = (txt, toks)
    return txt, toks


def collect_moved_subtrees_and_includes(
    root: Path,
    mod_to_side: dict[str, str],
    all_arms: dict[str, list[ModArm]],
    resolved_entries: dict[str, list[Path]],
    tok_cache: dict[Path, tuple[str, list[Token]]] | None = None,
) -> tuple[dict[Path, int], set[Path], list[str]]:
    """Collect every file in moved module subtrees with its depth and verify include! safety."""
    errors: list[str] = []
    # Map every file in moved subtrees to its module depth (`0` for moved entry and its root includes).
    moved_file_depth: dict[Path, int] = {}
    moved_file_side: dict[Path, str] = {}
    moved_root_files: set[Path] = set()
    # Track all include sites per target across the entire game tree to detect shared-include conflicts.
    all_include_sites: dict[Path, list[tuple[IncludeSite, str]]] = {}

    # Walk moved modules first to discover every entry file, child mod file, and included file.
    def walk_moved_file(path: Path, side: str, depth: int, is_path_loaded: bool) -> None:
        resolved = path.resolve()
        if resolved in moved_file_depth:
            if moved_file_depth[resolved] != depth or moved_file_side[resolved] != side:
                rel = resolved.relative_to(root).as_posix()
                errors.append(
                    f"{rel}: file reached at conflicting module scopes ({moved_file_side[resolved]} depth {moved_file_depth[resolved]} vs {side} depth {depth})"
                )
            return
        moved_file_depth[resolved] = depth
        moved_file_side[resolved] = side
        if depth == 0:
            moved_root_files.add(resolved)
        txt, tokens = read_tokens_cached(resolved, tok_cache)
        for site in scan_includes_in_tokens(tokens, txt, resolved):
            eff_depth = depth + site.mod_depth
            if not site.is_literal:
                if eff_depth == 0:
                    rel = resolved.relative_to(root).as_posix()
                    errors.append(f"{rel}:{site.line}: unresolved non-literal root-scope `include!({site.raw_arg})`")
                continue
            if site.target is None or not site.target.is_file():
                rel = resolved.relative_to(root).as_posix()
                errors.append(f"{rel}:{site.line}: `include!({site.raw_arg})` target does not exist")
                continue
            all_include_sites.setdefault(site.target, []).append((site, f"{side}:d={eff_depth}"))
            walk_moved_file(site.target, side, eff_depth, is_path_loaded)

        children, cerrs = scan_child_mod_decls(txt, resolved, root, depth, is_path_loaded, tokens=tokens)
        errors.extend(cerrs)
        for child_path, child_depth, child_path_loaded in children:
            walk_moved_file(child_path, side, child_depth, child_path_loaded)

    for mod_name in sorted(set(all_arms) & set(mod_to_side)):
        side = mod_to_side[mod_name]
        if side not in ("client", "server"):
            continue
        arms = all_arms[mod_name]
        entries = resolved_entries.get(mod_name, [])
        for idx, entry in enumerate(entries):
            arm_has_path = arms[idx].path_value is not None if idx < len(arms) else False
            walk_moved_file(entry, side, 0, arm_has_path)

    # Scan all remaining `.rs` files in `game.rs` / `game/` for includes pointing at any moved root include.
    game_rs = (root / "crates/sunrise-client/src/game.rs").resolve()
    game_dir = (root / "crates/sunrise-client/src/game").resolve()
    other_files: list[Path] = []
    if game_rs.is_file():
        other_files.append(game_rs)
    if game_dir.is_dir():
        other_files.extend(sorted(p.resolve() for p in game_dir.rglob("*.rs")))
    for mod_name in sorted(set(all_arms) & set(mod_to_side)):
        if mod_to_side[mod_name] == "game":
            other_files.extend(resolved_entries.get(mod_name, []))

    seen_other: set[Path] = set()
    for path in other_files:
        if path in moved_file_depth or path in seen_other or not path.is_file():
            continue
        seen_other.add(path)
        txt = path.read_text(encoding="utf-8")
        if "include" not in txt:
            continue
        tokens = tokenize_rust(txt)
        for site in scan_includes_in_tokens(tokens, txt, path):
            if site.target is not None:
                all_include_sites.setdefault(site.target, []).append((site, f"game:d={site.mod_depth}"))

    for target, usages in sorted(all_include_sites.items()):
        scopes = {scope for _, scope in usages}
        if target in moved_root_files and len(scopes) > 1:
            trel = target.relative_to(root).as_posix()
            desc = ", ".join(
                f"{s.includer.relative_to(root).as_posix()}:{s.line} ({sc})" for s, sc in usages
            )
            errors.append(f"{trel}: include! target is shared across conflicting scopes: {desc}")

    return moved_file_depth, moved_root_files, errors


def selector_targets(
    root: Path,
    mod_to_side: dict[str, str],
    tok_cache: dict[Path, tuple[str, list[Token]]] | None = None,
) -> list[tuple[Path, str]]:
    """Return `[(path, expected_content)]` for every non-module-decl selector file that would change."""
    updates: list[tuple[Path, str]] = []

    ignored_dir = root / "tools/ci/ignored-tests"
    if ignored_dir.is_dir():
        for p in sorted(ignored_dir.glob("*.txt")):
            old = p.read_text(encoding="utf-8")
            new = rewrite_ignored_tests_file(old, mod_to_side)
            if new != old:
                updates.append((p, new))

    exc_path = root / "tools/ci/ignored-test-exceptions.json"
    if exc_path.is_file():
        old = exc_path.read_text(encoding="utf-8")
        new = rewrite_ignored_exceptions_json(old, mod_to_side)
        if new != old:
            updates.append((exc_path, new))

    tools_dir = root / "tools"
    if tools_dir.is_dir():
        for p in sorted(tools_dir.glob("test_*_parity.py")):
            old = p.read_text(encoding="utf-8")
            new = rewrite_game_paths_in_text(old, mod_to_side, full=False)
            if new != old:
                updates.append((p, new))

    areas_path = root / "tools/test_areas.toml"
    if areas_path.is_file():
        old = areas_path.read_text(encoding="utf-8")
        new = rewrite_test_areas_toml(old, mod_to_side)
        if new != old:
            updates.append((areas_path, new))

    fnr_dir = root / "tools/ci/fn-replace-check"
    if fnr_dir.is_dir():
        for p in sorted(fnr_dir.rglob("sunrise-mod.toml")):
            old = p.read_text(encoding="utf-8")
            new = rewrite_state_type_paths(rewrite_game_paths_in_text(old, mod_to_side, full=True))
            if new != old:
                updates.append((p, new))

    draw_test = root / "crates/cargo-sunrise-mod/tests/installed_client_draw.rs"
    if draw_test.is_file():
        old = draw_test.read_text(encoding="utf-8")
        new = rewrite_state_type_paths(rewrite_game_paths_in_text(old, mod_to_side, full=True))
        if new != old:
            updates.append((draw_test, new))

    docs_dir = root / "docs"
    if docs_dir.is_dir():
        for p in sorted(docs_dir.rglob("*.md")):
            old = p.read_text(encoding="utf-8", errors="replace")
            if "game::" not in old:
                continue
            new = rewrite_doc_file(old, mod_to_side)
            if new != old:
                updates.append((p, new))

    game_dir = root / "crates/sunrise-client/src/game"
    if game_dir.is_dir():
        for p in sorted(game_dir.rglob("*.rs")):
            resolved = p.resolve()
            if tok_cache is not None and resolved in tok_cache:
                old, toks = tok_cache[resolved]
                if "game::" not in old:
                    continue
                new = rewrite_rust_test_literals(old, mod_to_side, tokens=toks)
            else:
                old = p.read_text(encoding="utf-8")
                if "game::" not in old:
                    continue
                new = rewrite_rust_test_literals(old, mod_to_side)
            if new != old:
                updates.append((p, new))

    return updates


def resolve_rustfmt() -> str | None:
    env_bin = os.environ.get("RUSTFMT")
    if env_bin is not None:
        return shutil.which(env_bin) if env_bin else None
    return shutil.which("rustfmt")


def run_rustfmt(paths: list[Path]) -> None:
    rustfmt = resolve_rustfmt()
    if not rustfmt:
        raise SystemExit("error: rustfmt not found on PATH (install rustfmt or pass --no-fmt)")
    rs_paths = sorted({p for p in paths if p.suffix == ".rs" and p.is_file()})
    batch = 100
    for start in range(0, len(rs_paths), batch):
        subprocess.run(
            [rustfmt, "--edition", "2024", *(str(p) for p in rs_paths[start : start + batch])],
            check=True,
        )


def run_fix(root: Path, table: dict[str, tuple[str, str]], field_specs: dict[str, FieldSpec], *, fmt: bool) -> int:
    game_rs = root / "crates/sunrise-client/src/game.rs"
    game_dir = root / "crates/sunrise-client/src/game"
    mod_to_side = {m: s for m, (s, _) in table.items()}
    tok_cache: dict[Path, tuple[str, list[Token]]] = {}

    all_arms, current_parents, resolved_entries, errors = collect_all_arms(root)
    for m in sorted(set(all_arms) - set(table)):
        errors.append(f"module `{m}` (in {current_parents[m]}) is missing from game_module_sides.tsv")

    side_arms: dict[str, list[ModArm]] = {"client": [], "server": []}
    for mod_name in sorted(set(all_arms) & set(table)):
        target_side = mod_to_side[mod_name]
        if target_side in ("client", "server"):
            side_arms[target_side].extend(all_arms[mod_name])

    # Stage all writes and deletes in memory so any refusal leaves the tree untouched.
    planned_writes: dict[Path, str] = {}
    planned_deletes: list[Path] = []
    changed_rs: list[Path] = []

    for side in ("client", "server"):
        new_path = (game_dir / SIDE_NEW_FILE[side]).resolve()
        old_path = (game_dir / SIDE_OLD_FILE[side]).resolve()
        src_path = new_path if new_path.is_file() else old_path
        if not src_path.is_file():
            errors.append(f"neither {new_path} nor {old_path} exists")
            continue
        src_text = src_path.read_text(encoding="utf-8")
        new_text, errs = rewrite_state_file(src_text, side, side_arms[side], field_specs, root)
        errors.extend(errs)
        if not errs:
            if not new_path.is_file() or new_path.read_text(encoding="utf-8") != new_text:
                planned_writes[new_path] = new_text
                changed_rs.append(new_path)
            if old_path.is_file():
                planned_deletes.append(old_path)

    old_game = game_rs.read_text(encoding="utf-8")
    new_game, errs = rewrite_game_rs(old_game, table, all_arms, root)
    errors.extend(errs)
    if not errs and new_game != old_game:
        planned_writes[game_rs.resolve()] = new_game
        changed_rs.append(game_rs.resolve())

    moved_file_depth, moved_root_files, sub_errs = collect_moved_subtrees_and_includes(
        root, mod_to_side, all_arms, resolved_entries, tok_cache=tok_cache
    )
    errors.extend(sub_errs)

    vis_files = 0
    vis_rewrites = 0
    for path, depth in sorted(moved_file_depth.items()):
        old_txt, toks = read_tokens_cached(path, tok_cache)
        new_txt, n_hit, _, v_errs = analyze_file_visibility(
            old_txt, path, root, depth, moved_root_files, tokens=toks
        )
        errors.extend(v_errs)
        if not v_errs and n_hit > 0:
            planned_writes[path] = new_txt
            changed_rs.append(path)
            vis_files += 1
            vis_rewrites += n_hit

    sel_updates = selector_targets(root, mod_to_side, tok_cache=tok_cache)
    for path, new_txt in sel_updates:
        resolved = path.resolve()
        if resolved in planned_writes:
            # Apply selector rewrites on top of already-staged visibility rewrites for the same Rust file.
            new_txt = rewrite_rust_test_literals(planned_writes[resolved], mod_to_side)
        planned_writes[resolved] = new_txt
        if resolved.suffix == ".rs":
            changed_rs.append(resolved)

    errors = list(dict.fromkeys(errors))
    if errors:
        for e in errors:
            print(f"error: {e}", file=sys.stderr)
        return 1

    if fmt and changed_rs and not resolve_rustfmt():
        print("error: rustfmt not found on PATH (install rustfmt or pass --no-fmt)", file=sys.stderr)
        return 1

    for path, content in planned_writes.items():
        path.write_text(content, encoding="utf-8")
    for path in planned_deletes:
        path.unlink()

    if fmt and changed_rs:
        run_rustfmt(changed_rs)

    print(
        f"Applied module sides: {len(side_arms['client'])} client decls, {len(side_arms['server'])} server decls, "
        f"{vis_rewrites} pub(super) rewrites across {vis_files} files, {len(sel_updates)} selector files updated."
    )
    return 0


def run_check(root: Path, table: dict[str, tuple[str, str]], field_specs: dict[str, FieldSpec]) -> int:
    game_rs = root / "crates/sunrise-client/src/game.rs"
    game_dir = root / "crates/sunrise-client/src/game"
    mod_to_side = {m: s for m, (s, _) in table.items()}
    problems: list[str] = []

    for side in ("client", "server"):
        new_path = game_dir / SIDE_NEW_FILE[side]
        old_path = game_dir / SIDE_OLD_FILE[side]
        if old_path.is_file():
            problems.append(f"{old_path.relative_to(root).as_posix()}: legacy state file still exists; run --fix")
        if not new_path.is_file():
            problems.append(f"{new_path.relative_to(root).as_posix()}: missing side file; run --fix")

    if problems:
        for p in problems:
            print(f"error: {p}", file=sys.stderr)
        return 1

    all_arms, current_parents, resolved_entries, collect_errs = collect_all_arms(root)
    problems.extend(collect_errs)
    for mod_name in sorted(set(all_arms) - set(table)):
        problems.append(
            f"module `{mod_name}` declared in `{current_parents[mod_name]}` is missing from game_module_sides.tsv"
        )
    for mod_name in sorted(set(table) - set(all_arms)):
        problems.append(f"module `{mod_name}` in game_module_sides.tsv is not declared in game.rs or side files")

    game_text = game_rs.read_text(encoding="utf-8")
    reexports, reexp_errs = parse_game_reexports(game_text, root)
    problems.extend(reexp_errs)

    for mod_name in sorted(set(all_arms) & set(table)):
        want = mod_to_side[mod_name]
        actual = current_parents[mod_name]
        if actual != want:
            problems.append(f"module `{mod_name}` is in `{actual}`, expected `{want}` per game_module_sides.tsv")
        if want in ("client", "server"):
            expected_cfgs, expected_vis, cfg_err = cfg_attrs_for_reexport(all_arms[mod_name])
            if cfg_err:
                problems.append(cfg_err)
                continue
            rlist = reexports.get(mod_name, [])
            if len(rlist) != 1 or rlist[0].side_mod != SIDE_MOD_NAME[want]:
                problems.append(
                    f"crates/sunrise-client/src/game.rs: moved module `{mod_name}` missing re-export from `{SIDE_MOD_NAME[want]}`"
                )
            else:
                r = rlist[0]
                if r.vis != expected_vis:
                    problems.append(
                        f"crates/sunrise-client/src/game.rs:{r.line}: re-export of `{mod_name}` has visibility `{r.vis}`, expected `{expected_vis}`"
                    )
                actual_cfgs = [a.raw_text + "\n" for a in r.attrs if a.name in ("cfg", "cfg_attr")]
                if actual_cfgs != expected_cfgs:
                    problems.append(
                        f"crates/sunrise-client/src/game.rs:{r.line}: re-export of `{mod_name}` has cfg attributes {actual_cfgs!r}, expected {expected_cfgs!r}"
                    )
        elif mod_name in reexports:
            problems.append(f"crates/sunrise-client/src/game.rs: game-side module `{mod_name}` has stale re-export")

    for side in ("client", "server"):
        smod = SIDE_MOD_NAME[side]
        sname = SIDE_STRUCT_NAME[side]
        path = game_dir / SIDE_NEW_FILE[side]
        rel = path.relative_to(root).as_posix()
        txt = path.read_text(encoding="utf-8")
        if f"pub(in crate::game::{smod}) use super::*;" not in txt:
            problems.append(f"{rel}: missing `pub(in crate::game::{smod}) use super::*;`")
        try:
            problems.extend(check_state_struct_rules(txt, sname, rel))
            fields = parse_struct_fields(txt, sname, rel)
        except ValueError as err:
            problems.append(f"{rel}: {err}")
            continue
        items, item_errs = scan_top_level_items(txt, rel, game_dir)
        problems.extend(item_errs)
        for item in items:
            if item.kind == "struct" and norm_ident(item.name) == sname:
                break
            if item.kind not in ("use_super_glob", "mod_decl"):
                problems.append(
                    f"{rel}:{item.line}: unexpected `{item.kind}` item before `struct {sname}` in side-file preamble"
                )
        seen: set[str] = set()
        for fname, actual_vis, _, _ in fields:
            seen.add(fname)
            spec = field_specs.get(fname)
            if spec is None:
                problems.append(f"{rel}: field `{sname}::{fname}` missing from game_field_owners.tsv")
                continue
            if spec.owner != side:
                problems.append(
                    f"{rel}: field `{sname}::{fname}` has owner `{spec.owner}` in game_field_owners.tsv, expected `{side}`"
                )
                continue
            allowed_private = f"pub(in crate::game::{smod})"
            if spec.visibility == "side":
                if actual_vis != allowed_private:
                    problems.append(
                        f"{rel}: field `{sname}::{fname}` has visibility `{actual_vis or '<private>'}`, "
                        f"wider than table requirement `{allowed_private}`"
                    )
            else:
                if actual_vis not in ("pub(in crate::game)", allowed_private):
                    problems.append(
                        f"{rel}: field `{sname}::{fname}` has visibility `{actual_vis or '<private>'}`, "
                        "wider than `pub(in crate::game)`"
                    )
        expected = {f for f, s in field_specs.items() if s.owner == side}
        for missing in sorted(expected - seen):
            problems.append(f"{rel}: expected `{side}` field `{missing}` missing from `{sname}`")

    tok_cache: dict[Path, tuple[str, list[Token]]] = {}
    moved_file_depth, moved_root_files, sub_errs = collect_moved_subtrees_and_includes(
        root, mod_to_side, all_arms, resolved_entries, tok_cache=tok_cache
    )
    problems.extend(sub_errs)

    for path, depth in sorted(moved_file_depth.items()):
        txt, toks = read_tokens_cached(path, tok_cache)
        _, n_hit, lines_hit, v_errs = analyze_file_visibility(
            txt, path, root, depth, moved_root_files, tokens=toks
        )
        problems.extend(v_errs)
        if n_hit > 0:
            rel = path.relative_to(root).as_posix()
            sample = ", ".join(str(x) for x in lines_hit[:5])
            problems.append(f"{rel}: {n_hit} root-scope `pub(super)` item(s) remain (line(s) {sample})")

    for path, _ in selector_targets(root, mod_to_side, tok_cache=tok_cache):
        rel = path.relative_to(root).as_posix()
        problems.append(f"{rel}: stale module path selector(s); run `python3 tools/game_module_sides.py --fix`")

    problems = list(dict.fromkeys(problems))
    for p in problems:
        print(f"error: {p}", file=sys.stderr)
    if problems:
        print(f"Found {len(problems)} module-side problem(s).", file=sys.stderr)
        return 1
    print(
        f"OK: {len(all_arms)} modules and {sum(s.owner in ('client', 'server') for s in field_specs.values())} state fields verified."
    )
    return 0


def run_list(root: Path, table: dict[str, tuple[str, str]], field_specs: dict[str, FieldSpec]) -> int:
    all_arms, current_parents, _, errs = collect_all_arms(root)
    for e in errs:
        print(f"error: {e}", file=sys.stderr)
    if errs:
        return 1
    print("module\ttable_side\tactual_parent\tstatus\treason")
    for mod_name in sorted(set(all_arms) | set(table)):
        want, reason = table.get(mod_name, ("<missing>", ""))
        actual = current_parents.get(mod_name, "<missing>")
        status = "ok" if want == actual else "drift"
        print(f"{mod_name}\t{want}\t{actual}\t{status}\t{reason}")
    side_private = sum(1 for s in field_specs.values() if s.owner in ("client", "server") and s.visibility == "side")
    side_wide = sum(1 for s in field_specs.values() if s.owner in ("client", "server") and s.visibility == "game")
    print(
        f"# summary: modules={len(table)} (client={sum(s=='client' for s,_ in table.values())}, "
        f"server={sum(s=='server' for s,_ in table.values())}, game={sum(s=='game' for s,_ in table.values())}); "
        f"fields private={side_private}, wide={side_wide}",
        file=sys.stderr,
    )
    return 0


def run_renames(
    before_json: Path,
    after_json: Path,
    table: dict[str, tuple[str, str]],
    out_path: Path | None,
) -> int:
    """Compute old_target -> new_target TSV for every moved function in `functions.json`."""
    before = json.loads(before_json.read_text(encoding="utf-8"))
    after = json.loads(after_json.read_text(encoding="utf-8"))
    mod_to_side = {m: s for m, (s, _) in table.items()}

    before_fns = before.get("functions", {})
    after_fns = after.get("functions", {})
    after_refused = after.get("refused", {})

    rows: list[tuple[str, str, str]] = []
    in_after_fns = 0
    in_after_refused = 0
    missing_from_after: list[tuple[str, str]] = []
    matched_after: set[str] = set()

    for old in sorted(before_fns):
        if not old.startswith("sunrise_client::game::"):
            continue
        tail = old[len("sunrise_client::game::") :]
        head = tail.split("::", 1)[0]
        new: str | None = None
        side: str | None = None
        if head in ("client_state", "server_state"):
            side = "client" if head == "client_state" else "server"
            new = f"sunrise_client::game::{SIDE_MOD_NAME[side]}::{tail[len(head) + 2:]}"
        elif head in mod_to_side and mod_to_side[head] in ("client", "server"):
            side = mod_to_side[head]
            new = f"sunrise_client::game::{SIDE_MOD_NAME[side]}::{tail}"
        if new is None or side is None:
            continue
        rows.append((old, new, side))
        if new in after_fns:
            in_after_fns += 1
            matched_after.add(new)
        elif new in after_refused:
            in_after_refused += 1
        else:
            missing_from_after.append((old, new))

    lines = ["old_target\tnew_target\tside\n"]
    for old, new, side in rows:
        lines.append(f"{old}\t{new}\t{side}\n")
    content = "".join(lines)
    if out_path is not None:
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(content, encoding="utf-8")
    else:
        sys.stdout.write(content)

    unchanged = len(set(before_fns) & set(after_fns))
    extra_added = sorted(set(after_fns) - set(before_fns) - matched_after)
    print(
        f"Renames: {len(rows)} moved ({in_after_fns} in after functions.json, "
        f"{in_after_refused} in after refused, {len(missing_from_after)} missing); "
        f"{unchanged} unchanged, {len(extra_added)} unexpected added.",
        file=sys.stderr,
    )
    if missing_from_after or extra_added:
        for old, new in missing_from_after[:10]:
            print(f"  missing: {old} -> {new}", file=sys.stderr)
        for added in extra_added[:10]:
            print(f"  unexpected added: {added}", file=sys.stderr)
        return 1
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Re-parent and verify sunrise-client gameplay module sides.")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--fix", action="store_true", help="regenerate side modules, visibilities, and selectors")
    mode.add_argument("--check", action="store_true", help="verify tree matches tables (fail closed)")
    mode.add_argument("--list", action="store_true", help="list modules with table side and current parent")
    mode.add_argument(
        "--renames",
        nargs=2,
        metavar=("BEFORE_JSON", "AFTER_JSON"),
        type=Path,
        help="emit old_target -> new_target TSV from before/after functions.json",
    )
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root directory")
    parser.add_argument("--table", type=Path, default=None, help="path to game_module_sides.tsv")
    parser.add_argument("--fields-table", type=Path, default=None, help="path to game_field_owners.tsv")
    parser.add_argument("--out", type=Path, default=None, help="output path for --renames")
    parser.add_argument("--no-fmt", action="store_true", help="skip rustfmt on modified Rust files")
    args = parser.parse_args(argv)

    root = args.root.resolve()
    table_path = args.table or (root / "tools/game_module_sides.tsv" if args.root != ROOT else DEFAULT_TABLE)
    fields_path = args.fields_table or (
        root / "tools/game_field_owners.tsv" if args.root != ROOT else DEFAULT_FIELDS_TABLE
    )
    table = load_module_sides(table_path)
    if args.renames:
        return run_renames(args.renames[0], args.renames[1], table, args.out)
    field_specs = load_field_specs(fields_path)
    if args.fix:
        return run_fix(root, table, field_specs, fmt=not args.no_fmt)
    if args.check:
        return run_check(root, table, field_specs)
    return run_list(root, table, field_specs)


if __name__ == "__main__":
    raise SystemExit(main())
