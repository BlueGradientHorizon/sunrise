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
  are rewritten to `pub(in crate::game)`, while `pub(super)` inside nested inline `mod`
  blocks is preserved.
- Exact test selectors, parity harness selectors, `tools/test_areas.toml`, `fn-replace-check`
  manifests, docs, and Rust child-process test selectors are rewritten from the table.

Usage:
    python3 tools/game_module_sides.py --fix [--no-fmt]
    python3 tools/game_module_sides.py --check
    python3 tools/game_module_sides.py --list
    python3 tools/game_module_sides.py --renames BEFORE_FUNCTIONS_JSON AFTER_FUNCTIONS_JSON [--out PATH]
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

MOD_DECL_RE = re.compile(r"^([ \t]*)(pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z0-9_]+)\s*;\s*$")
REEXPORT_RE = re.compile(
    r"^([ \t]*)(pub(?:\([^)]*\))?\s+)?use\s+(client_side|server_side)::([A-Za-z0-9_]+)\s*;\s*$"
)
SIDE_ALIAS_RE = re.compile(
    r"^([ \t]*)pub\(in crate::game\)\s+use\s+(client_side|server_side)\s+as\s+(client_state|server_state)\s*;\s*$"
)
PATH_ATTR_RE = re.compile(r'path\s*=\s*"([^"]+)"')
GAME_PATH_RE = re.compile(r"(?<![A-Za-z0-9_:])game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::)")
FULL_GAME_PATH_RE = re.compile(
    r"sunrise_client::game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::)"
)
STATE_TYPE_PATH_RE = re.compile(
    r"sunrise_client::game::(client_state|server_state|client_side|server_side)::(ClientState|ServerState)\b"
)
RUST_GAME_LITERAL_RE = re.compile(
    r'"(game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::[^"]*))"'
)
TEST_AREAS_FILTER_RE = re.compile(
    r'^(\s*\{\s*package\s*=\s*"sunrise-client",\s*target\s*=\s*"lib",\s*filter\s*=\s*")'
    r'(game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::[^"]*)?)'
    r'("\s*\},?\s*)$'
)


@dataclass(frozen=True)
class ModArm:
    name: str
    vis: str
    attrs: tuple[str, ...]
    indent: str = ""


@dataclass(frozen=True)
class FieldSpec:
    field: str
    owner: str
    visibility: str  # "side", "game", or "-"


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


def strip_rust_comments(text: str) -> str:
    """Replace Rust comments with spaces, preserving newlines and string literals."""
    out: list[str] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                out.append(" ")
                i += 1
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            depth = 1
            out.append("  ")
            i += 2
            while i < n and depth > 0:
                if text[i] == "/" and i + 1 < n and text[i + 1] == "*":
                    depth += 1
                    out.append("  ")
                    i += 2
                elif text[i] == "*" and i + 1 < n and text[i + 1] == "/":
                    depth -= 1
                    out.append("  ")
                    i += 2
                else:
                    out.append("\n" if text[i] == "\n" else " ")
                    i += 1
            continue
        if c in ("r", "b", "c"):
            j = i + (1 if c in ("b", "c") and i + 1 < n and text[i + 1] == "r" else 0)
            if j < n and text[j] == "r":
                k = j + 1
                while k < n and text[k] == "#":
                    k += 1
                if k < n and text[k] == '"' and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
                    end_seq = '"' + ("#" * (k - (j + 1)))
                    end = text.find(end_seq, k + 1)
                    end = n if end == -1 else end + len(end_seq)
                    out.append(text[i:end])
                    i = end
                    continue
        if c == '"' or (
            c in ("b", "c")
            and i + 1 < n
            and text[i + 1] == '"'
            and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_"))
        ):
            start = i
            if c != '"':
                i += 1
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                elif text[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
            out.append(text[start:i])
            continue
        if c == "'":
            if i + 2 < n and text[i + 1] == "\\":
                end = text.find("'", i + 2)
                if end != -1 and end - i <= 12 and "\n" not in text[i:end]:
                    out.append(text[i : end + 1])
                    i = end + 1
                    continue
            elif i + 2 < n and text[i + 2] == "'" and text[i + 1] != "\n":
                out.append(text[i : i + 3])
                i += 3
                continue
        out.append(c)
        i += 1
    return "".join(out)


def rewrite_root_pub_super(text: str) -> tuple[str, int, list[int]]:
    """Rewrite `pub(super)` -> `pub(in crate::game)` at root module scope (`mod_depth == 0`).

    Returns `(new_text, replacement_count, 1based_line_numbers)`.
    """
    out: list[str] = []
    i, n = 0, len(text)
    brace_depth = 0
    mod_brace_depths: list[int] = []
    pending_mod = False
    replaced = 0
    lines_hit: list[int] = []
    lineno = 1

    def advance_chunk(chunk: str) -> None:
        nonlocal lineno
        lineno += chunk.count("\n")
        out.append(chunk)

    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            end = text.find("\n", i)
            end = n if end == -1 else end
            advance_chunk(text[i:end])
            i = end
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            depth = 1
            start = i
            i += 2
            while i < n and depth > 0:
                if text[i] == "/" and i + 1 < n and text[i + 1] == "*":
                    depth += 1
                    i += 2
                elif text[i] == "*" and i + 1 < n and text[i + 1] == "/":
                    depth -= 1
                    i += 2
                else:
                    i += 1
            advance_chunk(text[start:i])
            continue
        if c in ("r", "b", "c"):
            j = i + (1 if c in ("b", "c") and i + 1 < n and text[i + 1] == "r" else 0)
            if j < n and text[j] == "r":
                k = j + 1
                while k < n and text[k] == "#":
                    k += 1
                if k < n and text[k] == '"' and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
                    end_seq = '"' + ("#" * (k - (j + 1)))
                    end = text.find(end_seq, k + 1)
                    end = n if end == -1 else end + len(end_seq)
                    advance_chunk(text[i:end])
                    i = end
                    continue
        if c == '"' or (
            c in ("b", "c")
            and i + 1 < n
            and text[i + 1] == '"'
            and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_"))
        ):
            start = i
            if c != '"':
                i += 1
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                elif text[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
            advance_chunk(text[start:i])
            continue
        if c == "'":
            if i + 2 < n and text[i + 1] == "\\":
                end = text.find("'", i + 2)
                if end != -1 and end - i <= 12 and "\n" not in text[i:end]:
                    advance_chunk(text[i : end + 1])
                    i = end + 1
                    continue
            elif i + 2 < n and text[i + 2] == "'" and text[i + 1] != "\n":
                advance_chunk(text[i : i + 3])
                i += 3
                continue

        if c == "{":
            brace_depth += 1
            if pending_mod:
                mod_brace_depths.append(brace_depth)
                pending_mod = False
            out.append(c)
            i += 1
            continue
        if c == "}":
            if mod_brace_depths and mod_brace_depths[-1] == brace_depth:
                mod_brace_depths.pop()
            brace_depth = max(0, brace_depth - 1)
            out.append(c)
            i += 1
            continue
        if c == ";" and pending_mod:
            pending_mod = False
            out.append(c)
            i += 1
            continue
        if c == "\n":
            lineno += 1
            out.append(c)
            i += 1
            continue

        if c.isalpha() or c == "_":
            j = i + 1
            while j < n and (text[j].isalnum() or text[j] == "_"):
                j += 1
            word = text[i:j]
            if word == "mod":
                prev = text[max(0, i - 2) : i]
                if not prev.endswith("::") and not prev.endswith("."):
                    pending_mod = True
                out.append(word)
                i = j
                continue
            if word == "pub" and not mod_brace_depths:
                m = re.match(r"\s*\(\s*super\s*\)", text[j:])
                if m:
                    matched_span = text[j : j + m.end()]
                    lines_hit.append(lineno)
                    lineno += matched_span.count("\n")
                    out.append("pub(in crate::game)")
                    i = j + m.end()
                    replaced += 1
                    continue
            out.append(word)
            i = j
            continue

        out.append(c)
        i += 1

    return "".join(out), replaced, lines_hit


def root_scope_includes(rs_file: Path) -> list[Path]:
    """Return files included via literal `include!("...")` at root module scope (`mod_depth == 0`)."""
    found: list[Path] = []
    visited: set[Path] = set()

    def scan(path: Path) -> None:
        resolved = path.resolve()
        if resolved in visited or not resolved.is_file():
            return
        visited.add(resolved)
        text = strip_rust_comments(resolved.read_text(encoding="utf-8"))
        i, n = 0, len(text)
        brace_depth = 0
        mod_depths: list[int] = []
        pending_mod = False
        while i < n:
            c = text[i]
            if c == '"':
                i += 1
                while i < n:
                    if text[i] == "\\":
                        i += 2
                    elif text[i] == '"':
                        i += 1
                        break
                    else:
                        i += 1
                continue
            if c == "{":
                brace_depth += 1
                if pending_mod:
                    mod_depths.append(brace_depth)
                    pending_mod = False
                i += 1
                continue
            if c == "}":
                if mod_depths and mod_depths[-1] == brace_depth:
                    mod_depths.pop()
                brace_depth = max(0, brace_depth - 1)
                i += 1
                continue
            if c == ";" and pending_mod:
                pending_mod = False
                i += 1
                continue
            if c.isalpha() or c == "_":
                j = i + 1
                while j < n and (text[j].isalnum() or text[j] == "_"):
                    j += 1
                word = text[i:j]
                if word == "mod":
                    prev = text[max(0, i - 2) : i]
                    if not prev.endswith("::") and not prev.endswith("."):
                        pending_mod = True
                elif word == "include" and not mod_depths:
                    m = re.match(r'\s*!\s*\(\s*"([^"]+)"\s*\)', text[j:])
                    if m:
                        target = (resolved.parent / m.group(1)).resolve()
                        if target.is_file():
                            found.append(target)
                            scan(target)
                        i = j + m.end()
                        continue
                i = j
                continue
            i += 1

    scan(rs_file)
    return found


def collect_attr_block(lines: list[str], start: int) -> tuple[list[str], int]:
    """Collect contiguous `#[...]` attribute lines starting at `start`."""
    attrs: list[str] = []
    i = start
    while i < len(lines):
        s = lines[i].strip()
        if s.startswith("#["):
            buf = lines[i]
            open_b = s.count("[")
            close_b = s.count("]")
            while open_b > close_b and i + 1 < len(lines):
                i += 1
                buf += lines[i]
                open_b += lines[i].count("[")
                close_b += lines[i].count("]")
            attrs.append(buf)
            i += 1
        else:
            break
    return attrs, i


def parse_file_mod_decls(text: str) -> dict[str, list[ModArm]]:
    """Parse top-level out-of-line `mod <name>;` declarations from a Rust file."""
    lines = text.splitlines(keepends=True)
    decls: dict[str, list[ModArm]] = {}
    i = 0
    brace_depth = 0
    while i < len(lines):
        if brace_depth == 0:
            attrs, j = collect_attr_block(lines, i)
            if j < len(lines):
                m = MOD_DECL_RE.match(lines[j])
                if m:
                    arm = ModArm(
                        name=m.group(3),
                        vis=(m.group(2) or "").strip(),
                        attrs=tuple(attrs),
                        indent=m.group(1),
                    )
                    decls.setdefault(arm.name, []).append(arm)
                    i = j + 1
                    continue
        # Track brace depth across lines (comments stripped per line is sufficient for top-level items)
        code = strip_rust_comments(lines[i])
        brace_depth = max(0, brace_depth + code.count("{") - code.count("}"))
        i += 1
    return decls


def parse_game_reexports(text: str) -> dict[str, tuple[str, str, tuple[str, ...]]]:
    """Parse `use (client_side|server_side)::<mod>;` re-exports at top level of `game.rs`."""
    lines = text.splitlines(keepends=True)
    reexports: dict[str, tuple[str, str, tuple[str, ...]]] = {}
    i = 0
    brace_depth = 0
    while i < len(lines):
        if brace_depth == 0:
            attrs, j = collect_attr_block(lines, i)
            if j < len(lines):
                m = REEXPORT_RE.match(lines[j])
                if m:
                    vis = (m.group(2) or "").strip()
                    side_mod = m.group(3)
                    mod_name = m.group(4)
                    reexports[mod_name] = (side_mod, vis, tuple(attrs))
                    i = j + 1
                    continue
        code = strip_rust_comments(lines[i])
        brace_depth = max(0, brace_depth + code.count("{") - code.count("}"))
        i += 1
    return reexports


def normalize_attr_path(attr: str, to_side: bool) -> str:
    """Rewrite `#[path = "game/..."]` <-> `#[path = "..."]` for side vs game parent."""
    m = PATH_ATTR_RE.search(attr)
    if not m:
        return attr
    raw = m.group(1)
    if to_side:
        new_path = raw[5:] if raw.startswith("game/") else raw
    else:
        new_path = raw if raw.startswith("game/") else f"game/{raw}"
    return attr[: m.start(1)] + new_path + attr[m.end(1) :]


def side_vis_for(orig_vis: str) -> str:
    return orig_vis if orig_vis in ("pub", "pub(crate)") else "pub(in crate::game)"


def reexport_vis_for(orig_vis: str) -> str:
    if orig_vis in ("pub", "pub(crate)"):
        return orig_vis
    return orig_vis if orig_vis else "pub(in crate::game)"


def shared_cfg_attrs(arms: list[ModArm]) -> list[str]:
    """Return `#[cfg(...)]` / `#[cfg_attr(...)]` attrs that apply to every arm of a module."""
    if len(arms) != 1:
        # Multi-arm cfg pair (e.g. feature / not(feature)): module exists in all configs.
        return []
    return [a for a in arms[0].attrs if "#[cfg(" in a or "#[cfg_attr(" in a]


def find_struct_span(text: str, struct_name: str) -> tuple[int, int, int, int]:
    """Return `(decl_start, body_open_brace, body_close_brace, decl_end)` for `struct <struct_name> { ... }`."""
    clean = strip_rust_comments(text)
    m = re.search(rf"\bpub(?:\([^)]*\))?\s+struct\s+{re.escape(struct_name)}\b[^{{;]*\{{", clean)
    if not m:
        raise ValueError(f"struct `{struct_name}` not found")
    open_idx = m.end() - 1
    depth = 0
    i = open_idx
    while i < len(clean):
        if clean[i] == "{":
            depth += 1
        elif clean[i] == "}":
            depth -= 1
            if depth == 0:
                return m.start(), open_idx, i, i + 1
        i += 1
    raise ValueError(f"unclosed struct `{struct_name}`")


def parse_struct_fields(text: str, struct_name: str) -> list[tuple[str, str, int, int]]:
    """Return `[(field_name, visibility_str, vis_start, vis_end)]` for top-level fields of `struct_name`."""
    _, open_idx, close_idx, _ = find_struct_span(text, struct_name)
    body = text[open_idx + 1 : close_idx]
    clean_body = strip_rust_comments(body)
    fields: list[tuple[str, str, int, int]] = []
    # Match fields at brace/paren/angle depth 0 inside the struct body.
    # Lines inside the struct body have field declarations of the form `[vis] <name>:`
    field_re = re.compile(
        r"^([ \t]*)(pub(?:\s*\([^)]*\))?[ \t]+)?([A-Za-z_][A-Za-z0-9_]*)[ \t]*:(?!:)",
        re.M,
    )
    # Track nesting depth by scanning clean_body up to each match start.
    pos = 0
    depth = 0
    for m in field_re.finditer(clean_body):
        seg = clean_body[pos : m.start()]
        for ch in seg:
            if ch in "{[(<":
                depth += 1
            elif ch in "}])>":
                depth = max(0, depth - 1)
        pos = m.start()
        if depth != 0:
            continue
        vis_raw = (m.group(2) or "").strip()
        fname = m.group(3)
        vis_start = open_idx + 1 + m.start(1) + len(m.group(1))
        vis_end = open_idx + 1 + (m.end(2) if m.group(2) else m.start(3))
        fields.append((fname, vis_raw, vis_start, vis_end))
    return fields


def check_state_struct_rules(text: str, struct_name: str, rel: str) -> list[str]:
    """Check Wall Rules 1 & 2: no derives and no Deref/AsRef impls on ClientState/ServerState."""
    problems: list[str] = []
    decl_start, _, _, _ = find_struct_span(text, struct_name)
    # Inspect contiguous attributes immediately preceding the struct declaration
    before_lines = text[:decl_start].splitlines()
    for line in reversed(before_lines):
        s = line.strip()
        if not s or s.startswith("//"):
            continue
        if s.startswith("#["):
            if "derive(" in s:
                problems.append(f"{rel}: `{struct_name}` must not carry `#[derive(...)]`")
        else:
            break
    clean = strip_rust_comments(text)
    if re.search(rf"\bimpl\b[^{{;]*\b(?:Deref|DerefMut|AsRef|AsMut)\b[^{{;]*\bfor\s+{struct_name}\b", clean):
        problems.append(f"{rel}: `{struct_name}` must not implement Deref/DerefMut/AsRef/AsMut")
    return problems


def rewrite_state_file(
    src_text: str,
    side: str,
    moved_arms: list[ModArm],
    field_specs: dict[str, FieldSpec],
) -> tuple[str, list[str]]:
    """Build the updated `{client,server}_side.rs` text and return `(new_text, errors)`."""
    side_mod = SIDE_MOD_NAME[side]
    struct_name = SIDE_STRUCT_NAME[side]
    errors: list[str] = []

    decl_start, _, _, _ = find_struct_span(src_text, struct_name)
    # Keep leading `//!` module doc comments and any attributes attached directly to the struct.
    preamble_lines = src_text[:decl_start].splitlines(keepends=True)
    doc_lines: list[str] = []
    idx = 0
    while idx < len(preamble_lines) and (
        preamble_lines[idx].lstrip().startswith("//!") or not preamble_lines[idx].strip()
    ):
        if preamble_lines[idx].lstrip().startswith("//!"):
            doc_lines.append(preamble_lines[idx])
        idx += 1

    # Preserve attributes directly attached to the struct declaration (if any)
    struct_attr_lines: list[str] = []
    rev = len(preamble_lines) - 1
    while rev >= idx:
        s = preamble_lines[rev].strip()
        if s.startswith("#[") or s.startswith("///"):
            struct_attr_lines.append(preamble_lines[rev])
            rev -= 1
        else:
            break
    struct_attr_lines.reverse()

    mod_lines: list[str] = []
    for arm in moved_arms:
        for a in arm.attrs:
            mod_lines.append(normalize_attr_path(a, to_side=True))
        mod_lines.append(f"{side_vis_for(arm.vis)} mod {arm.name};\n")

    header = "".join(doc_lines)
    header += f"#[allow(unused_imports)]\npub(in crate::game::{side_mod}) use super::*;\n\n"
    if mod_lines:
        header += "".join(mod_lines) + "\n"
    header += "".join(struct_attr_lines)

    rest = src_text[decl_start:]
    # Rewrite field visibilities inside `struct {Client,Server}State`
    fields = parse_struct_fields(rest, struct_name)
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
) -> tuple[str, list[str]]:
    """Regenerate `crates/sunrise-client/src/game.rs` module declarations and re-exports."""
    lines = game_text.splitlines(keepends=True)
    out: list[str] = []
    errors: list[str] = []
    emitted_reexports: set[str] = set()
    emitted_side_containers: set[str] = set()
    emitted_game_mods: set[str] = set()

    def emit_side_container(side: str, indent: str) -> None:
        if side in emitted_side_containers:
            return
        emitted_side_containers.add(side)
        smod = SIDE_MOD_NAME[side]
        old_alias = "client_state" if side == "client" else "server_state"
        out.append(f'{indent}#[path = "game/{smod}.rs"]\n')
        out.append(f"{indent}pub(in crate::game) mod {smod};\n")
        out.append(f"{indent}#[allow(unused_imports)]\n")
        out.append(f"{indent}pub(in crate::game) use {smod} as {old_alias};\n")

    def emit_moved_reexport(mod_name: str, target_side: str, indent: str) -> None:
        if mod_name in emitted_reexports:
            return
        emitted_reexports.add(mod_name)
        arms = all_arms[mod_name]
        smod = SIDE_MOD_NAME[target_side]
        for cfg in shared_cfg_attrs(arms):
            out.append(cfg)
        out.append(f"{indent}#[allow(unused_imports)]\n")
        out.append(f"{indent}{reexport_vis_for(arms[0].vis)} use {smod}::{mod_name};\n")

    def emit_game_mod(mod_name: str, indent: str) -> None:
        if mod_name in emitted_game_mods:
            return
        emitted_game_mods.add(mod_name)
        for arm in all_arms[mod_name]:
            for a in arm.attrs:
                out.append(normalize_attr_path(a, to_side=False))
            vis_prefix = f"{arm.vis} " if arm.vis else ""
            out.append(f"{indent}{vis_prefix}mod {mod_name};\n")

    i = 0
    brace_depth = 0
    while i < len(lines):
        if brace_depth == 0:
            attrs, j = collect_attr_block(lines, i)
            if j < len(lines):
                m_mod = MOD_DECL_RE.match(lines[j])
                if m_mod:
                    indent = m_mod.group(1)
                    name = m_mod.group(3)
                    if name in ("client_state", "client_side"):
                        emit_side_container("client", indent)
                        i = j + 1
                        continue
                    if name in ("server_state", "server_side"):
                        emit_side_container("server", indent)
                        i = j + 1
                        continue
                    if name not in table:
                        errors.append(f"crates/sunrise-client/src/game.rs: undeclared module `{name}` missing from table")
                        out.extend(lines[i : j + 1])
                        i = j + 1
                        continue
                    target_side = table[name][0]
                    if target_side in ("client", "server"):
                        emit_moved_reexport(name, target_side, indent)
                    else:
                        out.extend(lines[i : j + 1])
                        emitted_game_mods.add(name)
                    i = j + 1
                    continue

                m_alias = SIDE_ALIAS_RE.match(lines[j])
                if m_alias:
                    side = "client" if m_alias.group(2) == "client_side" else "server"
                    emit_side_container(side, m_alias.group(1))
                    i = j + 1
                    continue

                m_reexp = REEXPORT_RE.match(lines[j])
                if m_reexp:
                    indent = m_reexp.group(1)
                    name = m_reexp.group(4)
                    if name not in table:
                        errors.append(f"crates/sunrise-client/src/game.rs: re-exported module `{name}` missing from table")
                        out.extend(lines[i : j + 1])
                        i = j + 1
                        continue
                    target_side = table[name][0]
                    if target_side in ("client", "server"):
                        emit_moved_reexport(name, target_side, indent)
                    else:
                        emit_game_mod(name, indent)
                    i = j + 1
                    continue

        code = strip_rust_comments(lines[i])
        brace_depth = max(0, brace_depth + code.count("{") - code.count("}"))
        out.append(lines[i])
        i += 1

    return "".join(out), errors


def resolve_module_entry_files(root: Path, arms: list[ModArm], from_side: bool) -> list[Path]:
    """Resolve entry `.rs` file path(s) for a module's declaration arm(s)."""
    src_root = root / "crates/sunrise-client/src"
    files: list[Path] = []
    for arm in arms:
        path_val = None
        for a in arm.attrs:
            pm = PATH_ATTR_RE.search(a)
            if pm:
                path_val = pm.group(1)
                break
        if path_val:
            if path_val.startswith("game/"):
                cand = src_root / path_val
            else:
                cand = (src_root / "game" / path_val) if from_side else (src_root / path_val)
                if not cand.is_file():
                    cand = src_root / "game" / path_val
        else:
            c1 = src_root / "game" / f"{arm.name}.rs"
            c2 = src_root / "game" / arm.name / "mod.rs"
            cand = c1 if c1.is_file() else c2
        if cand.is_file():
            files.append(cand.resolve())
    return files


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


def rewrite_rust_test_literals(text: str, mod_to_side: dict[str, str]) -> str:
    """Rewrite `"game::<mod>::..."` string literals in Rust source (skipping comments)."""
    out: list[str] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            end = text.find("\n", i)
            end = n if end == -1 else end
            out.append(text[i:end])
            i = end
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            depth = 1
            start = i
            i += 2
            while i < n and depth > 0:
                if text[i] == "/" and i + 1 < n and text[i + 1] == "*":
                    depth += 1
                    i += 2
                elif text[i] == "*" and i + 1 < n and text[i + 1] == "/":
                    depth -= 1
                    i += 2
                else:
                    i += 1
            out.append(text[start:i])
            continue
        if c in ("r", "b", "c"):
            j = i + (1 if c in ("b", "c") and i + 1 < n and text[i + 1] == "r" else 0)
            if j < n and text[j] == "r":
                k = j + 1
                while k < n and text[k] == "#":
                    k += 1
                if k < n and text[k] == '"' and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
                    end_seq = '"' + ("#" * (k - (j + 1)))
                    end = text.find(end_seq, k + 1)
                    end = n if end == -1 else end + len(end_seq)
                    out.append(text[i:end])
                    i = end
                    continue
        if c == '"':
            start = i
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                elif text[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
            lit = text[start:i]
            m = RUST_GAME_LITERAL_RE.fullmatch(lit)
            if m and m.group(3) in mod_to_side:
                mod_name = m.group(3)
                tail = m.group(4)
                side = mod_to_side[mod_name]
                if side in ("client", "server"):
                    lit = f'"game::{SIDE_MOD_NAME[side]}::{mod_name}{tail}"'
                else:
                    lit = f'"game::{mod_name}{tail}"'
            out.append(lit)
            continue
        out.append(c)
        i += 1
    return "".join(out)


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
        # Group consecutive lines sharing the same `(mod_or_prefix, suffix)` (already expanded multi-side).
        j = i + 1
        while j < len(lines):
            mj = TEST_AREAS_FILTER_RE.match(lines[j])
            if mj and mj.group(4) == mod_or_prefix and (mj.group(5) or "") == suffix:
                j += 1
            else:
                break

        if suffix:
            # Explicit `game::<mod>::...` filter.
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
            # Prefix filter `game::<prefix>` (no trailing `::`): find all sides matching modules with this prefix.
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
    # Only rewrite bare `game::<mod>::` selectors when preceded by whitespace, backtick, or quote
    pat = re.compile(r"([\s`'\"])game::(?:(client_side|server_side)::)?([A-Za-z0-9_]+)(::)")

    def repl(m: re.Match[str]) -> str:
        mod_name = m.group(3)
        if mod_name not in mod_to_side:
            return m.group(0)
        return m.group(1) + normalize_game_prefix(mod_name, mod_to_side, full=False)

    return pat.sub(repl, new)


def collect_all_arms(root: Path) -> tuple[dict[str, list[ModArm]], dict[str, str]]:
    """Collect every out-of-line gameplay `mod <name>;` across `game.rs`, `client_side.rs`, `server_side.rs`.

    Returns `(mod_name -> [ModArm], mod_name -> current_parent)` where current_parent is
    `"game"`, `"client"`, or `"server"`.
    """
    game_rs = root / "crates/sunrise-client/src/game.rs"
    game_dir = root / "crates/sunrise-client/src/game"
    arms: dict[str, list[ModArm]] = {}
    parents: dict[str, str] = {}

    for parent, path in [
        ("game", game_rs),
        ("client", game_dir / "client_side.rs"),
        ("server", game_dir / "server_side.rs"),
    ]:
        if not path.is_file():
            continue
        decls = parse_file_mod_decls(path.read_text(encoding="utf-8"))
        for name, mod_arms in decls.items():
            if name in SIDE_CONTAINERS:
                continue
            if name in arms:
                raise SystemExit(
                    f"module `{name}` declared in multiple parents ({parents[name]} and {parent})"
                )
            arms[name] = mod_arms
            parents[name] = parent
    return arms, parents


def selector_targets(root: Path, mod_to_side: dict[str, str]) -> list[tuple[Path, str]]:
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
            old = p.read_text(encoding="utf-8")
            if '"game::' not in old:
                continue
            new = rewrite_rust_test_literals(old, mod_to_side)
            if new != old:
                updates.append((p, new))

    return updates


def run_rustfmt(paths: list[Path]) -> None:
    rustfmt = os.environ.get("RUSTFMT") or shutil.which("rustfmt")
    if not rustfmt:
        return
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

    all_arms, current_parents = collect_all_arms(root)
    missing_from_table = sorted(set(all_arms) - set(table))
    if missing_from_table:
        for m in missing_from_table:
            print(f"error: module `{m}` (in {current_parents[m]}) is missing from game_module_sides.tsv", file=sys.stderr)
        return 1

    # Determine moved module arms for each side (sorted alphabetically by module name)
    side_arms: dict[str, list[ModArm]] = {"client": [], "server": []}
    for mod_name in sorted(all_arms):
        target_side = mod_to_side[mod_name]
        if target_side in ("client", "server"):
            side_arms[target_side].extend(all_arms[mod_name])

    changed_rs: list[Path] = []

    # 1. Generate client_side.rs and server_side.rs (removing client_state.rs / server_state.rs if present)
    for side in ("client", "server"):
        new_path = game_dir / SIDE_NEW_FILE[side]
        old_path = game_dir / SIDE_OLD_FILE[side]
        src_path = new_path if new_path.is_file() else old_path
        if not src_path.is_file():
            print(f"error: neither {new_path} nor {old_path} exists", file=sys.stderr)
            return 1
        src_text = src_path.read_text(encoding="utf-8")
        new_text, errs = rewrite_state_file(src_text, side, side_arms[side], field_specs)
        if errs:
            for e in errs:
                print(f"error: {e}", file=sys.stderr)
            return 1
        if not new_path.is_file() or new_path.read_text(encoding="utf-8") != new_text:
            new_path.write_text(new_text, encoding="utf-8")
            changed_rs.append(new_path)
        if old_path.is_file():
            old_path.unlink()

    # 2. Rewrite game.rs
    old_game = game_rs.read_text(encoding="utf-8")
    new_game, errs = rewrite_game_rs(old_game, table, all_arms)
    if errs:
        for e in errs:
            print(f"error: {e}", file=sys.stderr)
        return 1
    if new_game != old_game:
        game_rs.write_text(new_game, encoding="utf-8")
        changed_rs.append(game_rs)

    # 3. Rewrite root-scope `pub(super)` -> `pub(in crate::game)` in moved entry files and root `include!`d files
    vis_files = 0
    vis_rewrites = 0
    moved_entry_files: list[Path] = []
    for mod_name in sorted(all_arms):
        target_side = mod_to_side[mod_name]
        if target_side in ("client", "server"):
            from_side = current_parents[mod_name] != "game"
            for entry in resolve_module_entry_files(root, all_arms[mod_name], from_side=from_side):
                moved_entry_files.append(entry)
                moved_entry_files.extend(root_scope_includes(entry))
    for path in sorted(set(moved_entry_files)):
        old_txt = path.read_text(encoding="utf-8")
        new_txt, n, _ = rewrite_root_pub_super(old_txt)
        if n > 0:
            path.write_text(new_txt, encoding="utf-8")
            changed_rs.append(path)
            vis_files += 1
            vis_rewrites += n

    # 4. Rewrite test selectors, parity scripts, test_areas.toml, fn-replace-check, docs, Rust child selectors
    sel_updates = selector_targets(root, mod_to_side)
    for path, new_txt in sel_updates:
        path.write_text(new_txt, encoding="utf-8")
        if path.suffix == ".rs":
            changed_rs.append(path)

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

    # 1. Check side files exist and legacy state files are gone
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

    # 2. Check module placement, missing table entries, re-exports, and glob imports
    all_arms, current_parents = collect_all_arms(root)
    for mod_name in sorted(set(all_arms) - set(table)):
        problems.append(
            f"module `{mod_name}` declared in `{current_parents[mod_name]}` is missing from game_module_sides.tsv"
        )
    for mod_name in sorted(set(table) - set(all_arms)):
        problems.append(f"module `{mod_name}` in game_module_sides.tsv is not declared in game.rs or side files")

    game_text = game_rs.read_text(encoding="utf-8")
    reexports = parse_game_reexports(game_text)

    for mod_name in sorted(set(all_arms) & set(table)):
        want = mod_to_side[mod_name]
        actual = current_parents[mod_name]
        if actual != want:
            problems.append(f"module `{mod_name}` is in `{actual}`, expected `{want}` per game_module_sides.tsv")
        if want in ("client", "server"):
            if mod_name not in reexports or reexports[mod_name][0] != SIDE_MOD_NAME[want]:
                problems.append(
                    f"crates/sunrise-client/src/game.rs: moved module `{mod_name}` missing re-export from `{SIDE_MOD_NAME[want]}`"
                )
        elif mod_name in reexports:
            problems.append(f"crates/sunrise-client/src/game.rs: game-side module `{mod_name}` has stale re-export")

    # 3. Check side files: glob import, field visibility, no derives/Deref
    for side in ("client", "server"):
        smod = SIDE_MOD_NAME[side]
        sname = SIDE_STRUCT_NAME[side]
        path = game_dir / SIDE_NEW_FILE[side]
        rel = path.relative_to(root).as_posix()
        txt = path.read_text(encoding="utf-8")
        if f"pub(in crate::game::{smod}) use super::*;" not in txt:
            problems.append(f"{rel}: missing `pub(in crate::game::{smod}) use super::*;`")
        problems.extend(check_state_struct_rules(txt, sname, rel))

        try:
            fields = parse_struct_fields(txt, sname)
        except ValueError as err:
            problems.append(f"{rel}: {err}")
            continue
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
            norm_vis = re.sub(r"\s+", " ", actual_vis).replace("( ", "(").replace(" )", ")")
            if spec.visibility == "side":
                if norm_vis != allowed_private:
                    problems.append(
                        f"{rel}: field `{sname}::{fname}` has visibility `{actual_vis or '<private>'}`, "
                        f"wider than table requirement `{allowed_private}`"
                    )
            else:
                if norm_vis not in ("pub(in crate::game)", allowed_private):
                    problems.append(
                        f"{rel}: field `{sname}::{fname}` has visibility `{actual_vis or '<private>'}`, "
                        "wider than `pub(in crate::game)`"
                    )
        expected = {f for f, s in field_specs.items() if s.owner == side}
        for missing in sorted(expected - seen):
            problems.append(f"{rel}: expected `{side}` field `{missing}` missing from `{sname}`")

    # 4. Check no root-scope `pub(super)` left in moved entry files or root-level `include!`d files
    checked_files: set[Path] = set()
    for mod_name in sorted(set(all_arms) & set(table)):
        if mod_to_side[mod_name] not in ("client", "server"):
            continue
        from_side = current_parents[mod_name] != "game"
        for entry in resolve_module_entry_files(root, all_arms[mod_name], from_side=from_side):
            for target in [entry, *root_scope_includes(entry)]:
                if target in checked_files:
                    continue
                checked_files.add(target)
                _, n, lines_hit = rewrite_root_pub_super(target.read_text(encoding="utf-8"))
                if n > 0:
                    rel = target.relative_to(root).as_posix()
                    sample = ", ".join(str(x) for x in lines_hit[:5])
                    problems.append(f"{rel}: {n} root-scope `pub(super)` item(s) remain (line(s) {sample})")

    # 5. Check selector files
    for path, _ in selector_targets(root, mod_to_side):
        rel = path.relative_to(root).as_posix()
        problems.append(f"{rel}: stale module path selector(s); run `python3 tools/game_module_sides.py --fix`")

    for p in problems:
        print(f"error: {p}", file=sys.stderr)
    if problems:
        print(f"Found {len(problems)} module-side problem(s).", file=sys.stderr)
        return 1
    print(f"OK: {len(all_arms)} modules and {sum(s.owner in ('client', 'server') for s in field_specs.values())} state fields verified.")
    return 0


def run_list(root: Path, table: dict[str, tuple[str, str]], field_specs: dict[str, FieldSpec]) -> int:
    all_arms, current_parents = collect_all_arms(root)
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
