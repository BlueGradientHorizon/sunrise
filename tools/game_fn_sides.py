#!/usr/bin/env python3
"""Split one-sided functions out of two-sided `sunrise-client` gameplay modules and `game.rs`.

Reads `tools/game_fn_sides.tsv` (`module`, `impl`, `name`, `cfg`, `side`, `reason`) and moves
`client` functions into exact-depth mirror modules under `game::client_side::<m>_client` and
`server` functions into `game::server_side::<m>_server` (with `module = game` / `game::<submod>`
for `game.rs` moving into `game_client` / `game_server` under `crates/sunrise-client/src/game/`),
leaving `game` rows in place:
- Preserves function text, attributes, doc comments, and `impl` headers (including generics and
  where-clauses); moves whole one-sided trait `impl` blocks and side-exclusive `macro_rules!`.
- Mirrors submodule hierarchies (`mod foo;`, inline `mod foo { ... }`, `mod foo { include!(...); }`,
  and root-scope `include!(...)`) at the same depth so `super::` paths resolve unchanged.
- For `game.rs` (parent module `crate::game`): keeps macros and staying items in place (its private
  items are already visible to all children), rewrites `super::` -> `crate::` in copied `use` items,
  and maps moved `pub(super)` items to `pub(crate)` so crate-wide callers keep compiling.
- Widens staying private / `pub(super)` items and local struct fields in split child modules to
  `pub(in crate::game)`, re-exports moved non-test free functions back into their original module,
  and rewrites test, parity, `test_areas.toml`, `fn-replace-check`, `port_map`, doc, and Rust
  test selectors.

Usage:
    python3 tools/game_fn_sides.py --fix [--no-fmt]
    python3 tools/game_fn_sides.py --check
    python3 tools/game_fn_sides.py --list
    python3 tools/game_fn_sides.py --renames BEFORE_FUNCTIONS_JSON AFTER_FUNCTIONS_JSON [--out PATH]
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import re
import shutil
import subprocess
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_TABLE = Path(__file__).resolve().with_name("game_fn_sides.tsv")
DEFAULT_MOD_TABLE = Path(__file__).resolve().with_name("game_module_sides.tsv")

SIDE_MOD_NAME = {"client": "client_side", "server": "server_side"}
SIDE_CONTAINERS = frozenset({"client_state", "server_state", "client_side", "server_side"})
MIRROR_MOD_RE = re.compile(r"^([A-Za-z0-9_]+)_(client|server)$")
MOD_DECL_RE = re.compile(r"^([ \t]*)(pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z0-9_]+)\s*;\s*$")
PATH_ATTR_RE = re.compile(r'path\s*=\s*"([^"]+)"')
TEST_AREAS_FILTER_RE = re.compile(
    r'^(\s*\{\s*package\s*=\s*"sunrise-client",\s*target\s*=\s*"lib",\s*filter\s*=\s*")'
    r'(game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::[^"]*)?)'
    r'("\s*\},?\s*)$'
)


@dataclass(frozen=True)
class FnKey:
    module: str  # relative to game, e.g. "pointer_capture" or "commands::execute"
    impl: str  # normalized impl header or "-"
    name: str
    cfg: str  # "; "-joined normalized cfg attrs or "-"


@dataclass(frozen=True)
class FnRow:
    key: FnKey
    side: str  # "client", "server", or "game"
    reason: str

    @property
    def top_module(self) -> str:
        return self.key.module.split("::", 1)[0]


@dataclass
class Token:
    kind: str
    text: str
    start: int
    end: int
    line: int


@dataclass
class ParsedItem:
    kind: str
    name: str
    start: int
    end: int
    start_line: int
    end_line: int
    kw_start: int
    vis: str
    vis_start: int
    vis_end: int
    attrs: tuple[str, ...]
    cfg_attrs: tuple[str, ...]
    norm_attrs: tuple[str, ...] = ()
    is_test_attr: bool = False
    is_trait_impl: bool = False
    impl_header: str = ""
    impl_open_brace: int = 0
    impl_close_brace: int = 0
    children: list["ParsedItem"] = field(default_factory=list)
    path_attr: str | None = None
    include_rel: str | None = None
    struct_body_start: int = 0
    struct_body_end: int = 0
    is_tuple_struct: bool = False
    source_file: Path | None = None


@dataclass
class ModNode:
    orig_mod: str  # relative to game, e.g. "commands::execute"
    file_path: Path
    decl_kind: str  # "root", "out_of_line", "inline", "inline_include"
    name: str
    cfg_attrs: tuple[str, ...]
    parent: "ModNode | None" = None
    children: list["ModNode"] = field(default_factory=list)
    inline_item: ParsedItem | None = None
    src: str = ""
    items: list[ParsedItem] = field(default_factory=list)
    extra_files: list[tuple[Path, str, list[ParsedItem], tuple[str, ...]]] = field(default_factory=list)
    mod_attrs: tuple[str, ...] = ()


def load_fn_sides(table_path: Path) -> dict[FnKey, FnRow]:
    """Load `tools/game_fn_sides.tsv` and fail closed on schema, truncation, emptiness, or key errors."""
    rows: dict[FnKey, FnRow] = {}
    expected = ("module", "impl", "name", "cfg", "side", "reason")
    with table_path.open(newline="", encoding="utf-8") as fp:
        reader = csv.reader(fp, delimiter="\t")
        header = next(reader, None)
        if not header:
            raise SystemExit(f"{table_path}: missing header; expected columns {list(expected)}")
        norm_header = [h.strip() for h in header]
        if any(col not in norm_header for col in expected):
            raise SystemExit(f"{table_path}: expected columns {list(expected)}, got {norm_header}")
        col_idx = {col: norm_header.index(col) for col in expected}
        expected_len = len(norm_header)
        for lineno, raw_row in enumerate(reader, 2):
            if not any(cell.strip() for cell in raw_row):
                continue
            if len(raw_row) != expected_len:
                raise SystemExit(
                    f"{table_path}:{lineno}: expected {expected_len} tab-separated columns, got {len(raw_row)}"
                )
            mod = raw_row[col_idx["module"]].strip()
            impl = raw_row[col_idx["impl"]].strip()
            name = raw_row[col_idx["name"]].strip()
            cfg = raw_row[col_idx["cfg"]].strip()
            side = raw_row[col_idx["side"]].strip()
            reason = raw_row[col_idx["reason"]].strip()
            if not mod or not impl or not name or not cfg or side not in ("client", "server", "game"):
                raise SystemExit(f"{table_path}:{lineno}: invalid row ({mod!r}, {impl!r}, {name!r}, {side!r})")
            key = FnKey(module=mod, impl=impl, name=name, cfg=cfg)
            if key in rows:
                raise SystemExit(f"{table_path}:{lineno}: duplicate function identity {key}")
            rows[key] = FnRow(key=key, side=side, reason=reason)
    if not rows:
        raise SystemExit(f"{table_path}: table is empty (0 rows)")
    return rows


def orig_game_mod_path(orig_mod_rel: str) -> str:
    """Return canonical module path rooted at `game` (`game`, `game::nested`, `game::commands::execute`)."""
    if orig_mod_rel == "game" or orig_mod_rel.startswith("game::"):
        return orig_mod_rel
    return f"game::{orig_mod_rel}"


def resolve_top_entry(
    root: Path,
    top_name: str,
    regular_mods: dict[str, tuple[str, Path, tuple[str, ...]]],
) -> tuple[str, Path, tuple[str, ...]] | None:
    if top_name == "game":
        game_rs = (root / "crates/sunrise-client/src/game.rs").resolve()
        return ("game", game_rs, ()) if game_rs.is_file() else None
    return regular_mods.get(top_name)


def load_optional_module_sides(mod_table_path: Path) -> dict[str, str]:
    """Return `module -> side` from `game_module_sides.tsv` if present."""
    if not mod_table_path.is_file():
        return {}
    out: dict[str, str] = {}
    with mod_table_path.open(newline="", encoding="utf-8") as fp:
        reader = csv.DictReader(fp, delimiter="\t")
        for lineno, r in enumerate(reader, 2):
            mod = r.get("module")
            side = r.get("side")
            if mod is None or side is None:
                raise SystemExit(f"{mod_table_path}:{lineno}: truncated module-sides row")
            out[mod.strip()] = side.strip()
    return out


def mirror_modules_from_table(rows: dict[FnKey, FnRow]) -> dict[str, set[str]]:
    """Return `{'client': {'<m>_client', ...}, 'server': {'<m>_server', ...}}` required by table."""
    res: dict[str, set[str]] = {"client": set(), "server": set()}
    for row in rows.values():
        if row.side in ("client", "server"):
            res[row.side].add(f"{row.top_module}_{row.side}")
    return res


def lex_rust(src: str) -> list[Token]:
    """Tokenize Rust source skipping comments, recording byte offsets and 1-based line numbers."""
    tokens: list[Token] = []
    i, n = 0, len(src)
    line = 1
    while i < n:
        c = src[i]
        if c in " \t\r":
            i += 1
            continue
        if c == "\n":
            line += 1
            i += 1
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            end = src.find("\n", i)
            i = n if end == -1 else end
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            start_line = line
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
                    if src[i] == "\n":
                        line += 1
                    i += 1
            if depth > 0:
                raise ValueError(f"line {start_line}: unclosed block comment")
            continue
        if c in ("r", "b", "c"):
            j = i + (1 if c in ("b", "c") and i + 1 < n and src[i + 1] == "r" else 0)
            if j < n and src[j] == "r":
                k = j + 1
                while k < n and src[k] == "#":
                    k += 1
                if k < n and src[k] == '"':
                    start, start_line = i, line
                    end_seq = '"' + ("#" * (k - (j + 1)))
                    end = src.find(end_seq, k + 1)
                    if end == -1:
                        raise ValueError(f"line {start_line}: unclosed raw string literal")
                    end += len(end_seq)
                    line += src[start:end].count("\n")
                    tokens.append(Token("str", src[start:end], start, end, start_line))
                    i = end
                    continue
        if c == '"' or (c in ("b", "c") and i + 1 < n and src[i + 1] == '"'):
            start, start_line = i, line
            if c != '"':
                i += 1
            i += 1
            closed = False
            while i < n:
                ch = src[i]
                if ch == "\\":
                    if i + 1 < n and src[i + 1] == "\n":
                        line += 1
                    i += 2
                elif ch == '"':
                    i += 1
                    closed = True
                    break
                else:
                    if ch == "\n":
                        line += 1
                    i += 1
            if not closed:
                raise ValueError(f"line {start_line}: unclosed string literal")
            tokens.append(Token("str", src[start:i], start, i, start_line))
            continue
        if c == "r" and i + 1 < n and src[i + 1] == "#" and i + 2 < n and (src[i + 2].isalpha() or src[i + 2] == "_"):
            start = i
            j = i + 2
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(Token("ident", src[start:j], start, j, line))
            i = j
            continue
        if c == "b" and i + 1 < n and src[i + 1] == "'":
            start = i
            j = i + 2
            if j < n and src[j] == "\\":
                end = src.find("'", j + 2)
                if end == -1 or end - start > 14 or "\n" in src[start:end]:
                    raise ValueError(f"line {line}: unclosed byte char literal")
                j = end + 1
            elif j + 1 < n and src[j + 1] == "'" and src[j] not in ("\n", "'"):
                j = j + 2
            else:
                raise ValueError(f"line {line}: invalid byte char literal")
            tokens.append(Token("char", src[start:j], start, j, line))
            i = j
            continue
        if c == "'":
            start = i
            if i + 2 < n and src[i + 1] == "\\":
                end = src.find("'", i + 3)
                if end != -1 and end - i <= 14 and "\n" not in src[i:end]:
                    tokens.append(Token("char", src[start : end + 1], start, end + 1, line))
                    i = end + 1
                    continue
                raise ValueError(f"line {line}: unclosed escaped char literal")
            if i + 2 < n and src[i + 2] == "'" and src[i + 1] not in ("\n", "'"):
                tokens.append(Token("char", src[start : i + 3], start, i + 3, line))
                i = i + 3
                continue
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(Token("lifetime", src[start:j], start, j, line))
            i = j
            continue
        if c.isalpha() or c == "_":
            start = i
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(Token("ident", src[start:j], start, j, line))
            i = j
            continue
        if c.isdigit():
            start = i
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(Token("other", src[start:j], start, j, line))
            i = j
            continue
        if i + 1 < n and src[i : i + 2] in ("::", "->", "=>", "..", "==", "!=", "<=", ">=", "&&", "||"):
            tokens.append(Token("punct", src[i : i + 2], i, i + 2, line))
            i += 2
            continue
        tokens.append(Token("punct", c, i, i + 1, line))
        i += 1
    return tokens


def skip_delim(tokens: list[Token], idx: int, open_ch: str, close_ch: str) -> int:
    depth = 0
    n = len(tokens)
    while idx < n:
        t = tokens[idx].text
        if t == open_ch:
            depth += 1
        elif t == close_ch:
            depth -= 1
            if depth == 0:
                return idx + 1
        idx += 1
    return n


def skip_angle(tokens: list[Token], idx: int) -> int:
    depth = 0
    n = len(tokens)
    while idx < n:
        t = tokens[idx].text
        if t == "<":
            depth += 1
        elif t == ">":
            depth -= 1
            if depth == 0:
                return idx + 1
        elif t in ("(", "[", "{"):
            close = {"(": ")", "[": "]", "{": "}"}[t]
            idx = skip_delim(tokens, idx, t, close)
            continue
        idx += 1
    return n


def line_starts(src: str) -> list[int]:
    starts = [0]
    pos = 0
    while True:
        idx = src.find("\n", pos)
        if idx == -1:
            break
        starts.append(idx + 1)
        pos = idx + 1
    return starts


def offset_to_line(starts: list[int], off: int) -> int:
    lo, hi = 0, len(starts) - 1
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if starts[mid] <= off:
            lo = mid
        else:
            hi = mid - 1
    return lo + 1


def parse_rust_string_literal(tok_text: str) -> str | None:
    """Decode a Rust string or raw-string literal token (`"..."`, `r"..."`, `r#"..."#`)."""
    raw_m = re.fullmatch(r'r(#*)"(.*)"\1', tok_text, re.DOTALL)
    if raw_m:
        return raw_m.group(2)
    if len(tok_text) >= 2 and tok_text[0] == '"' and tok_text[-1] == '"':
        inner = tok_text[1:-1]
        out: list[str] = []
        i, n = 0, len(inner)
        while i < n:
            if inner[i] == "\\" and i + 1 < n:
                nxt = inner[i + 1]
                esc = {"n": "\n", "r": "\r", "t": "\t", "\\": "\\", '"': '"', "'": "'", "0": "\0"}
                if nxt in esc:
                    out.append(esc[nxt])
                    i += 2
                    continue
                return None
            out.append(inner[i])
            i += 1
        return "".join(out)
    return None


def check_balanced_tokens(tokens: list[Token], source_file: Path | None = None) -> None:
    """Fail closed if `()`, `[]`, or `{}` delimiters are unbalanced."""
    stack: list[tuple[str, int]] = []
    match = {"(": ")", "[": "]", "{": "}"}
    closes = {")", "]", "}"}
    loc = str(source_file) if source_file is not None else "<input>"
    for tok in tokens:
        if tok.text in match:
            stack.append((match[tok.text], tok.line))
        elif tok.text in closes:
            if not stack or stack[-1][0] != tok.text:
                raise ValueError(f"{loc}:{tok.line}: unmatched closing delimiter `{tok.text}`")
            stack.pop()
    if stack:
        exp, ln = stack[-1]
        raise ValueError(f"{loc}:{ln}: unclosed delimiter (expected `{exp}`)")


def _find_matching_block_comment_start(src: str, close_end: int, min_bound: int) -> int | None:
    """Find opening `/*` matching `*/` at `src[close_end-2:close_end]`, handling nesting."""
    if close_end - 2 < min_bound or src[close_end - 2 : close_end] != "*/":
        return None
    depth = 1
    j = close_end - 2
    while j >= min_bound + 2:
        pair = src[j - 2 : j]
        if pair == "*/":
            depth += 1
            j -= 2
        elif pair == "/*":
            depth -= 1
            j -= 2
            if depth == 0:
                return j
        else:
            j -= 1
    return None


def leading_doc_start(src: str, attr_or_kw_start: int, min_bound: int) -> int:
    """Walk backwards from `attr_or_kw_start` over contiguous outer line and block comments (never `//!` or `/*!`)."""
    line_start = src.rfind("\n", min_bound, attr_or_kw_start)
    cur_line_start = (line_start + 1) if line_start != -1 else min_bound
    best = cur_line_start
    pos = cur_line_start

    same_line_prefix = src[cur_line_start:attr_or_kw_start].rstrip(" \t")
    if same_line_prefix.endswith("*/"):
        cursor = cur_line_start + len(same_line_prefix)
        while cursor > min_bound:
            open_pos = _find_matching_block_comment_start(src, cursor, min_bound)
            if open_pos is None or src[open_pos : open_pos + 3] == "/*!":
                break
            open_nl = src.rfind("\n", min_bound, open_pos)
            open_line_start = (open_nl + 1) if open_nl != -1 else min_bound
            before_open = src[open_line_start:open_pos].rstrip(" \t")
            if before_open == "":
                best = open_line_start
                pos = open_line_start
                break
            if before_open.endswith("*/"):
                best = open_pos
                cursor = open_line_start + len(before_open)
                continue
            best = open_pos
            return best

    while pos > min_bound:
        prev_nl = src.rfind("\n", min_bound, pos - 1)
        prev_start = (prev_nl + 1) if prev_nl != -1 else min_bound
        line_txt = src[prev_start : pos - 1]
        stripped = line_txt.strip()
        if not stripped:
            break
        if stripped.startswith("//!") or stripped.startswith("/*!"):
            break
        if stripped.startswith("///") or stripped.startswith("//"):
            best = prev_start
            pos = prev_start
            continue
        if stripped.endswith("*/"):
            close_end = prev_start + len(line_txt.rstrip(" \t\r"))
            open_pos = _find_matching_block_comment_start(src, close_end, min_bound)
            if open_pos is None or src[open_pos : open_pos + 3] == "/*!":
                break
            open_nl = src.rfind("\n", min_bound, open_pos)
            open_line_start = (open_nl + 1) if open_nl != -1 else min_bound
            if src[open_line_start:open_pos].strip() == "":
                best = open_line_start
                pos = open_line_start
                continue
            break
        break
    return best


def normalize_cfg_list(attrs: list[str] | tuple[str, ...]) -> tuple[str, ...]:
    out = []
    for a in attrs:
        norm = re.sub(r"\s+", " ", a).strip()
        if norm.startswith("#[cfg("):
            out.append(norm)
    return tuple(out)


def normalize_attr_list(attrs: list[str] | tuple[str, ...]) -> tuple[str, ...]:
    return tuple(re.sub(r"\s+", " ", a).strip() for a in attrs)


def attr_head_name(attr: str) -> str:
    m = re.match(r"^#\[\s*((?:[A-Za-z_][A-Za-z0-9_]*::)*[A-Za-z_][A-Za-z0-9_]*)", attr.strip())
    return m.group(1) if m else ""


def has_cfg_attr(attrs: list[str] | tuple[str, ...]) -> bool:
    return any(attr_head_name(a) in ("cfg_attr", "core::cfg_attr", "std::cfg_attr") for a in attrs)


def has_top_level_inner_attrs(src: str) -> list[int]:
    """Return line numbers of any `#![...]` inner attributes at depth 0."""
    tokens = lex_rust(src)
    lines: list[int] = []
    i, n = 0, len(tokens)
    while i < n:
        if tokens[i].text == "#" and i + 2 < n and tokens[i + 1].text == "!" and tokens[i + 2].text == "[":
            lines.append(tokens[i].line)
            i = skip_delim(tokens, i + 2, "[", "]")
            continue
        if tokens[i].text in ("(", "[", "{"):
            i = skip_delim(tokens, i, tokens[i].text, {"(": ")", "[": "]", "{": "}"}[tokens[i].text])
            continue
        i += 1
    return lines


def parse_rust_items(src: str, source_file: Path | None = None) -> list[ParsedItem]:
    """Parse Rust items recursively with exact byte and line spans."""
    tokens = lex_rust(src)
    check_balanced_tokens(tokens, source_file)
    lstarts = line_starts(src)

    def parse_range(start_i: int, end_i: int, min_byte: int, impl_hdr: str = "", is_trait: bool = False) -> list[ParsedItem]:
        out: list[ParsedItem] = []
        i = start_i
        prev_end = min_byte
        while i < end_i:
            if tokens[i].text == "#" and i + 1 < end_i and tokens[i + 1].text == "!" and i + 2 < end_i and tokens[i + 2].text == "[":
                nxt = skip_delim(tokens, i + 2, "[", "]")
                prev_end = tokens[nxt - 1].end
                i = nxt
                continue

            item_tok_start = i
            attrs: list[str] = []
            while i < end_i and tokens[i].text == "#" and i + 1 < end_i and tokens[i + 1].text == "[":
                a_s = tokens[i].start
                nxt = skip_delim(tokens, i + 1, "[", "]")
                attrs.append(src[a_s : tokens[nxt - 1].end])
                i = nxt
            if i >= end_i:
                break

            kw_tok = i
            vis_start = tokens[i].start
            vis_end = vis_start
            vis = ""
            if i < end_i and tokens[i].kind == "ident" and tokens[i].text == "pub":
                j = i + 1
                if j < end_i and tokens[j].text == "(":
                    j = skip_delim(tokens, j, "(", ")")
                vis_end = tokens[j - 1].end
                vis = src[vis_start:vis_end]
                i = j

            q_start = i
            while i < end_i and tokens[i].kind == "ident" and tokens[i].text in ("default", "unsafe", "async", "const", "extern"):
                if tokens[i].text == "const":
                    if i + 1 < end_i and tokens[i + 1].kind == "ident" and tokens[i + 1].text in ("fn", "unsafe", "extern"):
                        i += 1
                        continue
                    break
                kw = tokens[i].text
                i += 1
                if kw == "extern" and i < end_i and tokens[i].kind == "str":
                    i += 1

            if i >= end_i:
                break

            tok = tokens[i]
            raw_first_byte = tokens[item_tok_start].start
            item_start = leading_doc_start(src, raw_first_byte, prev_end)
            cfg_attrs = normalize_cfg_list(attrs)
            norm_attrs = normalize_attr_list(attrs)
            has_test_attr = any(
                a.strip() == "#[test]" or re.search(r"::test\]$", a.strip()) for a in attrs
            )
            path_attr = None
            for a in attrs:
                pm = PATH_ATTR_RE.search(a)
                if pm:
                    path_attr = pm.group(1)

            def make_end(end_tok_idx: int) -> int:
                e = tokens[end_tok_idx].end
                nl = src.find("\n", e)
                if nl != -1 and src[e:nl].strip() == "":
                    return nl + 1
                return e

            # 1. fn
            if tok.kind == "ident" and tok.text == "fn" and i + 1 < end_i and tokens[i + 1].kind == "ident":
                fn_name = tokens[i + 1].text
                k = i + 2
                while k < end_i and tokens[k].text not in ("{", ";"):
                    if tokens[k].text == "<":
                        k = skip_angle(tokens, k)
                    elif tokens[k].text in ("(", "["):
                        k = skip_delim(tokens, k, tokens[k].text, ")" if tokens[k].text == "(" else "]")
                    else:
                        k += 1
                if k < end_i and tokens[k].text == "{":
                    nxt = skip_delim(tokens, k, "{", "}")
                    item_end = make_end(nxt - 1)
                    out.append(
                        ParsedItem(
                            kind="method" if impl_hdr else "fn",
                            name=fn_name,
                            start=item_start,
                            end=item_end,
                            start_line=offset_to_line(lstarts, tokens[item_tok_start].start),
                            end_line=tokens[nxt - 1].line,
                            kw_start=tokens[kw_tok].start,
                            vis=vis,
                            vis_start=vis_start,
                            vis_end=vis_end,
                            attrs=tuple(attrs),
                            cfg_attrs=cfg_attrs,
                            norm_attrs=norm_attrs,
                            is_test_attr=has_test_attr,
                            is_trait_impl=is_trait,
                            impl_header=impl_hdr,
                            source_file=source_file,
                        )
                    )
                    prev_end = item_end
                    i = nxt
                    continue
                if k < end_i and tokens[k].text == ";":
                    prev_end = make_end(k)
                    i = k + 1
                    continue

            # 2. impl
            if not impl_hdr and tok.kind == "ident" and tok.text == "impl":
                hdr_start_byte = tokens[q_start].start
                k = i + 1
                trait_flag = False
                while k < end_i and tokens[k].text != "{":
                    if tokens[k].text == "<":
                        k = skip_angle(tokens, k)
                    elif tokens[k].text in ("(", "["):
                        k = skip_delim(tokens, k, tokens[k].text, ")" if tokens[k].text == "(" else "]")
                    else:
                        if tokens[k].kind == "ident" and tokens[k].text == "for":
                            if not (k + 1 < end_i and tokens[k + 1].text == "<"):
                                trait_flag = True
                        k += 1
                if k < end_i and tokens[k].text == "{":
                    hdr_text = re.sub(r"\s+", " ", src[hdr_start_byte : tokens[k].start]).strip()
                    nxt = skip_delim(tokens, k, "{", "}")
                    item_end = make_end(nxt - 1)
                    children = parse_range(k + 1, nxt - 1, tokens[k].end, impl_hdr=hdr_text, is_trait=trait_flag)
                    out.append(
                        ParsedItem(
                            kind="impl",
                            name=hdr_text,
                            start=item_start,
                            end=item_end,
                            start_line=offset_to_line(lstarts, tokens[item_tok_start].start),
                            end_line=tokens[nxt - 1].line,
                            kw_start=tokens[kw_tok].start,
                            vis=vis,
                            vis_start=vis_start,
                            vis_end=vis_end,
                            attrs=tuple(attrs),
                            cfg_attrs=cfg_attrs,
                            norm_attrs=norm_attrs,
                            is_trait_impl=trait_flag,
                            impl_header=hdr_text,
                            impl_open_brace=tokens[k].start,
                            impl_close_brace=tokens[nxt - 1].start,
                            children=children,
                            source_file=source_file,
                        )
                    )
                    prev_end = item_end
                    i = nxt
                    continue

            # 3. mod
            if not impl_hdr and tok.kind == "ident" and tok.text == "mod" and i + 1 < end_i and tokens[i + 1].kind == "ident":
                mname = tokens[i + 1].text
                if i + 2 < end_i and tokens[i + 2].text == ";":
                    item_end = make_end(i + 2)
                    out.append(
                        ParsedItem(
                            kind="mod_ext",
                            name=mname,
                            start=item_start,
                            end=item_end,
                            start_line=offset_to_line(lstarts, tokens[item_tok_start].start),
                            end_line=tokens[i + 2].line,
                            kw_start=tokens[kw_tok].start,
                            vis=vis,
                            vis_start=vis_start,
                            vis_end=vis_end,
                            attrs=tuple(attrs),
                            cfg_attrs=cfg_attrs,
                            norm_attrs=norm_attrs,
                            path_attr=path_attr,
                            source_file=source_file,
                        )
                    )
                    prev_end = item_end
                    i = i + 3
                    continue
                if i + 2 < end_i and tokens[i + 2].text == "{":
                    nxt = skip_delim(tokens, i + 2, "{", "}")
                    item_end = make_end(nxt - 1)
                    inner_toks = tokens[i + 3 : nxt - 1]
                    inc_rel_mod: str | None = None
                    p_in = 0
                    while p_in < len(inner_toks) and (inner_toks[p_in].kind == "ident" or inner_toks[p_in].text == "::"):
                        p_in += 1
                    mpath_in = "".join(t.text for t in inner_toks[:p_in])
                    if (
                        mpath_in in ("include", "std::include", "core::include")
                        and p_in + 3 < len(inner_toks)
                        and inner_toks[p_in].text == "!"
                        and inner_toks[p_in + 1].text in ("(", "[", "{")
                    ):
                        cl = {"(": ")", "[": "]", "{": "}"}[inner_toks[p_in + 1].text]
                        arg_end = skip_delim(inner_toks, p_in + 1, inner_toks[p_in + 1].text, cl)
                        tail_toks = inner_toks[arg_end:]
                        if (not tail_toks) or (len(tail_toks) == 1 and tail_toks[0].text == ";"):
                            arg_toks = inner_toks[p_in + 2 : arg_end - 1]
                            if len(arg_toks) == 1 and arg_toks[0].kind == "str":
                                inc_rel_mod = parse_rust_string_literal(arg_toks[0].text)
                    if inc_rel_mod is not None:
                        out.append(
                            ParsedItem(
                                kind="mod_include",
                                name=mname,
                                start=item_start,
                                end=item_end,
                                start_line=offset_to_line(lstarts, tokens[item_tok_start].start),
                                end_line=tokens[nxt - 1].line,
                                kw_start=tokens[kw_tok].start,
                                vis=vis,
                                vis_start=vis_start,
                                vis_end=vis_end,
                                attrs=tuple(attrs),
                                cfg_attrs=cfg_attrs,
                                norm_attrs=norm_attrs,
                                include_rel=inc_rel_mod,
                                impl_open_brace=tokens[i + 2].start,
                                impl_close_brace=tokens[nxt - 1].start,
                                source_file=source_file,
                            )
                        )
                    else:
                        children = parse_range(i + 3, nxt - 1, tokens[i + 2].end)
                        out.append(
                            ParsedItem(
                                kind="mod_inline",
                                name=mname,
                                start=item_start,
                                end=item_end,
                                start_line=offset_to_line(lstarts, tokens[item_tok_start].start),
                                end_line=tokens[nxt - 1].line,
                                kw_start=tokens[kw_tok].start,
                                vis=vis,
                                vis_start=vis_start,
                                vis_end=vis_end,
                                attrs=tuple(attrs),
                                cfg_attrs=cfg_attrs,
                                norm_attrs=norm_attrs,
                                impl_open_brace=tokens[i + 2].start,
                                impl_close_brace=tokens[nxt - 1].start,
                                children=children,
                                source_file=source_file,
                            )
                        )
                    prev_end = item_end
                    i = nxt
                    continue

            # 4. use
            if not impl_hdr and tok.kind == "ident" and tok.text == "use":
                k = i + 1
                while k < end_i and tokens[k].text != ";":
                    if tokens[k].text in ("{", "(", "["):
                        close = {"{": "}", "(": ")", "[": "]"}[tokens[k].text]
                        k = skip_delim(tokens, k, tokens[k].text, close)
                    else:
                        k += 1
                if k < end_i:
                    item_end = make_end(k)
                    out.append(
                        ParsedItem(
                            kind="use",
                            name=src[tokens[i + 1].start : tokens[k].start].strip(),
                            start=item_start,
                            end=item_end,
                            start_line=offset_to_line(lstarts, tokens[item_tok_start].start),
                            end_line=tokens[k].line,
                            kw_start=tokens[kw_tok].start,
                            vis=vis,
                            vis_start=vis_start,
                            vis_end=vis_end,
                            attrs=tuple(attrs),
                            cfg_attrs=cfg_attrs,
                            norm_attrs=norm_attrs,
                            source_file=source_file,
                        )
                    )
                    prev_end = item_end
                    i = k + 1
                    continue

            # 5. struct / enum / trait / type / const / static
            if tok.kind == "ident" and tok.text in ("struct", "enum", "trait", "type", "const", "static"):
                kind = tok.text
                name_idx = i + 1
                if kind == "static" and name_idx < end_i and tokens[name_idx].text == "mut":
                    name_idx += 1
                iname = tokens[name_idx].text if name_idx < end_i else ""
                k = name_idx + 1
                s_open = s_close = 0
                is_tuple = False
                if kind == "struct":
                    if k < end_i and tokens[k].text == "<":
                        k = skip_angle(tokens, k)
                    if k < end_i and tokens[k].text == "(":
                        s_open = tokens[k].start
                        nxt = skip_delim(tokens, k, "(", ")")
                        s_close = tokens[nxt - 1].start
                        is_tuple = True
                        k = nxt
                        while k < end_i and tokens[k].text != ";":
                            k += 1
                        if k < end_i:
                            k += 1
                    else:
                        while k < end_i and tokens[k].text not in ("{", ";"):
                            if tokens[k].text == "<":
                                k = skip_angle(tokens, k)
                            elif tokens[k].text in ("(", "["):
                                k = skip_delim(tokens, k, tokens[k].text, ")" if tokens[k].text == "(" else "]")
                            else:
                                k += 1
                        if k < end_i and tokens[k].text == "{":
                            s_open = tokens[k].start
                            nxt = skip_delim(tokens, k, "{", "}")
                            s_close = tokens[nxt - 1].start
                            k = nxt
                        elif k < end_i and tokens[k].text == ";":
                            k += 1
                elif kind in ("type", "const", "static"):
                    while k < end_i:
                        t = tokens[k].text
                        if t == ";":
                            k += 1
                            break
                        if t in ("(", "[", "{"):
                            k = skip_delim(tokens, k, t, {"(": ")", "[": "]", "{": "}"}[t])
                            continue
                        k += 1
                else:
                    while k < end_i:
                        t = tokens[k].text
                        if t == ";":
                            k += 1
                            break
                        if t == "<":
                            k = skip_angle(tokens, k)
                            continue
                        if t == "{":
                            k = skip_delim(tokens, k, "{", "}")
                            break
                        if t in ("(", "["):
                            k = skip_delim(tokens, k, t, ")" if t == "(" else "]")
                            continue
                        k += 1
                end_tok = min(k - 1, end_i - 1)
                item_end = make_end(end_tok)
                out.append(
                    ParsedItem(
                        kind=kind,
                        name=iname,
                        start=item_start,
                        end=item_end,
                        start_line=offset_to_line(lstarts, tokens[item_tok_start].start),
                        end_line=tokens[end_tok].line,
                        kw_start=tokens[kw_tok].start,
                        vis=vis,
                        vis_start=vis_start,
                        vis_end=vis_end,
                        attrs=tuple(attrs),
                        cfg_attrs=cfg_attrs,
                        norm_attrs=norm_attrs,
                        struct_body_start=s_open,
                        struct_body_end=s_close,
                        is_tuple_struct=is_tuple,
                        source_file=source_file,
                    )
                )
                prev_end = item_end
                i = max(k, i + 1)
                continue

            # 6. macro_rules! / include!("...") / other macro_invoc
            if tok.kind == "ident":
                p = i
                while p < end_i and (tokens[p].kind == "ident" or tokens[p].text == "::"):
                    p += 1
                if p < end_i and tokens[p].text == "!":
                    mpath = "".join(t.text for t in tokens[i:p])
                    k = p + 1
                    is_mrules = mpath == "macro_rules"
                    mname = mpath
                    if is_mrules and k < end_i and tokens[k].kind == "ident":
                        mname = tokens[k].text
                        k += 1
                    if k < end_i and tokens[k].text in ("{", "(", "["):
                        close = {"{": "}", "(": ")", "[": "]"}[tokens[k].text]
                        nxt = skip_delim(tokens, k, tokens[k].text, close)
                        body_txt = src[tokens[k].end : tokens[nxt - 1].start].strip()
                        arg_toks = tokens[k + 1 : nxt - 1]
                        if nxt < end_i and tokens[nxt].text == ";":
                            nxt += 1
                        item_end = make_end(nxt - 1)
                        inc_rel = None
                        kind = "macro_rules" if is_mrules else "macro_invoc"
                        if mpath in ("include", "std::include", "core::include"):
                            decoded = (
                                parse_rust_string_literal(arg_toks[0].text)
                                if len(arg_toks) == 1 and arg_toks[0].kind == "str"
                                else None
                            )
                            if decoded is not None:
                                kind = "root_include"
                                inc_rel = decoded
                            else:
                                kind = "bad_include"
                                inc_rel = body_txt
                        out.append(
                            ParsedItem(
                                kind=kind,
                                name=mname,
                                start=item_start,
                                end=item_end,
                                start_line=offset_to_line(lstarts, tokens[item_tok_start].start),
                                end_line=tokens[nxt - 1].line,
                                kw_start=tokens[kw_tok].start,
                                vis=vis,
                                vis_start=vis_start,
                                vis_end=vis_end,
                                attrs=tuple(attrs),
                                cfg_attrs=cfg_attrs,
                                norm_attrs=norm_attrs,
                                include_rel=inc_rel,
                                source_file=source_file,
                            )
                        )
                        prev_end = item_end
                        i = nxt
                        continue

            # Fallback
            k = i
            while k < end_i:
                t = tokens[k].text
                if t == ";":
                    k += 1
                    break
                if t == "{":
                    k = skip_delim(tokens, k, "{", "}")
                    break
                if t in ("(", "["):
                    k = skip_delim(tokens, k, t, ")" if t == "(" else "]")
                    continue
                k += 1
            end_tok = min(max(k - 1, i), end_i - 1)
            item_end = make_end(end_tok)
            out.append(
                ParsedItem(
                    kind="other",
                    name=tok.text,
                    start=item_start,
                    end=item_end,
                    start_line=offset_to_line(lstarts, tokens[item_tok_start].start),
                    end_line=tokens[end_tok].line,
                    kw_start=tokens[kw_tok].start,
                    vis=vis,
                    vis_start=vis_start,
                    vis_end=vis_end,
                    attrs=tuple(attrs),
                    cfg_attrs=cfg_attrs,
                    source_file=source_file,
                )
            )
            prev_end = item_end
            i = max(k, i + 1)
        return out

    return parse_range(0, len(tokens), 0)


def node_cfg_chain(node: ModNode) -> tuple[str, ...]:
    """Return all `#[cfg(...)]` attrs from root down to `node`."""
    parts: list[tuple[str, ...]] = []
    cur: ModNode | None = node
    while cur is not None:
        if cur.cfg_attrs:
            parts.append(cur.cfg_attrs)
        cur = cur.parent
    out: list[str] = []
    for p in reversed(parts):
        out.extend(p)
    return tuple(out)


def item_fn_key(
    mod_rel: str,
    it: ParsedItem,
    parent_impl: ParsedItem | None,
    node_cfgs: tuple[str, ...] = (),
) -> FnKey:
    if parent_impl is not None:
        cfgs = (*node_cfgs, *parent_impl.cfg_attrs, *it.cfg_attrs)
        return FnKey(module=mod_rel, impl=parent_impl.impl_header, name=it.name, cfg="; ".join(cfgs) or "-")
    cfgs = (*node_cfgs, *it.cfg_attrs)
    return FnKey(module=mod_rel, impl="-", name=it.name, cfg="; ".join(cfgs) or "-")


BENIGN_MOD_ATTRS = frozenset({"allow", "expect", "warn", "deny", "doc"})
CARRY_MOD_ATTRS = frozenset({"allow", "expect", "warn", "deny"})
LOCATION_MACROS = frozenset({"module_path", "file", "line", "column"})
INCLUDE_MACROS = frozenset({"include", "include_str", "include_bytes"})


def _rel_str(path: Path, root: Path | None) -> str:
    if root is not None:
        try:
            return path.resolve().relative_to(root.resolve()).as_posix()
        except ValueError:
            pass
    return path.as_posix()


def effective_attrs(norm_attrs: tuple[str, ...], frag_cfgs: tuple[str, ...]) -> tuple[str, ...]:
    """Combine enclosing `root_include` cfgs with item attrs canonically without duplication."""
    extra = tuple(c for c in frag_cfgs if c not in norm_attrs)
    return (*extra, *norm_attrs)


def validate_mod_attrs(
    attrs: tuple[str, ...], fpath: Path, line: int, mod_name: str, errors: list[str], root: Path | None = None
) -> tuple[str, ...]:
    """Validate attributes on a `mod` declaration; refuse `cfg_attr` and unsupported attrs, return benign carry attrs."""
    carried: list[str] = []
    rel = _rel_str(fpath, root)
    for raw in attrs:
        norm = re.sub(r"\s+", " ", raw).strip()
        head = attr_head_name(norm)
        short = head.rsplit("::", 1)[-1]
        if short == "cfg_attr":
            errors.append(f"{rel}:{line}: unsupported `#[cfg_attr(...)]` on module `{mod_name}`: `{norm}`")
            continue
        if short in ("cfg", "path", "doc"):
            continue
        if short in CARRY_MOD_ATTRS:
            carried.append(norm)
            continue
        errors.append(f"{rel}:{line}: unsupported attribute `{norm}` on module `{mod_name}`")
    return tuple(carried)


def parse_top_mod_decls(
    root: Path,
    regular_names: set[str] | frozenset[str] = frozenset(),
    *,
    errors: list[str] | None = None,
    all_regular_entries: list[Path] | None = None,
) -> tuple[dict[str, tuple[str, Path, tuple[str, ...]]], dict[str, dict[str, tuple[Path, tuple[str, ...]]]]]:
    """Return `(regular_mods, mirror_mods)` across `game.rs`, `client_side.rs`, `server_side.rs`.

    `regular_mods`: `name -> (parent_side, entry_file, cfg_attrs)`
    `mirror_mods`: `side -> {mirror_mod_name -> (entry_file, cfg_attrs)}`
    """
    src_root = root / "crates/sunrise-client/src"
    game_dir = src_root / "game"
    regular: dict[str, tuple[str, Path, tuple[str, ...]]] = {}
    mirrors: dict[str, dict[str, tuple[Path, tuple[str, ...]]]] = {"client": {}, "server": {}}
    errs = errors if errors is not None else []

    for parent_side, path in [
        ("game", src_root / "game.rs"),
        ("client", game_dir / "client_side.rs"),
        ("server", game_dir / "server_side.rs"),
    ]:
        if not path.is_file():
            continue
        try:
            items = parse_rust_items(path.read_text(encoding="utf-8"), path)
        except ValueError as exc:
            errs.append(f"{_rel_str(path, root)}: parse error: {exc}")
            continue
        for it in items:
            if it.kind != "mod_ext" or it.name in SIDE_CONTAINERS:
                continue
            if it.path_attr:
                if it.path_attr.startswith("game/"):
                    entry = (src_root / it.path_attr).resolve()
                else:
                    entry = ((game_dir / it.path_attr) if parent_side != "game" else (src_root / it.path_attr)).resolve()
                    if not entry.is_file():
                        alt = (game_dir / it.path_attr).resolve()
                        if alt.is_file():
                            entry = alt
            else:
                c1 = game_dir / f"{it.name}.rs"
                c2 = game_dir / it.name / "mod.rs"
                entry = (c1 if c1.is_file() else c2).resolve()

            m = MIRROR_MOD_RE.match(it.name)
            is_mirror_name = (
                m is not None
                and it.name not in regular_names
                and (bool(regular_names) or entry.name.endswith(f"_{m.group(2)}.rs"))
            )
            if is_mirror_name and m is not None:
                m_side = m.group(2)
                rel_p = _rel_str(path, root)
                if m_side != parent_side:
                    errs.append(f"{rel_p}:{it.start_line}: mirror module `{it.name}` declared in `{path.name}` instead of `{SIDE_MOD_NAME[m_side]}.rs`")
                    continue
                validate_mod_attrs(it.attrs, path, it.start_line, it.name, errs, root)
                if not entry.is_file():
                    errs.append(f"{rel_p}:{it.start_line}: mirror module `{it.name}` target `{it.path_attr or entry.name}` does not exist")
                if it.name in mirrors[parent_side]:
                    errs.append(f"{rel_p}:{it.start_line}: duplicate top-level mirror module declaration `{it.name}`")
                    continue
                mirrors[parent_side][it.name] = (entry, it.cfg_attrs)
            else:
                if has_cfg_attr(it.attrs):
                    errs.append(f"{_rel_str(path, root)}:{it.start_line}: unsupported `#[cfg_attr(...)]` on top-level module `{it.name}`")
                if all_regular_entries is not None and entry.is_file():
                    all_regular_entries.append(entry)
                if it.name not in regular:
                    regular[it.name] = (parent_side, entry, it.cfg_attrs)
    return regular, mirrors


def build_mod_tree(
    top_name: str,
    entry_file: Path,
    top_cfg: tuple[str, ...] = (),
    *,
    errors: list[str] | None = None,
    root: Path | None = None,
    is_mirror: bool = False,
) -> ModNode:
    """Build complete module tree for `top_name`, including root-scope `include!` fragments."""
    errs = errors if errors is not None else []
    seen_includes: set[tuple[Path, Path]] = set()

    def walk_file(
        rs_file: Path,
        mod_rel: str,
        decl_kind: str,
        name: str,
        cfg: tuple[str, ...],
        parent: ModNode | None,
        mod_attrs: tuple[str, ...] = (),
    ) -> ModNode:
        src = rs_file.read_text(encoding="utf-8")
        try:
            items = parse_rust_items(src, rs_file)
        except ValueError as exc:
            errs.append(f"{_rel_str(rs_file, root)}: parse error: {exc}")
            items = []
        node = ModNode(
            orig_mod=mod_rel,
            file_path=rs_file,
            decl_kind=decl_kind,
            name=name,
            cfg_attrs=cfg,
            parent=parent,
            src=src,
            items=items,
            mod_attrs=mod_attrs,
        )
        attach_children(node, items, rs_file, src, mod_rel, ())
        return node

    def attach_children(
        node: ModNode,
        items: list[ParsedItem],
        host_file: Path,
        host_src: str,
        host_mod: str,
        frag_cfgs: tuple[str, ...],
    ) -> None:
        for it in items:
            if it.kind == "bad_include":
                norm_inc = re.sub(r"\s+", "", it.include_rel or "")
                is_build_env_inc = (not is_mirror) and bool(
                    re.fullmatch(r'concat!\(env!\("(?:OUT_DIR|CARGO_MANIFEST_DIR)"\),"[^"]+",?\)', norm_inc)
                )
                if not is_build_env_inc:
                    errs.append(
                        f"{_rel_str(host_file, root)}:{it.start_line}: unsupported non-literal `include!({it.include_rel})`"
                    )
            elif it.kind == "root_include" and it.include_rel:
                if has_cfg_attr(it.attrs):
                    errs.append(
                        f"{_rel_str(host_file, root)}:{it.start_line}: unsupported `#[cfg_attr(...)]` on `include!({it.include_rel!r})`"
                    )
                inc_file = (host_file.parent / it.include_rel).resolve()
                edge = (host_file.resolve(), inc_file)
                if edge in seen_includes:
                    errs.append(f"{_rel_str(host_file, root)}:{it.start_line}: cyclic `include!({it.include_rel!r})`")
                    continue
                if not inc_file.is_file():
                    errs.append(
                        f"{_rel_str(host_file, root)}:{it.start_line}: `include!({it.include_rel!r})` target does not exist"
                    )
                    continue
                seen_includes.add(edge)
                inc_src = inc_file.read_text(encoding="utf-8")
                try:
                    inc_items = parse_rust_items(inc_src, inc_file)
                except ValueError as exc:
                    errs.append(f"{_rel_str(inc_file, root)}: parse error: {exc}")
                    inc_items = []
                child_frag_cfgs = (*frag_cfgs, *it.cfg_attrs)
                node.extra_files.append((inc_file, inc_src, inc_items, child_frag_cfgs))
                attach_children(node, inc_items, inc_file, inc_src, host_mod, child_frag_cfgs)
            elif it.kind == "mod_ext":
                # Out-of-line `mod` items in `game.rs` are independent top-level gameplay modules, not children of `game`.
                if top_name == "game" and node.parent is None:
                    continue
                m_attrs = validate_mod_attrs(it.attrs, host_file, it.start_line, it.name, errs, root)
                if it.path_attr:
                    child_file = (host_file.parent / it.path_attr).resolve()
                else:
                    base_dir = host_file.parent if host_file.name in ("mod.rs", "lib.rs") else host_file.with_suffix("")
                    raw_name = it.name[2:] if it.name.startswith("r#") else it.name
                    c1 = base_dir / f"{raw_name}.rs"
                    c2 = base_dir / raw_name / "mod.rs"
                    child_file = c1 if c1.is_file() else c2
                if child_file.is_file():
                    child_node = walk_file(
                        child_file,
                        f"{host_mod}::{it.name}",
                        "out_of_line",
                        it.name,
                        (*frag_cfgs, *it.cfg_attrs),
                        node,
                        mod_attrs=m_attrs,
                    )
                    node.children.append(child_node)
            elif it.kind == "mod_include" and it.include_rel:
                m_attrs = validate_mod_attrs(it.attrs, host_file, it.start_line, it.name, errs, root)
                inc_file = (host_file.parent / it.include_rel).resolve()
                if not inc_file.is_file():
                    errs.append(
                        f"{_rel_str(host_file, root)}:{it.start_line}: `mod {it.name} {{ include!({it.include_rel!r}); }}` target does not exist"
                    )
                    continue
                child_node = walk_file(
                    inc_file,
                    f"{host_mod}::{it.name}",
                    "inline_include",
                    it.name,
                    (*frag_cfgs, *it.cfg_attrs),
                    node,
                    mod_attrs=m_attrs,
                )
                node.children.append(child_node)
            elif it.kind == "mod_inline":
                m_attrs = validate_mod_attrs(it.attrs, host_file, it.start_line, it.name, errs, root)
                sub = ModNode(
                    orig_mod=f"{host_mod}::{it.name}",
                    file_path=host_file,
                    decl_kind="inline",
                    name=it.name,
                    cfg_attrs=(*frag_cfgs, *it.cfg_attrs),
                    parent=node,
                    inline_item=it,
                    src=host_src,
                    items=it.children,
                    mod_attrs=m_attrs,
                )
                attach_children(sub, it.children, host_file, host_src, sub.orig_mod, ())
                node.children.append(sub)

    return walk_file(entry_file, top_name, "root", top_name, top_cfg, None)


def node_all_fragments(node: ModNode) -> list[tuple[Path, str, list[ParsedItem], tuple[str, ...]]]:
    """Return primary `(file_path, src, items, frag_cfgs)` followed by any root-scope `include!` fragments."""
    return [(node.file_path, node.src, node.items, ()), *node.extra_files]


def collect_tree_functions(
    root_node: ModNode,
    *,
    dup_errors: list[str] | None = None,
    root: Path | None = None,
) -> dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]]:
    """Map `FnKey -> (node, file_path, src, item, parent_impl, frag_cfgs)` across every node and fragment."""
    found: dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]] = {}
    dups = dup_errors if dup_errors is not None else []

    def record(
        k: FnKey,
        entry: tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]],
    ) -> None:
        if k in found:
            prev_fp, prev_it = found[k][1], found[k][3]
            cur_fp, cur_it = entry[1], entry[3]
            dups.append(
                f"duplicate function key `{k.module}::{k.impl}::{k.name}` ({k.cfg}) at "
                f"{_rel_str(prev_fp, root)}:{prev_it.start_line} and {_rel_str(cur_fp, root)}:{cur_it.start_line}"
            )
            return
        found[k] = entry

    def walk(node: ModNode) -> None:
        ncfgs = node_cfg_chain(node)
        for fpath, fsrc, fitems, fcfgs in node_all_fragments(node):
            eff_ncfgs = (*ncfgs, *fcfgs)
            for it in fitems:
                if it.kind == "fn":
                    k = item_fn_key(node.orig_mod, it, None, eff_ncfgs)
                    record(k, (node, fpath, fsrc, it, None, fcfgs))
                elif it.kind == "impl":
                    for ch in it.children:
                        if ch.kind == "method":
                            k = item_fn_key(node.orig_mod, ch, it, eff_ncfgs)
                            record(k, (node, fpath, fsrc, ch, it, fcfgs))
        for ch_node in node.children:
            walk(ch_node)

    walk(root_node)
    return found


def collect_mirror_tree_functions(
    mirror_entry: Path,
    top_name: str,
    side: str,
    top_cfg: tuple[str, ...] = (),
    *,
    errors: list[str] | None = None,
    root: Path | None = None,
) -> dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]]:
    """Walk an existing mirror tree and index its functions using canonical (un-suffixed) `orig_mod` keys."""
    if not mirror_entry.is_file():
        return {}
    mirror_root = build_mod_tree(top_name, mirror_entry, top_cfg, errors=errors, root=root, is_mirror=True)
    return collect_tree_functions(mirror_root, dup_errors=errors, root=root)


def _strip_type_generics(type_str: str) -> str:
    """Strip `<...>` generic arguments and outer path prefixes from a Rust type name."""
    out: list[str] = []
    depth = 0
    for ch in type_str:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth = max(0, depth - 1)
        elif depth == 0:
            out.append(ch)
    cleaned = re.sub(r"\s+", " ", "".join(out)).strip()
    return cleaned.rsplit("::", 1)[-1].strip()


def owner_overlap_group(impl_hdr: str, is_trait_impl: bool) -> str:
    """Map an impl header to an overlap equivalence class (e.g. `impl Game<'_>` and `impl Game<'static>` share one)."""
    if impl_hdr == "-":
        return "-"
    s = impl_hdr.strip()
    if s.startswith("unsafe "):
        s = s[7:].strip()
    if not s.startswith("impl"):
        return s
    rest = s[4:].strip()
    if rest.startswith("<"):
        depth = 0
        for idx, ch in enumerate(rest):
            if ch == "<":
                depth += 1
            elif ch == ">":
                depth -= 1
                if depth == 0:
                    rest = rest[idx + 1 :].strip()
                    break
    rest = re.split(r"\bwhere\b", rest, maxsplit=1)[0].strip()
    if is_trait_impl:
        m = re.match(r"^(.+?)\s+for\s+(.+)$", rest)
        if m:
            return f"trait:{_strip_type_generics(m.group(1))} for {_strip_type_generics(m.group(2))}"
        return f"trait:{_strip_type_generics(rest)}"
    return f"inherent:{_strip_type_generics(rest)}"


def _split_top_level_commas(text: str) -> list[str]:
    parts: list[str] = []
    cur: list[str] = []
    depth = 0
    in_str = False
    esc = False
    for ch in text:
        if in_str:
            cur.append(ch)
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
            continue
        if ch == '"':
            in_str = True
            cur.append(ch)
        elif ch in "([{<":
            depth += 1
            cur.append(ch)
        elif ch in ")]}>":
            depth = max(0, depth - 1)
            cur.append(ch)
        elif ch == "," and depth == 0:
            part = "".join(cur).strip()
            if part:
                parts.append(part)
            cur = []
        else:
            cur.append(ch)
    tail = "".join(cur).strip()
    if tail:
        parts.append(tail)
    return parts


def _cfg_conjuncts(cfg_str: str) -> set[str]:
    """Expand a `"; "`-joined `#[cfg(...)]` string into normalized conjunctive predicates."""
    if not cfg_str or cfg_str == "-":
        return set()
    conjuncts: set[str] = set()

    def add_pred(pred: str) -> None:
        p = re.sub(r"\s+", " ", pred).strip()
        if p.startswith("all(") and p.endswith(")"):
            for sub in _split_top_level_commas(p[4:-1]):
                add_pred(sub)
        elif p:
            conjuncts.add(p)

    for raw in cfg_str.split(";"):
        s = raw.strip()
        if s.startswith("#[cfg(") and s.endswith(")]"):
            add_pred(s[6:-2])
    return conjuncts


def cfgs_proven_disjoint(cfg_a: str, cfg_b: str) -> bool:
    """Return True only when two conjunctive cfg expressions are syntactically mutually exclusive."""
    ca = _cfg_conjuncts(cfg_a)
    cb = _cfg_conjuncts(cfg_b)
    if "any()" in ca or "any()" in cb:
        return True
    for p in ca:
        if f"not({p})" in cb:
            return True
    for q in cb:
        if f"not({q})" in ca:
            return True
    for key in ("target_os", "target_arch", "target_family", "target_env"):
        pat = re.compile(rf'^{key}\s*=\s*"([^"]+)"$')
        vals_a = {m.group(1) for p in ca for m in [pat.match(p)] if m}
        vals_b = {m.group(1) for q in cb for m in [pat.match(q)] if m}
        if vals_a and vals_b and vals_a.isdisjoint(vals_b):
            return True
    return False


def validate_top_overlaps_and_cfg_attr(
    top_rows: dict[FnKey, FnRow],
    orig_fns: dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]],
    mirror_fns: dict[str, dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]]],
    errors: list[str],
    root: Path | None = None,
) -> None:
    """Fail closed on `#[cfg_attr(...)]` on listed/mirrored/colliding items and on non-disjoint overlapping items."""
    listed_names = {(k.module, k.name) for k in top_rows} | {
        (k.module, k.name) for s in ("client", "server") for k in mirror_fns[s]
    }
    by_group: dict[tuple[str, str, str], list[tuple[FnKey, str, Path, ParsedItem, ParsedItem | None]]] = defaultdict(list)

    for loc, fdict in [("orig", orig_fns), ("client", mirror_fns["client"]), ("server", mirror_fns["server"])]:
        for k, (_node, fpath, _fsrc, it, pimpl, _fcfgs) in fdict.items():
            is_relevant = k in top_rows or loc != "orig" or (k.module, k.name) in listed_names
            if is_relevant:
                if has_cfg_attr(it.attrs):
                    errors.append(
                        f"{_rel_str(fpath, root)}:{it.start_line}: unsupported `#[cfg_attr(...)]` on `{k.module}::{k.name}`"
                    )
                if pimpl is not None and has_cfg_attr(pimpl.attrs):
                    errors.append(
                        f"{_rel_str(fpath, root)}:{pimpl.start_line}: unsupported `#[cfg_attr(...)]` on impl `{pimpl.impl_header}` containing `{k.name}`"
                    )
            elif any(
                re.search(r"#\[\s*(?:(?:core|std)::)?cfg_attr\s*\([^)]*\bcfg\s*\(", a)
                for a in (*it.attrs, *(pimpl.attrs if pimpl is not None else ()))
            ):
                errors.append(
                    f"{_rel_str(fpath, root)}:{it.start_line}: unsupported conditional `#[cfg_attr(..., cfg(...))]` on `{k.module}::{k.name}`"
                )
            is_trait = pimpl.is_trait_impl if pimpl is not None else False
            grp = (k.module, owner_overlap_group(k.impl, is_trait), k.name)
            by_group[grp].append((k, loc, fpath, it, pimpl))

    for (_mod, _owner, _name), occs in sorted(by_group.items()):
        if len(occs) < 2:
            continue
        if not any(k in top_rows or loc != "orig" for k, loc, _fp, _it, _pi in occs):
            continue
        for idx in range(len(occs)):
            k1, loc1, fp1, it1, _ = occs[idx]
            for jdx in range(idx + 1, len(occs)):
                k2, loc2, fp2, it2, _ = occs[jdx]
                if not cfgs_proven_disjoint(k1.cfg, k2.cfg):
                    errors.append(
                        f"overlapping function definitions for `{k1.module}::{k1.name}`: "
                        f"`{k1.impl}` ({k1.cfg}) at {_rel_str(fp1, root)}:{it1.start_line} ({loc1}) and "
                        f"`{k2.impl}` ({k2.cfg}) at {_rel_str(fp2, root)}:{it2.start_line} ({loc2})"
                    )


def check_orphan_mirror_files(
    root: Path,
    regular_mods: dict[str, tuple[str, Path, tuple[str, ...]]],
    all_regular_entries: list[Path],
    existing_mirrors: dict[str, dict[str, tuple[Path, tuple[str, ...]]]],
    errors: list[str],
) -> None:
    """Fail closed if any `*_{client,server}.rs` file under `game/` is unreachable from declared modules/mirrors."""
    src_root = root / "crates/sunrise-client/src"
    game_dir = src_root / "game"
    if not game_dir.is_dir():
        return
    reachable: set[Path] = set()

    def collect_tree_paths(node: ModNode) -> None:
        reachable.add(node.file_path.resolve())
        for fp, _, _, _ in node.extra_files:
            reachable.add(fp.resolve())
        for ch in node.children:
            collect_tree_paths(ch)

    game_rs = src_root / "game.rs"
    if game_rs.is_file():
        collect_tree_paths(build_mod_tree("game", game_rs.resolve()))
    for name, (_ps, entry, cfg) in regular_mods.items():
        if entry.is_file():
            collect_tree_paths(build_mod_tree(name, entry, cfg))
    for entry in all_regular_entries:
        if entry.is_file() and entry.resolve() not in reachable:
            collect_tree_paths(build_mod_tree(entry.stem, entry))
    for side in ("client", "server"):
        for mname, (mentry, mcfg) in existing_mirrors[side].items():
            if mentry.is_file():
                collect_tree_paths(build_mod_tree(mname, mentry, mcfg))

    for p in sorted(game_dir.rglob("*.rs")):
        if not re.search(r"_(?:client|server)\.rs$", p.name):
            continue
        if p.resolve() not in reachable:
            errors.append(
                f"{_rel_str(p, root)}: orphan mirror file not declared in game/client_side/server_side module graph"
            )


def _scan_macro_calls(tokens: list[Token]) -> list[tuple[str, str, int]]:
    """Return `[(full_path, short_name, line)]` for macro invocations, skipping `stringify!(...)` bodies."""
    out: list[tuple[str, str, int]] = []
    i, n = 0, len(tokens)
    while i < n:
        t = tokens[i]
        if t.kind == "ident":
            # Skip `foo.bar!` (not valid) and `macro_rules! name` definition head
            if i > 0 and tokens[i - 1].text == ".":
                i += 1
                continue
            p = i
            while p < n and (tokens[p].kind == "ident" or tokens[p].text == "::"):
                p += 1
            if p < n and tokens[p].text == "!":
                mpath = "".join(tok.text for tok in tokens[i:p])
                short = mpath.rsplit("::", 1)[-1]
                if mpath == "macro_rules":
                    # Skip the defined macro name right after `macro_rules!`
                    i = p + 2
                    continue
                if short == "stringify" and p + 1 < n and tokens[p + 1].text in ("(", "[", "{"):
                    cl = {"(": ")", "[": "]", "{": "}"}[tokens[p + 1].text]
                    i = skip_delim(tokens, p + 1, tokens[p + 1].text, cl)
                    continue
                out.append((mpath, short, t.line))
                i = p + 1
                continue
        i += 1
    return out


def find_relative_includes_in_src(src: str) -> list[str]:
    """Return any `include!`/`include_str!`/`include_bytes!` macro names invoked in `src`."""
    try:
        tokens = lex_rust(src)
    except ValueError:
        return []
    hits: list[str] = []
    for mpath, short, _ln in _scan_macro_calls(tokens):
        if short in INCLUDE_MACROS and mpath in (short, f"std::{short}", f"core::{short}"):
            if f"{mpath}!" not in hits:
                hits.append(f"{mpath}!")
    return hits


def collect_location_sensitive_macros(trees: list[ModNode]) -> dict[str, str]:
    """Compute fixed-point map of local `macro_rules!` names expanding to `module_path!`/`file!`/`line!`/`column!`."""
    macro_bodies: dict[str, list[str]] = defaultdict(list)

    def walk(node: ModNode) -> None:
        for _fp, fsrc, fitems, _fcfgs in node_all_fragments(node):
            for it in fitems:
                if it.kind == "macro_rules":
                    macro_bodies[it.name].append(fsrc[it.start : it.end])
        for ch in node.children:
            walk(ch)

    for tr in trees:
        walk(tr)

    sensitive: dict[str, str] = {}
    calls_by_macro: dict[str, list[tuple[str, str]]] = {}
    for mname, bodies in macro_bodies.items():
        calls: list[tuple[str, str]] = []
        for b in bodies:
            try:
                toks = lex_rust(b)
            except ValueError:
                continue
            for mpath, short, _ln in _scan_macro_calls(toks):
                calls.append((mpath, short))
                if short in LOCATION_MACROS and mpath in (short, f"std::{short}", f"core::{short}"):
                    sensitive.setdefault(mname, f"{mname}! -> {short}!")
        calls_by_macro[mname] = calls

    changed = True
    while changed:
        changed = False
        for mname, calls in calls_by_macro.items():
            if mname in sensitive:
                continue
            for mpath, short in calls:
                if mpath == short and short in sensitive:
                    sensitive[mname] = f"{mname}! -> {sensitive[short]}"
                    changed = True
                    break
    return sensitive


def find_location_macro_uses(src: str, sensitive_local: dict[str, str]) -> list[str]:
    """Return location-sensitive built-ins or local macros invoked in `src`."""
    try:
        tokens = lex_rust(src)
    except ValueError:
        return []
    hits: list[str] = []
    for mpath, short, _ln in _scan_macro_calls(tokens):
        if short in LOCATION_MACROS and mpath in (short, f"std::{short}", f"core::{short}"):
            label = f"{short}!"
            if label not in hits:
                hits.append(label)
        elif mpath == short and short in sensitive_local:
            label = sensitive_local[short]
            if label not in hits:
                hits.append(label)
    return hits


ALLOWED_MIRROR_ITEM_KINDS = frozenset({"use", "macro_rules", "mod_ext", "mod_include", "mod_inline", "fn", "impl"})


def _canonicalize_use_tree_tokens(toks: list[str]) -> list[str]:
    """Normalize braced `use` groups (`{b, a}` -> `{a, b}`, single `{a}` -> `a`, drop trailing commas)."""
    out: list[str] = []
    i, n = 0, len(toks)
    while i < n:
        if toks[i] == "{":
            depth = 1
            j = i + 1
            while j < n and depth > 0:
                if toks[j] == "{":
                    depth += 1
                elif toks[j] == "}":
                    depth -= 1
                j += 1
            inner = toks[i + 1 : j - 1]
            # Split inner at depth-0 commas
            branches: list[list[str]] = []
            cur: list[str] = []
            bdepth = 0
            for tk in inner:
                if tk in ("{", "(", "["):
                    bdepth += 1
                    cur.append(tk)
                elif tk in ("}", ")", "]"):
                    bdepth -= 1
                    cur.append(tk)
                elif tk == "," and bdepth == 0:
                    if cur:
                        branches.append(_canonicalize_use_tree_tokens(cur))
                        cur = []
                else:
                    cur.append(tk)
            if cur:
                branches.append(_canonicalize_use_tree_tokens(cur))
            branches.sort()
            if len(branches) == 1 and branches[0] != ["self"]:
                out.extend(branches[0])
            else:
                out.append("{")
                for idx, br in enumerate(branches):
                    if idx > 0:
                        out.append(",")
                    out.extend(br)
                out.append("}")
            i = j
            continue
        out.append(toks[i])
        i += 1
    return out


def _canonical_item_tokens(src: str, it: ParsedItem) -> tuple[str, ...]:
    """Return formatting-invariant token sequence for a Rust item (ignoring whitespace & trailing commas)."""
    raw_toks = [t.text for t in lex_rust(src[it.start : it.end])]
    no_trail: list[str] = []
    for idx, tk in enumerate(raw_toks):
        if tk == "," and idx + 1 < len(raw_toks) and raw_toks[idx + 1] in (")", "]", "}"):
            continue
        no_trail.append(tk)
    if it.kind == "use":
        no_trail = _canonicalize_use_tree_tokens(no_trail)
    return tuple(no_trail)


def _canonical_mod_inline_header(src: str, it: ParsedItem) -> tuple[str, ...]:
    raw_toks = [t.text for t in lex_rust(src[it.start : it.impl_open_brace])]
    no_trail = [
        tk for idx, tk in enumerate(raw_toks)
        if not (tk == "," and idx + 1 < len(raw_toks) and raw_toks[idx + 1] in (")", "]", "}"))
    ]
    return tuple(no_trail)


def _diff_mirror_items(
    rel: str,
    actual_items: list[ParsedItem],
    actual_src: str,
    expected_items: list[ParsedItem],
    expected_src: str,
    *,
    allow_reorder_methods: bool = False,
) -> list[str]:
    errs: list[str] = []
    for it in actual_items:
        if it.kind not in ALLOWED_MIRROR_ITEM_KINDS:
            errs.append(f"{rel}:{it.start_line}: unexpected hand-written `{it.kind}` item `{it.name}` in mirror file")

    act_uses = Counter(_canonical_item_tokens(actual_src, it) for it in actual_items if it.kind == "use")
    exp_uses = Counter(_canonical_item_tokens(expected_src, it) for it in expected_items if it.kind == "use")
    if act_uses != exp_uses:
        extra_u = act_uses - exp_uses
        missing_u = exp_uses - act_uses
        if extra_u:
            sample = " ".join(next(iter(extra_u)))
            errs.append(f"{rel}: unexpected `use` item in mirror file: `{sample[:100]}`")
        if missing_u:
            sample = " ".join(next(iter(missing_u)))
            errs.append(f"{rel}: missing generated `use` item in mirror file: `{sample[:100]}`")

    act_other = [it for it in actual_items if it.kind != "use" and it.kind in ALLOWED_MIRROR_ITEM_KINDS]
    exp_other = [it for it in expected_items if it.kind != "use"]
    if len(act_other) != len(exp_other):
        act_desc = [(it.kind, it.name) for it in act_other]
        exp_desc = [(it.kind, it.name) for it in exp_other]
        errs.append(
            f"{rel}: mirror file item count mismatch (actual {len(act_other)} vs generated {len(exp_other)}; "
            f"extra={set(act_desc) - set(exp_desc)}, missing={set(exp_desc) - set(act_desc)})"
        )
        return errs

    for a_it, e_it in zip(act_other, exp_other):
        if a_it.kind != e_it.kind or a_it.name != e_it.name:
            errs.append(
                f"{rel}:{a_it.start_line}: mirror item `{a_it.kind} {a_it.name}` does not match generated `{e_it.kind} {e_it.name}`"
            )
            continue
        if a_it.kind == "mod_inline":
            if _canonical_mod_inline_header(actual_src, a_it) != _canonical_mod_inline_header(expected_src, e_it):
                errs.append(f"{rel}:{a_it.start_line}: inline mirror mod `{a_it.name}` header differs from generated output")
            inner_act_src = actual_src[a_it.impl_open_brace + 1 : a_it.impl_close_brace]
            if has_top_level_inner_attrs(inner_act_src):
                errs.append(f"{rel}:{a_it.start_line}: disallowed inner attribute `#![...]` inside mirror mod `{a_it.name}`")
            errs.extend(
                _diff_mirror_items(
                    rel,
                    a_it.children,
                    actual_src,
                    e_it.children,
                    expected_src,
                    allow_reorder_methods=allow_reorder_methods,
                )
            )
        elif a_it.kind == "impl" and allow_reorder_methods and not a_it.is_trait_impl:
            a_hdr = _canonical_mod_inline_header(actual_src, a_it)
            e_hdr = _canonical_mod_inline_header(expected_src, e_it)
            a_inner = actual_src[a_it.impl_open_brace + 1 : a_it.impl_close_brace]
            a_ch_ok = all(ch.kind == "method" for ch in a_it.children) and not has_top_level_inner_attrs(a_inner)
            a_methods = Counter(_canonical_item_tokens(actual_src, ch) for ch in a_it.children)
            e_methods = Counter(_canonical_item_tokens(expected_src, ch) for ch in e_it.children)
            if a_hdr != e_hdr or not a_ch_ok or a_methods != e_methods:
                errs.append(
                    f"{rel}:{a_it.start_line}: mirror item `{a_it.kind} {a_it.name}` differs from tool-generated output"
                )
        else:
            if _canonical_item_tokens(actual_src, a_it) != _canonical_item_tokens(expected_src, e_it):
                errs.append(
                    f"{rel}:{a_it.start_line}: mirror item `{a_it.kind} {a_it.name}` differs from tool-generated output"
                )
    return errs


def diff_mirror_file(
    rel: str, actual_src: str, expected_src: str, *, allow_reorder_methods: bool = False
) -> list[str]:
    """Compare an existing mirror file on disk against the tool-generated output (formatting-invariant)."""
    errs: list[str] = []
    inner_attr_lines = has_top_level_inner_attrs(actual_src)
    if inner_attr_lines:
        errs.append(f"{rel}:{inner_attr_lines[0]}: disallowed inner attribute `#![...]` in mirror file")
    try:
        actual_items = parse_rust_items(actual_src)
    except ValueError as exc:
        return [f"{rel}: parse error: {exc}"]
    try:
        expected_items = parse_rust_items(expected_src)
    except ValueError as exc:
        return [f"{rel}: generated mirror parse error: {exc}"]
    errs.extend(
        _diff_mirror_items(
            rel,
            actual_items,
            actual_src,
            expected_items,
            expected_src,
            allow_reorder_methods=allow_reorder_methods,
        )
    )
    return errs


def rewrite_vis_to_game(vis: str, *, preserve_side: str | None = None) -> str | None:
    norm = re.sub(r"\s+", " ", vis).replace("( ", "(").replace(" )", ")")
    if norm in ("pub", "pub(crate)", "pub(in crate::game)"):
        return None
    if preserve_side in SIDE_MOD_NAME and norm == f"pub(in crate::game::{SIDE_MOD_NAME[preserve_side]})":
        return None
    return "pub(in crate::game)"


def widen_struct_fields_edits(src: str, item: ParsedItem, only_super: bool = False) -> list[tuple[int, int, str]]:
    if item.kind != "struct" or not item.struct_body_start or item.struct_body_end <= item.struct_body_start:
        return []
    body = src[item.struct_body_start + 1 : item.struct_body_end]
    base = item.struct_body_start + 1
    tokens = lex_rust(body)
    edits: list[tuple[int, int, str]] = []
    i, n = 0, len(tokens)
    while i < n:
        while i < n and tokens[i].text == "#" and i + 1 < n and tokens[i + 1].text == "[":
            i = skip_delim(tokens, i + 1, "[", "]")
        if i >= n:
            break
        vstart = tokens[i].start
        vis, vend = "", vstart
        if tokens[i].kind == "ident" and tokens[i].text == "pub":
            j = i + 1
            if j < n and tokens[j].text == "(":
                j = skip_delim(tokens, j, "(", ")")
            vend = tokens[j - 1].end
            vis = body[vstart:vend]
            i = j
        if i >= n:
            break
        norm_vis = re.sub(r"\s+", " ", vis).replace("( ", "(").replace(" )", ")")
        if norm_vis == "" and not only_super:
            edits.append((base + vstart, base + vstart, "pub(in crate::game) "))
        elif norm_vis in ("pub(super)", "pub(self)", "pub(in super)") or norm_vis.startswith("pub(in super::"):
            edits.append((base + vstart, base + vend, "pub(in crate::game)"))
        while i < n:
            t = tokens[i].text
            if t == ",":
                i += 1
                break
            if t == "<":
                i = skip_angle(tokens, i)
            elif t in ("(", "[", "{"):
                i = skip_delim(tokens, i, t, {"(": ")", "[": "]", "{": "}"}[t])
            else:
                i += 1
    return edits


def widen_thread_local_edits(src: str, item: ParsedItem, only_super: bool = False) -> list[tuple[int, int, str]]:
    """Widen `static` items inside `thread_local! { ... }` to `pub(in crate::game)`."""
    if item.kind != "macro_invoc" or item.name != "thread_local":
        return []
    brace = src.find("{", item.kw_start, item.end)
    close = src.rfind("}", item.kw_start, item.end)
    if brace == -1 or close <= brace:
        return []
    body = src[brace + 1 : close]
    base = brace + 1
    inner_items = parse_rust_items(body, item.source_file)
    edits: list[tuple[int, int, str]] = []
    for it in inner_items:
        if it.kind != "static":
            continue
        rep = rewrite_vis_to_game(it.vis)
        if not rep:
            continue
        if it.vis == "":
            if not only_super:
                edits.append((base + it.vis_start, base + it.vis_start, f"{rep} "))
        elif not only_super or "super" in it.vis:
            edits.append((base + it.vis_start, base + it.vis_end, rep))
    return edits


def thread_local_statics(src: str, item: ParsedItem) -> list[tuple[str, tuple[str, ...]]]:
    """Return `[(name, cfg_attrs)]` for every `static` inside `thread_local! { ... }`."""
    if item.kind != "macro_invoc" or item.name != "thread_local":
        return []
    brace = src.find("{", item.kw_start, item.end)
    close = src.rfind("}", item.kw_start, item.end)
    if brace == -1 or close <= brace:
        return []
    body = src[brace + 1 : close]
    return [(it.name, (*item.cfg_attrs, *it.cfg_attrs)) for it in parse_rust_items(body, item.source_file) if it.kind == "static" and it.name]


def rewrite_use_stmt_for_mirror(
    use_text: str, orig_canon_mod: str, local_child_names: set[str], side_mod: str, *, is_game_root: bool = False
) -> str:
    tokens = lex_rust(use_text)
    use_idx = None
    attr_strs: list[str] = []
    i = 0
    while i < len(tokens):
        if tokens[i].text == "#" and i + 1 < len(tokens) and tokens[i + 1].text == "[":
            a_s = tokens[i].start
            nxt = skip_delim(tokens, i + 1, "[", "]")
            a_txt = use_text[a_s : tokens[nxt - 1].end]
            if "unused_imports" not in a_txt:
                attr_strs.append(a_txt)
            i = nxt
            continue
        if tokens[i].kind == "ident" and tokens[i].text == "pub":
            j = i + 1
            if j < len(tokens) and tokens[j].text == "(":
                j = skip_delim(tokens, j, "(", ")")
            i = j
            continue
        if tokens[i].kind == "ident" and tokens[i].text == "use":
            use_idx = i
            break
        i += 1
    if use_idx is None:
        return use_text
    prefix_clean = "".join(f"{a}\n" for a in attr_strs)
    after_use = use_text[tokens[use_idx].end :].lstrip()
    m = re.match(r"^((?:r#)?[A-Za-z_][A-Za-z0-9_]*)\b", after_use)
    if m and m.group(1) in local_child_names and m.group(1) not in ("super", "self", "crate"):
        after_use = f"{orig_canon_mod}::{after_use}"
    elif after_use.startswith("self::"):
        after_use = f"{orig_canon_mod}::{after_use[6:]}"
    elif is_game_root and after_use.startswith("super::"):
        after_use = f"crate::{after_use[7:]}"
    return f"#[allow(unused_imports)]\n{prefix_clean}pub(in crate::game::{side_mod}) use {after_use}"


def rewrite_moved_fn_vis(
    fn_text: str,
    *,
    is_trait_impl: bool,
    is_test_fn: bool,
    is_game_root: bool = False,
    side: str | None = None,
) -> str:
    if is_trait_impl or is_test_fn:
        return fn_text
    tokens = lex_rust(fn_text)
    i = 0
    while i < len(tokens) and tokens[i].text == "#" and i + 1 < len(tokens) and tokens[i + 1].text == "[":
        i = skip_delim(tokens, i + 1, "[", "]")
    if i >= len(tokens):
        return fn_text
    if tokens[i].kind == "ident" and tokens[i].text == "pub":
        j = i + 1
        if j < len(tokens) and tokens[j].text == "(":
            j = skip_delim(tokens, j, "(", ")")
        vis = fn_text[tokens[i].start : tokens[j - 1].end]
        norm = re.sub(r"\s+", " ", vis).replace("( ", "(").replace(" )", ")")
        if is_game_root and norm in ("pub(super)", "pub(in super)"):
            return fn_text[: tokens[i].start] + "pub(crate)" + fn_text[tokens[j - 1].end :]
        rep = rewrite_vis_to_game(vis, preserve_side=side)
        if rep:
            return fn_text[: tokens[i].start] + rep + fn_text[tokens[j - 1].end :]
        return fn_text
    return fn_text[: tokens[i].start] + "pub(in crate::game) " + fn_text[tokens[i].start :]


def mirror_mod_rel(orig_mod_rel: str, top_name: str, side: str) -> str:
    """`commands::execute` -> `server_side::commands_server::execute`, `game` -> `server_side::game_server`."""
    side_mod = SIDE_MOD_NAME[side]
    rest = orig_mod_rel[len(top_name) :]
    return f"{side_mod}::{top_name}_{side}{rest}"


def mirror_file_path(orig_file: Path, side: str) -> Path:
    if orig_file.name == "game.rs" and (orig_file.parent / "game").is_dir():
        return orig_file.parent / "game" / f"game_{side}.rs"
    return orig_file.with_name(f"{orig_file.stem}_{side}.rs")


def apply_edits(src: str, edits: list[tuple[int, int, str]]) -> str:
    edits_sorted = sorted(edits, key=lambda e: (e[0], e[1]), reverse=True)
    out = src
    last_start = len(src) + 1
    for s, e, rep in edits_sorted:
        if e > last_start:
            raise RuntimeError(f"overlapping edits: ({s}, {e}) vs previous start {last_start}")
        out = out[:s] + rep + out[e:]
        last_start = s
    return out


def is_cfg_test_node(node: ModNode) -> bool:
    return any(re.search(r"#\[cfg\([^)]*\btest\b", c) for c in node.cfg_attrs)


def macro_side_exclusive(node: ModNode, macro_name: str, desired_side: dict[FnKey, str]) -> str | None:
    """If `macro_name!` in `node` is ONLY used by moved functions of one side, return that side."""
    pat = re.compile(rf"\b{re.escape(macro_name)}\s*!")
    used_sides: set[str] = set()

    def scan_node(cur: ModNode) -> None:
        ncfgs = node_cfg_chain(cur)
        for _fp, fsrc, fitems, fcfgs in node_all_fragments(cur):
            eff_ncfgs = (*ncfgs, *fcfgs)
            for it in fitems:
                if it.kind == "macro_rules":
                    continue
                if it.kind == "fn":
                    if pat.search(fsrc[it.start : it.end]):
                        used_sides.add(desired_side.get(item_fn_key(cur.orig_mod, it, None, eff_ncfgs), "game"))
                elif it.kind == "impl":
                    for ch in it.children:
                        if ch.kind == "method" and pat.search(fsrc[ch.start : ch.end]):
                            used_sides.add(desired_side.get(item_fn_key(cur.orig_mod, ch, it, eff_ncfgs), "game"))
                elif pat.search(fsrc[it.start : it.end]):
                    used_sides.add("game")
        for ch_n in cur.children:
            scan_node(ch_n)

    scan_node(node)
    if used_sides == {"client"}:
        return "client"
    if used_sides == {"server"}:
        return "server"
    return None


def collect_mirror_files(mirror_entry: Path) -> set[Path]:
    """Return every `.rs` file belonging to an existing mirror module tree."""
    if not mirror_entry.is_file():
        return set()
    files: set[Path] = set()
    root = build_mod_tree("mirror", mirror_entry)

    def walk(n: ModNode) -> None:
        files.add(n.file_path.resolve())
        for fp, _, _, _ in n.extra_files:
            files.add(fp.resolve())
        for ch in n.children:
            walk(ch)

    walk(root)
    return files


def plan_side_file_mirror_decls(
    root: Path,
    side: str,
    expected_decls: dict[str, tuple[str, tuple[str, ...]]],
    regular_names: set[str] | frozenset[str] = frozenset(),
    errors: list[str] | None = None,
) -> tuple[Path, str] | None:
    """Compute updated `{client,server}_side.rs` content without writing to disk."""
    side_file = root / "crates/sunrise-client/src/game" / f"{SIDE_MOD_NAME[side]}.rs"
    if not side_file.is_file():
        return None
    old_txt = side_file.read_text(encoding="utf-8")
    items = parse_rust_items(old_txt, side_file)

    edits: list[tuple[int, int, str]] = []
    for it in items:
        if it.kind == "mod_ext" and it.name not in regular_names:
            m = MIRROR_MOD_RE.match(it.name)
            if m and m.group(2) == side and (it.path_attr or "").endswith(f"_{side}.rs"):
                edits.append((it.start, it.end, ""))
    cleaned = apply_edits(old_txt, edits)

    if expected_decls:
        struct_kw = "struct ClientState" if side == "client" else "struct ServerState"
        c_items = parse_rust_items(cleaned, side_file)
        insert_at = None
        for it in c_items:
            if it.kind == "struct" and it.name == ("ClientState" if side == "client" else "ServerState"):
                insert_at = it.start
                break
        if insert_at is None:
            msg = f"{side_file}: could not locate `{struct_kw}`"
            if errors is not None:
                errors.append(msg)
                return None
            raise SystemExit(msg)
        block_lines: list[str] = []
        for mod_name in sorted(expected_decls):
            rel_path, cfgs = expected_decls[mod_name]
            for c in cfgs:
                block_lines.append(f"{c}\n")
            block_lines.append(f'#[path = "{rel_path}"]\n')
            block_lines.append(f"pub(in crate::game) mod {mod_name};\n")
        block = "".join(block_lines)
        prefix = cleaned[:insert_at].rstrip("\n") + "\n\n"
        suffix = cleaned[insert_at:].lstrip("\n")
        new_txt = prefix + block + "\n" + suffix
    else:
        new_txt = re.sub(r"\n{3,}", "\n\n", cleaned)

    if new_txt != old_txt:
        return (side_file.resolve(), new_txt)
    return None


def sync_side_file_mirror_decls(
    root: Path,
    side: str,
    expected_decls: dict[str, tuple[str, tuple[str, ...]]],
    regular_names: set[str] | frozenset[str] = frozenset(),
) -> Path | None:
    """Update `<m>_<side>` declarations in `{client,server}_side.rs` and return path if changed."""
    planned = plan_side_file_mirror_decls(root, side, expected_decls, regular_names)
    if planned is not None:
        side_file, new_txt = planned
        side_file.write_text(new_txt, encoding="utf-8")
        return side_file
    return None


def rewrite_root_pub_super_check(text: str) -> tuple[int, list[int]]:
    """Count root-scope `pub(super)` occurrences in a mirror file (outside comments/strings/nested mods)."""
    tokens = lex_rust(text)
    brace_depth = 0
    mod_depths: list[int] = []
    pending_mod = False
    hits: list[int] = []
    i, n = 0, len(tokens)
    while i < n:
        t = tokens[i]
        if t.text == "{":
            brace_depth += 1
            if pending_mod:
                mod_depths.append(brace_depth)
                pending_mod = False
        elif t.text == "}":
            if mod_depths and mod_depths[-1] == brace_depth:
                mod_depths.pop()
            brace_depth = max(0, brace_depth - 1)
        elif t.text == ";" and pending_mod:
            pending_mod = False
        elif t.kind == "ident" and t.text == "mod":
            if i == 0 or tokens[i - 1].text not in ("::", "."):
                pending_mod = True
        elif t.kind == "ident" and t.text == "pub" and not mod_depths:
            if i + 3 < n and tokens[i + 1].text == "(" and tokens[i + 2].text == "super" and tokens[i + 3].text == ")":
                hits.append(t.line)
        i += 1
    return len(hits), hits


def build_selector_maps(
    rows: dict[FnKey, FnRow], mod_sides: dict[str, str], *, errors: list[str] | None = None
) -> tuple[dict[str, str], dict[str, str], dict[str, set[str]]]:
    """Return `(test_path_map, fnr_target_map, mod_mirror_test_sides)`, failing closed on split-side cfg conflicts."""
    test_map: dict[str, str] = {}
    fnr_map: dict[str, str] = {}
    mod_test_sides: dict[str, set[str]] = defaultdict(set)
    errs = errors if errors is not None else []
    seen_conflicts: set[tuple[str, str, str]] = set()

    def set_checked(mapping: dict[str, str], src: str, dst: str, label: str) -> None:
        prev = mapping.get(src)
        if prev is not None and prev != dst:
            key = (label, src, min(prev, dst) + " vs " + max(prev, dst))
            if key not in seen_conflicts:
                seen_conflicts.add(key)
                errs.append(f"conflicting {label} selector for `{src}`: `{prev}` vs `{dst}` (cfg variants map to different sides)")
            return
        mapping[src] = dst

    sorted_rows = sorted(
        rows.values(),
        key=lambda r: (r.key.module, r.key.impl, r.key.name, r.key.cfg, r.side),
    )
    for row in sorted_rows:
        k = row.key
        top = row.top_module
        mod_side = mod_sides.get(top, "game") if top != "game" else "game"
        base_orig = orig_game_mod_path(k.module)
        orig_variants = [base_orig]
        if mod_side in ("client", "server"):
            orig_variants.append(f"game::{SIDE_MOD_NAME[mod_side]}::{k.module}")
        if row.side in ("client", "server"):
            new_mod = f"game::{mirror_mod_rel(k.module, top, row.side)}"
            if k.impl == "-":
                mod_test_sides[top].add(row.side)
                for ov in orig_variants:
                    set_checked(test_map, f"{ov}::{k.name}", f"{new_mod}::{k.name}", "test")
                    set_checked(fnr_map, f"sunrise_client::{ov}::{k.name}", f"sunrise_client::{new_mod}::{k.name}", "fn-replace")
                other = "server" if row.side == "client" else "client"
                other_mod = f"game::{mirror_mod_rel(k.module, top, other)}"
                set_checked(test_map, f"{other_mod}::{k.name}", f"{new_mod}::{k.name}", "test")
            else:
                owner = impl_owner_for_target(k.impl)
                if owner:
                    for ov in orig_variants:
                        set_checked(
                            fnr_map,
                            f"sunrise_client::{ov}::{owner}::{k.name}",
                            f"sunrise_client::{new_mod}::{owner}::{k.name}",
                            "fn-replace",
                        )
        else:
            canon_mod = f"game::{SIDE_MOD_NAME[mod_side]}::{k.module}" if mod_side in ("client", "server") else base_orig
            if k.impl == "-":
                for ov in orig_variants:
                    set_checked(test_map, f"{ov}::{k.name}", f"{canon_mod}::{k.name}", "test")
                    set_checked(fnr_map, f"sunrise_client::{ov}::{k.name}", f"sunrise_client::{canon_mod}::{k.name}", "fn-replace")
                for s in ("client", "server"):
                    stale_mod = f"game::{mirror_mod_rel(k.module, top, s)}"
                    set_checked(test_map, f"{stale_mod}::{k.name}", f"{canon_mod}::{k.name}", "test")
                    set_checked(fnr_map, f"sunrise_client::{stale_mod}::{k.name}", f"sunrise_client::{canon_mod}::{k.name}", "fn-replace")
            else:
                owner = impl_owner_for_target(k.impl)
                if owner:
                    for ov in orig_variants:
                        set_checked(
                            fnr_map,
                            f"sunrise_client::{ov}::{owner}::{k.name}",
                            f"sunrise_client::{canon_mod}::{owner}::{k.name}",
                            "fn-replace",
                        )
                    for s in ("client", "server"):
                        stale_mod = f"game::{mirror_mod_rel(k.module, top, s)}"
                        set_checked(
                            fnr_map,
                            f"sunrise_client::{stale_mod}::{owner}::{k.name}",
                            f"sunrise_client::{canon_mod}::{owner}::{k.name}",
                            "fn-replace",
                        )

    test_map = {k: v for k, v in test_map.items() if k != v}
    fnr_map = {k: v for k, v in fnr_map.items() if k != v}
    return test_map, fnr_map, mod_test_sides


def impl_owner_for_target(impl_hdr: str) -> str | None:
    """Convert `impl Game<'_>` -> `Game<'_>`, `impl Trait for Type<'_>` -> `<Type<'_> as Trait>`."""
    s = impl_hdr.strip()
    if not s.startswith("impl"):
        return None
    rest = s[4:].strip()
    if rest.startswith("<"):
        depth = 0
        for idx, ch in enumerate(rest):
            if ch == "<":
                depth += 1
            elif ch == ">":
                depth -= 1
                if depth == 0:
                    rest = rest[idx + 1 :].strip()
                    break
    # Strip trailing where clause
    rest = re.split(r"\bwhere\b", rest, maxsplit=1)[0].strip()
    m = re.match(r"^(.+?)\s+for\s+(.+)$", rest)
    if m:
        return f"<{m.group(2).strip()} as {m.group(1).strip()}>"
    return rest


def rewrite_exact_map(text: str, mapping: dict[str, str], pat: re.Pattern[str]) -> str:
    if not mapping:
        return text
    return pat.sub(lambda m: mapping.get(m.group(0), m.group(0)), text)


def rewrite_test_areas_for_mirrors(text: str, mod_test_sides: dict[str, set[str]], test_map: dict[str, str]) -> str:
    """Expand module-level `game::<m>::` filters in `tools/test_areas.toml` to include mirror modules with tests."""
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    i = 0
    while i < len(lines):
        m = TEST_AREAS_FILTER_RE.match(lines[i])
        if not m:
            out.append(lines[i])
            i += 1
            continue
        head, full_filter, _old_side, mod_name, suffix, tail = m.groups()
        suffix = suffix or ""
        # Exact test filter match
        if full_filter in test_map:
            out.append(f"{head}{test_map[full_filter]}{tail}")
            i += 1
            continue
        # Module filter `game::[side::]<m>::` (or prefix matching a split module with mirror tests)
        if suffix == "::" and mod_name in mod_test_sides:
            # Consume any already-emitted mirror filter lines right after this line
            j = i + 1
            while j < len(lines):
                mj = TEST_AREAS_FILTER_RE.match(lines[j])
                if mj and mj.group(5) == "::" and mj.group(4) in (f"{mod_name}_client", f"{mod_name}_server"):
                    j += 1
                else:
                    break
            out.append(lines[i])
            for s in ("client", "server"):
                if s in mod_test_sides[mod_name]:
                    out.append(f"{head}game::{SIDE_MOD_NAME[s]}::{mod_name}_{s}::{tail}")
            i = j
            continue
        out.append(lines[i])
        i += 1
    return "".join(out)


def build_port_map_file_moves(
    root: Path, rows: dict[FnKey, FnRow], mod_sides: dict[str, str], *, errors: list[str] | None = None
) -> dict[tuple[str, str], str]:
    """Return `(old_file_rel, fn_name) -> target_file_rel` for `tools/port_map/*.tsv` rows, failing closed on conflicts."""
    regular_mods, _ = parse_top_mod_decls(root, set(mod_sides))
    rows_by_top: dict[str, list[FnRow]] = defaultdict(list)
    for row in sorted(rows.values(), key=lambda r: (r.key.module, r.key.impl, r.key.name, r.key.cfg, r.side)):
        rows_by_top[row.top_module].append(row)

    moves: dict[tuple[str, str], str] = {}
    errs = errors if errors is not None else []
    seen_conflicts: set[tuple[str, str]] = set()

    def set_move(src_rel: str, fn_name: str, dest: str) -> None:
        key = (src_rel, fn_name)
        prev = moves.get(key)
        if prev is not None and prev != dest:
            if key not in seen_conflicts:
                seen_conflicts.add(key)
                errs.append(f"conflicting port_map move for `{src_rel}::{fn_name}`: `{prev}` vs `{dest}`")
            return
        moves[key] = dest

    for top_name, top_rows in sorted(rows_by_top.items()):
        resolved = resolve_top_entry(root, top_name, regular_mods)
        if resolved is None:
            continue
        _ps, entry_file, top_cfg = resolved
        tree = build_mod_tree(top_name, entry_file, top_cfg)
        mod_nodes: dict[str, list[ModNode]] = defaultdict(list)

        def collect_nodes(n: ModNode) -> None:
            mod_nodes[n.orig_mod].append(n)
            for ch in n.children:
                collect_nodes(ch)

        collect_nodes(tree)
        staying_by_file: dict[str, set[str]] = defaultdict(set)
        orig_fns = collect_tree_functions(tree)
        desired = {r.key: r.side for r in top_rows}
        for k, (_n, fpath, _fsrc, _it, _pi, _fc) in orig_fns.items():
            if desired.get(k, "game") == "game":
                staying_by_file[fpath.resolve().relative_to(root).as_posix()].add(k.name)

        for row in top_rows:
            nodes = mod_nodes.get(row.key.module, [])
            if not nodes:
                continue
            for n in nodes:
                orig_main_rel = n.file_path.resolve().relative_to(root).as_posix()
                orig_all_rels = [orig_main_rel, *(fp.resolve().relative_to(root).as_posix() for fp, _, _, _ in n.extra_files)]
                c_rel = mirror_file_path(n.file_path, "client").resolve().relative_to(root).as_posix()
                s_rel = mirror_file_path(n.file_path, "server").resolve().relative_to(root).as_posix()
                fn_name = row.key.name
                if row.side in ("client", "server"):
                    dest = c_rel if row.side == "client" else s_rel
                    other = s_rel if row.side == "client" else c_rel
                    for src_rel in (*orig_all_rels, other):
                        if src_rel != dest and fn_name not in staying_by_file.get(src_rel, set()):
                            set_move(src_rel, fn_name, dest)
                else:
                    for stale_rel in (c_rel, s_rel):
                        if stale_rel != orig_main_rel:
                            set_move(stale_rel, fn_name, orig_main_rel)
    return moves


def selector_targets(
    root: Path,
    rows: dict[FnKey, FnRow],
    mod_sides: dict[str, str],
    *,
    planned_writes: dict[Path, str] | None = None,
    planned_deletes: set[Path] | None = None,
    errors: list[str] | None = None,
) -> list[tuple[Path, str]]:
    """Return `[(path, expected_content)]` for every selector file that would change."""
    pw = {p.resolve(): txt for p, txt in (planned_writes or {}).items()}
    pd = {p.resolve() for p in (planned_deletes or set())}

    def v_exists(p: Path) -> bool:
        rp = p.resolve()
        if rp in pw:
            return True
        if rp in pd:
            return False
        return rp.is_file()

    def v_read(p: Path, *, replace_errors: bool = False) -> str:
        rp = p.resolve()
        if rp in pw:
            return pw[rp]
        return rp.read_text(encoding="utf-8", errors="replace" if replace_errors else "strict")

    test_map, fnr_map, mod_test_sides = build_selector_maps(rows, mod_sides, errors=errors)
    test_pat = re.compile(r"(?<![A-Za-z0-9_:])game::[A-Za-z0-9_:]+")
    fnr_pat = re.compile(r"sunrise_client::game::[A-Za-z0-9_:<>', ]+::[A-Za-z0-9_]+")
    updates: list[tuple[Path, str]] = []

    ignored_dir = root / "tools/ci/ignored-tests"
    if ignored_dir.is_dir():
        for p in sorted(ignored_dir.glob("*.txt")):
            old = v_read(p)
            out_lines = []
            for ln in old.splitlines(keepends=True):
                st = ln.lstrip()
                if not st or st.startswith("#"):
                    out_lines.append(ln)
                    continue
                m = re.match(r"^(\s*sunrise-client\s+)(game::\S+)(.*)$", ln, re.S)
                if m and m.group(2) in test_map:
                    out_lines.append(m.group(1) + test_map[m.group(2)] + m.group(3))
                else:
                    out_lines.append(ln)
            new = "".join(out_lines)
            if new != old:
                updates.append((p, new))
        readme = ignored_dir / "README.md"
        if v_exists(readme):
            old = v_read(readme)
            if "game::" in old:
                new = rewrite_exact_map(rewrite_exact_map(old, fnr_map, fnr_pat), test_map, test_pat)
                if new != old:
                    updates.append((readme, new))

    ci_dir = root / "tools/ci"
    if ci_dir.is_dir():
        for p in sorted(ci_dir.glob("daily-move-*.tsv")):
            old = v_read(p)
            out_lines = []
            for ln in old.splitlines(keepends=True):
                st = ln.lstrip()
                if not st or st.startswith("#"):
                    out_lines.append(ln)
                    continue
                eol = "\r\n" if ln.endswith("\r\n") else ("\n" if ln.endswith("\n") else "")
                body = ln[: -len(eol)] if eol else ln
                parts = body.split("\t")
                if len(parts) >= 3 and parts[0] == "sunrise-client" and parts[2] in test_map:
                    parts[2] = test_map[parts[2]]
                    out_lines.append("\t".join(parts) + eol)
                else:
                    out_lines.append(ln)
            new = "".join(out_lines)
            if new != old:
                updates.append((p, new))

    exc_path = root / "tools/ci/ignored-test-exceptions.json"
    if v_exists(exc_path):
        old = v_read(exc_path)
        data = json.loads(old)
        changed = False
        for entry in data:
            if entry.get("package") == "sunrise-client" and isinstance(entry.get("test"), str):
                t = entry["test"]
                if t in test_map and test_map[t] != t:
                    entry["test"] = test_map[t]
                    changed = True
        if changed:
            updates.append((exc_path, json.dumps(data, indent=2, ensure_ascii=False) + "\n"))

    tools_dir = root / "tools"
    if tools_dir.is_dir():
        for p in sorted(tools_dir.glob("test_*_parity.py")):
            old = v_read(p)
            new = rewrite_exact_map(old, test_map, test_pat)
            if new != old:
                updates.append((p, new))

    areas_path = root / "tools/test_areas.toml"
    if v_exists(areas_path):
        old = v_read(areas_path)
        new = rewrite_test_areas_for_mirrors(old, mod_test_sides, test_map)
        if new != old:
            updates.append((areas_path, new))

    fnr_dir = root / "tools/ci/fn-replace-check"
    if fnr_dir.is_dir():
        for p in sorted(fnr_dir.rglob("sunrise-mod.toml")):
            old = v_read(p)
            new = rewrite_exact_map(old, fnr_map, fnr_pat)
            if new != old:
                updates.append((p, new))

    draw_test = root / "crates/cargo-sunrise-mod/tests/installed_client_draw.rs"
    if v_exists(draw_test):
        old = v_read(draw_test)
        new = rewrite_exact_map(old, fnr_map, fnr_pat)
        if new != old:
            updates.append((draw_test, new))

    docs_dir = root / "docs"
    if docs_dir.is_dir():
        for p in sorted(docs_dir.rglob("*.md")):
            if p.relative_to(root).as_posix() == "docs/modding/src/how-to/native-mods/16-repinning-side-module-targets.md":
                continue
            old = v_read(p, replace_errors=True)
            if "game::" not in old:
                continue
            new = rewrite_exact_map(rewrite_exact_map(old, fnr_map, fnr_pat), test_map, test_pat)
            if new != old:
                updates.append((p, new))

    port_map_dir = root / "tools/port_map"
    if port_map_dir.is_dir():
        known_stale: set[tuple[str, str, str]] = set()
        stale_file = root / "tools/ci/port_map_known_stale.txt"
        if v_exists(stale_file):
            for ln in v_read(stale_file).splitlines():
                if ln.strip() and not ln.lstrip().startswith("#"):
                    parts = ln.split("\t")
                    if len(parts) >= 3:
                        known_stale.add((parts[0], parts[1], parts[2]))
        port_moves = build_port_map_file_moves(root, rows, mod_sides, errors=errors)
        if port_moves or pd:
            for p in sorted(port_map_dir.glob("*.tsv")):
                rel_p = p.relative_to(root).as_posix()
                old = v_read(p)
                out_lines = []
                for ln in old.splitlines(keepends=True):
                    st = ln.lstrip()
                    if not st or st.startswith("#"):
                        out_lines.append(ln)
                        continue
                    eol = "\r\n" if ln.endswith("\r\n") else ("\n" if ln.endswith("\n") else "")
                    body = ln[: -len(eol)] if eol else ln
                    parts = body.split("\t")
                    if len(parts) >= 3 and parts[1].strip() in ("port", "inline"):
                        target = parts[2].strip()
                        if (rel_p, parts[0].strip(), target) not in known_stale:
                            file_, sep, rest = target.partition("::")
                            if sep:
                                f_clean = file_.strip()
                                fn_name = rest.rpartition("::")[2].strip()
                                new_file = port_moves.get((f_clean, fn_name))
                                if not new_file and not v_exists(root / f_clean):
                                    base_f = re.sub(r"_(?:client|server)(?=\.rs$|/)", "", f_clean, count=1)
                                    if base_f != f_clean and v_exists(root / base_f):
                                        new_file = base_f
                                if new_file and new_file != f_clean:
                                    parts[2] = parts[2].replace(f"{file_}::", f"{new_file}::", 1)
                                    out_lines.append("\t".join(parts) + eol)
                                    continue
                    out_lines.append(ln)
                new = "".join(out_lines)
                if new != old:
                    updates.append((p, new))

    game_dir = root / "crates/sunrise-client/src/game"
    if game_dir.is_dir() and test_map:
        lit_pat = re.compile(r'"(game::[A-Za-z0-9_:]+)"')
        rs_candidates = {p.resolve() for p in game_dir.rglob("*.rs")}
        for p in pw:
            if p.suffix == ".rs" and p.is_relative_to(game_dir.resolve()):
                rs_candidates.add(p)
        rs_candidates -= pd
        for p in sorted(rs_candidates):
            old = v_read(p)
            if '"game::' not in old:
                continue
            new = lit_pat.sub(lambda m: f'"{test_map[m.group(1)]}"' if m.group(1) in test_map else m.group(0), old)
            if new != old:
                updates.append((p, new))

    return updates


def run_rustfmt(paths: list[Path]) -> None:
    rustfmt = os.environ.get("RUSTFMT") or shutil.which("rustfmt")
    if not rustfmt:
        return
    rs_paths = sorted({p for p in paths if p.suffix == ".rs" and p.is_file()})
    for start in range(0, len(rs_paths), 100):
        subprocess.run(
            [rustfmt, "--edition", "2024", "--config", "skip_children=true", *(str(p) for p in rs_paths[start : start + 100])],
            check=True,
        )


def _collect_macros_for_placement(
    orig_tree: ModNode,
    mirror_trees: dict[str, ModNode | None],
    placement: dict[FnKey, str],
    fn_source_text: dict[FnKey, str],
    *,
    errors: list[str] | None = None,
    root: Path | None = None,
) -> tuple[
    dict[tuple[str, tuple[str, ...], str], list[tuple[ParsedItem, str, Path]]],
    dict[tuple[str, tuple[str, ...]], list[tuple[str, tuple[str, ...]]]],
    dict[tuple[str, tuple[str, ...]], list[str]],
]:
    """Partition `macro_rules!` across `orig_tree` and `mirror_trees` for a given `placement`."""
    moved_macros: dict[tuple[str, tuple[str, ...], str], list[tuple[ParsedItem, str, Path]]] = defaultdict(list)
    staying_macros: dict[tuple[str, tuple[str, ...]], list[tuple[str, tuple[str, ...]]]] = defaultdict(list)
    restored_macros: dict[tuple[str, tuple[str, ...]], list[str]] = defaultdict(list)

    def collect_orig(node: ModNode) -> None:
        nid = (node.orig_mod, node_cfg_chain(node))
        if node.orig_mod != "game":
            for fp, fsrc, fitems, fcfgs in node_all_fragments(node):
                for it in fitems:
                    if it.kind == "macro_rules":
                        excl = macro_side_exclusive(node, it.name, placement)
                        eff_cfgs = effective_attrs(it.cfg_attrs, fcfgs)
                        if excl in ("client", "server"):
                            extra_prefix = "".join(f"{c}\n" for c in fcfgs if c not in it.norm_attrs)
                            moved_macros[(nid[0], nid[1], excl)].append((it, extra_prefix + fsrc[it.start : it.end], fp))
                        else:
                            staying_macros[nid].append((it.name, eff_cfgs))
        for ch in node.children:
            collect_orig(ch)

    collect_orig(orig_tree)

    def collect_mirror(mnode: ModNode, mside: str) -> None:
        nid = (mnode.orig_mod, node_cfg_chain(mnode))
        for fp, fsrc, fitems, fcfgs in node_all_fragments(mnode):
            for it in fitems:
                if it.kind == "macro_rules":
                    pat = re.compile(rf"\b{re.escape(it.name)}\s*!")
                    used_sides: set[str] = set()
                    for kk, src_txt in fn_source_text.items():
                        if kk.module == mnode.orig_mod or kk.module.startswith(mnode.orig_mod + "::"):
                            if pat.search(src_txt):
                                used_sides.add(placement.get(kk, "game"))
                    eff_cfgs = effective_attrs(it.cfg_attrs, fcfgs)
                    if used_sides == {"client"} or used_sides == {"server"}:
                        target_s = next(iter(used_sides))
                        extra_prefix = "".join(f"{c}\n" for c in fcfgs if c not in it.norm_attrs)
                        moved_macros[(nid[0], nid[1], target_s)].append((it, extra_prefix + fsrc[it.start : it.end], fp))
                    elif not used_sides:
                        if errors is not None:
                            errors.append(
                                f"{_rel_str(fp, root)}:{it.start_line}: unused/hand-written `macro_rules! {it.name}` in mirror file"
                            )
                    else:
                        restored_macros[nid].append(fsrc[it.start : it.end])
                        staying_macros[nid].append((it.name, eff_cfgs))
        for ch in mnode.children:
            collect_mirror(ch, mside)

    for mside in ("client", "server"):
        if mirror_trees[mside] is not None:
            collect_mirror(mirror_trees[mside], mside)  # type: ignore[arg-type]

    return moved_macros, staying_macros, restored_macros


def _render_mirror_tree(
    orig_tree: ModNode,
    side: str,
    top_name: str,
    parent_side: str,
    regular_mods: dict[str, tuple[str, Path, tuple[str, ...]]],
    placement: dict[FnKey, str],
    orig_fns: dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]],
    mirror_fns: dict[str, dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]]],
    fn_source_text: dict[FnKey, str],
    fn_node_id: dict[FnKey, tuple[str, tuple[str, ...]]],
    trait_impl_source: dict[tuple[str, tuple[str, ...], str, tuple[str, ...]], tuple[str, str]],
    impl_header_prefix: dict[tuple[str, tuple[str, ...], str, tuple[str, ...]], str],
    moved_macros: dict[tuple[str, tuple[str, ...], str], list[tuple[ParsedItem, str, Path]]],
    staying_macros: dict[tuple[str, tuple[str, ...]], list[tuple[str, tuple[str, ...]]]],
    node_moved_fns: dict[tuple[str, tuple[str, ...], str], list[FnKey]],
) -> dict[Path, str]:
    """Render all mirror files for `(orig_tree, side)` in memory without writing to disk."""
    rendered: dict[Path, str] = {}

    def canon_orig_mod(orig_mod_rel: str) -> str:
        if top_name == "game":
            return f"crate::{orig_game_mod_path(orig_mod_rel)}"
        if parent_side in ("client", "server"):
            return f"crate::game::{SIDE_MOD_NAME[parent_side]}::{orig_mod_rel}"
        return f"crate::game::{orig_mod_rel}"

    def node_has_side(node: ModNode) -> bool:
        nid = (node.orig_mod, node_cfg_chain(node))
        if node_moved_fns.get((nid[0], nid[1], side)):
            return True
        return any(node_has_side(ch) for ch in node.children)

    if not node_has_side(orig_tree):
        return rendered

    side_mod = SIDE_MOD_NAME[side]

    def build_mirror(node: ModNode, anc_macros: list[tuple[str, str, tuple[str, ...]]]) -> str:
        nid = (node.orig_mod, node_cfg_chain(node))
        ncfgs = nid[1]
        cmod = canon_orig_mod(node.orig_mod)
        is_game_root = node.orig_mod == "game"
        lines: list[str] = []
        local_child_mods = {
            it.name
            for _fp, _fs, fitems, _fc in node_all_fragments(node)
            for it in fitems
            if it.kind in ("mod_ext", "mod_inline", "mod_include")
        }
        if is_game_root:
            local_child_mods |= set(regular_mods) | SIDE_CONTAINERS
        active_children = [ch for ch in node.children if node_has_side(ch)]

        local_macro_names = {
            it.name
            for _fp, _fs, fitems, _fc in node_all_fragments(node)
            for it in fitems
            if it.kind == "macro_rules"
        }
        seen_uses: set[str] = set()
        for _fp, fsrc, fitems, fcfgs in node_all_fragments(node):
            frag_prefix = "".join(f"{c}\n" for c in fcfgs)
            for it in fitems:
                if it.kind == "use":
                    if is_game_root and it.vis != "":
                        continue
                    norm_u = re.sub(r"\s+", "", it.name)
                    if f"crate::game::{SIDE_MOD_NAME['client']}::{top_name}_client" in norm_u or f"crate::game::{SIDE_MOD_NAME['server']}::{top_name}_server" in norm_u:
                        continue
                    if norm_u in local_macro_names or (norm_u.startswith("self::") and norm_u[6:] in local_macro_names):
                        continue
                    rw_body = rewrite_use_stmt_for_mirror(
                        fsrc[it.start : it.end], cmod, local_child_mods, side_mod, is_game_root=is_game_root
                    ).rstrip() + "\n"
                    extra_u_cfgs = "".join(f"{c}\n" for c in fcfgs if c not in it.cfg_attrs)
                    rw = extra_u_cfgs + rw_body if extra_u_cfgs else rw_body
                    if rw not in seen_uses:
                        seen_uses.add(rw)
                        lines.append(rw)
            _ = frag_prefix

        for mmod_canon, mname, mcfgs in anc_macros:
            cfgs = "".join(f"{c}\n" for c in mcfgs)
            lines.append(f"{cfgs}#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {mmod_canon}::{mname};\n")

        lines.append(f"#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {cmod}::*;\n")

        moved_macro_names = {
            it.name
            for s in ("client", "server")
            for it, _txt, _fp in moved_macros.get((nid[0], nid[1], s), [])
        }
        moved_here_keys = set(node_moved_fns.get((nid[0], nid[1], side), []))
        emitted_expl: set[tuple[str, tuple[str, ...]]] = set()
        if not is_game_root:
            for _fp, _fs, fitems, fcfgs in node_all_fragments(node):
                eff_ncfgs = (*ncfgs, *fcfgs)
                for it in fitems:
                    eff_item_cfgs = effective_attrs(it.cfg_attrs, fcfgs)
                    ekey = (it.name, eff_item_cfgs)
                    if it.kind in ("struct", "enum", "type", "const", "static", "trait"):
                        if ekey not in emitted_expl:
                            emitted_expl.add(ekey)
                            cfgs = "".join(f"{c}\n" for c in eff_item_cfgs)
                            lines.append(f"{cfgs}#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {cmod}::{it.name};\n")
                    elif it.kind == "macro_rules" and it.name not in moved_macro_names:
                        if ekey not in emitted_expl:
                            emitted_expl.add(ekey)
                            cfgs = "".join(f"{c}\n" for c in eff_item_cfgs)
                            lines.append(f"{cfgs}#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {cmod}::{it.name};\n")
                    elif it.kind == "macro_invoc" and it.name == "thread_local":
                        for sname, scfgs in thread_local_statics(_fs, it):
                            eff_scfgs = effective_attrs(scfgs, fcfgs)
                            skey = (sname, eff_scfgs)
                            if skey not in emitted_expl:
                                emitted_expl.add(skey)
                                cfgs = "".join(f"{c}\n" for c in eff_scfgs)
                                lines.append(f"{cfgs}#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {cmod}::{sname};\n")
                    elif it.kind == "fn" and not it.is_test_attr:
                        k = item_fn_key(node.orig_mod, it, None, eff_ncfgs)
                        norm_it_vis = re.sub(r"\s+", " ", it.vis).replace("( ", "(").replace(" )", ")")
                        parent_side_narrow = (
                            parent_side in SIDE_MOD_NAME
                            and parent_side != side
                            and norm_it_vis == f"pub(in crate::game::{SIDE_MOD_NAME[parent_side]})"
                        )
                        if not parent_side_narrow and k not in moved_here_keys and placement.get(k, "game") == "game" and ekey not in emitted_expl:
                            emitted_expl.add(ekey)
                            cfgs = "".join(f"{c}\n" for c in eff_item_cfgs)
                            lines.append(f"{cfgs}#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {cmod}::{it.name};\n")

        child_macros = list(anc_macros)
        for mname, mcfgs in staying_macros.get(nid, []):
            child_macros.append((cmod, mname, mcfgs))

        for _mit, mtxt, _mfp in moved_macros.get((nid[0], nid[1], side), []):
            lines.append("\n" + mtxt)

        def emit_child_mod(ch: ModNode) -> None:
            attr_prefix = "".join(f"{c}\n" for c in ch.cfg_attrs) + "".join(f"{a}\n" for a in ch.mod_attrs)
            if ch.decl_kind == "out_of_line":
                ch_file = mirror_file_path(ch.file_path, side)
                ch_body = build_mirror(ch, child_macros)
                rendered[ch_file.resolve()] = ch_body
                rel_p = os.path.relpath(ch_file, mirror_file_path(node.file_path, side).parent).replace(os.sep, "/")
                lines.append(f'\n{attr_prefix}#[path = "{rel_p}"]\npub(in crate::game) mod {ch.name};\n')
            elif ch.decl_kind == "inline_include":
                ch_file = mirror_file_path(ch.file_path, side)
                ch_body = build_mirror(ch, child_macros)
                rendered[ch_file.resolve()] = ch_body
                rel_p = os.path.relpath(ch_file, mirror_file_path(node.file_path, side).parent).replace(os.sep, "/")
                lines.append(f'\n{attr_prefix}pub(in crate::game) mod {ch.name} {{\n    include!("{rel_p}");\n}}\n')
            elif ch.decl_kind == "inline":
                ch_body = build_mirror(ch, child_macros)
                lines.append(f"\n{attr_prefix}pub(in crate::game) mod {ch.name} {{\n{ch_body}}}\n")

        for ch in active_children:
            if not is_cfg_test_node(ch):
                emit_child_mod(ch)

        keys_here = sorted(moved_here_keys, key=lambda kk: (kk.impl, kk.name, kk.cfg))
        free_keys = [kk for kk in keys_here if kk.impl == "-"]
        methods_by_impl: dict[tuple[str, tuple[str, ...]], list[FnKey]] = defaultdict(list)
        emitted_trait_impls: set[tuple[str, tuple[str, ...], str, tuple[str, ...]]] = set()

        for kk in keys_here:
            if kk.impl == "-":
                continue
            src_entry = orig_fns.get(kk) or mirror_fns["client"].get(kk) or mirror_fns["server"].get(kk)
            assert src_entry is not None
            pimpl = src_entry[4]
            fcfgs = src_entry[5]
            assert pimpl is not None
            eff_iattrs = effective_attrs(pimpl.norm_attrs, fcfgs)
            if pimpl.is_trait_impl:
                tkey = (nid[0], nid[1], pimpl.impl_header, eff_iattrs)
                if tkey not in emitted_trait_impls:
                    emitted_trait_impls.add(tkey)
                    lines.append("\n" + trait_impl_source[tkey][0])
            else:
                methods_by_impl[(pimpl.impl_header, eff_iattrs)].append(kk)

        for kk in free_keys:
            src_entry = orig_fns.get(kk) or mirror_fns["client"].get(kk) or mirror_fns["server"].get(kk)
            assert src_entry is not None
            it = src_entry[3]
            lines.append(
                "\n"
                + rewrite_moved_fn_vis(
                    fn_source_text[kk],
                    is_trait_impl=False,
                    is_test_fn=it.is_test_attr,
                    is_game_root=is_game_root,
                    side=side,
                )
            )

        for (ihdr, iattrs), mkeys in sorted(methods_by_impl.items()):
            ikey = (nid[0], nid[1], ihdr, iattrs)
            prefix = impl_header_prefix[ikey]
            lines.append("\n" + prefix + "\n")
            for kk in mkeys:
                src_entry = orig_fns.get(kk) or mirror_fns["client"].get(kk) or mirror_fns["server"].get(kk)
                assert src_entry is not None
                it = src_entry[3]
                lines.append(
                    rewrite_moved_fn_vis(
                        fn_source_text[kk],
                        is_trait_impl=False,
                        is_test_fn=it.is_test_attr,
                        is_game_root=is_game_root,
                        side=side,
                    )
                )
            lines.append("}\n")

        for ch in active_children:
            if is_cfg_test_node(ch):
                emit_child_mod(ch)

        return "".join(lines)

    root_mirror_file = mirror_file_path(orig_tree.file_path, side)
    rendered[root_mirror_file.resolve()] = build_mirror(orig_tree, [])
    return rendered


def _capture_fn_sources(
    orig_fns: dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]],
    mirror_fns: dict[str, dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]]],
    placement: dict[FnKey, str],
    errors: list[str],
    *,
    check_trait_consistency: bool = True,
) -> tuple[
    dict[FnKey, str],
    dict[FnKey, tuple[str, tuple[str, ...]]],
    dict[tuple[str, tuple[str, ...], str, tuple[str, ...]], tuple[str, str]],
    dict[tuple[str, tuple[str, ...], str, tuple[str, ...]], str],
]:
    fn_source_text: dict[FnKey, str] = {}
    fn_node_id: dict[FnKey, tuple[str, tuple[str, ...]]] = {}
    trait_impl_source: dict[tuple[str, tuple[str, ...], str, tuple[str, ...]], tuple[str, str]] = {}
    impl_header_prefix: dict[tuple[str, tuple[str, ...], str, tuple[str, ...]], str] = {}

    for _loc, fdict in [("game", orig_fns), ("client", mirror_fns["client"]), ("server", mirror_fns["server"])]:
        for k, (n, _fp, fsrc, it, pimpl, fcfgs) in fdict.items():
            nid = (n.orig_mod, node_cfg_chain(n))
            raw_fn = fsrc[it.start : it.end]
            if pimpl is None and fcfgs:
                extra_fn_cfgs = "".join(f"{c}\n" for c in fcfgs if c not in it.norm_attrs)
                raw_fn = extra_fn_cfgs + raw_fn
            fn_source_text[k] = raw_fn
            fn_node_id[k] = nid
            if pimpl is not None:
                eff_iattrs = effective_attrs(pimpl.norm_attrs, fcfgs)
                ikey = (nid[0], nid[1], pimpl.impl_header, eff_iattrs)
                extra_impl_cfgs = "".join(f"{c}\n" for c in fcfgs if c not in pimpl.norm_attrs)
                impl_header_prefix.setdefault(ikey, extra_impl_cfgs + fsrc[pimpl.start : pimpl.impl_open_brace + 1])
                if pimpl.is_trait_impl:
                    want = placement.get(k, "game")
                    prev = trait_impl_source.get(ikey)
                    if check_trait_consistency and prev is not None and prev[1] != want:
                        errors.append(
                            f"trait impl `{pimpl.impl_header}` in `{n.orig_mod}` has mixed sides ({prev[1]} vs {want})"
                        )
                    trait_impl_source[ikey] = (extra_impl_cfgs + fsrc[pimpl.start : pimpl.end], want)

    return fn_source_text, fn_node_id, trait_impl_source, impl_header_prefix


def run_fix(root: Path, rows: dict[FnKey, FnRow], mod_sides: dict[str, str], *, fmt: bool) -> int:
    game_dir = root / "crates/sunrise-client/src/game"
    regular_names = set(mod_sides)
    errors: list[str] = []
    all_regular_entries: list[Path] = []
    regular_mods, existing_mirrors = parse_top_mod_decls(
        root, regular_names, errors=errors, all_regular_entries=all_regular_entries
    )
    check_orphan_mirror_files(root, regular_mods, all_regular_entries, existing_mirrors, errors)

    game_rs_path = root / "crates/sunrise-client/src/game.rs"
    game_root_tree = build_mod_tree("game", game_rs_path.resolve()) if game_rs_path.is_file() else None

    rows_by_top: dict[str, dict[FnKey, FnRow]] = defaultdict(dict)
    for k, r in rows.items():
        rows_by_top[r.top_module][k] = r

    touched_tops = sorted(
        set(rows_by_top)
        | {m[: -len(f"_{s}")] for s in ("client", "server") for m in existing_mirrors[s]}
    )

    planned_writes: dict[Path, str] = {}
    planned_deletes: set[Path] = set()
    expected_side_decls: dict[str, dict[str, tuple[str, tuple[str, ...]]]] = {"client": {}, "server": {}}
    moved_counts = {"client": 0, "server": 0}

    for top_name in touched_tops:
        resolved = resolve_top_entry(root, top_name, regular_mods)
        if resolved is None:
            errors.append(f"top-level module `{top_name}` in game_fn_sides.tsv is not declared in game")
            continue
        parent_side, entry_file, top_cfg = resolved
        top_errors_before = len(errors)
        orig_tree = build_mod_tree(top_name, entry_file, top_cfg, errors=errors, root=root)
        orig_fns = collect_tree_functions(orig_tree, dup_errors=errors, root=root)

        mirror_trees: dict[str, ModNode | None] = {"client": None, "server": None}
        mirror_fns: dict[str, dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]]] = {
            "client": {},
            "server": {},
        }
        old_mirror_files: dict[str, set[Path]] = {"client": set(), "server": set()}
        for side in ("client", "server"):
            mname = f"{top_name}_{side}"
            if mname in existing_mirrors[side]:
                mentry, mcfg = existing_mirrors[side][mname]
                if mentry.is_file():
                    mtree = build_mod_tree(top_name, mentry, mcfg, errors=errors, root=root, is_mirror=True)
                    mirror_trees[side] = mtree
                    mirror_fns[side] = collect_tree_functions(mtree, dup_errors=errors, root=root)
                    old_mirror_files[side] = collect_mirror_files(mentry)

        top_rows = rows_by_top.get(top_name, {})
        desired_side: dict[FnKey, str] = {k: r.side for k, r in top_rows.items()}

        validate_top_overlaps_and_cfg_attr(top_rows, orig_fns, mirror_fns, errors, root=root)

        all_known_keys = set(orig_fns) | set(mirror_fns["client"]) | set(mirror_fns["server"])
        for k, _r in top_rows.items():
            if k not in all_known_keys:
                errors.append(f"table row `{k.module}::{k.impl}::{k.name}` ({k.cfg}) not found in `{top_name}`")
        for side in ("client", "server"):
            if side == parent_side:
                continue
            for k in sorted(mirror_fns[side], key=lambda x: (x.module, x.impl, x.name, x.cfg)):
                if k not in top_rows:
                    errors.append(f"unlisted function `{k.module}::{k.name}` in `{top_name}_{side}`")

        # Validate existing mirror files contain only tool-generated content before modifying anything
        if len(errors) == top_errors_before:
            current_placement: dict[FnKey, str] = {}
            for k in orig_fns:
                current_placement[k] = "game"
            for side in ("client", "server"):
                for k in mirror_fns[side]:
                    current_placement[k] = side
            cur_fn_src, cur_fn_nid, cur_trait_src, cur_impl_pref = _capture_fn_sources(
                orig_fns, mirror_fns, current_placement, errors, check_trait_consistency=False
            )
            cur_moved_macros, cur_staying_macros, _cur_restored = _collect_macros_for_placement(
                orig_tree, mirror_trees, current_placement, cur_fn_src, errors=errors, root=root
            )
            cur_node_moved: dict[tuple[str, tuple[str, ...], str], list[FnKey]] = defaultdict(list)
            for side in ("client", "server"):
                for k in mirror_fns[side]:
                    nid = cur_fn_nid[k]
                    cur_node_moved[(nid[0], nid[1], side)].append(k)
            for side in ("client", "server"):
                if mirror_trees[side] is None:
                    continue
                expected_cur = _render_mirror_tree(
                    orig_tree,
                    side,
                    top_name,
                    parent_side,
                    regular_mods,
                    current_placement,
                    orig_fns,
                    mirror_fns,
                    cur_fn_src,
                    cur_fn_nid,
                    cur_trait_src,
                    cur_impl_pref,
                    cur_moved_macros,
                    cur_staying_macros,
                    cur_node_moved,
                )
                for mf in sorted(old_mirror_files[side]):
                    if mf not in expected_cur:
                        errors.append(f"{_rel_str(mf, root)}: unexpected mirror file not generated by tool")
                    else:
                        errors.extend(
                            diff_mirror_file(
                                _rel_str(mf, root),
                                mf.read_text(encoding="utf-8"),
                                expected_cur[mf],
                                allow_reorder_methods=True,
                            )
                        )

        if len(errors) > top_errors_before:
            continue

        fn_source_text, fn_node_id, trait_impl_source, impl_header_prefix = _capture_fn_sources(
            orig_fns, mirror_fns, desired_side, errors, check_trait_consistency=True
        )
        if len(errors) > top_errors_before:
            continue

        orig_node_by_nid: dict[tuple[str, tuple[str, ...]], ModNode] = {}

        def index_orig_nodes(n: ModNode) -> None:
            orig_node_by_nid[(n.orig_mod, node_cfg_chain(n))] = n
            for ch in n.children:
                index_orig_nodes(ch)

        index_orig_nodes(orig_tree)

        macro_trees = [orig_tree, *(t for t in mirror_trees.values() if t is not None)]
        if game_root_tree is not None and top_name != "game":
            macro_trees.append(game_root_tree)
        sensitive_macros = collect_location_sensitive_macros(macro_trees)

        node_moved_fns: dict[tuple[str, tuple[str, ...], str], list[FnKey]] = defaultdict(list)
        for k, r in sorted(top_rows.items(), key=lambda kv: (kv[0].module, kv[0].impl, kv[0].name, kv[0].cfg)):
            if r.side in ("client", "server"):
                nid = fn_node_id[k]
                node_moved_fns[(nid[0], nid[1], r.side)].append(k)
                moved_counts[r.side] += 1
                src_entry = orig_fns.get(k) or mirror_fns["client"].get(k) or mirror_fns["server"].get(k)
                assert src_entry is not None
                src_fp, src_it, src_pimpl = src_entry[1], src_entry[3], src_entry[4]
                check_txt = fn_source_text[k]
                loc_hits = find_location_macro_uses(check_txt, sensitive_macros)
                if loc_hits:
                    errors.append(
                        f"{_rel_str(src_fp, root)}:{src_it.start_line}: moved function `{k.module}::{k.name}` uses location-sensitive macro(s) {', '.join(loc_hits)}"
                    )
                orig_node = orig_node_by_nid.get(nid)
                if orig_node is not None:
                    dest_file = mirror_file_path(orig_node.file_path, r.side).resolve()
                    if src_fp.resolve().parent != dest_file.parent:
                        inc_hits = find_relative_includes_in_src(check_txt)
                        if inc_hits:
                            errors.append(
                                f"{_rel_str(src_fp, root)}:{src_it.start_line}: moved function `{k.module}::{k.name}` uses {', '.join(inc_hits)} and moving to `{_rel_str(dest_file, root)}` changes directory"
                            )
                _ = src_pimpl

        moved_macros, staying_macros, restored_macros = _collect_macros_for_placement(
            orig_tree, mirror_trees, desired_side, fn_source_text, errors=errors, root=root
        )
        for (m_mod, m_cfgs, m_side), mlist in moved_macros.items():
            orig_node = orig_node_by_nid.get((m_mod, m_cfgs))
            if orig_node is None:
                continue
            dest_file = mirror_file_path(orig_node.file_path, m_side).resolve()
            for mit, mtxt, mfp in mlist:
                if mfp.resolve().parent != dest_file.parent:
                    inc_hits = find_relative_includes_in_src(mtxt)
                    if inc_hits:
                        errors.append(
                            f"{_rel_str(mfp, root)}:{mit.start_line}: moved `macro_rules! {mit.name}` uses {', '.join(inc_hits)} and moving to `{_rel_str(dest_file, root)}` changes directory"
                        )

        if len(errors) > top_errors_before:
            continue

        def node_has_side(node: ModNode, side: str) -> bool:
            nid = (node.orig_mod, node_cfg_chain(node))
            if node_moved_fns.get((nid[0], nid[1], side)):
                return True
            return any(node_has_side(ch, side) for ch in node.children)

        # 1. Stage mirror files for client and server
        for side in ("client", "server"):
            rendered_files = _render_mirror_tree(
                orig_tree,
                side,
                top_name,
                parent_side,
                regular_mods,
                desired_side,
                orig_fns,
                mirror_fns,
                fn_source_text,
                fn_node_id,
                trait_impl_source,
                impl_header_prefix,
                moved_macros,
                staying_macros,
                node_moved_fns,
            )
            new_files = set(rendered_files)
            for mf_path, mf_body in rendered_files.items():
                planned_writes[mf_path] = mf_body
            if rendered_files:
                root_mirror_file = mirror_file_path(orig_tree.file_path, side)
                rel_from_game = root_mirror_file.relative_to(game_dir).as_posix()
                expected_side_decls[side][f"{top_name}_{side}"] = (rel_from_game, top_cfg)
            for stale in sorted(old_mirror_files[side] - new_files):
                planned_deletes.add(stale.resolve())

        # 2. Stage original tree edits
        edits_by_file: dict[Path, list[tuple[int, int, str]]] = defaultdict(list)

        def edit_orig_node(node: ModNode, parent_split: bool) -> None:
            nid = (node.orig_mod, node_cfg_chain(node))
            ncfgs = nid[1]
            is_game_root = node.orig_mod == "game"
            node_split = node_has_side(node, "client") or node_has_side(node, "server")
            widen_super = (not is_game_root) and (node_split or parent_split)

            restore_free: list[str] = []
            restore_methods: dict[tuple[str, tuple[str, ...]], list[str]] = defaultdict(list)
            restore_trait_impls: list[str] = []
            restored_tkeys: set[tuple[str, tuple[str, ...], str, tuple[str, ...]]] = set()

            for side in ("client", "server"):
                for kk, (mn, _fp, _fsrc, it, pimpl, fcfgs) in mirror_fns[side].items():
                    if (mn.orig_mod, node_cfg_chain(mn)) != nid:
                        continue
                    if desired_side.get(kk, "game") == "game" and kk not in orig_fns:
                        if pimpl is None:
                            restore_free.append(
                                rewrite_moved_fn_vis(
                                    fn_source_text[kk],
                                    is_trait_impl=False,
                                    is_test_fn=it.is_test_attr,
                                    is_game_root=is_game_root,
                                    side=None,
                                )
                            )
                        elif pimpl.is_trait_impl:
                            eff_iattrs = effective_attrs(pimpl.norm_attrs, fcfgs)
                            tkey = (nid[0], nid[1], pimpl.impl_header, eff_iattrs)
                            if tkey not in restored_tkeys:
                                restored_tkeys.add(tkey)
                                restore_trait_impls.append(trait_impl_source[tkey][0])
                        else:
                            eff_iattrs = effective_attrs(pimpl.norm_attrs, fcfgs)
                            restore_methods[(pimpl.impl_header, eff_iattrs)].append(
                                rewrite_moved_fn_vis(
                                    fn_source_text[kk],
                                    is_trait_impl=False,
                                    is_test_fn=it.is_test_attr,
                                    is_game_root=is_game_root,
                                    side=None,
                                )
                            )

            moved_macro_names_here = {
                it.name
                for s in ("client", "server")
                for it, _txt, _fp in moved_macros.get((nid[0], nid[1], s), [])
            }

            for fpath, fsrc, fitems, fcfgs in node_all_fragments(node):
                eff_ncfgs = (*ncfgs, *fcfgs)
                edits = edits_by_file[fpath]
                existing_macro_uses = {
                    m.group(1)
                    for u in fitems
                    if u.kind == "use"
                    for m in [re.fullmatch(r"(?:self::)?([A-Za-z_][A-Za-z0-9_]*)", u.name.strip())]
                    if m
                }
                for it in fitems:
                    if it.kind == "fn":
                        kk = item_fn_key(node.orig_mod, it, None, eff_ncfgs)
                        want = desired_side.get(kk, "game")
                        if want in ("client", "server"):
                            if it.is_test_attr:
                                edits.append((it.start, it.end, ""))
                            else:
                                dest = mirror_mod_rel(node.orig_mod, top_name, want)
                                cfgs = "".join(f"{c}\n" for c in it.cfg_attrs)
                                if is_game_root and it.vis in ("pub", "pub(crate)", "pub(super)"):
                                    re_vis = it.vis
                                elif it.vis in ("pub", "pub(crate)"):
                                    re_vis = it.vis
                                else:
                                    re_vis = "pub(in crate::game)"
                                edits.append((it.start, it.end, f"{cfgs}#[allow(unused_imports)]\n{re_vis} use crate::game::{dest}::{it.name};\n"))
                        elif (not is_game_root) and node_split and not it.is_test_attr:
                            rep = rewrite_vis_to_game(
                                it.vis,
                                preserve_side=parent_side if parent_side in SIDE_MOD_NAME else None,
                            )
                            if rep:
                                edits.append((it.vis_start, it.vis_end, f"{rep} " if it.vis == "" else rep))
                        elif widen_super and "super" in it.vis:
                            rep = rewrite_vis_to_game(it.vis)
                            if rep:
                                edits.append((it.vis_start, it.vis_end, rep))

                    elif it.kind == "impl":
                        if it.is_trait_impl:
                            sides = {desired_side.get(item_fn_key(node.orig_mod, ch, it, eff_ncfgs), "game") for ch in it.children if ch.kind == "method"}
                            if len(sides) == 1 and next(iter(sides)) in ("client", "server"):
                                edits.append((it.start, it.end, ""))
                        else:
                            moved_ch = [ch for ch in it.children if ch.kind == "method" and desired_side.get(item_fn_key(node.orig_mod, ch, it, eff_ncfgs), "game") in ("client", "server")]
                            stay_ch = [ch for ch in it.children if ch not in moved_ch]
                            if moved_ch and not stay_ch:
                                edits.append((it.start, it.end, ""))
                            else:
                                for ch in moved_ch:
                                    edits.append((ch.start, ch.end, ""))
                                if widen_super:
                                    for ch in stay_ch:
                                        if ch.kind == "method" and not ch.is_test_attr:
                                            if node_split:
                                                rep = rewrite_vis_to_game(
                                                    ch.vis,
                                                    preserve_side=parent_side if parent_side in SIDE_MOD_NAME else None,
                                                )
                                                if rep:
                                                    edits.append((ch.vis_start, ch.vis_end, f"{rep} " if ch.vis == "" else rep))
                                            elif "super" in ch.vis:
                                                rep = rewrite_vis_to_game(ch.vis)
                                                if rep:
                                                    edits.append((ch.vis_start, ch.vis_end, rep))
                                        elif ch.kind == "const" and node_split:
                                            rep = rewrite_vis_to_game(ch.vis)
                                            if rep:
                                                edits.append((ch.vis_start, ch.vis_end, f"{rep} " if ch.vis == "" else rep))

                    elif (not is_game_root) and it.kind in ("struct", "enum", "type", "const", "static", "trait", "mod_ext", "mod_inline", "mod_include"):
                        if node_split:
                            rep = rewrite_vis_to_game(it.vis)
                            if rep:
                                edits.append((it.vis_start, it.vis_end, f"{rep} " if it.vis == "" else rep))
                            if it.kind == "struct":
                                edits.extend(widen_struct_fields_edits(fsrc, it, only_super=False))
                        elif widen_super:
                            if "super" in it.vis:
                                rep = rewrite_vis_to_game(it.vis)
                                if rep:
                                    edits.append((it.vis_start, it.vis_end, rep))
                            if it.kind == "struct":
                                edits.extend(widen_struct_fields_edits(fsrc, it, only_super=True))

                    elif it.kind == "use":
                        norm_u = re.sub(r"\s+", "", it.name)
                        rm = re.search(rf"crate::game::(client_side|server_side)::{re.escape(top_name)}_(client|server)(?:::[A-Za-z0-9_:]+)?::([A-Za-z0-9_]+)$", norm_u)
                        if rm and f"{top_name}_{rm.group(2)}" not in regular_mods:
                            cur_side = rm.group(2)
                            fname = rm.group(3)
                            want_keys = [
                                kk
                                for kk in top_rows
                                if kk.module == node.orig_mod and kk.impl == "-" and kk.name == fname and fn_node_id.get(kk) == nid
                            ]
                            want = desired_side[want_keys[0]] if want_keys else "game"
                            if want == "game":
                                edits.append((it.start, it.end, ""))
                                continue
                            if want == cur_side and want_keys:
                                m_entry = mirror_fns[cur_side].get(want_keys[0])
                                if m_entry is not None:
                                    m_vis = re.sub(r"\s+", " ", m_entry[3].vis).replace("( ", "(").replace(" )", ")")
                                    u_vis = re.sub(r"\s+", " ", it.vis).replace("( ", "(").replace(" )", ")")
                                    if (
                                        m_vis == f"pub(in crate::game::{SIDE_MOD_NAME[cur_side]})"
                                        and u_vis == "pub(in crate::game)"
                                        and any("unused_imports" in a for a in it.attrs)
                                    ):
                                        edits.append((it.start, it.end, ""))
                                        continue
                            if want != cur_side:
                                dest = mirror_mod_rel(node.orig_mod, top_name, want)
                                cfgs = "".join(f"{c}\n" for c in it.cfg_attrs)
                                if is_game_root and it.vis in ("pub", "pub(crate)", "pub(super)"):
                                    re_vis = it.vis
                                elif it.vis in ("pub", "pub(crate)"):
                                    re_vis = it.vis
                                else:
                                    re_vis = "pub(in crate::game)"
                                edits.append((it.start, it.end, f"{cfgs}#[allow(unused_imports)]\n{re_vis} use crate::game::{dest}::{fname};\n"))
                                continue
                        if is_game_root:
                            if node_split and it.vis == "" and not any("unused_imports" in a for a in it.attrs):
                                ins_at = it.kw_start if not it.attrs else fsrc.find("#[", it.start)
                                if ins_at == -1 or ins_at > it.kw_start:
                                    ins_at = it.kw_start
                                edits.append((ins_at, ins_at, "#[allow(unused_imports)]\n"))
                        elif widen_super:
                            if node_split and not any("unused_imports" in a for a in it.attrs):
                                ins_at = it.kw_start if not it.attrs else fsrc.find("#[", it.start)
                                if ins_at == -1 or ins_at > it.kw_start:
                                    ins_at = it.kw_start
                                edits.append((ins_at, ins_at, "#[allow(unused_imports)]\n"))
                            if "super" in it.vis:
                                rep = rewrite_vis_to_game(it.vis)
                                if rep:
                                    edits.append((it.vis_start, it.vis_end, rep))

                    elif (not is_game_root) and it.kind == "macro_invoc" and it.name == "thread_local":
                        if node_split:
                            edits.extend(widen_thread_local_edits(fsrc, it, only_super=False))
                        elif widen_super:
                            edits.extend(widen_thread_local_edits(fsrc, it, only_super=True))

                    elif (not is_game_root) and it.kind == "macro_rules" and node_split:
                        if it.name in moved_macro_names_here:
                            edits.append((it.start, it.end, ""))
                        elif it.name not in existing_macro_uses:
                            cfgs = "".join(f"{c}\n" for c in it.cfg_attrs)
                            edits.append((it.end, it.end, f"{cfgs}#[allow(unused_imports)]\npub(in crate::game) use {it.name};\n"))

            rest_macros_here = restored_macros.get(nid, [])
            if rest_macros_here or restore_free or restore_methods or restore_trait_impls:
                chunks: list[str] = []
                for mtxt in rest_macros_here:
                    chunks.append("\n" + mtxt)
                for txt in restore_free:
                    chunks.append("\n" + txt)
                for (ihdr, iattrs), mlist in sorted(restore_methods.items()):
                    prefix = impl_header_prefix[(nid[0], nid[1], ihdr, iattrs)]
                    chunks.append("\n" + prefix + "\n" + "".join(mlist) + "}\n")
                for txt in restore_trait_impls:
                    chunks.append("\n" + txt)
                ins_pos = node.inline_item.impl_close_brace if node.inline_item is not None else len(node.src)
                for it in node.items:
                    if it.kind in ("mod_inline", "mod_ext", "mod_include") and any(
                        "test" in c for c in it.cfg_attrs
                    ):
                        ins_pos = it.start
                        break
                edits_by_file[node.file_path].append((ins_pos, ins_pos, "".join(chunks) + "\n"))

            for ch in node.children:
                edit_orig_node(ch, False if is_game_root else node_split)

        edit_orig_node(orig_tree, False)

        for path, edits in sorted(edits_by_file.items()):
            if not edits:
                continue
            rp = path.resolve()
            old = planned_writes.get(rp, rp.read_text(encoding="utf-8"))
            new = apply_edits(old, edits)
            if new != old or rp in planned_writes:
                planned_writes[rp] = new

    # 3. Plan mirror module declarations in client_side.rs and server_side.rs
    for side in ("client", "server"):
        planned = plan_side_file_mirror_decls(root, side, expected_side_decls[side], regular_names, errors=errors)
        if planned is not None:
            planned_writes[planned[0]] = planned[1]

    # 4. Plan test/parity/fn-replace/doc/Rust selector updates against the virtual post-edit tree
    sel_updates = selector_targets(
        root,
        rows,
        mod_sides,
        planned_writes=planned_writes,
        planned_deletes=planned_deletes,
        errors=errors,
    )
    for path, new_txt in sel_updates:
        planned_writes[path.resolve()] = new_txt

    if errors:
        for e in errors:
            print(f"error: {e}", file=sys.stderr)
        print(f"Refused --fix: {len(errors)} error(s); no files modified.", file=sys.stderr)
        return 1

    # Commit all deletes and writes only after every check passed
    changed_rs: list[Path] = []
    for stale in sorted(planned_deletes - set(planned_writes)):
        if stale.is_file():
            stale.unlink()

    for path, content in sorted(planned_writes.items()):
        if not path.is_file() or path.read_text(encoding="utf-8") != content:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
            if path.suffix == ".rs":
                changed_rs.append(path)

    if fmt and changed_rs:
        run_rustfmt(changed_rs)

    print(
        f"Applied fn sides: {moved_counts['client']} client fns, {moved_counts['server']} server fns, "
        f"{len(expected_side_decls['client'])} client mirror mods, {len(expected_side_decls['server'])} server mirror mods, "
        f"{len(sel_updates)} selector files updated."
    )
    return 0


def write_if_changed(path: Path, content: str, changed_rs: list[Path]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.is_file() or path.read_text(encoding="utf-8") != content:
        path.write_text(content, encoding="utf-8")
        changed_rs.append(path)


def run_check(root: Path, rows: dict[FnKey, FnRow], mod_sides: dict[str, str]) -> int:
    """Verify tree matches `tools/game_fn_sides.tsv` (fail closed)."""
    problems: list[str] = []
    all_regular_entries: list[Path] = []
    regular_mods, existing_mirrors = parse_top_mod_decls(
        root, set(mod_sides), errors=problems, all_regular_entries=all_regular_entries
    )
    check_orphan_mirror_files(root, regular_mods, all_regular_entries, existing_mirrors, problems)
    expected_mirrors = mirror_modules_from_table(rows)

    game_rs_path = root / "crates/sunrise-client/src/game.rs"
    game_root_tree = build_mod_tree("game", game_rs_path.resolve()) if game_rs_path.is_file() else None

    for side in ("client", "server"):
        actual = set(existing_mirrors[side])
        want = expected_mirrors[side]
        for extra in sorted(actual - want):
            problems.append(f"{SIDE_MOD_NAME[side]}.rs: undeclared mirror module `{extra}` not backed by game_fn_sides.tsv")
        for missing in sorted(want - actual):
            problems.append(f"{SIDE_MOD_NAME[side]}.rs: missing mirror module `{missing}` required by game_fn_sides.tsv")

    rows_by_top: dict[str, dict[FnKey, FnRow]] = defaultdict(dict)
    for k, r in rows.items():
        rows_by_top[r.top_module][k] = r

    all_tops = sorted(
        set(rows_by_top)
        | {m[: -len(f"_{s}")] for s in ("client", "server") for m in existing_mirrors[s]}
    )

    for top_name in all_tops:
        resolved = resolve_top_entry(root, top_name, regular_mods)
        if resolved is None:
            problems.append(f"top-level module `{top_name}` missing from game.rs / side files")
            continue
        parent_side, entry_file, top_cfg = resolved
        top_problems_before = len(problems)
        orig_tree = build_mod_tree(top_name, entry_file, top_cfg, errors=problems, root=root)
        orig_fns = collect_tree_functions(orig_tree, dup_errors=problems, root=root)
        mirror_trees: dict[str, ModNode | None] = {"client": None, "server": None}
        mirror_fns: dict[str, dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None, tuple[str, ...]]]] = {
            "client": {},
            "server": {},
        }
        mirror_file_sets: dict[str, set[Path]] = {"client": set(), "server": set()}
        for side in ("client", "server"):
            mname = f"{top_name}_{side}"
            if mname in existing_mirrors[side]:
                mentry, mcfg = existing_mirrors[side][mname]
                if mentry.is_file():
                    mtree = build_mod_tree(top_name, mentry, mcfg, errors=problems, root=root, is_mirror=True)
                    mirror_trees[side] = mtree
                    mirror_fns[side] = collect_tree_functions(mtree, dup_errors=problems, root=root)
                    mirror_file_sets[side] = collect_mirror_files(mentry)
                    for mf in sorted(mirror_file_sets[side]):
                        mf_txt = mf.read_text(encoding="utf-8")
                        cnt, lns = rewrite_root_pub_super_check(mf_txt)
                        if cnt > 0:
                            rel = mf.relative_to(root).as_posix()
                            problems.append(f"{rel}: {cnt} root-scope `pub(super)` item(s) remain (line(s) {', '.join(str(x) for x in lns[:5])})")

        top_rows = rows_by_top.get(top_name, {})
        desired_side: dict[FnKey, str] = {k: r.side for k, r in top_rows.items()}
        validate_top_overlaps_and_cfg_attr(top_rows, orig_fns, mirror_fns, problems, root=root)

        for side in ("client", "server"):
            for k in sorted(mirror_fns[side], key=lambda x: (x.module, x.impl, x.name, x.cfg)):
                if k not in top_rows:
                    problems.append(f"{SIDE_MOD_NAME[side]}::{top_name}_{side}: unlisted function `{k.module}::{k.name}`")
                elif top_rows[k].side != side:
                    problems.append(f"{SIDE_MOD_NAME[side]}::{top_name}_{side}: `{k.module}::{k.name}` has table side `{top_rows[k].side}`")

        for k, r in sorted(top_rows.items(), key=lambda kv: (kv[0].module, kv[0].impl, kv[0].name, kv[0].cfg)):
            in_orig = k in orig_fns
            in_c = k in mirror_fns["client"]
            in_s = k in mirror_fns["server"]
            if r.side == "game":
                if not in_orig:
                    problems.append(f"`{k.module}::{k.name}` (side=game) missing from original module")
                if in_c or in_s:
                    problems.append(f"`{k.module}::{k.name}` (side=game) still present in mirror module")
            elif r.side == "client":
                if not in_c:
                    problems.append(f"`{k.module}::{k.name}` (side=client) missing from `{top_name}_client`")
                if in_orig or in_s:
                    problems.append(f"`{k.module}::{k.name}` (side=client) duplicated in orig/server")
            elif r.side == "server":
                if not in_s:
                    problems.append(f"`{k.module}::{k.name}` (side=server) missing from `{top_name}_server`")
                if in_orig or in_c:
                    problems.append(f"`{k.module}::{k.name}` (side=server) duplicated in orig/client")

        # Location-sensitive macro and directory-changed include checks on moved/mirrored functions
        macro_trees = [orig_tree, *(t for t in mirror_trees.values() if t is not None)]
        if game_root_tree is not None and top_name != "game":
            macro_trees.append(game_root_tree)
        sensitive_macros = collect_location_sensitive_macros(macro_trees)
        orig_node_by_nid: dict[tuple[str, tuple[str, ...]], ModNode] = {}

        def index_orig(n: ModNode) -> None:
            orig_node_by_nid[(n.orig_mod, node_cfg_chain(n))] = n
            for ch in n.children:
                index_orig(ch)

        index_orig(orig_tree)
        for side in ("client", "server"):
            for k, (mn, mfp, fsrc, mit, _mp, _mfc) in sorted(
                mirror_fns[side].items(), key=lambda kv: (kv[0].module, kv[0].impl, kv[0].name, kv[0].cfg)
            ):
                fn_txt = fsrc[mit.start : mit.end]
                loc_hits = find_location_macro_uses(fn_txt, sensitive_macros)
                if loc_hits:
                    problems.append(
                        f"{_rel_str(mfp, root)}:{mit.start_line}: moved function `{k.module}::{k.name}` uses location-sensitive macro(s) {', '.join(loc_hits)}"
                    )
                onode = orig_node_by_nid.get((mn.orig_mod, node_cfg_chain(mn)))
                if onode is not None and onode.file_path.resolve().parent != mfp.resolve().parent:
                    inc_hits = find_relative_includes_in_src(fn_txt)
                    if inc_hits:
                        problems.append(
                            f"{_rel_str(mfp, root)}:{mit.start_line}: moved function `{k.module}::{k.name}` uses {', '.join(inc_hits)} in changed directory"
                        )

        # Verify every existing mirror file contains ONLY what the tool generates
        if len(problems) == top_problems_before:
            fn_src, fn_nid, trait_src, impl_pref = _capture_fn_sources(
                orig_fns, mirror_fns, desired_side, problems, check_trait_consistency=True
            )
            m_macros, s_macros, _r_macros = _collect_macros_for_placement(
                orig_tree, mirror_trees, desired_side, fn_src, errors=problems, root=root
            )
            node_moved: dict[tuple[str, tuple[str, ...], str], list[FnKey]] = defaultdict(list)
            for k, r in top_rows.items():
                if r.side in ("client", "server") and k in fn_nid:
                    nid = fn_nid[k]
                    node_moved[(nid[0], nid[1], r.side)].append(k)
            for side in ("client", "server"):
                if mirror_trees[side] is None:
                    continue
                expected_files = _render_mirror_tree(
                    orig_tree,
                    side,
                    top_name,
                    parent_side,
                    regular_mods,
                    desired_side,
                    orig_fns,
                    mirror_fns,
                    fn_src,
                    fn_nid,
                    trait_src,
                    impl_pref,
                    m_macros,
                    s_macros,
                    node_moved,
                )
                for mf in sorted(mirror_file_sets[side]):
                    if mf not in expected_files:
                        problems.append(f"{_rel_str(mf, root)}: unexpected mirror file not generated by tool")
                    else:
                        problems.extend(diff_mirror_file(_rel_str(mf, root), mf.read_text(encoding="utf-8"), expected_files[mf]))
        else:
            for side in ("client", "server"):
                for mf in sorted(mirror_file_sets[side]):
                    mf_txt = mf.read_text(encoding="utf-8")
                    if has_top_level_inner_attrs(mf_txt):
                        problems.append(f"{_rel_str(mf, root)}: disallowed inner attribute `#![...]` in mirror file")
                    try:
                        for it in parse_rust_items(mf_txt, mf):
                            if it.kind not in ALLOWED_MIRROR_ITEM_KINDS:
                                problems.append(
                                    f"{_rel_str(mf, root)}:{it.start_line}: unexpected hand-written `{it.kind}` item `{it.name}` in mirror file"
                                )
                    except ValueError:
                        pass

    for path, _ in selector_targets(root, rows, mod_sides, errors=problems):
        rel = path.relative_to(root).as_posix()
        problems.append(f"{rel}: stale moved-function selector(s); run `python3 tools/game_fn_sides.py --fix`")

    for p in problems:
        print(f"error: {p}", file=sys.stderr)
    if problems:
        print(f"Found {len(problems)} fn-side problem(s).", file=sys.stderr)
        return 1
    c_n = sum(1 for r in rows.values() if r.side == "client")
    s_n = sum(1 for r in rows.values() if r.side == "server")
    g_n = sum(1 for r in rows.values() if r.side == "game")
    print(f"OK: {len(rows)} function rows verified (client={c_n}, server={s_n}, game={g_n}).")
    return 0


def run_list(root: Path, rows: dict[FnKey, FnRow], mod_sides: dict[str, str]) -> int:
    regular_mods, existing_mirrors = parse_top_mod_decls(root, set(mod_sides))
    cache_orig: dict[str, set[FnKey]] = {}
    cache_m: dict[tuple[str, str], set[FnKey]] = {}

    def get_actual(k: FnKey) -> str:
        top = k.module.split("::", 1)[0]
        if top not in cache_orig:
            resolved = resolve_top_entry(root, top, regular_mods)
            if resolved is not None:
                cache_orig[top] = set(collect_tree_functions(build_mod_tree(top, resolved[1], resolved[2])))
        for s in ("client", "server"):
            if (top, s) not in cache_m:
                mname = f"{top}_{s}"
                if mname in existing_mirrors[s]:
                    mentry, mcfg = existing_mirrors[s][mname]
                    cache_m[(top, s)] = set(collect_mirror_tree_functions(mentry, top, s, mcfg))
                else:
                    cache_m[(top, s)] = set()
        locs = []
        if k in cache_orig.get(top, set()):
            locs.append("game")
        if k in cache_m[(top, "client")]:
            locs.append("client")
        if k in cache_m[(top, "server")]:
            locs.append("server")
        return ",".join(locs) if locs else "<missing>"

    print("module\timpl\tname\tcfg\ttable_side\tactual\tstatus\treason")
    for k, r in sorted(rows.items(), key=lambda kv: (kv[0].module, kv[0].impl, kv[0].name, kv[0].cfg)):
        actual = get_actual(k)
        status = "ok" if actual == r.side else "drift"
        print(f"{k.module}\t{k.impl}\t{k.name}\t{k.cfg}\t{r.side}\t{actual}\t{status}\t{r.reason}")
    return 0


def run_renames(
    before_json: Path,
    after_json: Path,
    rows: dict[FnKey, FnRow],
    mod_sides: dict[str, str],
    out_path: Path | None,
) -> int:
    """Emit `old_target\tnew_target\tside` for every moved function in `functions.json`."""
    before = json.loads(before_json.read_text(encoding="utf-8"))
    after = json.loads(after_json.read_text(encoding="utf-8"))
    before_fns = before.get("functions", {})
    before_refused = before.get("refused", {})
    after_fns = after.get("functions", {})
    after_refused = after.get("refused", {})

    # Build fn-level lookup: (module_rel, name) -> side
    fn_moves: dict[tuple[str, str], str] = {}
    for r in rows.values():
        if r.side in ("client", "server"):
            fn_moves[(r.key.module, r.key.name)] = r.side

    out_rows: list[tuple[str, str, str]] = []
    in_after = 0
    in_refused = 0
    missing: list[tuple[str, str]] = []
    matched_after: set[str] = set()

    for old in sorted(before_fns):
        if old in after_fns:
            continue
        if not old.startswith("sunrise_client::game::"):
            continue
        tail = old[len("sunrise_client::game::") :]
        # Skip targets already under a side container
        if tail.startswith(("client_side::", "server_side::")):
            continue
        top = tail.split("::", 1)[0]
        fn_name = tail.rsplit("::", 1)[-1]
        # Try game.rs mirror candidates, submodule mirror candidates, and whole-module candidate
        cands: list[tuple[str, str]] = []
        for s in ("client", "server"):
            cands.append((s, f"sunrise_client::game::{SIDE_MOD_NAME[s]}::game_{s}::{tail}"))
        for s in ("client", "server"):
            cands.append((s, f"sunrise_client::game::{SIDE_MOD_NAME[s]}::{top}_{s}::{tail[len(top) + 2:]}" if "::" in tail else f"sunrise_client::game::{SIDE_MOD_NAME[s]}::{top}_{s}"))
        ms = mod_sides.get(top)
        if ms in ("client", "server"):
            cands.append((ms, f"sunrise_client::game::{SIDE_MOD_NAME[ms]}::{tail}"))

        hit = None
        for s, cand in cands:
            if cand in after_fns:
                hit = (s, cand)
                break
        if hit:
            out_rows.append((old, hit[1], hit[0]))
            in_after += 1
            matched_after.add(hit[1])
        else:
            ref_hit = None
            for s, cand in cands:
                if cand in after_refused:
                    ref_hit = (s, cand)
                    break
            if ref_hit:
                out_rows.append((old, ref_hit[1], ref_hit[0]))
                in_refused += 1
            else:
                missing.append((old, cands[0][1] if cands else "?"))

    lines = ["old_target\tnew_target\tside\n"]
    for old, new, side in out_rows:
        lines.append(f"{old}\t{new}\t{side}\n")
    content = "".join(lines)
    if out_path is not None:
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(content, encoding="utf-8")
    else:
        sys.stdout.write(content)

    unchanged = len(set(before_fns) & set(after_fns))
    raw_extra = sorted(set(after_fns) - set(before_fns) - matched_after)
    newly_selectable: list[str] = []
    extra_added: list[str] = []
    for added in raw_extra:
        was_refused = added in before_refused
        if not was_refused and added.startswith("sunrise_client::game::"):
            tail = added[len("sunrise_client::game::") :]
            for s in ("client", "server"):
                pref = f"{SIDE_MOD_NAME[s]}::"
                if tail.startswith(pref):
                    rest = tail[len(pref) :]
                    if f"sunrise_client::game::{rest}" in before_refused:
                        was_refused = True
                    m = re.match(rf"^([A-Za-z0-9_]+)_{s}(::.*)$", rest)
                    if m and f"sunrise_client::game::{m.group(1)}{m.group(2)}" in before_refused:
                        was_refused = True
                    if rest.startswith(f"game_{s}::") and f"sunrise_client::game::{rest[len(f'game_{s}::'):]}" in before_refused:
                        was_refused = True
        if was_refused:
            newly_selectable.append(added)
        else:
            extra_added.append(added)
    print(
        f"Renames: {len(out_rows)} moved ({in_after} in after functions.json, "
        f"{in_refused} in after refused, {len(missing)} missing); "
        f"{unchanged} unchanged, {len(newly_selectable)} newly selectable, {len(extra_added)} unexpected added.",
        file=sys.stderr,
    )
    if missing or extra_added:
        for old, new in missing[:10]:
            print(f"  missing: {old} -> {new}", file=sys.stderr)
        for added in extra_added[:10]:
            print(f"  unexpected added: {added}", file=sys.stderr)
        return 1
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Split and verify one-sided functions in sunrise-client gameplay modules.")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--fix", action="store_true", help="move functions, update mirror files, declarations, and selectors")
    mode.add_argument("--check", action="store_true", help="verify tree matches tools/game_fn_sides.tsv (fail closed)")
    mode.add_argument("--list", action="store_true", help="list function rows with table side and current location")
    mode.add_argument(
        "--renames",
        nargs=2,
        metavar=("BEFORE_JSON", "AFTER_JSON"),
        type=Path,
        help="emit old_target -> new_target TSV from before/after functions.json",
    )
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root directory")
    parser.add_argument("--table", type=Path, default=None, help="path to game_fn_sides.tsv")
    parser.add_argument("--module-table", type=Path, default=None, help="path to game_module_sides.tsv")
    parser.add_argument("--out", type=Path, default=None, help="output path for --renames")
    parser.add_argument("--no-fmt", action="store_true", help="skip rustfmt on modified Rust files")
    args = parser.parse_args(argv)

    root = args.root.resolve()
    table_path = args.table or (root / "tools/game_fn_sides.tsv" if args.root != ROOT else DEFAULT_TABLE)
    mod_table_path = args.module_table or (root / "tools/game_module_sides.tsv" if args.root != ROOT else DEFAULT_MOD_TABLE)
    rows = load_fn_sides(table_path)
    mod_sides = load_optional_module_sides(mod_table_path)

    if args.renames:
        return run_renames(args.renames[0], args.renames[1], rows, mod_sides, args.out)
    if args.fix:
        return run_fix(root, rows, mod_sides, fmt=not args.no_fmt)
    if args.check:
        return run_check(root, rows, mod_sides)
    return run_list(root, rows, mod_sides)


if __name__ == "__main__":
    raise SystemExit(main())
