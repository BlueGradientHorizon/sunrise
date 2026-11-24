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
from collections import defaultdict
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
    extra_files: list[tuple[Path, str, list[ParsedItem]]] = field(default_factory=list)


def load_fn_sides(table_path: Path) -> dict[FnKey, FnRow]:
    """Load `tools/game_fn_sides.tsv` and fail closed on schema or key errors."""
    rows: dict[FnKey, FnRow] = {}
    with table_path.open(newline="", encoding="utf-8") as fp:
        reader = csv.DictReader(fp, delimiter="\t")
        expected = {"module", "impl", "name", "cfg", "side", "reason"}
        if not reader.fieldnames or not expected.issubset(reader.fieldnames):
            raise SystemExit(f"{table_path}: expected columns {sorted(expected)}")
        for lineno, r in enumerate(reader, 2):
            mod = r["module"].strip()
            impl = r["impl"].strip()
            name = r["name"].strip()
            cfg = r["cfg"].strip()
            side = r["side"].strip()
            reason = r["reason"].strip()
            if not mod or not impl or not name or not cfg or side not in ("client", "server", "game"):
                raise SystemExit(f"{table_path}:{lineno}: invalid row ({mod!r}, {impl!r}, {name!r}, {side!r})")
            key = FnKey(module=mod, impl=impl, name=name, cfg=cfg)
            if key in rows:
                raise SystemExit(f"{table_path}:{lineno}: duplicate function identity {key}")
            rows[key] = FnRow(key=key, side=side, reason=reason)
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
        for r in reader:
            out[r["module"].strip()] = r["side"].strip()
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
                    end = n if end == -1 else end + len(end_seq)
                    line += src[start:end].count("\n")
                    tokens.append(Token("str", src[start:end], start, end, start_line))
                    i = end
                    continue
        if c == '"' or (c in ("b", "c") and i + 1 < n and src[i + 1] == '"'):
            start, start_line = i, line
            if c != '"':
                i += 1
            i += 1
            while i < n:
                ch = src[i]
                if ch == "\\":
                    if i + 1 < n and src[i + 1] == "\n":
                        line += 1
                    i += 2
                elif ch == '"':
                    i += 1
                    break
                else:
                    if ch == "\n":
                        line += 1
                    i += 1
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
                end = src.find("'", j + 1)
                j = end + 1 if end != -1 else j + 2
            else:
                j = min(n, j + 2)
            tokens.append(Token("char", src[start:j], start, j, line))
            i = j
            continue
        if c == "'":
            start = i
            if i + 2 < n and src[i + 1] == "\\":
                end = src.find("'", i + 2)
                if end != -1 and end - i <= 12 and "\n" not in src[i:end]:
                    tokens.append(Token("char", src[start : end + 1], start, end + 1, line))
                    i = end + 1
                    continue
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


def leading_doc_start(src: str, attr_or_kw_start: int, min_bound: int) -> int:
    """Walk backwards from `attr_or_kw_start` over contiguous outer doc/line comments (never `//!` or `/*!`)."""
    line_start = src.rfind("\n", min_bound, attr_or_kw_start)
    cur_line_start = (line_start + 1) if line_start != -1 else min_bound
    best = cur_line_start
    pos = cur_line_start
    while pos > min_bound:
        prev_nl = src.rfind("\n", min_bound, pos - 1)
        prev_start = (prev_nl + 1) if prev_nl != -1 else min_bound
        stripped = src[prev_start : pos - 1].strip()
        if stripped.startswith("//!") or stripped.startswith("/*!"):
            break
        if stripped.startswith("///") or stripped.startswith("//"):
            best = prev_start
            pos = prev_start
        else:
            break
    return best


def normalize_cfg_list(attrs: list[str] | tuple[str, ...]) -> tuple[str, ...]:
    out = []
    for a in attrs:
        norm = re.sub(r"\s+", " ", a).strip()
        if norm.startswith("#[cfg("):
            out.append(norm)
    return tuple(out)


def parse_rust_items(src: str, source_file: Path | None = None) -> list[ParsedItem]:
    """Parse Rust items recursively with exact byte and line spans."""
    tokens = lex_rust(src)
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
                    if tokens[k].text in ("(", "["):
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
                    inner_src = src[tokens[i + 2].end : tokens[nxt - 1].start]
                    inc_m = re.match(r'\s*include!\s*\(\s*"([^"]+)"\s*\)\s*;\s*$', inner_src)
                    if inc_m:
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
                                include_rel=inc_m.group(1),
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
                            if tokens[k].text in ("(", "["):
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
                else:
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
                        if nxt < end_i and tokens[nxt].text == ";":
                            nxt += 1
                        item_end = make_end(nxt - 1)
                        inc_rel = None
                        kind = "macro_rules" if is_mrules else "macro_invoc"
                        if mpath in ("include", "std::include", "core::include"):
                            sm = re.fullmatch(r'"([^"]+)"', body_txt)
                            if sm:
                                kind = "root_include"
                                inc_rel = sm.group(1)
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


def parse_top_mod_decls(
    root: Path, regular_names: set[str] | frozenset[str] = frozenset()
) -> tuple[dict[str, tuple[str, Path, tuple[str, ...]]], dict[str, dict[str, tuple[Path, tuple[str, ...]]]]]:
    """Return `(regular_mods, mirror_mods)` across `game.rs`, `client_side.rs`, `server_side.rs`.

    `regular_mods`: `name -> (parent_side, entry_file, cfg_attrs)`
    `mirror_mods`: `side -> {mirror_mod_name -> (entry_file, cfg_attrs)}`
    """
    src_root = root / "crates/sunrise-client/src"
    game_dir = src_root / "game"
    regular: dict[str, tuple[str, Path, tuple[str, ...]]] = {}
    mirrors: dict[str, dict[str, tuple[Path, tuple[str, ...]]]] = {"client": {}, "server": {}}

    for parent_side, path in [
        ("game", src_root / "game.rs"),
        ("client", game_dir / "client_side.rs"),
        ("server", game_dir / "server_side.rs"),
    ]:
        if not path.is_file():
            continue
        items = parse_rust_items(path.read_text(encoding="utf-8"), path)
        for it in items:
            if it.kind != "mod_ext" or it.name in SIDE_CONTAINERS:
                continue
            if it.path_attr:
                if it.path_attr.startswith("game/"):
                    entry = (src_root / it.path_attr).resolve()
                else:
                    entry = ((game_dir / it.path_attr) if parent_side != "game" else (src_root / it.path_attr)).resolve()
                    if not entry.is_file():
                        entry = (game_dir / it.path_attr).resolve()
            else:
                c1 = game_dir / f"{it.name}.rs"
                c2 = game_dir / it.name / "mod.rs"
                entry = (c1 if c1.is_file() else c2).resolve()

            m = MIRROR_MOD_RE.match(it.name) if parent_side in ("client", "server") else None
            if (
                m
                and m.group(2) == parent_side
                and it.name not in regular_names
                and entry.name.endswith(f"_{parent_side}.rs")
            ):
                mirrors[parent_side][it.name] = (entry, it.cfg_attrs)
            elif it.name not in regular:
                regular[it.name] = (parent_side, entry, it.cfg_attrs)
    return regular, mirrors


def build_mod_tree(top_name: str, entry_file: Path, top_cfg: tuple[str, ...] = ()) -> ModNode:
    """Build complete module tree for `top_name`, including root-scope `include!` fragments."""

    def walk_file(rs_file: Path, mod_rel: str, decl_kind: str, name: str, cfg: tuple[str, ...], parent: ModNode | None) -> ModNode:
        src = rs_file.read_text(encoding="utf-8")
        items = parse_rust_items(src, rs_file)
        node = ModNode(
            orig_mod=mod_rel,
            file_path=rs_file,
            decl_kind=decl_kind,
            name=name,
            cfg_attrs=cfg,
            parent=parent,
            src=src,
            items=items,
        )
        attach_children(node, items, rs_file, src, mod_rel)
        return node

    def attach_children(
        node: ModNode, items: list[ParsedItem], host_file: Path, host_src: str, host_mod: str
    ) -> None:
        for it in items:
            if it.kind == "root_include" and it.include_rel:
                inc_file = (host_file.parent / it.include_rel).resolve()
                if inc_file.is_file():
                    inc_src = inc_file.read_text(encoding="utf-8")
                    inc_items = parse_rust_items(inc_src, inc_file)
                    node.extra_files.append((inc_file, inc_src, inc_items))
                    attach_children(node, inc_items, inc_file, inc_src, host_mod)
            elif it.kind == "mod_ext":
                # Out-of-line `mod` items in `game.rs` are independent top-level gameplay modules, not children of `game`.
                if top_name == "game" and node.parent is None:
                    continue
                if it.path_attr:
                    child_file = (host_file.parent / it.path_attr).resolve()
                else:
                    base_dir = host_file.parent if host_file.name in ("mod.rs", "lib.rs") else host_file.with_suffix("")
                    raw_name = it.name[2:] if it.name.startswith("r#") else it.name
                    c1 = base_dir / f"{raw_name}.rs"
                    c2 = base_dir / raw_name / "mod.rs"
                    child_file = c1 if c1.is_file() else c2
                if child_file.is_file():
                    child_node = walk_file(child_file, f"{host_mod}::{it.name}", "out_of_line", it.name, it.cfg_attrs, node)
                    node.children.append(child_node)
            elif it.kind == "mod_include" and it.include_rel:
                inc_file = (host_file.parent / it.include_rel).resolve()
                if inc_file.is_file():
                    child_node = walk_file(inc_file, f"{host_mod}::{it.name}", "inline_include", it.name, it.cfg_attrs, node)
                    node.children.append(child_node)
            elif it.kind == "mod_inline":
                sub = ModNode(
                    orig_mod=f"{host_mod}::{it.name}",
                    file_path=host_file,
                    decl_kind="inline",
                    name=it.name,
                    cfg_attrs=it.cfg_attrs,
                    parent=node,
                    inline_item=it,
                    src=host_src,
                    items=it.children,
                )
                attach_children(sub, it.children, host_file, host_src, sub.orig_mod)
                node.children.append(sub)

    return walk_file(entry_file, top_name, "root", top_name, top_cfg, None)


def node_all_fragments(node: ModNode) -> list[tuple[Path, str, list[ParsedItem]]]:
    """Return primary `(file_path, src, items)` followed by any root-scope `include!` fragments."""
    return [(node.file_path, node.src, node.items), *node.extra_files]


def collect_tree_functions(root_node: ModNode) -> dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None]]:
    """Map `FnKey -> (node, file_path, src, item, parent_impl)` across every node and fragment."""
    found: dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None]] = {}

    def walk(node: ModNode) -> None:
        ncfgs = node_cfg_chain(node)
        for fpath, fsrc, fitems in node_all_fragments(node):
            for it in fitems:
                if it.kind == "fn":
                    k = item_fn_key(node.orig_mod, it, None, ncfgs)
                    found[k] = (node, fpath, fsrc, it, None)
                elif it.kind == "impl":
                    for ch in it.children:
                        if ch.kind == "method":
                            k = item_fn_key(node.orig_mod, ch, it, ncfgs)
                            found[k] = (node, fpath, fsrc, ch, it)
        for ch_node in node.children:
            walk(ch_node)

    walk(root_node)
    return found


def collect_mirror_tree_functions(
    mirror_entry: Path, top_name: str, side: str, top_cfg: tuple[str, ...] = ()
) -> dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None]]:
    """Walk an existing mirror tree and index its functions using canonical (un-suffixed) `orig_mod` keys."""
    if not mirror_entry.is_file():
        return {}
    mirror_root = build_mod_tree(top_name, mirror_entry, top_cfg)
    return collect_tree_functions(mirror_root)


def rewrite_vis_to_game(vis: str) -> str | None:
    norm = re.sub(r"\s+", " ", vis).replace("( ", "(").replace(" )", ")")
    if norm in ("pub", "pub(crate)", "pub(in crate::game)"):
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
    fn_text: str, *, is_trait_impl: bool, is_test_fn: bool, is_game_root: bool = False
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
        rep = rewrite_vis_to_game(vis)
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
        for _fp, fsrc, fitems in node_all_fragments(cur):
            for it in fitems:
                if it.kind == "macro_rules":
                    continue
                if it.kind == "fn":
                    if pat.search(fsrc[it.start : it.end]):
                        used_sides.add(desired_side.get(item_fn_key(cur.orig_mod, it, None, ncfgs), "game"))
                elif it.kind == "impl":
                    for ch in it.children:
                        if ch.kind == "method" and pat.search(fsrc[ch.start : ch.end]):
                            used_sides.add(desired_side.get(item_fn_key(cur.orig_mod, ch, it, ncfgs), "game"))
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
        for fp, _, _ in n.extra_files:
            files.add(fp.resolve())
        for ch in n.children:
            walk(ch)

    walk(root)
    return files


def sync_side_file_mirror_decls(
    root: Path,
    side: str,
    expected_decls: dict[str, tuple[str, tuple[str, ...]]],
    regular_names: set[str] | frozenset[str] = frozenset(),
) -> Path | None:
    """Update `<m>_<side>` declarations in `{client,server}_side.rs` and return path if changed."""
    side_file = root / "crates/sunrise-client/src/game" / f"{SIDE_MOD_NAME[side]}.rs"
    if not side_file.is_file():
        return None
    old_txt = side_file.read_text(encoding="utf-8")
    items = parse_rust_items(old_txt, side_file)

    # Remove all existing mirror mod declarations (excluding regular modules in game_module_sides.tsv)
    edits: list[tuple[int, int, str]] = []
    for it in items:
        if it.kind == "mod_ext" and it.name not in regular_names:
            m = MIRROR_MOD_RE.match(it.name)
            if m and m.group(2) == side and (it.path_attr or "").endswith(f"_{side}.rs"):
                edits.append((it.start, it.end, ""))
    cleaned = apply_edits(old_txt, edits)

    # Re-insert expected mirror declarations alphabetically right before `struct {Client,Server}State`
    if expected_decls:
        struct_kw = "struct ClientState" if side == "client" else "struct ServerState"
        # Find the struct declaration start (including preceding attrs/docs)
        c_items = parse_rust_items(cleaned, side_file)
        insert_at = None
        for it in c_items:
            if it.kind == "struct" and it.name == ("ClientState" if side == "client" else "ServerState"):
                insert_at = it.start
                break
        if insert_at is None:
            raise SystemExit(f"{side_file}: could not locate `{struct_kw}`")
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
    rows: dict[FnKey, FnRow], mod_sides: dict[str, str]
) -> tuple[dict[str, str], dict[str, str], dict[str, set[str]]]:
    """Return `(test_path_map, fnr_target_map, mod_mirror_test_sides)`.

    `test_path_map`: exact `game::...::<fn>` (canonical or pre-move) -> `game::<side>_side::<top>_<side>...::<fn>`
    `fnr_target_map`: exact `sunrise_client::game::...::<fn>` -> new path (for fn-replace-check/docs)
    `mod_mirror_test_sides`: `top_module -> set of sides ('client','server') that have moved free/test fns`
    """
    test_map: dict[str, str] = {}
    fnr_map: dict[str, str] = {}
    mod_test_sides: dict[str, set[str]] = defaultdict(set)

    for row in rows.values():
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
                    test_map[f"{ov}::{k.name}"] = f"{new_mod}::{k.name}"
                    fnr_map[f"sunrise_client::{ov}::{k.name}"] = f"sunrise_client::{new_mod}::{k.name}"
                # Also map reverse/other-side mirror path for idempotency when rows change
                other = "server" if row.side == "client" else "client"
                other_mod = f"game::{mirror_mod_rel(k.module, top, other)}"
                test_map[f"{other_mod}::{k.name}"] = f"{new_mod}::{k.name}"
            else:
                # Extract owner type from impl header
                owner = impl_owner_for_target(k.impl)
                if owner:
                    for ov in orig_variants:
                        fnr_map[f"sunrise_client::{ov}::{owner}::{k.name}"] = f"sunrise_client::{new_mod}::{owner}::{k.name}"
        else:
            # Row stays in `game` (or `mod_side`): map stale mirror paths back to canonical location
            canon_mod = f"game::{SIDE_MOD_NAME[mod_side]}::{k.module}" if mod_side in ("client", "server") else base_orig
            if k.impl == "-":
                for s in ("client", "server"):
                    stale_mod = f"game::{mirror_mod_rel(k.module, top, s)}"
                    test_map[f"{stale_mod}::{k.name}"] = f"{canon_mod}::{k.name}"
                    fnr_map[f"sunrise_client::{stale_mod}::{k.name}"] = f"sunrise_client::{canon_mod}::{k.name}"
            else:
                owner = impl_owner_for_target(k.impl)
                if owner:
                    for s in ("client", "server"):
                        stale_mod = f"game::{mirror_mod_rel(k.module, top, s)}"
                        fnr_map[f"sunrise_client::{stale_mod}::{owner}::{k.name}"] = f"sunrise_client::{canon_mod}::{owner}::{k.name}"

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
    root: Path, rows: dict[FnKey, FnRow], mod_sides: dict[str, str]
) -> dict[tuple[str, str], str]:
    """Return `(old_file_rel, fn_name) -> target_file_rel` for `tools/port_map/*.tsv` rows."""
    regular_mods, _ = parse_top_mod_decls(root, set(mod_sides))
    rows_by_top: dict[str, list[FnRow]] = defaultdict(list)
    for row in rows.values():
        rows_by_top[row.top_module].append(row)

    moves: dict[tuple[str, str], str] = {}
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
        # Collect staying fn names per file so we never rewrite a name that still exists in the original file.
        staying_by_file: dict[str, set[str]] = defaultdict(set)
        orig_fns = collect_tree_functions(tree)
        desired = {r.key: r.side for r in top_rows}
        for k, (_n, fpath, _fsrc, _it, _pi) in orig_fns.items():
            if desired.get(k, "game") == "game":
                staying_by_file[fpath.resolve().relative_to(root).as_posix()].add(k.name)

        for row in top_rows:
            nodes = mod_nodes.get(row.key.module, [])
            if not nodes:
                continue
            for n in nodes:
                orig_main_rel = n.file_path.resolve().relative_to(root).as_posix()
                orig_all_rels = [orig_main_rel, *(fp.resolve().relative_to(root).as_posix() for fp, _, _ in n.extra_files)]
                c_rel = mirror_file_path(n.file_path, "client").resolve().relative_to(root).as_posix()
                s_rel = mirror_file_path(n.file_path, "server").resolve().relative_to(root).as_posix()
                fn_name = row.key.name
                if row.side in ("client", "server"):
                    dest = c_rel if row.side == "client" else s_rel
                    other = s_rel if row.side == "client" else c_rel
                    for src_rel in (*orig_all_rels, other):
                        if src_rel != dest and fn_name not in staying_by_file.get(src_rel, set()):
                            moves[(src_rel, fn_name)] = dest
                else:
                    for stale_rel in (c_rel, s_rel):
                        if stale_rel != orig_main_rel:
                            moves[(stale_rel, fn_name)] = orig_main_rel
    return moves


def selector_targets(root: Path, rows: dict[FnKey, FnRow], mod_sides: dict[str, str]) -> list[tuple[Path, str]]:
    """Return `[(path, expected_content)]` for every selector file that would change."""
    test_map, fnr_map, mod_test_sides = build_selector_maps(rows, mod_sides)
    test_pat = re.compile(r"(?<![A-Za-z0-9_:])game::[A-Za-z0-9_:]+")
    fnr_pat = re.compile(r"sunrise_client::game::[A-Za-z0-9_:<>', ]+::[A-Za-z0-9_]+")
    updates: list[tuple[Path, str]] = []

    ignored_dir = root / "tools/ci/ignored-tests"
    if ignored_dir.is_dir():
        for p in sorted(ignored_dir.glob("*.txt")):
            old = p.read_text(encoding="utf-8")
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
        if readme.is_file():
            old = readme.read_text(encoding="utf-8")
            if "game::" in old:
                new = rewrite_exact_map(rewrite_exact_map(old, fnr_map, fnr_pat), test_map, test_pat)
                if new != old:
                    updates.append((readme, new))

    ci_dir = root / "tools/ci"
    if ci_dir.is_dir():
        for p in sorted(ci_dir.glob("daily-move-*.tsv")):
            old = p.read_text(encoding="utf-8")
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
    if exc_path.is_file():
        old = exc_path.read_text(encoding="utf-8")
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
            old = p.read_text(encoding="utf-8")
            new = rewrite_exact_map(old, test_map, test_pat)
            if new != old:
                updates.append((p, new))

    areas_path = root / "tools/test_areas.toml"
    if areas_path.is_file():
        old = areas_path.read_text(encoding="utf-8")
        new = rewrite_test_areas_for_mirrors(old, mod_test_sides, test_map)
        if new != old:
            updates.append((areas_path, new))

    fnr_dir = root / "tools/ci/fn-replace-check"
    if fnr_dir.is_dir():
        for p in sorted(fnr_dir.rglob("sunrise-mod.toml")):
            old = p.read_text(encoding="utf-8")
            new = rewrite_exact_map(old, fnr_map, fnr_pat)
            if new != old:
                updates.append((p, new))

    draw_test = root / "crates/cargo-sunrise-mod/tests/installed_client_draw.rs"
    if draw_test.is_file():
        old = draw_test.read_text(encoding="utf-8")
        new = rewrite_exact_map(old, fnr_map, fnr_pat)
        if new != old:
            updates.append((draw_test, new))

    docs_dir = root / "docs"
    if docs_dir.is_dir():
        for p in sorted(docs_dir.rglob("*.md")):
            if p.relative_to(root).as_posix() == "docs/modding/src/how-to/native-mods/16-repinning-side-module-targets.md":
                continue
            old = p.read_text(encoding="utf-8", errors="replace")
            if "game::" not in old:
                continue
            new = rewrite_exact_map(rewrite_exact_map(old, fnr_map, fnr_pat), test_map, test_pat)
            if new != old:
                updates.append((p, new))

    port_map_dir = root / "tools/port_map"
    if port_map_dir.is_dir():
        known_stale: set[tuple[str, str, str]] = set()
        stale_file = root / "tools/ci/port_map_known_stale.txt"
        if stale_file.is_file():
            for ln in stale_file.read_text(encoding="utf-8").splitlines():
                if ln.strip() and not ln.lstrip().startswith("#"):
                    parts = ln.split("\t")
                    if len(parts) >= 3:
                        known_stale.add((parts[0], parts[1], parts[2]))
        port_moves = build_port_map_file_moves(root, rows, mod_sides)
        if port_moves:
            for p in sorted(port_map_dir.glob("*.tsv")):
                rel_p = p.relative_to(root).as_posix()
                old = p.read_text(encoding="utf-8")
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
                                fn_name = rest.rpartition("::")[2].strip()
                                new_file = port_moves.get((file_.strip(), fn_name))
                                if new_file and new_file != file_.strip():
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
        for p in sorted(game_dir.rglob("*.rs")):
            old = p.read_text(encoding="utf-8")
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


def run_fix(root: Path, rows: dict[FnKey, FnRow], mod_sides: dict[str, str], *, fmt: bool) -> int:
    game_dir = root / "crates/sunrise-client/src/game"
    regular_names = set(mod_sides)
    regular_mods, existing_mirrors = parse_top_mod_decls(root, regular_names)

    rows_by_top: dict[str, dict[FnKey, FnRow]] = defaultdict(dict)
    for k, r in rows.items():
        rows_by_top[r.top_module][k] = r

    # Include any module that currently has a mirror declared even if its table rows moved back to `game`
    touched_tops = sorted(
        set(rows_by_top)
        | {m[: -len(f"_{s}")] for s in ("client", "server") for m in existing_mirrors[s]}
    )

    changed_rs: list[Path] = []
    expected_side_decls: dict[str, dict[str, tuple[str, tuple[str, ...]]]] = {"client": {}, "server": {}}
    moved_counts = {"client": 0, "server": 0}
    files_created = 0
    files_deleted = 0

    for top_name in touched_tops:
        resolved = resolve_top_entry(root, top_name, regular_mods)
        if resolved is None:
            print(f"error: top-level module `{top_name}` in game_fn_sides.tsv is not declared in game", file=sys.stderr)
            return 1
        parent_side, entry_file, top_cfg = resolved
        orig_tree = build_mod_tree(top_name, entry_file, top_cfg)
        orig_fns = collect_tree_functions(orig_tree)

        def canon_orig_mod(orig_mod_rel: str) -> str:
            if top_name == "game":
                return f"crate::{orig_game_mod_path(orig_mod_rel)}"
            if parent_side in ("client", "server"):
                return f"crate::game::{SIDE_MOD_NAME[parent_side]}::{orig_mod_rel}"
            return f"crate::game::{orig_mod_rel}"

        mirror_trees: dict[str, ModNode | None] = {"client": None, "server": None}
        mirror_fns: dict[str, dict[FnKey, tuple[ModNode, Path, str, ParsedItem, ParsedItem | None]]] = {
            "client": {},
            "server": {},
        }
        old_mirror_files: dict[str, set[Path]] = {"client": set(), "server": set()}
        for side in ("client", "server"):
            mname = f"{top_name}_{side}"
            if mname in existing_mirrors[side]:
                mentry, mcfg = existing_mirrors[side][mname]
                mtree = build_mod_tree(top_name, mentry, mcfg)
                mirror_trees[side] = mtree
                mirror_fns[side] = collect_tree_functions(mtree)
                old_mirror_files[side] = collect_mirror_files(mentry)

        top_rows = rows_by_top.get(top_name, {})
        desired_side: dict[FnKey, str] = {k: r.side for k, r in top_rows.items()}

        # Validate every table row exists somewhere, and no stray functions sit in existing mirrors
        all_known_keys = set(orig_fns) | set(mirror_fns["client"]) | set(mirror_fns["server"])
        for k, r in top_rows.items():
            if k not in all_known_keys:
                print(f"error: table row `{k.module}::{k.impl}::{k.name}` ({k.cfg}) not found in `{top_name}`", file=sys.stderr)
                return 1
        for side in ("client", "server"):
            if side == parent_side:
                continue
            for k in mirror_fns[side]:
                if k not in top_rows:
                    print(f"error: unlisted function `{k.module}::{k.name}` in `{top_name}_{side}`", file=sys.stderr)
                    return 1

        # Capture source text of every function (from wherever it currently lives!) and trait impls
        fn_source_text: dict[FnKey, str] = {}
        fn_node_id: dict[FnKey, tuple[str, tuple[str, ...]]] = {}
        trait_impl_source: dict[tuple[str, tuple[str, ...], str, tuple[str, ...]], tuple[str, str]] = {}
        impl_header_prefix: dict[tuple[str, tuple[str, ...], str, tuple[str, ...]], str] = {}

        for _loc, fdict in [("game", orig_fns), ("client", mirror_fns["client"]), ("server", mirror_fns["server"])]:
            for k, (n, _fp, fsrc, it, pimpl) in fdict.items():
                nid = (n.orig_mod, node_cfg_chain(n))
                fn_source_text[k] = fsrc[it.start : it.end]
                fn_node_id[k] = nid
                if pimpl is not None:
                    ikey = (nid[0], nid[1], pimpl.impl_header, pimpl.cfg_attrs)
                    impl_header_prefix.setdefault(ikey, fsrc[pimpl.start : pimpl.impl_open_brace + 1])
                    if pimpl.is_trait_impl:
                        want = desired_side.get(k, "game")
                        prev = trait_impl_source.get(ikey)
                        if prev is not None and prev[1] != want:
                            print(f"error: trait impl `{pimpl.impl_header}` in `{n.orig_mod}` has mixed sides ({prev[1]} vs {want})", file=sys.stderr)
                            return 1
                        trait_impl_source[ikey] = (fsrc[pimpl.start : pimpl.end], want)

        # Map each (orig_mod, node_cfg_chain) -> its moved functions per side
        node_moved_fns: dict[tuple[str, tuple[str, ...], str], list[FnKey]] = defaultdict(list)
        for k, r in top_rows.items():
            if r.side in ("client", "server"):
                nid = fn_node_id[k]
                node_moved_fns[(nid[0], nid[1], r.side)].append(k)
                moved_counts[r.side] += 1

        # Determine side-exclusive macro_rules! per node (checking both orig and existing mirrors)
        moved_macros: dict[tuple[str, tuple[str, ...], str], list[tuple[ParsedItem, str]]] = defaultdict(list)
        staying_macros: dict[tuple[str, tuple[str, ...]], list[tuple[str, tuple[str, ...]]]] = defaultdict(list)
        restored_macros: dict[tuple[str, tuple[str, ...]], list[str]] = defaultdict(list)

        def collect_macros(node: ModNode) -> None:
            nid = (node.orig_mod, node_cfg_chain(node))
            # Macros in `game.rs` stay in `game.rs` so the rest of the crate (including mirrors) sees them lexically.
            if node.orig_mod != "game":
                for _fp, fsrc, fitems in node_all_fragments(node):
                    for it in fitems:
                        if it.kind == "macro_rules":
                            excl = macro_side_exclusive(node, it.name, desired_side)
                            if excl in ("client", "server"):
                                moved_macros[(nid[0], nid[1], excl)].append((it, fsrc[it.start : it.end]))
                            else:
                                staying_macros[nid].append((it.name, it.cfg_attrs))
            for ch in node.children:
                collect_macros(ch)

        collect_macros(orig_tree)

        def collect_mirror_macros(mnode: ModNode, mside: str) -> None:
            nid = (mnode.orig_mod, node_cfg_chain(mnode))
            for _fp, fsrc, fitems in node_all_fragments(mnode):
                for it in fitems:
                    if it.kind == "macro_rules":
                        # Check whether all users are still exclusively on one side
                        pat = re.compile(rf"\b{re.escape(it.name)}\s*!")
                        used_sides: set[str] = set()
                        for kk, src_txt in fn_source_text.items():
                            if kk.module == mnode.orig_mod or kk.module.startswith(mnode.orig_mod + "::"):
                                if pat.search(src_txt):
                                    used_sides.add(desired_side.get(kk, "game"))
                        if used_sides == {"client"} or used_sides == {"server"}:
                            target_s = next(iter(used_sides))
                            moved_macros[(nid[0], nid[1], target_s)].append((it, fsrc[it.start : it.end]))
                        elif not used_sides:
                            moved_macros[(nid[0], nid[1], mside)].append((it, fsrc[it.start : it.end]))
                        else:
                            restored_macros[nid].append(fsrc[it.start : it.end])
                            staying_macros[nid].append((it.name, it.cfg_attrs))
            for ch in mnode.children:
                collect_mirror_macros(ch, mside)

        for mside in ("client", "server"):
            if mirror_trees[mside] is not None:
                collect_mirror_macros(mirror_trees[mside], mside)  # type: ignore[arg-type]

        def node_has_side(node: ModNode, side: str) -> bool:
            nid = (node.orig_mod, node_cfg_chain(node))
            if node_moved_fns.get((nid[0], nid[1], side)):
                return True
            return any(node_has_side(ch, side) for ch in node.children)

        # 1. Generate mirror files for client and server
        for side in ("client", "server"):
            new_files: set[Path] = set()
            if node_has_side(orig_tree, side):
                side_mod = SIDE_MOD_NAME[side]

                def build_mirror(node: ModNode, anc_macros: list[tuple[str, str, tuple[str, ...]]]) -> str:
                    nid = (node.orig_mod, node_cfg_chain(node))
                    ncfgs = nid[1]
                    cmod = canon_orig_mod(node.orig_mod)
                    is_game_root = node.orig_mod == "game"
                    lines: list[str] = []
                    local_child_mods = {
                        it.name
                        for _fp, _fs, fitems in node_all_fragments(node)
                        for it in fitems
                        if it.kind in ("mod_ext", "mod_inline", "mod_include")
                    }
                    if is_game_root:
                        # Re-exported child modules in `game.rs` (from `client_side`/`server_side`) also live in `crate::game`.
                        local_child_mods |= set(regular_mods) | SIDE_CONTAINERS
                    active_children = [ch for ch in node.children if node_has_side(ch, side)]

                    # Copy `use` statements from orig node + root_include fragments
                    local_macro_names = {
                        it.name
                        for _fp, _fs, fitems in node_all_fragments(node)
                        for it in fitems
                        if it.kind == "macro_rules"
                    }
                    seen_uses: set[str] = set()
                    for _fp, fsrc, fitems in node_all_fragments(node):
                        for it in fitems:
                            if it.kind == "use":
                                # In `game.rs`, `pub*` uses are already imported via `use crate::game::*;`.
                                if is_game_root and it.vis != "":
                                    continue
                                norm_u = re.sub(r"\s+", "", it.name)
                                if f"crate::game::{SIDE_MOD_NAME['client']}::{top_name}_client" in norm_u or f"crate::game::{SIDE_MOD_NAME['server']}::{top_name}_server" in norm_u:
                                    continue
                                if norm_u in local_macro_names or (norm_u.startswith("self::") and norm_u[6:] in local_macro_names):
                                    continue
                                rw = rewrite_use_stmt_for_mirror(
                                    fsrc[it.start : it.end], cmod, local_child_mods, side_mod, is_game_root=is_game_root
                                ).rstrip() + "\n"
                                if rw not in seen_uses:
                                    seen_uses.add(rw)
                                    lines.append(rw)

                    for mmod_canon, mname, mcfgs in anc_macros:
                        cfgs = "".join(f"{c}\n" for c in mcfgs)
                        lines.append(f"{cfgs}#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {mmod_canon}::{mname};\n")

                    # Glob-import staying items from original module
                    lines.append(f"#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {cmod}::*;\n")

                    # Explicit imports for staying local items so they shadow `use super::*;` (not needed for `game.rs` root)
                    moved_macro_names = {
                        it.name
                        for s in ("client", "server")
                        for it, _ in moved_macros.get((nid[0], nid[1], s), [])
                    }
                    moved_here_keys = set(node_moved_fns.get((nid[0], nid[1], side), []))
                    emitted_expl: set[tuple[str, tuple[str, ...]]] = set()
                    if not is_game_root:
                        for _fp, _fs, fitems in node_all_fragments(node):
                            for it in fitems:
                                ekey = (it.name, it.cfg_attrs)
                                if it.kind in ("struct", "enum", "type", "const", "static", "trait"):
                                    if ekey not in emitted_expl:
                                        emitted_expl.add(ekey)
                                        cfgs = "".join(f"{c}\n" for c in it.cfg_attrs)
                                        lines.append(f"{cfgs}#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {cmod}::{it.name};\n")
                                elif it.kind == "macro_rules" and it.name not in moved_macro_names:
                                    if ekey not in emitted_expl:
                                        emitted_expl.add(ekey)
                                        cfgs = "".join(f"{c}\n" for c in it.cfg_attrs)
                                        lines.append(f"{cfgs}#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {cmod}::{it.name};\n")
                                elif it.kind == "macro_invoc" and it.name == "thread_local":
                                    for sname, scfgs in thread_local_statics(_fs, it):
                                        skey = (sname, scfgs)
                                        if skey not in emitted_expl:
                                            emitted_expl.add(skey)
                                            cfgs = "".join(f"{c}\n" for c in scfgs)
                                            lines.append(f"{cfgs}#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {cmod}::{sname};\n")
                                elif it.kind == "fn" and not it.is_test_attr:
                                    k = item_fn_key(node.orig_mod, it, None, ncfgs)
                                    if k not in moved_here_keys and desired_side.get(k, "game") == "game" and ekey not in emitted_expl:
                                        emitted_expl.add(ekey)
                                        cfgs = "".join(f"{c}\n" for c in it.cfg_attrs)
                                        lines.append(f"{cfgs}#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use {cmod}::{it.name};\n")

                    child_macros = list(anc_macros)
                    for mname, mcfgs in staying_macros.get(nid, []):
                        child_macros.append((cmod, mname, mcfgs))

                    # Emit moved side-exclusive macro_rules!
                    for _mit, mtxt in moved_macros.get((nid[0], nid[1], side), []):
                        lines.append("\n" + mtxt)

                    def emit_child_mod(ch: ModNode) -> None:
                        cfgs = "".join(f"{c}\n" for c in ch.cfg_attrs)
                        if ch.decl_kind == "out_of_line":
                            ch_file = mirror_file_path(ch.file_path, side)
                            ch_body = build_mirror(ch, child_macros)
                            write_if_changed(ch_file, ch_body, changed_rs)
                            new_files.add(ch_file.resolve())
                            rel_p = os.path.relpath(ch_file, mirror_file_path(node.file_path, side).parent).replace(os.sep, "/")
                            lines.append(f'\n{cfgs}#[path = "{rel_p}"]\npub(in crate::game) mod {ch.name};\n')
                        elif ch.decl_kind == "inline_include":
                            ch_file = mirror_file_path(ch.file_path, side)
                            ch_body = build_mirror(ch, child_macros)
                            write_if_changed(ch_file, ch_body, changed_rs)
                            new_files.add(ch_file.resolve())
                            rel_p = os.path.relpath(ch_file, mirror_file_path(node.file_path, side).parent).replace(os.sep, "/")
                            lines.append(f'\n{cfgs}pub(in crate::game) mod {ch.name} {{\n    include!("{rel_p}");\n}}\n')
                        elif ch.decl_kind == "inline":
                            ch_body = build_mirror(ch, child_macros)
                            lines.append(f"\n{cfgs}pub(in crate::game) mod {ch.name} {{\n{ch_body}}}\n")

                    # 1. Non-test child mirror mods first
                    for ch in active_children:
                        if not is_cfg_test_node(ch):
                            emit_child_mod(ch)

                    # 2. Moved functions and impl blocks
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
                        assert pimpl is not None
                        if pimpl.is_trait_impl:
                            tkey = (nid[0], nid[1], pimpl.impl_header, pimpl.cfg_attrs)
                            if tkey not in emitted_trait_impls:
                                emitted_trait_impls.add(tkey)
                                lines.append("\n" + trait_impl_source[tkey][0])
                        else:
                            methods_by_impl[(pimpl.impl_header, pimpl.cfg_attrs)].append(kk)

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
                            )
                        )

                    for (ihdr, icfgs), mkeys in sorted(methods_by_impl.items()):
                        ikey = (nid[0], nid[1], ihdr, icfgs)
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
                                )
                            )
                        lines.append("}\n")

                    # 3. #[cfg(test)] child mirror mods last (avoids clippy::items_after_test_module)
                    for ch in active_children:
                        if is_cfg_test_node(ch):
                            emit_child_mod(ch)

                    return "".join(lines)

                root_mirror_file = mirror_file_path(orig_tree.file_path, side)
                root_body = build_mirror(orig_tree, [])
                if not root_mirror_file.is_file():
                    files_created += 1
                write_if_changed(root_mirror_file, root_body, changed_rs)
                new_files.add(root_mirror_file.resolve())
                rel_from_game = root_mirror_file.relative_to(game_dir).as_posix()
                expected_side_decls[side][f"{top_name}_{side}"] = (rel_from_game, top_cfg)

            # Delete obsolete mirror files from previous runs
            for stale in sorted(old_mirror_files[side] - new_files):
                if stale.is_file():
                    stale.unlink()
                    files_deleted += 1

        # 2. Edit original tree (remove moved items, restore moved-back items, widen staying items)
        edits_by_file: dict[Path, list[tuple[int, int, str]]] = defaultdict(list)

        def edit_orig_node(node: ModNode, parent_split: bool) -> None:
            nid = (node.orig_mod, node_cfg_chain(node))
            ncfgs = nid[1]
            is_game_root = node.orig_mod == "game"
            node_split = node_has_side(node, "client") or node_has_side(node, "server")
            widen_super = (not is_game_root) and (node_split or parent_split)

            # Functions moving back from mirror to this orig node
            restore_free: list[str] = []
            restore_methods: dict[tuple[str, tuple[str, ...]], list[str]] = defaultdict(list)
            restore_trait_impls: list[str] = []
            restored_tkeys: set[tuple[str, tuple[str, ...], str, tuple[str, ...]]] = set()

            for side in ("client", "server"):
                for kk, (mn, _fp, _fsrc, it, pimpl) in mirror_fns[side].items():
                    if (mn.orig_mod, node_cfg_chain(mn)) != nid:
                        continue
                    if desired_side.get(kk, "game") == "game" and kk not in orig_fns:
                        if pimpl is None:
                            restore_free.append(fn_source_text[kk])
                        elif pimpl.is_trait_impl:
                            tkey = (nid[0], nid[1], pimpl.impl_header, pimpl.cfg_attrs)
                            if tkey not in restored_tkeys:
                                restored_tkeys.add(tkey)
                                restore_trait_impls.append(trait_impl_source[tkey][0])
                        else:
                            restore_methods[(pimpl.impl_header, pimpl.cfg_attrs)].append(fn_source_text[kk])

            moved_macro_names_here = {
                it.name
                for s in ("client", "server")
                for it, _ in moved_macros.get((nid[0], nid[1], s), [])
            }

            for fpath, fsrc, fitems in node_all_fragments(node):
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
                        kk = item_fn_key(node.orig_mod, it, None, ncfgs)
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
                            rep = rewrite_vis_to_game(it.vis)
                            if rep:
                                edits.append((it.vis_start, it.vis_end, f"{rep} " if it.vis == "" else rep))
                        elif widen_super and "super" in it.vis:
                            rep = rewrite_vis_to_game(it.vis)
                            if rep:
                                edits.append((it.vis_start, it.vis_end, rep))

                    elif it.kind == "impl":
                        if it.is_trait_impl:
                            sides = {desired_side.get(item_fn_key(node.orig_mod, ch, it, ncfgs), "game") for ch in it.children if ch.kind == "method"}
                            if len(sides) == 1 and next(iter(sides)) in ("client", "server"):
                                edits.append((it.start, it.end, ""))
                        else:
                            moved_ch = [ch for ch in it.children if ch.kind == "method" and desired_side.get(item_fn_key(node.orig_mod, ch, it, ncfgs), "game") in ("client", "server")]
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
                                                rep = rewrite_vis_to_game(ch.vis)
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
                        if rm:
                            cur_side = rm.group(2)
                            fname = rm.group(3)
                            want_sides = [
                                desired_side[kk]
                                for kk in top_rows
                                if kk.module == node.orig_mod and kk.impl == "-" and kk.name == fname and fn_node_id.get(kk) == nid
                            ]
                            want = want_sides[0] if want_sides else "game"
                            if want == "game":
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

            # Append any functions or macros restored from mirror back to orig
            rest_macros_here = restored_macros.get(nid, [])
            if rest_macros_here or restore_free or restore_methods or restore_trait_impls:
                chunks: list[str] = []
                for mtxt in rest_macros_here:
                    chunks.append("\n" + mtxt)
                for txt in restore_free:
                    chunks.append("\n" + txt)
                for (ihdr, icfgs), mlist in sorted(restore_methods.items()):
                    prefix = impl_header_prefix[(nid[0], nid[1], ihdr, icfgs)]
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
            old = path.read_text(encoding="utf-8")
            new = apply_edits(old, edits)
            if new != old:
                path.write_text(new, encoding="utf-8")
                changed_rs.append(path)

    # 3. Sync mirror module declarations in client_side.rs and server_side.rs
    for side in ("client", "server"):
        changed = sync_side_file_mirror_decls(root, side, expected_side_decls[side], regular_names)
        if changed is not None:
            changed_rs.append(changed)

    # 4. Rewrite test/parity/fn-replace/doc/Rust selectors
    sel_updates = selector_targets(root, rows, mod_sides)
    for path, new_txt in sel_updates:
        path.write_text(new_txt, encoding="utf-8")
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
    regular_mods, existing_mirrors = parse_top_mod_decls(root, set(mod_sides))
    expected_mirrors = mirror_modules_from_table(rows)

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
        _ps, entry_file, top_cfg = resolved
        orig_fns = collect_tree_functions(build_mod_tree(top_name, entry_file, top_cfg))
        mirror_fns: dict[str, dict[FnKey, tuple]] = {"client": {}, "server": {}}
        for side in ("client", "server"):
            mname = f"{top_name}_{side}"
            if mname in existing_mirrors[side]:
                mentry, mcfg = existing_mirrors[side][mname]
                mirror_fns[side] = collect_mirror_tree_functions(mentry, top_name, side, mcfg)
                # Check no root-scope pub(super) in mirror files
                for mf in sorted(collect_mirror_files(mentry)):
                    cnt, lns = rewrite_root_pub_super_check(mf.read_text(encoding="utf-8"))
                    if cnt > 0:
                        rel = mf.relative_to(root).as_posix()
                        problems.append(f"{rel}: {cnt} root-scope `pub(super)` item(s) remain (line(s) {', '.join(str(x) for x in lns[:5])})")

        top_rows = rows_by_top.get(top_name, {})
        # Fail closed on any unlisted function in mirror modules
        for side in ("client", "server"):
            for k in sorted(mirror_fns[side], key=lambda x: (x.module, x.impl, x.name, x.cfg)):
                if k not in top_rows:
                    problems.append(f"{SIDE_MOD_NAME[side]}::{top_name}_{side}: unlisted function `{k.module}::{k.name}`")
                elif top_rows[k].side != side:
                    problems.append(f"{SIDE_MOD_NAME[side]}::{top_name}_{side}: `{k.module}::{k.name}` has table side `{top_rows[k].side}`")

        # Verify every row in table sits in its exact target location
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

    for path, _ in selector_targets(root, rows, mod_sides):
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
