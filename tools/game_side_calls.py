#!/usr/bin/env python3
"""Enforce side-private visibility on functions inside `client_side` and `server_side` modules.

Every inherent method, associated function, and free function defined inside a module of the
`client_side` or `server_side` tree (per `tools/game_module_sides.tsv`, mirror modules included,
including `#[path]`, `include!`, and multi-arm `#[cfg]` modules; excluding the six container
`::new` constructors inside `client_side.rs` and `server_side.rs`, which `game_module_sides.py`
governs) must be visible at most within its own side (`pub(in crate::game::client_side)` or
`pub(in crate::game::server_side)`, or narrower), unless listed in `tools/game_side_calls.tsv`.

Trait impls (`impl Trait for ...`) and item-generating macro invocations in side modules cannot be
narrowed by visibility; each is recorded as a row of its own kind, and `--check` fails if an
unrecorded one appears.

Compiler-only invariant: this Python tool enforces syntax, visibility bounds, table integrity,
recorded trait impls / macro items, and the `--against` identity ratchet, but it CANNOT prove
whether a listed function still has an outside caller. Pruning stale rows from
`tools/game_side_calls.tsv` requires compiler verification across all targets and feature
configurations (`default`, `--no-default-features`, `--features state-digest`, workspace clippy).

Usage:
    python3 tools/game_side_calls.py --check [--against REF [--pull-request-merge]]
    python3 tools/game_side_calls.py --fix [--no-fmt]
    python3 tools/game_side_calls.py --list
"""

from __future__ import annotations

import argparse
import csv
import importlib.util
import os
import re
import shutil
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_TABLE = Path(__file__).resolve().with_name("game_side_calls.tsv")
DEFAULT_MOD_TABLE = Path(__file__).resolve().with_name("game_module_sides.tsv")


def _load_game_fn_sides():
    existing = sys.modules.get("game_fn_sides")
    if existing is not None:
        return existing
    gfs_path = Path(__file__).resolve().with_name("game_fn_sides.py")
    spec = importlib.util.spec_from_file_location("game_fn_sides", gfs_path)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    sys.modules["game_fn_sides"] = mod
    spec.loader.exec_module(mod)
    return mod


gfs = _load_game_fn_sides()

SIDE_MOD_NAME = {"client": "client_side", "server": "server_side"}
VALID_SIDES = ("client", "server")
VALID_KINDS = ("fn", "macro_invoc", "macro_method", "method", "trait_impl")
FN_LIKE_KINDS = frozenset({"fn", "macro_method", "method"})
CALLER_ORDER = ("client", "server", "game", "generated", "test")
VALID_CALLERS = frozenset(CALLER_ORDER)
LINT_ATTRS = frozenset({"allow", "expect", "warn", "deny"})
IGNORED_MOD_MACROS = frozenset({"thread_local", "std::thread_local"})
TABLE_COLUMNS = ("side", "module", "impl", "name", "cfg", "kind", "callers", "reason")


@dataclass(frozen=True)
class CallKey:
    """Canonical identity of a side function, trait impl, or macro item."""

    side: str
    module: str
    impl: str
    name: str
    cfg: str
    kind: str

    def sort_tuple(self) -> tuple[str, str, str, str, str, str]:
        return (self.side, self.module, self.kind, self.impl, self.name, self.cfg)


@dataclass(frozen=True)
class CallRow:
    key: CallKey
    callers: tuple[str, ...]  # empty tuple when "-"
    reason: str


@dataclass
class FoundItem:
    key: CallKey
    file_path: Path
    line: int
    vis: str
    vis_class: str  # "wide" | "narrow" | "na"
    vis_start: int
    vis_end: int
    depth: int


def norm_vis(vis: str) -> str:
    return re.sub(r"\s+", " ", vis).replace("( ", "(").replace(" )", ")").strip()


def target_side_vis(side: str) -> str:
    return f"pub(in crate::game::{SIDE_MOD_NAME[side]})"


def _rel_str(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def parse_call_table_text(text: str, source_label: str) -> tuple[dict[CallKey, CallRow], list[str]]:
    """Parse `game_side_calls.tsv` content and fail closed on schema, truncation, order, or duplicate errors."""
    rows: dict[CallKey, CallRow] = {}
    errors: list[str] = []
    reader = csv.reader(text.splitlines(), delimiter="\t")
    header = next(reader, None)
    if not header:
        raise SystemExit(f"{source_label}: missing header; expected columns {list(TABLE_COLUMNS)}")
    norm_header = [h.strip() for h in header]
    if tuple(norm_header) != TABLE_COLUMNS:
        raise SystemExit(f"{source_label}: expected columns {list(TABLE_COLUMNS)}, got {norm_header}")

    prev_sort: tuple[str, str, str, str, str, str] | None = None
    prev_line = 1
    for lineno, raw_row in enumerate(reader, 2):
        if not any(cell.strip() for cell in raw_row):
            continue
        if len(raw_row) != len(TABLE_COLUMNS):
            raise SystemExit(
                f"{source_label}:{lineno}: expected {len(TABLE_COLUMNS)} tab-separated columns, got {len(raw_row)}"
            )
        side, mod, impl, name, cfg, kind, callers_raw, reason = (c.strip() for c in raw_row)
        if side not in VALID_SIDES:
            raise SystemExit(f"{source_label}:{lineno}: invalid side {side!r}")
        if not mod or not impl or not name or not cfg or not reason:
            raise SystemExit(f"{source_label}:{lineno}: empty required field in row {raw_row!r}")
        if kind not in VALID_KINDS:
            raise SystemExit(f"{source_label}:{lineno}: invalid kind {kind!r}")
        if kind == "trait_impl":
            if name != "-" or callers_raw != "-":
                raise SystemExit(f"{source_label}:{lineno}: trait_impl row must have name='-' and callers='-'")
            callers: tuple[str, ...] = ()
        elif kind == "macro_invoc":
            if callers_raw != "-":
                raise SystemExit(f"{source_label}:{lineno}: macro_invoc row must have callers='-'")
            callers = ()
        else:
            if callers_raw == "-":
                raise SystemExit(f"{source_label}:{lineno}: {kind} row must list non-empty callers")
            parts = [c.strip() for c in callers_raw.split(",") if c.strip()]
            if not parts or len(parts) != len(callers_raw.split(",")):
                raise SystemExit(f"{source_label}:{lineno}: malformed callers {callers_raw!r}")
            bad = [c for c in parts if c not in VALID_CALLERS]
            if bad:
                raise SystemExit(f"{source_label}:{lineno}: unknown caller kind(s) {bad!r}")
            if len(set(parts)) != len(parts):
                raise SystemExit(f"{source_label}:{lineno}: duplicate caller kind in {callers_raw!r}")
            canon_parts = tuple(c for c in CALLER_ORDER if c in parts)
            if tuple(parts) != canon_parts:
                raise SystemExit(
                    f"{source_label}:{lineno}: callers {callers_raw!r} not in canonical order {','.join(canon_parts)!r}"
                )
            if set(canon_parts) == {side}:
                raise SystemExit(
                    f"{source_label}:{lineno}: {side} {kind} `{mod}::{name}` cannot list only own-side caller `{side}`"
                )
            callers = canon_parts

        key = CallKey(side=side, module=mod, impl=impl, name=name, cfg=cfg, kind=kind)
        if key in rows:
            raise SystemExit(f"{source_label}:{lineno}: duplicate row identity {key}")
        sk = key.sort_tuple()
        if prev_sort is not None and sk < prev_sort:
            errors.append(
                f"{source_label}:{lineno}: table is not canonically sorted (`{side}/{mod}/{kind}/{name}` after line {prev_line})"
            )
        prev_sort = sk
        prev_line = lineno
        rows[key] = CallRow(key=key, callers=callers, reason=reason)

    if not rows:
        raise SystemExit(f"{source_label}: table is empty (0 rows)")
    return rows, errors


def load_call_table(table_path: Path) -> tuple[dict[CallKey, CallRow], list[str]]:
    if not table_path.is_file():
        raise SystemExit(f"{table_path}: table file does not exist")
    return parse_call_table_text(table_path.read_text(encoding="utf-8"), str(table_path))


def format_call_table(rows: list[CallRow]) -> str:
    """Serialize `CallRow` records in canonical sorted TSV order."""
    out = ["\t".join(TABLE_COLUMNS) + "\n"]
    for r in sorted(rows, key=lambda x: x.key.sort_tuple()):
        callers_s = ",".join(r.callers) if r.callers else "-"
        out.append(
            f"{r.key.side}\t{r.key.module}\t{r.key.impl}\t{r.key.name}\t{r.key.cfg}\t{r.key.kind}\t{callers_s}\t{r.reason}\n"
        )
    return "".join(out)


def classify_vis(vis: str, side: str, depth: int) -> str:
    """Classify normalized Rust visibility at `depth` under `{client,server}_side` as 'narrow', 'wide', or 'refuse'."""
    nv = norm_vis(vis)
    side_mod = SIDE_MOD_NAME[side]
    if nv in ("", "pub(self)", "pub(in self)"):
        return "narrow"
    if nv in ("pub(super)", "pub(in super)"):
        return "narrow"
    if nv in ("pub", "pub(crate)", "pub(in crate)", "pub(in crate::game)"):
        return "wide"
    m = re.fullmatch(r"pub\(in\s+(.+)\)", nv)
    if not m:
        return "refuse"
    path = re.sub(r"\s+", "", m.group(1))
    if path == f"crate::game::{side_mod}" or path.startswith(f"crate::game::{side_mod}::"):
        return "narrow"
    if path == "self" or path.startswith("self::"):
        return "narrow"
    if path in ("crate", "crate::game"):
        return "wide"
    segs = path.split("::")
    if segs and all(s == "super" for s in segs):
        return "narrow" if len(segs) <= depth + 1 else "wide"
    if segs and segs[0] == "super":
        super_cnt = 0
        while super_cnt < len(segs) and segs[super_cnt] == "super":
            super_cnt += 1
        # At `depth`, `depth + 1` supers reach `crate::game::{side_mod}`.
        if super_cnt <= depth + 1:
            return "narrow"
        if super_cnt == depth + 2 and len(segs) > super_cnt and segs[super_cnt] == side_mod:
            return "narrow"
        return "wide"
    return "refuse"


def is_benign_lint_cfg_attr(attr_text: str) -> bool:
    """Return True only for `#[cfg_attr(<cond>, allow|expect|warn|deny(...))]` with pure lint inner attrs."""
    try:
        toks = gfs.lex_rust(attr_text)
    except ValueError:
        return False
    # Expect `# [ [core|std ::] cfg_attr ( ... ) ]`
    i = 0
    if len(toks) < 6 or toks[0].text != "#" or toks[1].text != "[":
        return False
    i = 2
    path_parts = []
    while i < len(toks) and (toks[i].kind == "ident" or toks[i].text == "::"):
        path_parts.append(toks[i].text)
        i += 1
    if "".join(path_parts) not in ("cfg_attr", "core::cfg_attr", "std::cfg_attr"):
        return False
    if i >= len(toks) or toks[i].text != "(":
        return False
    end_paren = gfs.skip_delim(toks, i, "(", ")")
    if end_paren >= len(toks) or toks[end_paren].text != "]":
        return False
    inner = toks[i + 1 : end_paren - 1]
    # Split inner on top-level commas; element 0 is cfg predicate, elements 1.. are applied attrs
    parts: list[list[gfs.Token]] = []
    cur: list[gfs.Token] = []
    depth = 0
    for tk in inner:
        if tk.text in ("(", "[", "{"):
            depth += 1
            cur.append(tk)
        elif tk.text in (")", "]", "}"):
            depth -= 1
            cur.append(tk)
        elif tk.text == "," and depth == 0:
            parts.append(cur)
            cur = []
        else:
            cur.append(tk)
    if cur:
        parts.append(cur)
    if len(parts) < 2:
        return False
    for sub in parts[1:]:
        if not sub or sub[0].kind != "ident" or sub[0].text not in LINT_ATTRS:
            return False
    return True


def check_attrs_cfg_attr(
    attrs: tuple[str, ...], fpath: Path, line: int, label: str, errors: list[str], root: Path
) -> None:
    for raw in attrs:
        head = gfs.attr_head_name(raw)
        if head in ("cfg_attr", "core::cfg_attr", "std::cfg_attr") and not is_benign_lint_cfg_attr(raw):
            norm = re.sub(r"\s+", " ", raw).strip()
            errors.append(f"{_rel_str(fpath, root)}:{line}: unsupported `#[cfg_attr(...)]` on {label}: `{norm}`")


def check_macro_visibility_in_tokens(
    src: str, fpath: Path, root: Path, errors: list[str]
) -> None:
    """Fail closed if a macro invocation directly precedes `fn` (visibility written through a macro)."""
    try:
        toks = gfs.lex_rust(src)
    except ValueError:
        return
    n = len(toks)
    i = 0
    while i < n:
        # Skip `macro_rules! name { ... }` bodies (checked separately by `scan_macro_rules_fns`)
        if toks[i].kind == "ident" and toks[i].text == "macro_rules" and i + 2 < n and toks[i + 1].text == "!":
            k = i + 3
            if k < n and toks[k].text in ("(", "[", "{"):
                cl = {"(": ")", "[": "]", "{": "}"}[toks[k].text]
                i = gfs.skip_delim(toks, k, toks[k].text, cl)
                continue
        if toks[i].kind == "ident" and i + 2 < n and toks[i + 1].text == "!" and toks[i + 2].text in ("(", "["):
            cl = ")" if toks[i + 2].text == "(" else "]"
            after = gfs.skip_delim(toks, i + 2, toks[i + 2].text, cl)
            # Check if immediately followed by optional qualifiers and `fn <ident>` (without `;` between)
            q = after
            while q < n and toks[q].kind == "ident" and toks[q].text in ("default", "unsafe", "async", "const", "extern"):
                if toks[q].text == "extern" and q + 1 < n and toks[q + 1].kind == "str":
                    q += 2
                else:
                    q += 1
            if q + 1 < n and toks[q].kind == "ident" and toks[q].text == "fn" and toks[q + 1].kind == "ident":
                # Exclude `macro!(...)` statement not ending with `;`? A `macro!(...)` without `;` before `fn` is invalid unless it expands to visibility/attrs!
                errors.append(
                    f"{_rel_str(fpath, root)}:{toks[i].line}: visibility or prefix written through macro `{toks[i].text}!` before `fn {toks[q + 1].text}`"
                )
            i = after
            continue
        i += 1


def scan_macro_rules_fns(
    m_item: gfs.ParsedItem,
    fsrc: str,
    fpath: Path,
    side: str,
    mod_rel: str,
    depth: int,
    eff_cfgs: tuple[str, ...],
    root: Path,
    found: dict[CallKey, FoundItem],
    errors: list[str],
) -> None:
    """Inspect a `macro_rules!` body for generated `fn` items; index literal wide fns and fail closed on dynamic vis/names."""
    body = fsrc[m_item.kw_start : m_item.end]
    base_off = m_item.kw_start
    try:
        toks = gfs.lex_rust(body)
    except ValueError as exc:
        errors.append(f"{_rel_str(fpath, root)}:{m_item.start_line}: macro `{m_item.name}` lex error: {exc}")
        return
    lstarts = gfs.line_starts(fsrc)
    n = len(toks)
    i = 0
    while i < n:
        if toks[i].kind == "ident" and toks[i].text == "fn" and i + 1 < n:
            # Skip `fn(...)` function pointer types (not a function definition)
            if toks[i + 1].text == "(":
                i += 1
                continue
            # Walk back over optional qualifiers (`unsafe`, `async`, `const`, `extern "..."`)
            q = i - 1
            while q >= 0:
                if toks[q].kind == "str" and q >= 1 and toks[q - 1].kind == "ident" and toks[q - 1].text == "extern":
                    q -= 2
                    continue
                if toks[q].kind == "ident" and toks[q].text in ("unsafe", "async", "const", "extern", "default"):
                    q -= 1
                    continue
                break
            # Check visibility right before `q`
            vis_str = ""
            vs = ve = 0
            has_meta_vis = False
            if q >= 0 and toks[q].kind == "ident" and toks[q].text == "pub":
                vs = base_off + toks[q].start
                ve = base_off + toks[q].end
                vis_str = fsrc[vs:ve]
            elif q >= 2 and toks[q].text == ")":
                # Find matching `(`
                d = 1
                p = q - 1
                while p >= 0 and d > 0:
                    if toks[p].text == ")":
                        d += 1
                    elif toks[p].text == "(":
                        d -= 1
                    p -= 1
                if p >= 0 and toks[p].kind == "ident" and toks[p].text == "pub":
                    vs = base_off + toks[p].start
                    ve = base_off + toks[q].end
                    vis_str = fsrc[vs:ve]
            elif q >= 1 and toks[q - 1].text == "$" and toks[q].kind == "ident":
                # e.g. `$vis fn ...`
                has_meta_vis = True
            elif q >= 2 and toks[q].text in ("*", "+", "?") and toks[q - 1].text == ")":
                # e.g. `$($vis)* fn ...`
                d = 1
                p = q - 2
                while p >= 0 and d > 0:
                    if toks[p].text == ")":
                        d += 1
                    elif toks[p].text == "(":
                        d -= 1
                    p -= 1
                if p >= 0 and toks[p].text == "$":
                    rep_txt = body[toks[p].start : toks[q].end]
                    if "vis" in rep_txt or "pub" in rep_txt:
                        has_meta_vis = True

            fn_line = gfs.offset_to_line(lstarts, base_off + toks[i].start)
            if has_meta_vis:
                errors.append(
                    f"{_rel_str(fpath, root)}:{fn_line}: macro_rules! `{m_item.name}` generates `fn` with metavariable visibility"
                )
                i += 1
                continue

            vclass = classify_vis(vis_str, side, depth) if vis_str else "narrow"
            if vclass == "refuse":
                errors.append(
                    f"{_rel_str(fpath, root)}:{fn_line}: macro_rules! `{m_item.name}` has unsupported visibility `{vis_str}`"
                )
                i += 1
                continue

            if toks[i + 1].text == "$" and i + 2 < n and toks[i + 2].kind == "ident":
                if vclass == "wide":
                    errors.append(
                        f"{_rel_str(fpath, root)}:{fn_line}: macro_rules! `{m_item.name}` generates wide `fn ${toks[i + 2].text}` with dynamic name"
                    )
                i += 1
                continue

            if toks[i + 1].kind == "ident":
                fn_name = toks[i + 1].text
                cfg_s = "; ".join((*eff_cfgs, *m_item.cfg_attrs)) or "-"
                key = CallKey(
                    side=side,
                    module=mod_rel,
                    impl=f"macro_rules! {m_item.name}",
                    name=fn_name,
                    cfg=cfg_s,
                    kind="macro_method",
                )
                if key in found:
                    errors.append(
                        f"{_rel_str(fpath, root)}:{fn_line}: duplicate macro_method `{m_item.name}::{fn_name}`"
                    )
                else:
                    found[key] = FoundItem(
                        key=key,
                        file_path=fpath,
                        line=fn_line,
                        vis=norm_vis(vis_str),
                        vis_class=vclass,
                        vis_start=vs,
                        vis_end=ve,
                        depth=depth,
                    )
        i += 1


def resolve_top_entry(src_root: Path, game_dir: Path, it: gfs.ParsedItem) -> Path:
    if it.path_attr:
        if it.path_attr.startswith("game/"):
            return (src_root / it.path_attr).resolve()
        cand = (game_dir / it.path_attr).resolve()
        if cand.is_file():
            return cand
        return (src_root / it.path_attr).resolve()
    c1 = game_dir / f"{it.name}.rs"
    c2 = game_dir / it.name / "mod.rs"
    return (c1 if c1.is_file() else c2).resolve()


def collect_side_scope(
    root: Path, mod_sides: dict[str, str], errors: list[str]
) -> tuple[dict[CallKey, FoundItem], dict[str, dict[str, Path]]]:
    """Walk every module under `client_side.rs` and `server_side.rs` (including multi-arm cfgs & mirrors)."""
    src_root = (root / "crates/sunrise-client/src").resolve()
    game_dir = src_root / "game"
    found: dict[CallKey, FoundItem] = {}
    top_entries: dict[str, dict[str, Path]] = {"client": {}, "server": {}}
    seen_macro_invocs: set[CallKey] = set()

    for side in VALID_SIDES:
        side_file = game_dir / f"{SIDE_MOD_NAME[side]}.rs"
        if not side_file.is_file():
            errors.append(f"{_rel_str(side_file, root)}: missing side container file")
            continue
        try:
            side_items = gfs.parse_rust_items(side_file.read_text(encoding="utf-8"), side_file)
        except ValueError as exc:
            errors.append(f"{_rel_str(side_file, root)}: parse error: {exc}")
            continue

        declared_mods: set[str] = set()
        seen_top_arms: set[tuple[str, tuple[str, ...]]] = set()
        for it in side_items:
            if it.kind in ("mod_inline", "mod_include"):
                errors.append(
                    f"{_rel_str(side_file, root)}:{it.start_line}: unsupported `{it.kind}` `{it.name}` in side container"
                )
                continue
            if it.kind == "fn":
                errors.append(
                    f"{_rel_str(side_file, root)}:{it.start_line}: unexpected free `fn {it.name}` in side container"
                )
                continue
            if it.kind == "impl":
                if it.is_trait_impl:
                    errors.append(
                        f"{_rel_str(side_file, root)}:{it.start_line}: unexpected trait impl `{it.impl_header}` in side container"
                    )
                    continue
                for ch in it.children:
                    if ch.kind == "method" and ch.name != "new":
                        errors.append(
                            f"{_rel_str(side_file, root)}:{ch.start_line}: unexpected method `{it.impl_header}::{ch.name}` in side container"
                        )
                continue
            if it.kind != "mod_ext" or it.name in gfs.SIDE_CONTAINERS:
                continue
            check_attrs_cfg_attr(it.attrs, side_file, it.start_line, f"module `{it.name}`", errors, root)
            arm_key = (it.name, it.cfg_attrs)
            if arm_key in seen_top_arms:
                errors.append(
                    f"{_rel_str(side_file, root)}:{it.start_line}: duplicate top-level module arm `{it.name}` ({'; '.join(it.cfg_attrs) or '-'})"
                )
                continue
            seen_top_arms.add(arm_key)
            entry = resolve_top_entry(src_root, game_dir, it)
            if not entry.is_file():
                errors.append(
                    f"{_rel_str(side_file, root)}:{it.start_line}: module `{it.name}` entry `{it.path_attr or entry.name}` does not exist"
                )
                continue
            declared_mods.add(it.name)
            top_entries[side][it.name] = entry
            tree = gfs.build_mod_tree(it.name, entry, it.cfg_attrs, errors=errors, root=root)
            _walk_side_node(tree, side, root, found, seen_macro_invocs, errors)

        for mname, mside in sorted(mod_sides.items()):
            if mside == side and mname not in declared_mods:
                errors.append(
                    f"{_rel_str(side_file, root)}: module `{mname}` listed as `{side}` in tools/game_module_sides.tsv is not declared"
                )

    return found, top_entries


def _walk_side_node(
    node: gfs.ModNode,
    side: str,
    root: Path,
    found: dict[CallKey, FoundItem],
    seen_macro_invocs: set[CallKey],
    errors: list[str],
) -> None:
    mod_rel = node.orig_mod
    depth = mod_rel.count("::")
    ncfgs = gfs.node_cfg_chain(node)

    for fpath, fsrc, fitems, fcfgs in gfs.node_all_fragments(node):
        if node.decl_kind != "inline":
            check_macro_visibility_in_tokens(fsrc, fpath, root, errors)
        eff_ncfgs = (*ncfgs, *fcfgs)
        for it in fitems:
            if it.kind == "fn":
                check_attrs_cfg_attr(it.attrs, fpath, it.start_line, f"fn `{mod_rel}::{it.name}`", errors, root)
                vclass = classify_vis(it.vis, side, depth)
                if vclass == "refuse":
                    errors.append(
                        f"{_rel_str(fpath, root)}:{it.start_line}: unsupported visibility `{it.vis}` on fn `{mod_rel}::{it.name}`"
                    )
                    continue
                cfg_s = "; ".join((*eff_ncfgs, *it.cfg_attrs)) or "-"
                key = CallKey(side=side, module=mod_rel, impl="-", name=it.name, cfg=cfg_s, kind="fn")
                if key in found:
                    prev = found[key]
                    errors.append(
                        f"duplicate fn `{side}::{mod_rel}::{it.name}` ({cfg_s}) at "
                        f"{_rel_str(prev.file_path, root)}:{prev.line} and {_rel_str(fpath, root)}:{it.start_line}"
                    )
                else:
                    found[key] = FoundItem(
                        key=key,
                        file_path=fpath,
                        line=it.start_line,
                        vis=norm_vis(it.vis),
                        vis_class=vclass,
                        vis_start=it.vis_start,
                        vis_end=it.vis_end,
                        depth=depth,
                    )
            elif it.kind == "impl":
                check_attrs_cfg_attr(it.attrs, fpath, it.start_line, f"impl `{it.impl_header}`", errors, root)
                impl_cfgs = (*eff_ncfgs, *it.cfg_attrs)
                if it.is_trait_impl:
                    cfg_s = "; ".join(impl_cfgs) or "-"
                    key = CallKey(side=side, module=mod_rel, impl=it.impl_header, name="-", cfg=cfg_s, kind="trait_impl")
                    if key in found:
                        prev = found[key]
                        errors.append(
                            f"duplicate trait_impl `{side}::{mod_rel}::{it.impl_header}` ({cfg_s}) at "
                            f"{_rel_str(prev.file_path, root)}:{prev.line} and {_rel_str(fpath, root)}:{it.start_line}"
                        )
                    else:
                        found[key] = FoundItem(
                            key=key,
                            file_path=fpath,
                            line=it.start_line,
                            vis="",
                            vis_class="na",
                            vis_start=it.start,
                            vis_end=it.start,
                            depth=depth,
                        )
                    continue

                for ch in it.children:
                    if ch.kind == "method":
                        check_attrs_cfg_attr(
                            ch.attrs, fpath, ch.start_line, f"method `{it.impl_header}::{ch.name}`", errors, root
                        )
                        vclass = classify_vis(ch.vis, side, depth)
                        if vclass == "refuse":
                            errors.append(
                                f"{_rel_str(fpath, root)}:{ch.start_line}: unsupported visibility `{ch.vis}` on `{it.impl_header}::{ch.name}`"
                            )
                            continue
                        cfg_s = "; ".join((*impl_cfgs, *ch.cfg_attrs)) or "-"
                        key = CallKey(
                            side=side,
                            module=mod_rel,
                            impl=it.impl_header,
                            name=ch.name,
                            cfg=cfg_s,
                            kind="method",
                        )
                        if key in found:
                            prev = found[key]
                            errors.append(
                                f"duplicate method `{side}::{mod_rel}::{it.impl_header}::{ch.name}` ({cfg_s}) at "
                                f"{_rel_str(prev.file_path, root)}:{prev.line} and {_rel_str(fpath, root)}:{ch.start_line}"
                            )
                        else:
                            found[key] = FoundItem(
                                key=key,
                                file_path=fpath,
                                line=ch.start_line,
                                vis=norm_vis(ch.vis),
                                vis_class=vclass,
                                vis_start=ch.vis_start,
                                vis_end=ch.vis_end,
                                depth=depth,
                            )
                    elif ch.kind == "macro_invoc":
                        cfg_s = "; ".join((*impl_cfgs, *ch.cfg_attrs)) or "-"
                        key = CallKey(
                            side=side,
                            module=mod_rel,
                            impl=it.impl_header,
                            name=ch.name,
                            cfg=cfg_s,
                            kind="macro_invoc",
                        )
                        if key not in seen_macro_invocs:
                            seen_macro_invocs.add(key)
                            found[key] = FoundItem(
                                key=key,
                                file_path=fpath,
                                line=ch.start_line,
                                vis="",
                                vis_class="na",
                                vis_start=ch.start,
                                vis_end=ch.start,
                                depth=depth,
                            )
                    elif ch.kind not in ("const", "type"):
                        errors.append(
                            f"{_rel_str(fpath, root)}:{ch.start_line}: unparseable item `{ch.kind} {ch.name}` inside `{it.impl_header}`"
                        )
            elif it.kind == "macro_rules":
                check_attrs_cfg_attr(it.attrs, fpath, it.start_line, f"macro_rules! `{it.name}`", errors, root)
                scan_macro_rules_fns(it, fsrc, fpath, side, mod_rel, depth, eff_ncfgs, root, found, errors)
            elif it.kind == "macro_invoc" and it.name not in IGNORED_MOD_MACROS:
                check_attrs_cfg_attr(it.attrs, fpath, it.start_line, f"macro `{it.name}!`", errors, root)
                cfg_s = "; ".join((*eff_ncfgs, *it.cfg_attrs)) or "-"
                key = CallKey(
                    side=side,
                    module=mod_rel,
                    impl="-",
                    name=it.name,
                    cfg=cfg_s,
                    kind="macro_invoc",
                )
                if key not in seen_macro_invocs:
                    seen_macro_invocs.add(key)
                    found[key] = FoundItem(
                        key=key,
                        file_path=fpath,
                        line=it.start_line,
                        vis="",
                        vis_class="na",
                        vis_start=it.start,
                        vis_end=it.start,
                        depth=depth,
                    )

    for ch_node in node.children:
        _walk_side_node(ch_node, side, root, found, seen_macro_invocs, errors)


def check_cfg_twins(
    found: dict[CallKey, FoundItem],
    rows: dict[CallKey, CallRow],
    root: Path,
    errors: list[str],
    *,
    for_fix: bool,
) -> None:
    """Fail closed if cfg-twin arms have differing visibility (or split table listing in --fix)."""
    groups: dict[tuple[str, str, str, str, str], list[FoundItem]] = defaultdict(list)
    for item in found.values():
        if item.key.kind not in ("method", "fn"):
            continue
        owner = gfs.owner_overlap_group(item.key.impl, False) if item.key.kind == "method" else "-"
        groups[(item.key.side, item.key.module, owner, item.key.name, item.key.kind)].append(item)

    for grp_key, items in sorted(groups.items()):
        if len(items) < 2:
            continue
        if for_fix:
            # Desired state after `--fix`: listed items stay wide (if wide), unlisted become narrow
            desired_wide = [it for it in items if it.key in rows and it.vis_class == "wide"]
            desired_narrow = [it for it in items if it.key not in rows or it.vis_class == "narrow"]
            if desired_wide and desired_narrow:
                desc = ", ".join(
                    f"{_rel_str(it.file_path, root)}:{it.line} ({it.key.cfg}, listed={it.key in rows}, vis={it.vis or 'private'})"
                    for it in sorted(items, key=lambda x: (x.file_path.as_posix(), x.line))
                )
                errors.append(
                    f"cfg-twin split visibility for `{grp_key[0]}::{grp_key[1]}::{grp_key[3]}`: {desc}"
                )
        else:
            vises = {it.vis for it in items}
            classes = {it.vis_class for it in items}
            if len(vises) > 1 or len(classes) > 1:
                desc = ", ".join(
                    f"{_rel_str(it.file_path, root)}:{it.line} ({it.key.cfg}, vis={it.vis or 'private'})"
                    for it in sorted(items, key=lambda x: (x.file_path.as_posix(), x.line))
                )
                errors.append(
                    f"cfg-twin differing visibility for `{grp_key[0]}::{grp_key[1]}::{grp_key[3]}`: {desc}"
                )


def resolve_baseline_ref(root: Path, against: str, pull_request_merge: bool) -> str:
    """Resolve the exact baseline commit for `--against`, matching `tools/game_module_sides.py`."""
    if pull_request_merge:
        parents_proc = subprocess.run(
            ["git", "rev-list", "--parents", "-n", "1", "HEAD"],
            cwd=root,
            capture_output=True,
            text=True,
        )
        if parents_proc.returncode != 0:
            raise SystemExit(f"cannot inspect HEAD parents: {parents_proc.stderr.strip()}")
        parents = parents_proc.stdout.split()[1:]
        if len(parents) != 2:
            raise SystemExit("--pull-request-merge needs HEAD to merge the pull request into a descendant of --against.")
        anc = subprocess.run(
            ["git", "merge-base", "--is-ancestor", against, parents[0]],
            cwd=root,
            capture_output=True,
            text=True,
        )
        if anc.returncode != 0:
            raise SystemExit("--pull-request-merge needs HEAD to merge the pull request into a descendant of --against.")
        return parents[0]
    proc = subprocess.run(
        ["git", "merge-base", against, "HEAD"],
        cwd=root,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise SystemExit(f"cannot resolve --against {against!r}: {proc.stderr.strip()}")
    return proc.stdout.strip()


def git_text_at(root: Path, rev: str, rel_path: str) -> str | None:
    """Read `rel_path` at `rev`, or return None when absent; fail closed if present but unreadable."""
    listed = subprocess.run(
        ["git", "ls-tree", "--name-only", rev, "--", rel_path],
        cwd=root,
        capture_output=True,
        text=True,
    )
    if listed.returncode != 0:
        raise SystemExit(f"cannot list {rel_path} at {rev}: {listed.stderr.strip()}")
    if rel_path not in listed.stdout.splitlines():
        return None
    shown = subprocess.run(["git", "show", f"{rev}:{rel_path}"], cwd=root, capture_output=True, text=True)
    if shown.returncode != 0:
        raise SystemExit(f"{rel_path} exists at {rev} but cannot be read: {shown.stderr.strip()}")
    return shown.stdout


def check_baseline_ratchet(
    root: Path, base_rev: str, table_rel: str, rows: dict[CallKey, CallRow]
) -> tuple[list[str], bool]:
    """Fail when `rows` contains any key absent from `table_rel` at `base_rev`. Returns `(problems, had_baseline)`."""
    base_text = git_text_at(root, base_rev, table_rel)
    if base_text is None:
        return [], False
    short_rev = base_rev[:10]
    base_rows, _base_sort_errs = parse_call_table_text(base_text, f"{short_rev}:{table_rel}")
    problems: list[str] = []
    for key in sorted(rows, key=lambda k: k.sort_tuple()):
        if key not in base_rows:
            problems.append(
                f"{table_rel}: new exception `{key.side}::{key.module}::{key.impl}::{key.name}` ({key.kind}, {key.cfg}) "
                f"absent at {short_rev} (call-wall list may only shrink)"
            )
    return problems, True


def plan_generated_use_removals(
    root: Path, narrowed_free_fns: set[tuple[str, str, str]], planned_writes: dict[Path, str], errors: list[str]
) -> None:
    """Drop tool-generated single-item `#[allow(unused_imports)]` opposite-side mirror/orig `use` lines for narrowed free fns."""
    if not narrowed_free_fns:
        return
    target_side_by_path: dict[str, str] = {}
    for side, mod_rel, name in narrowed_free_fns:
        target_side_by_path[f"crate::game::{SIDE_MOD_NAME[side]}::{mod_rel}::{name}"] = side

    game_dir = root / "crates/sunrise-client/src/game"
    candidates = [root / "crates/sunrise-client/src/game.rs", *sorted(game_dir.rglob("*.rs"))]
    for p in candidates:
        if not p.is_file():
            continue
        rp = p.resolve()
        src = planned_writes.get(rp, rp.read_text(encoding="utf-8"))
        if "unused_imports" not in src or "crate::game::" not in src:
            continue
        try:
            items = gfs.parse_rust_items(src, rp)
        except ValueError as exc:
            errors.append(f"{_rel_str(rp, root)}: parse error while scanning generated uses: {exc}")
            continue
        edits: list[tuple[int, int, str]] = []

        def scan_uses(item_list: list[gfs.ParsedItem]) -> None:
            for it in item_list:
                if it.kind == "mod_inline":
                    scan_uses(it.children)
                    continue
                if it.kind != "use":
                    continue
                norm_target = re.sub(r"\s+", "", it.name)
                fn_side = target_side_by_path.get(norm_target)
                if fn_side is None:
                    continue
                if not any("unused_imports" in a for a in it.attrs):
                    continue
                nv = norm_vis(it.vis)
                opp_side = "server" if fn_side == "client" else "client"
                is_opp_mirror_use = nv == target_side_vis(opp_side)
                is_orig_reexp = nv == "pub(in crate::game)" and bool(
                    re.match(
                        rf"^crate::game::{SIDE_MOD_NAME[fn_side]}::[A-Za-z0-9_]+_{fn_side}::",
                        norm_target,
                    )
                )
                if is_opp_mirror_use or is_orig_reexp:
                    edits.append((it.start, it.end, ""))

        scan_uses(items)
        if edits:
            planned_writes[rp] = gfs.apply_edits(src, edits)


def resolve_rustfmt() -> str | None:
    env_rf = os.environ.get("RUSTFMT")
    if env_rf:
        return env_rf if Path(env_rf).is_file() or shutil.which(env_rf) else None
    return shutil.which("rustfmt")


def run_rustfmt(rustfmt: str, paths: list[Path]) -> None:
    rs_paths = sorted({p for p in paths if p.suffix == ".rs" and p.is_file()})
    for start in range(0, len(rs_paths), 100):
        subprocess.run(
            [rustfmt, "--edition", "2024", "--config", "skip_children=true", *(str(p) for p in rs_paths[start : start + 100])],
            check=True,
        )


def run_check(
    root: Path,
    table_path: Path,
    rows: dict[CallKey, CallRow],
    table_errors: list[str],
    mod_sides: dict[str, str],
    *,
    against: str | None,
    pull_request_merge: bool,
) -> int:
    problems = list(table_errors)
    found, _ = collect_side_scope(root, mod_sides, problems)
    check_cfg_twins(found, rows, root, problems, for_fix=False)

    # 1. Every wide fn/method/macro_method and every trait_impl/macro_invoc in scope must be listed
    for key in sorted(found, key=lambda k: k.sort_tuple()):
        item = found[key]
        rel = _rel_str(item.file_path, root)
        if key.kind in FN_LIKE_KINDS:
            if item.vis_class == "wide" and key not in rows:
                problems.append(
                    f"{rel}:{item.line}: unlisted wide `{key.side}` {key.kind} `{key.module}::{key.impl}::{key.name}` "
                    f"(`{item.vis}`); narrow to `{target_side_vis(key.side)}` or list in tools/game_side_calls.tsv"
                )
        elif key.kind in ("trait_impl", "macro_invoc"):
            if key not in rows:
                problems.append(
                    f"{rel}:{item.line}: unrecorded `{key.side}` {key.kind} `{key.module}::{key.impl}::{key.name}` ({key.cfg})"
                )

    # 2. Every row in the table must match an existing item (and for fn-like rows, must still be wide)
    for key in sorted(rows, key=lambda k: k.sort_tuple()):
        if key not in found:
            problems.append(
                f"tools/game_side_calls.tsv: orphan {key.kind} row `{key.side}::{key.module}::{key.impl}::{key.name}` ({key.cfg}) not found in source"
            )
            continue
        item = found[key]
        if key.kind in FN_LIKE_KINDS and item.vis_class != "wide":
            rel = _rel_str(item.file_path, root)
            problems.append(
                f"{rel}:{item.line}: listed {key.kind} `{key.side}::{key.module}::{key.name}` is already `{item.vis or 'private'}` "
                "(remove its row from tools/game_side_calls.tsv)"
            )

    # 3. Optional baseline identity ratchet
    ratchet_note = ""
    if against:
        base_rev = resolve_baseline_ref(root, against, pull_request_merge)
        try:
            table_rel = table_path.resolve().relative_to(root.resolve()).as_posix()
        except ValueError:
            table_rel = "tools/game_side_calls.tsv"
        r_probs, had_base = check_baseline_ratchet(root, base_rev, table_rel, rows)
        problems.extend(r_probs)
        ratchet_note = f"; ratchet vs {base_rev[:10]}" if had_base else f"; no baseline table at {base_rev[:10]}"

    for p in problems:
        print(f"error: {p}", file=sys.stderr)
    if problems:
        print(f"Found {len(problems)} side-call problem(s).", file=sys.stderr)
        return 1

    fn_rows = sum(1 for k in rows if k.kind in FN_LIKE_KINDS)
    ti_rows = sum(1 for k in rows if k.kind == "trait_impl")
    mi_rows = sum(1 for k in rows if k.kind == "macro_invoc")
    in_scope_fns = sum(1 for k in found if k.kind in FN_LIKE_KINDS)
    print(
        f"OK: {in_scope_fns} in-scope side functions verified "
        f"({fn_rows} listed wide, {ti_rows} trait_impls, {mi_rows} macro_invocs{ratchet_note})."
    )
    return 0


def run_fix(
    root: Path,
    rows: dict[CallKey, CallRow],
    table_errors: list[str],
    mod_sides: dict[str, str],
    *,
    fmt: bool,
) -> int:
    errors = list(table_errors)
    found, _ = collect_side_scope(root, mod_sides, errors)
    check_cfg_twins(found, rows, root, errors, for_fix=True)

    # Refuse on unrecorded trait_impls / macro_invocs and orphan rows
    for key in sorted(found, key=lambda k: k.sort_tuple()):
        item = found[key]
        if key.kind in ("trait_impl", "macro_invoc") and key not in rows:
            errors.append(
                f"{_rel_str(item.file_path, root)}:{item.line}: unrecorded `{key.side}` {key.kind} `{key.module}::{key.impl}::{key.name}` ({key.cfg})"
            )
    for key in sorted(rows, key=lambda k: k.sort_tuple()):
        if key not in found:
            errors.append(
                f"tools/game_side_calls.tsv: orphan {key.kind} row `{key.side}::{key.module}::{key.impl}::{key.name}` ({key.cfg}) not found in source"
            )
        elif key.kind in FN_LIKE_KINDS and found[key].vis_class != "wide":
            item = found[key]
            errors.append(
                f"{_rel_str(item.file_path, root)}:{item.line}: listed {key.kind} `{key.side}::{key.module}::{key.name}` is already `{item.vis or 'private'}`"
            )

    # Stage visibility narrowing edits in memory
    edits_by_file: dict[Path, list[tuple[int, int, str]]] = defaultdict(list)
    narrowed_free_fns: set[tuple[str, str, str]] = set()
    narrowed_counts = {"client": 0, "server": 0}

    for key in sorted(found, key=lambda k: k.sort_tuple()):
        if key.kind not in FN_LIKE_KINDS:
            continue
        item = found[key]
        if item.vis_class != "wide":
            continue
        if key in rows:
            continue
        tv = target_side_vis(key.side)
        edits_by_file[item.file_path.resolve()].append((item.vis_start, item.vis_end, tv))
        narrowed_counts[key.side] += 1
        if key.kind == "fn":
            narrowed_free_fns.add((key.side, key.module, key.name))

    planned_writes: dict[Path, str] = {}
    for rp, edits in sorted(edits_by_file.items()):
        old_src = rp.read_text(encoding="utf-8")
        try:
            new_src = gfs.apply_edits(old_src, edits)
        except RuntimeError as exc:
            errors.append(f"{_rel_str(rp, root)}: {exc}")
            continue
        if new_src != old_src:
            planned_writes[rp] = new_src

    if not errors:
        plan_generated_use_removals(root, narrowed_free_fns, planned_writes, errors)

    rustfmt_bin: str | None = None
    if fmt and not errors and any(p.suffix == ".rs" for p in planned_writes):
        rustfmt_bin = resolve_rustfmt()
        if not rustfmt_bin:
            errors.append("rustfmt not found (pass --no-fmt to skip formatting)")

    if errors:
        for e in errors:
            print(f"error: {e}", file=sys.stderr)
        print(f"Refused --fix: {len(errors)} error(s); no files modified.", file=sys.stderr)
        return 1

    changed_rs: list[Path] = []
    for rp, new_src in sorted(planned_writes.items()):
        if not rp.is_file() or rp.read_text(encoding="utf-8") != new_src:
            rp.write_text(new_src, encoding="utf-8")
            if rp.suffix == ".rs":
                changed_rs.append(rp)

    if rustfmt_bin and changed_rs:
        run_rustfmt(rustfmt_bin, changed_rs)

    print(
        f"Applied side-call narrowing: {narrowed_counts['client']} client, {narrowed_counts['server']} server "
        f"across {len(changed_rs)} file(s)."
    )
    return 0


def run_list(rows: dict[CallKey, CallRow]) -> int:
    sys.stdout.write(format_call_table(list(rows.values())))
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Enforce side-private visibility on functions in sunrise-client client_side/server_side modules. "
            "Note: only the compiler can verify whether a listed function still has an outside caller; "
            "this tool checks syntax, visibility bounds, table integrity, trait impls, and --against ratchets."
        ),
    )
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="verify side visibility and tools/game_side_calls.tsv (fail closed)")
    mode.add_argument("--fix", action="store_true", help="narrow every unlisted wide function in side modules; never widen")
    mode.add_argument("--list", action="store_true", help="print canonical tools/game_side_calls.tsv rows")
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root directory")
    parser.add_argument("--table", type=Path, default=None, help="path to game_side_calls.tsv")
    parser.add_argument("--module-table", type=Path, default=None, help="path to game_module_sides.tsv")
    parser.add_argument("--against", default=None, help="baseline git ref for identity ratchet (list may only shrink)")
    parser.add_argument(
        "--pull-request-merge",
        action="store_true",
        help="use HEAD's first parent (verified descendant of --against) as baseline, matching CI merge commits",
    )
    parser.add_argument("--no-fmt", action="store_true", help="skip rustfmt on modified Rust files")
    args = parser.parse_args(argv)

    if args.pull_request_merge and not args.against:
        parser.error("--pull-request-merge requires --against")

    root = args.root.resolve()
    table_path = args.table or (root / "tools/game_side_calls.tsv" if args.root != ROOT else DEFAULT_TABLE)
    mod_table_path = args.module_table or (root / "tools/game_module_sides.tsv" if args.root != ROOT else DEFAULT_MOD_TABLE)
    rows, table_errors = load_call_table(table_path)
    if not mod_table_path.is_file():
        raise SystemExit(f"{mod_table_path}: module-sides table does not exist")
    mod_sides = gfs.load_optional_module_sides(mod_table_path)
    if not mod_sides:
        raise SystemExit(f"{mod_table_path}: module-sides table is empty")

    if args.list:
        if table_errors:
            for e in table_errors:
                print(f"error: {e}", file=sys.stderr)
            return 1
        return run_list(rows)
    if args.fix:
        return run_fix(root, rows, table_errors, mod_sides, fmt=not args.no_fmt)
    return run_check(
        root,
        table_path,
        rows,
        table_errors,
        mod_sides,
        against=args.against,
        pull_request_merge=args.pull_request_merge,
    )


if __name__ == "__main__":
    raise SystemExit(main())
