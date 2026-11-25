#!/usr/bin/env python3
"""Per-use side-boundary ledger built from `sunrise-client` MIR dumps and source.

Measures every place where code of one label (`client`, `server`, `common`) uses
something belonging to another side or crosses an indirect boundary:
  - `call`: statically resolved call to a function in a side tree other than the caller's label
  - `field`: read, write, or mutable borrow of a side-owned grouped or wrapped `Game` field
  - `dyn`: dynamic trait-object call from a body under `game::` (never dropped)
  - `generic`: generic-parameter trait or `Fn*` call from a body under `game::` (never dropped)
  - `fnptr`: side function item reified or passed as a value, and indirect `fn(...)` pointer calls
  - `closure-pass`: closure using side X passed to a callee of another label (or unsized to `dyn`)
  - `trait-impl`: hand-written trait impl method under `game::` whose body or `Self` is side code
  - `drop`: `Drop` impl with side uses and types holding side state with such a destructor
  - `callback-param`: non-test function under `game::` accepting or returning a game-taking callable

Coverage and exclusions:
  - Covered: production `sunrise-client --lib` MIR in three configurations (`default`,
    `no-default-features`, `state-digest`) plus non-test Rust source under `game.rs` / `game/`.
  - Not covered: `#[cfg(test)]` and integration test code (covered only by the function wall),
    and platform/arch/feature-gated code this host does not compile in those three configurations
    (run `--list-uncovered-cfgs` to list every source `#[cfg(...)]` predicate never compiled).
  - Attribution: a body's label is the side of its logical defining module (`game::client_side::*`
    -> `client`, `game::server_side::*` -> `server`, else `tools/game_module_sides.tsv`, else
    `common`). Items from `include!`, macros, and `OUT_DIR` generated files inherit the logical
    module where they expand. Closures, coroutines, and promoted constants fold into their
    enclosing function for `user_id` and `user_label`.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import csv
import json
import os
import platform
import re
import subprocess
import sys
import tempfile
import time
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_TABLE = Path(__file__).resolve().with_name("game_side_ledger.tsv")
DEFAULT_MODULE_TABLE = Path(__file__).resolve().with_name("game_module_sides.tsv")
DEFAULT_FN_TABLE = Path(__file__).resolve().with_name("game_fn_sides.tsv")
DEFAULT_FIELD_TABLE = Path(__file__).resolve().with_name("game_field_owners.tsv")
HZ_WRAPPER = Path("/home/umceko/hz-dev/hz")

CONFIG_SPECS: dict[str, dict[str, Any]] = {
    "default": {
        "cargo_args": (),
        "features": frozenset({"audio", "controllers", "sunrise-extras", "friends"}),
    },
    "no-default-features": {
        "cargo_args": ("--no-default-features",),
        "features": frozenset(),
    },
    "state-digest": {
        "cargo_args": ("--features", "state-digest"),
        "features": frozenset({"audio", "controllers", "sunrise-extras", "friends", "state-digest"}),
    },
}
DEFAULT_CONFIGS = ("default", "no-default-features", "state-digest")

LEDGER_COLUMNS = (
    "user_id",
    "user_label",
    "kind",
    "target_id",
    "target_side",
    "detail",
    "configs",
    "sites",
    "example_loc",
    "status",
    "group",
    "note",
)
VALID_LABELS = frozenset({"client", "server", "common"})
VALID_KINDS = frozenset(
    {
        "call",
        "field",
        "dyn",
        "generic",
        "fnptr",
        "closure-pass",
        "trait-impl",
        "drop",
        "callback-param",
    }
)
VALID_TARGET_SIDES = frozenset({"client", "server", "both", "unresolved", "common"})
VALID_STATUSES = frozenset({"route", "bridge", "debt", "unclassified"})
VALID_GROUPS = frozenset({"", "G1", "G2", "G3", "G4", "G5", "G6"})

CRATE_DIR_TO_MOD = {
    "sunrise-mod-vocab": "mod_vocab",
    "sunrise-mod-api": "mod_api",
    "sunrise-mod-runtime": "sunrise_mod_runtime",
    "sunrise-client-lib": "sunrise_client_lib",
    "sunrise-client": "sunrise_client",
    "sunrise-world": "sunrise_world",
    "sunrise-gameplay-common": "sunrise_gameplay_common",
    "sunrise-gameplay": "sunrise_gameplay",
    "sunrise-core": "sunrise_core",
    "sunrise-items": "sunrise_items",
    "sunrise-client-render": "sunrise_client_render",
}

WRAPPER_KIND_MAP = {
    "ClientOnly": ("client", "client_only"),
    "ClientShared": ("client", "client_shared"),
    "ServerOnly": ("server", "server_only"),
    "ServerShared": ("server", "server_shared"),
}

STD_CRATES = frozenset({"std", "core", "alloc"})

CALLBACK_GAME_MARKERS = frozenset(
    {
        "Game",
        "GameLevel",
        "ClientState",
        "ServerState",
        "Host",
        "ServerHost",
        "ClientHost",
        "LevelHost",
        "LevelWriteHost",
        "LevelRandomHost",
        "Backend",
        "BackendMut",
        "ClientBackendMut",
        "DrawContext",
        "BlockEntityDrawContext",
        "ClientSetupContext",
        "ReloadContext",
        "GoalHost",
        "SideLevel",
        "SpawnLevel",
        "HandWorld",
        "LevelAccess",
        "AttachmentStore",
        "BehaviorCxBackend",
        "Level",
    }
)

SAFE_NON_GAME_TYPES = frozenset(
    {
        "bool",
        "char",
        "str",
        "String",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "usize",
        "i8",
        "i16",
        "i32",
        "i64",
        "i128",
        "isize",
        "f32",
        "f64",
        "Vec3",
        "DVec3",
        "Vec2",
        "IVec3",
        "BlockPos",
        "Pos",
        "BlockStateId",
        "Identifier",
        "Text",
        "ItemStack",
        "Compound",
        "Value",
        "Map",
        "Minecart",
        "Mob",
        "EntityRef",
        "PlayerRef",
        "Titles",
        "Menu",
        "Env",
        "MerchantOffer",
        "TargetCandidate",
        "Candidate",
        "Slot",
        "SlotCandidate",
        "Packet",
        "ChunkPos",
        "Dim",
        "LevelDimension",
        "Path",
        "PathBuf",
        "Instant",
        "Duration",
        "Uuid",
        "LegacyRandom",
        "KeptPlayer",
        "BlockEntity",
        "BlockEntityScene",
        "Renderer",
        "Frame",
        "Session",
        "TooltipFont",
        "PickKind",
        "Option",
        "Result",
        "Vec",
        "Box",
        "Arc",
        "Rc",
        "Cow",
        "HashMap",
        "HashSet",
        "BTreeMap",
        "BTreeSet",
        "ControlFlow",
        "Ordering",
        "Infallible",
        "Error",
        "TextLinesRefusal",
        "AttachmentLifetime",
    }
)

def _parse_top_fn_header(line: str) -> tuple[str, str, str] | None:
    """Parse a MIR `fn <def_path>(<args>) [-> <ret>] {` header with balanced parentheses."""
    s = line.strip()
    if not s.endswith("{"):
        return None
    s = s[:-1].rstrip()
    m_pub = re.match(r"^(?:pub(?:\([^)]*\))?\s+)?fn\s+", s)
    if not m_pub:
        return None
    rest = s[m_pub.end() :]
    # Find the parameter-list `(` at angle/brace depth 0
    depth_a = depth_c = 0
    open_p = -1
    for i, ch in enumerate(rest):
        if ch == "<":
            depth_a += 1
        elif ch == ">":
            if i > 0 and rest[i - 1] == "-":
                pass
            elif depth_a > 0:
                depth_a -= 1
        elif ch == "{":
            depth_c += 1
        elif ch == "}":
            depth_c = max(0, depth_c - 1)
        elif ch == "(" and depth_a == 0 and depth_c == 0:
            open_p = i
            break
    if open_p <= 0:
        return None
    dp = rest[:open_p].strip()
    # Match balanced `)` for parameter list
    depth_p = 0
    close_p = -1
    for i in range(open_p, len(rest)):
        if rest[i] == "(":
            depth_p += 1
        elif rest[i] == ")":
            depth_p -= 1
            if depth_p == 0:
                close_p = i
                break
    if close_p == -1:
        return None
    args_str = rest[open_p + 1 : close_p].strip()
    after = rest[close_p + 1 :].strip()
    if not after:
        ret_ty = "()"
    elif after.startswith("->"):
        ret_ty = after[2:].strip()
    else:
        return None
    return (dp, args_str, ret_ty)


TOP_CONST_STATIC_RE = re.compile(r"^(?:pub(?:\([^)]*\))?\s+)?(const|static)(?:\s+mut)?\s+([^:=]+?):\s*(.+?)\s*=\s*\{\s*$")
ANON_CONST_RE = re.compile(r"^(.+?::\{constant#\d+\}):\s*(.+?)\s*=\s*\{\s*$")
ALLOC_START_RE = re.compile(r"^alloc\d+\s*\(")
LOCAL_DECL_RE = re.compile(r"^\s*let\s+(?:mut\s+)?(_\d+):\s*(.+?);\s*(?://.*)?$")
DEBUG_CLOSURE_RE = re.compile(r"^\s*debug\s+\S+\s*=>\s*_\d+;")
PROMOTED_HEADER_RE = re.compile(r"^const\s+(.+?)::promoted\[(\d+)\]:\s*(.+?)\s*=\s*\{\s*$")
IMPL_AT_RE = re.compile(r"<impl at ([^>:]+):(\d+):(\d+):\s*(\d+):(\d+)>")
SCOPE_LOC_RE = re.compile(r"//\s*scope\s+\d+\s+at\s+([^:]+):(\d+):(\d+):")
CLOSURE_TY_RE = re.compile(r"\{closure@[^}]+\}")
DROP_TERM_RE = re.compile(r"^\s*drop\((.+)\)\s*->\s*\[")
UNSIZE_RE = re.compile(r"=\s*(?:move|copy)\s+([^\s;]+)\s+as\s+(.+?)\s+\(PointerCoercion\(Unsize\)\);")
REIFY_FN_RE = re.compile(r"=\s*(.+?)\s+as\s+.+?\(PointerCoercion\(ReifyFnPointer\([^)]*\)(?:,\s*[^)]*)?\)\);")
FN_ITEM_LOCAL_RE = re.compile(r"fn\([^)]*\)(?:\s*->\s*[^\{]+)?\s*\{([^}]+)\}$")
CTOR_AGG_RE = re.compile(r"=\s*([A-Za-z0-9_:]+)(?::<[^>]*>)?\s*\{\s*([a-zA-Z0-9_]+)\s*:")
GENERIC_PARAM_RE = re.compile(r"^(?:&(?:'[a-zA-Z0-9_]+\s+)?(?:mut\s+)?)?([A-Z][A-Za-z0-9_]*)$")
ENCLOSING_SUFFIX_RE = re.compile(r"^(.*)::(?:\{(?:closure|coroutine|constant)#\d+\}|promoted\[\d+\])$")
CALL_TERM_SUFFIX_RE = re.compile(r"\s+->\s+\[(?:return:|unwind\b)")
NON_CALL_TERM_PREFIXES = (
    "drop(",
    "goto ",
    "switchInt(",
    "assert(",
    "falseEdge ",
    "falseUnwind ",
    "yield(",
    "asm!(",
    "inline_asm",
)


class LedgerError(Exception):
    """Fail-closed validation or parsing error."""


@dataclass(frozen=True)
class LedgerRow:
    user_id: str
    user_label: str
    kind: str
    target_id: str
    target_side: str
    detail: str
    configs: str
    sites: int
    example_loc: str
    status: str = "unclassified"
    group: str = ""
    note: str = ""

    @property
    def identity(self) -> tuple[str, str, str]:
        return (self.user_id, self.kind, self.target_id)

    def sort_key(self) -> tuple[str, str, str]:
        return (self.kind, self.user_id, self.target_id)

    def as_tsv_fields(self) -> list[str]:
        return [
            self.user_id,
            self.user_label,
            self.kind,
            self.target_id,
            self.target_side,
            self.detail,
            self.configs,
            str(self.sites),
            self.example_loc,
            self.status,
            self.group,
            self.note,
        ]


@dataclass
class Token:
    kind: str
    text: str
    start: int
    end: int
    line: int


def _skip_quoted_literal(text: str, i: int) -> int:
    """Advance past a double-quoted string or Rust char literal at `text[i]`, else return `i`."""
    n = len(text)
    ch = text[i]
    if ch == '"':
        j = i + 1
        while j < n:
            if text[j] == "\\" and j + 1 < n:
                j += 2
                continue
            if text[j] == '"':
                return j + 1
            j += 1
        return n
    if ch == "'":
        # Char literal: `'\''`, `'\n'`, `'\x7f'`, `'\u{...}'`, or `'c'` (not a lifetime `'a` / `'_`)
        if i + 2 < n and text[i + 1] == "\\":
            j = i + 3
            while j < n and text[j] != "'":
                j += 1
            return j + 1 if j < n else i
        if i + 2 < n and text[i + 2] == "'" and text[i + 1] != "'":
            return i + 3
    return i


def split_top_level(text: str, sep: str) -> list[str]:
    """Split text on `sep` at nesting depth 0, skipping `->` arrows and quoted strings/chars."""
    parts: list[str] = []
    cur: list[str] = []
    depth_angle = 0
    depth_paren = 0
    depth_brack = 0
    depth_brace = 0
    i = 0
    n = len(text)
    seplen = len(sep)
    while i < n:
        ch = text[i]
        if ch in ('"', "'"):
            nxt = _skip_quoted_literal(text, i)
            if nxt > i:
                cur.append(text[i:nxt])
                i = nxt
                continue
        if (
            depth_angle == 0
            and depth_paren == 0
            and depth_brack == 0
            and depth_brace == 0
            and text[i : i + seplen] == sep
        ):
            parts.append("".join(cur).strip())
            cur = []
            i += seplen
            continue
        if ch == "<":
            depth_angle += 1
        elif ch == ">":
            if i > 0 and text[i - 1] == "-":
                pass
            elif depth_angle > 0:
                depth_angle -= 1
        elif ch == "(":
            depth_paren += 1
        elif ch == ")":
            if depth_paren > 0:
                depth_paren -= 1
        elif ch == "[":
            depth_brack += 1
        elif ch == "]":
            if depth_brack > 0:
                depth_brack -= 1
        elif ch == "{":
            depth_brace += 1
        elif ch == "}":
            if depth_brace > 0:
                depth_brace -= 1
        cur.append(ch)
        i += 1
    tail = "".join(cur).strip()
    if tail or parts:
        parts.append(tail)
    return parts


def strip_lifetimes(ty: str) -> str:
    """Remove Rust lifetime tokens and normalize spacing without losing type arguments."""
    s = re.sub(r"\bfor\s*<[^>]*>\s*", "", ty)
    s = re.sub(r"&(?:'[a-zA-Z0-9_]+\s+)?(?:mut\s+)?", "", s)
    s = re.sub(r"'[a-zA-Z0-9_]+\s*,\s*", "", s)
    s = re.sub(r",\s*'[a-zA-Z0-9_]+", "", s)
    s = re.sub(r"<\s*'[a-zA-Z0-9_]+\s*>", "", s)
    s = re.sub(r"\s*\+\s*'[a-zA-Z0-9_]+", "", s)
    s = re.sub(r"\s+", " ", s).strip()
    s = re.sub(r"\s*<\s*", "<", s)
    s = re.sub(r"\s*>\s*", ">", s)
    s = re.sub(r"\s*,\s*", ", ", s)
    s = re.sub(r"\s*::\s*", "::", s)
    return s


def strip_all_generics(path: str) -> str:
    """Remove angle-bracket generic argument lists while respecting `->` arrows."""
    out: list[str] = []
    depth = 0
    i = 0
    n = len(path)
    while i < n:
        ch = path[i]
        if ch == "<":
            depth += 1
            i += 1
        elif ch == ">":
            if i > 0 and path[i - 1] == "-":
                if depth == 0:
                    out.append(ch)
            elif depth > 0:
                depth -= 1
            else:
                out.append(ch)
            i += 1
        else:
            if depth == 0:
                out.append(ch)
            i += 1
    res = "".join(out)
    while "::::" in res:
        res = res.replace("::::", "::")
    return res.rstrip(":")


def strip_refs_and_generics(ty: str) -> str:
    """Normalize a MIR type stripping refs/ptrs and generics WITHOUT unwrapping `Box<T>`."""
    s = ty.strip()
    while s.startswith("(") and s.endswith(")"):
        inner = s[1:-1].strip()
        if not inner:
            break
        s = inner
    s = re.sub(r"^(?:&(?:'[a-zA-Z0-9_]+\s+)?(?:mut\s+)?|\*(?:const|mut)\s+)+", "", s)
    return strip_all_generics(s).strip()


def norm_full_type(ty: str) -> str:
    """Normalize a MIR type into its bare crate-relative path without refs or generics."""
    s = ty.strip()
    while s.startswith("(") and s.endswith(")"):
        inner = s[1:-1].strip()
        if not inner:
            break
        s = inner
    s = re.sub(r"^(?:&(?:'[a-zA-Z0-9_]+\s+)?(?:mut\s+)?|\*(?:const|mut)\s+)+", "", s)
    s = s.strip()
    if s.startswith("std::boxed::Box<") or s.startswith("alloc::boxed::Box<") or s.startswith("Box<"):
        m = re.match(r"^(?:(?:std|alloc)::boxed::)?Box<(.+)>$", s)
        if m:
            args = split_top_level(m.group(1), ",")
            if args:
                return norm_full_type(args[0])
    return strip_all_generics(s).strip()


def norm_sig_type(ty: str) -> str:
    """Normalize a type preserving non-lifetime type arguments for unambiguous impl headers."""
    s = ty.strip()
    while s.startswith("(") and s.endswith(")") and "," not in s:
        s = s[1:-1].strip()
    s = re.sub(r"^(?:&(?:'[a-zA-Z0-9_]+\s+)?(?:mut\s+)?|\*(?:const|mut)\s+)+", "", s)
    return strip_lifetimes(s)


def short_type_name(ty: str) -> str:
    """Return the final path segment of a type name."""
    s = norm_full_type(ty)
    if "::" in s:
        return s.rsplit("::", 1)[-1]
    return s


def normalize_trait_name(tr: str) -> str:
    """Normalize a trait path by collapsing whitespace and stripping lifetime params."""
    s = tr.strip()
    if s.startswith("dyn "):
        s = s[4:].strip()
    s = re.sub(r"\s+", " ", s)
    return strip_lifetimes(s)


def short_trait_key(tr: str) -> str:
    """Return the unqualified base name of a trait."""
    s = strip_all_generics(normalize_trait_name(tr))
    if "::" in s:
        return s.rsplit("::", 1)[-1]
    return s


def is_generic_type_param(ty: str) -> bool:
    """Return True when a MIR type looks like an unsubstituted generic parameter."""
    s = ty.strip()
    while s.startswith("(") and s.endswith(")"):
        s = s[1:-1].strip()
    if s.startswith("dyn ") or "::" in s or "{" in s or "[" in s:
        return False
    m = GENERIC_PARAM_RE.match(s)
    if not m:
        return False
    ident = m.group(1)
    return ident not in ("Self", "Game", "GameLevel", "ClientState", "ServerState")


def split_qualified_path(callee: str) -> tuple[str | None, str | None, str, str]:
    """Parse `<SelfTy as Trait>::method::<gen>` or `<SelfTy>::method` or plain path."""
    s = callee.strip()
    if s.startswith("<"):
        depth = 0
        as_pos = -1
        close_pos = -1
        for i, ch in enumerate(s):
            if ch == "<":
                depth += 1
            elif ch == ">":
                if i > 0 and s[i - 1] == "-":
                    continue
                depth -= 1
                if depth == 0:
                    close_pos = i
                    break
            elif depth == 1 and s[i : i + 4] == " as " and as_pos == -1:
                as_pos = i
        if close_pos != -1 and s[close_pos + 1 :].startswith("::"):
            inner = s[1:close_pos]
            rest = s[close_pos + 3 :]
            method = rest.split("::<", 1)[0]
            mgen = rest[len(method) :]
            if as_pos != -1:
                self_ty = s[1:as_pos].strip()
                trait_ty = s[as_pos + 4 : close_pos].strip()
                return (self_ty, trait_ty, method, mgen)
            return (inner.strip(), None, method, mgen)
    return (None, None, s, "")


def normalize_loc_path(raw_path: str, root: Path) -> str:
    """Convert an absolute, toolchain, registry, or build-dir path into a portable location."""
    p = raw_path.replace("\\", "/")
    root_str = root.resolve().as_posix().rstrip("/") + "/"
    if p.startswith(root_str):
        p = p[len(root_str) :]
    m = re.search(r"(?:^|/)target/(?:[^/]+/)*build/([^/]+)/out/(.+)$", p)
    if m:
        return f"${{OUT_DIR}}/{m.group(2)}"
    m = re.search(r"(?:rustlib/src/rust|/rustc/[0-9a-f]+)/(library/.+)$", p)
    if m:
        return f"${{RUST_SRC}}/{m.group(1)}"
    m = re.search(r"\.cargo/registry/src/[^/]+/(.+)$", p)
    if m:
        return f"${{CARGO_REG}}/{m.group(1)}"
    if p.startswith("./"):
        p = p[2:]
    return p


def infer_crate_from_path(raw_path: str) -> str:
    """Infer the Rust crate path prefix for `$crate::` expansion from a source file path."""
    p = raw_path.replace("\\", "/")
    m = re.search(r"(?:^|/)crates/([^/]+)/", p)
    if m:
        cname = m.group(1)
        return CRATE_DIR_TO_MOD.get(cname, cname.replace("-", "_"))
    m = re.search(r"(?:^|/)build/([a-zA-Z0-9_-]+)-[0-9a-f]{8,}/out/", p)
    if m:
        cname = m.group(1)
        return CRATE_DIR_TO_MOD.get(cname, cname.replace("-", "_"))
    return "crate"


def lex_rust(src: str) -> list[Token]:
    """Tokenize Rust source skipping comments, handling raw/escaped strings and nested blocks."""
    tokens: list[Token] = []
    i = 0
    n = len(src)
    line = 1
    while i < n:
        ch = src[i]
        if ch in " \t\r":
            i += 1
            continue
        if ch == "\n":
            line += 1
            i += 1
            continue
        if ch == "/" and i + 1 < n and src[i + 1] == "/":
            nl = src.find("\n", i + 2)
            i = n if nl == -1 else nl
            continue
        if ch == "/" and i + 1 < n and src[i + 1] == "*":
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
                raise LedgerError(f"unclosed block comment starting at line {start_line}")
            continue
        # Raw strings r"...", r#"..."#, br#"..."#, cr#"..."#
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
                start_line = line
                j += 1
                closing = '"' + ("#" * hashes)
                end_pos = src.find(closing, j)
                if end_pos == -1:
                    raise LedgerError(f"unclosed raw string starting at line {start_line}")
                end_idx = end_pos + len(closing)
                line += src.count("\n", start, end_idx)
                tokens.append(Token("STR", src[start:end_idx], start, end_idx, start_line))
                i = end_idx
                continue
        # Raw identifiers r#ident
        if ch == "r" and i + 2 < n and src[i + 1] == "#" and (src[i + 2].isalpha() or src[i + 2] == "_"):
            start = i
            j = i + 2
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(Token("IDENT", src[start:j], start, j, line))
            i = j
            continue
        # Regular, byte, or C string literals
        if ch == '"' or (ch in ("b", "c") and i + 1 < n and src[i + 1] == '"'):
            start = i
            start_line = line
            j = i + (2 if ch in ("b", "c") else 1)
            while j < n:
                if src[j] == "\\":
                    if j + 1 < n and src[j + 1] == "\n":
                        line += 1
                    j += 2
                    continue
                if src[j] == "\n":
                    line += 1
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            else:
                raise LedgerError(f"unclosed string literal starting at line {start_line}")
            tokens.append(Token("STR", src[start:j], start, j, start_line))
            i = j
            continue
        # Byte char literal b'x' or b'\''
        if ch == "b" and i + 1 < n and src[i + 1] == "'":
            start = i
            j = i + 2
            if j < n and src[j] == "\\":
                j += 2
                while j < n and src[j] != "'":
                    j += 1
                if j < n:
                    j += 1
            elif j + 1 < n and src[j + 1] == "'":
                j += 2
            tokens.append(Token("CHAR", src[start:j], start, j, line))
            i = j
            continue
        # Char literal vs lifetime
        if ch == "'":
            start = i
            if i + 2 < n and src[i + 1] == "\\":
                j = i + 3
                while j < n and src[j] != "'" and src[j] != "\n":
                    j += 1
                if j < n and src[j] == "'":
                    j += 1
                    tokens.append(Token("CHAR", src[start:j], start, j, line))
                    i = j
                    continue
            if i + 2 < n and src[i + 1] != "'" and src[i + 1] != "\n" and src[i + 2] == "'":
                tokens.append(Token("CHAR", src[start : i + 3], start, i + 3, line))
                i += 3
                continue
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(Token("LIFETIME", src[start:j], start, j, line))
            i = j
            continue
        # Identifier or keyword
        if ch.isalpha() or ch == "_":
            start = i
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(Token("IDENT", src[start:j], start, j, line))
            i = j
            continue
        # Numbers
        if ch.isdigit():
            start = i
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            tokens.append(Token("NUM", src[start:j], start, j, line))
            i = j
            continue
        # Multi-character punctuation
        if i + 1 < n and src[i : i + 2] in ("::", "->", "=>", "..", "==", "!=", "<=", ">=", "&&", "||"):
            tokens.append(Token("PUNCT", src[i : i + 2], i, i + 2, line))
            i += 2
            continue
        tokens.append(Token("PUNCT", ch, i, i + 1, line))
        i += 1
    return tokens


def eval_cfg_expr(expr: str, features: frozenset[str], host_env: dict[str, Any]) -> bool:
    """Evaluate a Rust `cfg(...)` predicate against feature set and host platform."""
    s = expr.strip()
    if not s:
        return True
    if s.startswith("not(") and s.endswith(")"):
        return not eval_cfg_expr(s[4:-1], features, host_env)
    if s.startswith("all(") and s.endswith(")"):
        parts = [p for p in split_top_level(s[4:-1], ",") if p]
        return all(eval_cfg_expr(p, features, host_env) for p in parts)
    if s.startswith("any(") and s.endswith(")"):
        parts = [p for p in split_top_level(s[4:-1], ",") if p]
        return any(eval_cfg_expr(p, features, host_env) for p in parts)
    m = re.match(r'^feature\s*=\s*"([^"]+)"$', s)
    if m:
        return m.group(1) in features
    m = re.match(r'^target_os\s*=\s*"([^"]+)"$', s)
    if m:
        return m.group(1) == host_env["target_os"]
    m = re.match(r'^target_arch\s*=\s*"([^"]+)"$', s)
    if m:
        return m.group(1) == host_env["target_arch"]
    m = re.match(r'^target_family\s*=\s*"([^"]+)"$', s)
    if m:
        return m.group(1) == host_env["target_family"]
    if s == "unix":
        return host_env["target_family"] == "unix"
    if s == "windows":
        return host_env["target_family"] == "windows"
    if s == "debug_assertions":
        return True
    if s == "test":
        return False
    return False


def current_host_cfg_env() -> dict[str, str]:
    """Return host target_os, target_arch, and target_family matching rustc defaults."""
    plat = sys.platform
    if plat.startswith("linux"):
        tos, tfam = "linux", "unix"
    elif plat == "darwin":
        tos, tfam = "macos", "unix"
    elif plat in ("win32", "cygwin"):
        tos, tfam = "windows", "windows"
    else:
        tos, tfam = plat, "unix"
    mach = platform.machine().lower()
    arch_map = {"x86_64": "x86_64", "amd64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64"}
    tarch = arch_map.get(mach, mach)
    return {"target_os": tos, "target_arch": tarch, "target_family": tfam}


def parse_struct_fields_cfg(
    path: Path,
    struct_name: str,
    features: frozenset[str],
    host_env: dict[str, Any],
) -> list[tuple[str, str]]:
    """Parse active fields of a Rust struct honoring `#[cfg(...)]` attributes."""
    src = path.read_text(encoding="utf-8")
    toks = lex_rust(src)
    n = len(toks)
    i = 0
    while i < n:
        if toks[i].kind == "IDENT" and toks[i].text == "struct":
            if i + 1 < n and toks[i + 1].kind == "IDENT" and toks[i + 1].text == struct_name:
                j = i + 2
                while j < n and toks[j].text != "{" and toks[j].text != ";":
                    j += 1
                if j < n and toks[j].text == "{":
                    return _parse_struct_body_fields(src, toks, j + 1, features, host_env)
        i += 1
    raise LedgerError(f"{path}: struct `{struct_name}` not found")


def _parse_struct_body_fields(
    src: str,
    toks: list[Token],
    start_idx: int,
    features: frozenset[str],
    host_env: dict[str, Any],
) -> list[tuple[str, str]]:
    fields: list[tuple[str, str]] = []
    n = len(toks)
    i = start_idx
    while i < n and toks[i].text != "}":
        cfg_preds: list[str] = []
        while i < n and toks[i].text == "#" and i + 1 < n and toks[i + 1].text == "[":
            attr_start = toks[i + 1].start + 1
            depth = 1
            j = i + 2
            while j < n and depth > 0:
                if toks[j].text == "[":
                    depth += 1
                elif toks[j].text == "]":
                    depth -= 1
                j += 1
            attr_text = src[attr_start : toks[j - 1].start].strip()
            if attr_text.startswith("cfg(") and attr_text.endswith(")"):
                cfg_preds.append(attr_text[4:-1].strip())
            i = j
        if i >= n or toks[i].text == "}":
            break
        # Optional visibility `pub` / `pub(...)`
        if toks[i].kind == "IDENT" and toks[i].text == "pub":
            i += 1
            if i < n and toks[i].text == "(":
                depth = 1
                i += 1
                while i < n and depth > 0:
                    if toks[i].text == "(":
                        depth += 1
                    elif toks[i].text == ")":
                        depth -= 1
                    i += 1
        if i >= n or toks[i].kind != "IDENT":
            raise LedgerError(f"unexpected token {toks[i].text!r} at line {toks[i].line} in struct body")
        fname = toks[i].text
        i += 1
        if i >= n or toks[i].text != ":":
            raise LedgerError(f"expected `:` after field `{fname}` at line {toks[i - 1].line}")
        i += 1
        ty_start = toks[i].start if i < n else len(src)
        depth_a = depth_p = depth_b = depth_c = 0
        while i < n:
            t = toks[i].text
            if depth_a == 0 and depth_p == 0 and depth_b == 0 and depth_c == 0 and t in (",", "}"):
                break
            if t == "<":
                depth_a += 1
            elif t == ">":
                if depth_a > 0:
                    depth_a -= 1
            elif t == "(":
                depth_p += 1
            elif t == ")":
                depth_p = max(0, depth_p - 1)
            elif t == "[":
                depth_b += 1
            elif t == "]":
                depth_b = max(0, depth_b - 1)
            elif t == "{":
                depth_c += 1
            elif t == "}":
                depth_c = max(0, depth_c - 1)
            i += 1
        ty_end = toks[i].start if i < n else len(src)
        fty = " ".join(src[ty_start:ty_end].split())
        if all(eval_cfg_expr(p, features, host_env) for p in cfg_preds):
            fields.append((fname, fty))
        if i < n and toks[i].text == ",":
            i += 1
    return fields


def load_module_sides(path: Path) -> dict[str, str]:
    """Load `tools/game_module_sides.tsv`, failing closed on schema or row errors."""
    if not path.is_file():
        raise LedgerError(f"missing module sides table: {path}")
    raw = path.read_bytes()
    if b"\r" in raw or b"\x00" in raw:
        raise LedgerError(f"{path}: contains CR or NUL bytes")
    rows: dict[str, str] = {}
    expected = ("module", "side", "reason", "paths")
    reader = csv.reader(raw.decode("utf-8").splitlines(), delimiter="\t")
    header = next(reader, None)
    if header != list(expected):
        raise LedgerError(f"{path}: expected header {list(expected)}, got {header}")
    for lineno, r in enumerate(reader, 2):
        if not any(c.strip() for c in r):
            continue
        if len(r) != len(expected):
            raise LedgerError(f"{path}:{lineno}: expected {len(expected)} columns, got {len(r)}")
        mod, side = r[0].strip(), r[1].strip()
        if not mod or side not in ("client", "server", "game"):
            raise LedgerError(f"{path}:{lineno}: invalid module row ({mod!r}, {side!r})")
        if mod in rows:
            raise LedgerError(f"{path}:{lineno}: duplicate module {mod!r}")
        rows[mod] = side
    if not rows:
        raise LedgerError(f"{path}: table is empty")
    return rows


def load_fn_sides(path: Path) -> dict[tuple[str, str, str, str], str]:
    """Load `tools/game_fn_sides.tsv`, failing closed on schema or row errors."""
    if not path.is_file():
        raise LedgerError(f"missing function sides table: {path}")
    raw = path.read_bytes()
    if b"\r" in raw or b"\x00" in raw:
        raise LedgerError(f"{path}: contains CR or NUL bytes")
    rows: dict[tuple[str, str, str, str], str] = {}
    expected = ("module", "impl", "name", "cfg", "side", "reason")
    reader = csv.reader(raw.decode("utf-8").splitlines(), delimiter="\t")
    header = next(reader, None)
    if header != list(expected):
        raise LedgerError(f"{path}: expected header {list(expected)}, got {header}")
    for lineno, r in enumerate(reader, 2):
        if not any(c.strip() for c in r):
            continue
        if len(r) != len(expected):
            raise LedgerError(f"{path}:{lineno}: expected {len(expected)} columns, got {len(r)}")
        mod, impl, name, cfg, side = (r[i].strip() for i in range(5))
        if not mod or not impl or not name or not cfg or side not in ("client", "server", "game"):
            raise LedgerError(f"{path}:{lineno}: invalid function row ({mod!r}, {name!r}, {side!r})")
        key = (mod, impl, name, cfg)
        if key in rows:
            raise LedgerError(f"{path}:{lineno}: duplicate function key {key!r}")
        rows[key] = side
    if not rows:
        raise LedgerError(f"{path}: table is empty")
    return rows


def load_field_owners(path: Path) -> dict[str, dict[str, str]]:
    """Load `tools/game_field_owners.tsv`, failing closed on schema or row errors."""
    if not path.is_file():
        raise LedgerError(f"missing field owners table: {path}")
    raw = path.read_bytes()
    if b"\r" in raw or b"\x00" in raw:
        raise LedgerError(f"{path}: contains CR or NUL bytes")
    rows: dict[str, dict[str, str]] = {}
    expected = ("field", "type", "owner", "reason", "declaration_index", "visibility", "wrapper")
    reader = csv.reader(raw.decode("utf-8").splitlines(), delimiter="\t")
    header = next(reader, None)
    if header != list(expected):
        raise LedgerError(f"{path}: expected header {list(expected)}, got {header}")
    for lineno, r in enumerate(reader, 2):
        if not any(c.strip() for c in r):
            continue
        if len(r) != len(expected):
            raise LedgerError(f"{path}:{lineno}: expected {len(expected)} columns, got {len(r)}")
        field_name = r[0].strip()
        owner = r[2].strip()
        vis = r[5].strip()
        wrapper = r[6].strip()
        if not field_name or owner not in ("client", "server", "game"):
            raise LedgerError(f"{path}:{lineno}: invalid field owner ({field_name!r}, {owner!r})")
        if vis not in ("-", "side", "game"):
            raise LedgerError(f"{path}:{lineno}: invalid visibility {vis!r} for {field_name!r}")
        if wrapper not in ("-", "client_only", "client_shared", "server_only", "server_shared"):
            raise LedgerError(f"{path}:{lineno}: invalid wrapper {wrapper!r} for {field_name!r}")
        if field_name in rows:
            raise LedgerError(f"{path}:{lineno}: duplicate field {field_name!r}")
        rows[field_name] = {
            "field": field_name,
            "type": r[1].strip(),
            "owner": owner,
            "reason": r[3].strip(),
            "declaration_index": r[4].strip(),
            "visibility": vis,
            "wrapper": wrapper,
        }
    if not rows:
        raise LedgerError(f"{path}: table is empty")
    return rows


def load_ledger_tsv_text(text: str, label: str) -> tuple[list[LedgerRow], dict[tuple[str, str, str], int]]:
    """Parse and strictly validate a ledger TSV string, returning rows and line numbers."""
    if "\r" in text or "\x00" in text:
        raise LedgerError(f"{label}: contains CR or NUL bytes")
    lines = text.splitlines()
    if not lines:
        raise LedgerError(f"{label}: file is empty")
    header = lines[0].split("\t")
    if header != list(LEDGER_COLUMNS):
        raise LedgerError(f"{label}:1: expected header {list(LEDGER_COLUMNS)}, got {header}")
    rows: list[LedgerRow] = []
    line_map: dict[tuple[str, str, str], int] = {}
    for lineno, raw_line in enumerate(lines[1:], 2):
        if not raw_line:
            raise LedgerError(f"{label}:{lineno}: blank line inside TSV")
        cols = raw_line.split("\t")
        if len(cols) != len(LEDGER_COLUMNS):
            raise LedgerError(f"{label}:{lineno}: expected {len(LEDGER_COLUMNS)} columns, got {len(cols)}")
        (
            user_id,
            user_label,
            kind,
            target_id,
            target_side,
            detail,
            configs,
            sites_raw,
            example_loc,
            status,
            group,
            note,
        ) = cols
        if not user_id or not target_id or not detail or not configs or not example_loc:
            raise LedgerError(f"{label}:{lineno}: empty required column in row {cols[:5]!r}")
        if "<impl at " in user_id or "<impl at " in target_id:
            raise LedgerError(f"{label}:{lineno}: unstable `<impl at ...>` span in identity")
        if user_label not in VALID_LABELS:
            raise LedgerError(f"{label}:{lineno}: invalid user_label {user_label!r}")
        if kind not in VALID_KINDS:
            raise LedgerError(f"{label}:{lineno}: invalid kind {kind!r}")
        if target_side not in VALID_TARGET_SIDES:
            raise LedgerError(f"{label}:{lineno}: invalid target_side {target_side!r}")
        try:
            sites = int(sites_raw)
        except ValueError as exc:
            raise LedgerError(f"{label}:{lineno}: non-integer sites {sites_raw!r}") from exc
        if sites < 1:
            raise LedgerError(f"{label}:{lineno}: sites must be >= 1, got {sites}")
        if status not in VALID_STATUSES:
            raise LedgerError(f"{label}:{lineno}: invalid status {status!r}; expected one of {sorted(VALID_STATUSES)}")
        if group not in VALID_GROUPS:
            raise LedgerError(f"{label}:{lineno}: invalid group {group!r}; expected one of {sorted(VALID_GROUPS)}")
        row = LedgerRow(
            user_id=user_id,
            user_label=user_label,
            kind=kind,
            target_id=target_id,
            target_side=target_side,
            detail=detail,
            configs=configs,
            sites=sites,
            example_loc=example_loc,
            status=status,
            group=group,
            note=note,
        )
        ident = row.identity
        if ident in line_map:
            raise LedgerError(f"{label}:{lineno}: duplicate row identity {ident} (first at line {line_map[ident]})")
        line_map[ident] = lineno
        rows.append(row)
    return rows, line_map


def load_ledger_tsv(path: Path) -> tuple[list[LedgerRow], dict[tuple[str, str, str], int]]:
    """Read and strictly validate a ledger TSV file from disk."""
    if not path.is_file():
        raise LedgerError(f"missing ledger table: {path}")
    return load_ledger_tsv_text(path.read_text(encoding="utf-8"), str(path))


def format_ledger_tsv(rows: list[LedgerRow]) -> str:
    """Serialize sorted ledger rows into canonical LF-terminated TSV text."""
    ordered = sorted(rows, key=lambda r: r.sort_key())
    seen: set[tuple[str, str, str]] = set()
    out_lines = ["\t".join(LEDGER_COLUMNS)]
    for r in ordered:
        if r.identity in seen:
            raise LedgerError(f"internal error: duplicate generated identity {r.identity}")
        seen.add(r.identity)
        fields = r.as_tsv_fields()
        for fval in fields:
            if "\t" in fval or "\n" in fval or "\r" in fval:
                raise LedgerError(f"illegal control character in cell {fval!r} for {r.identity}")
        out_lines.append("\t".join(fields))
    return "\n".join(out_lines) + "\n"


def atomic_write_text(path: Path, text: str) -> bool:
    """Atomically replace `path` with `text`, returning False if bytes are already identical."""
    data = text.encode("utf-8")
    if path.is_file() and path.read_bytes() == data:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=str(path.parent))
    tmp_path = Path(tmp_name)
    try:
        with os.fdopen(fd, "wb") as fp:
            fp.write(data)
            fp.flush()
            os.fsync(fp.fileno())
        os.replace(tmp_path, path)
    except Exception:
        try:
            tmp_path.unlink(missing_ok=True)
        except OSError:
            pass
        raise
    return True


class SourceCache:
    """Lazy line cache for resolving `<impl at file:line:col>` spans to impl headers."""

    def __init__(self, root: Path, out_dirs: list[Path] | None = None):
        self.root = root.resolve()
        self.out_dirs = [p.resolve() for p in (out_dirs or []) if p.is_dir()]
        self._lines: dict[str, list[str]] = {}
        self._impl_cache: dict[tuple[str, int, int, int, int], dict[str, str]] = {}

    def _resolve_path(self, rel_or_abs: str) -> Path | None:
        p = Path(rel_or_abs)
        if p.is_absolute():
            return p if p.is_file() else None
        cand = self.root / rel_or_abs
        if cand.is_file():
            return cand
        norm = normalize_loc_path(rel_or_abs, self.root)
        if norm.startswith("${OUT_DIR}/"):
            sub = norm[len("${OUT_DIR}/") :]
            for od in self.out_dirs:
                oc = od / sub
                if oc.is_file():
                    return oc
        return None

    def get_lines(self, raw_path: str) -> list[str]:
        if raw_path not in self._lines:
            resolved = self._resolve_path(raw_path)
            if resolved is None:
                self._lines[raw_path] = []
            else:
                self._lines[raw_path] = resolved.read_text(encoding="utf-8", errors="replace").splitlines()
        return self._lines[raw_path]

    def resolve_impl_span(self, raw_path: str, l1: int, c1: int, l2: int, c2: int) -> dict[str, str]:
        """Resolve an `<impl at path:l1:c1: l2:c2>` span into trait/self metadata."""
        key = (raw_path, l1, c1, l2, c2)
        cached = self._impl_cache.get(key)
        if cached is not None:
            return cached
        crate_mod = infer_crate_from_path(raw_path)
        norm_file = normalize_loc_path(raw_path, self.root)
        lines = self.get_lines(raw_path)
        info: dict[str, str] = {"file": norm_file, "line": str(l1), "kind": "unknown", "trait": "", "self_ty": "", "raw": ""}
        if lines and 1 <= l1 <= len(lines):
            end_l = min(len(lines), max(l2, l1) + 4)
            parts: list[str] = []
            for ln in range(l1, end_l + 1):
                s = lines[ln - 1]
                if ln == l1 and c1 > 1:
                    s = s[c1 - 1 :]
                parts.append(s)
                if "{" in s or ln >= l2:
                    if ln >= l2 and ("{" in "".join(parts) or ")" in "".join(parts) or "impl" not in "".join(parts)):
                        break
            snippet = " ".join(" ".join(parts).split())
            info["raw"] = snippet[:180]
            if snippet.startswith("impl"):
                hdr = snippet.split("{", 1)[0].strip()
                hdr = re.split(r"\bwhere\b", hdr, maxsplit=1)[0].strip()
                rest = hdr[4:].strip()
                # Skip impl generic params `<...>`
                if rest.startswith("<"):
                    depth = 0
                    idx = 0
                    for idx, ch in enumerate(rest):
                        if ch == "<":
                            depth += 1
                        elif ch == ">":
                            if idx > 0 and rest[idx - 1] == "-":
                                continue
                            depth -= 1
                            if depth == 0:
                                rest = rest[idx + 1 :].strip()
                                break
                # Find top-level ` for ` outside `<>` / `()`
                for_pos = _find_top_level_keyword(rest, "for")
                if for_pos != -1:
                    tr_raw = rest[:for_pos].strip()
                    st_raw = rest[for_pos + 3 :].strip()
                    tr_raw = tr_raw.replace("$crate::", f"{crate_mod}::")
                    st_raw = st_raw.replace("$crate::", f"{crate_mod}::")
                    info["kind"] = "trait"
                    info["trait"] = normalize_trait_name(tr_raw)
                    info["self_ty"] = norm_sig_type(st_raw)
                else:
                    st_raw = rest.replace("$crate::", f"{crate_mod}::")
                    info["kind"] = "inherent"
                    info["self_ty"] = norm_sig_type(st_raw)
            else:
                # Derive macro span on #[derive(Trait)] or macro invocation
                raw_line = lines[l1 - 1]
                col_txt = raw_line[max(0, c1 - 1) : max(c2 - 1, c1)] if l1 == l2 else snippet
                col_txt = col_txt.strip()
                if "#[derive" in raw_line or re.match(r"^[A-Za-z_][A-Za-z0-9_:]*$", col_txt):
                    info["kind"] = "derive"
                    info["trait"] = normalize_trait_name(col_txt.replace("$crate::", f"{crate_mod}::"))
                else:
                    info["kind"] = "macro"
        self._impl_cache[key] = info
        return info


def _find_top_level_keyword(text: str, kw: str) -> int:
    """Find whitespace-delimited keyword `kw` at bracket depth 0."""
    depth_a = depth_p = depth_b = 0
    n = len(text)
    klen = len(kw)
    i = 0
    while i < n:
        ch = text[i]
        if ch == "<":
            depth_a += 1
        elif ch == ">":
            if i > 0 and text[i - 1] == "-":
                pass
            elif depth_a > 0:
                depth_a -= 1
        elif ch == "(":
            depth_p += 1
        elif ch == ")":
            depth_p = max(0, depth_p - 1)
        elif ch == "[":
            depth_b += 1
        elif ch == "]":
            depth_b = max(0, depth_b - 1)
        elif depth_a == 0 and depth_p == 0 and depth_b == 0:
            if text[i : i + klen] == kw:
                before_ok = i == 0 or text[i - 1].isspace()
                after_ok = i + klen == n or text[i + klen].isspace()
                if before_ok and after_ok:
                    return i
        i += 1
    return -1


def extract_projections_from_expr(expr: str, cur_locals: dict[str, str]) -> list[tuple[str, int, str, bool]]:
    """Extract all `(base_ty, field_idx, field_ty, is_deref_base)` projections in a MIR expression."""
    out: list[tuple[str, int, str, bool]] = []
    if ")." not in expr:
        return out
    n = len(expr)
    i = 0
    # Match `(<base>).<idx>: <fty>` by scanning for `).<digits>: `
    while i < n:
        pos = expr.find(").", i)
        if pos == -1:
            break
        j = pos + 2
        if j >= n or not expr[j].isdigit():
            i = pos + 2
            continue
        idx_start = j
        while j < n and expr[j].isdigit():
            j += 1
        if j + 1 >= n or expr[j : j + 2] != ": ":
            i = j
            continue
        field_idx = int(expr[idx_start:j])
        # Find matching `(` to the left of `pos`
        depth = 1
        left = pos - 1
        while left >= 0 and depth > 0:
            c = expr[left]
            if c == ")":
                depth += 1
            elif c == "(":
                depth -= 1
            left -= 1
        if depth != 0:
            i = j
            continue
        base_expr = expr[left + 2 : pos].strip()
        # Find matching `)` to the right of `j + 2` (wrapping outer `(<base>.<idx>: <fty>)`)
        ft_start = j + 2
        depth_p = 0
        depth_a = 0
        depth_b = 0
        depth_c = 0
        k = ft_start
        while k < n:
            c = expr[k]
            if c == "(":
                depth_p += 1
            elif c == ")":
                if depth_p == 0 and depth_a == 0 and depth_b == 0 and depth_c == 0:
                    break
                depth_p = max(0, depth_p - 1)
            elif c == "<":
                depth_a += 1
            elif c == ">":
                if k > 0 and expr[k - 1] == "-":
                    pass
                elif depth_a > 0:
                    depth_a -= 1
            elif c == "[":
                depth_b += 1
            elif c == "]":
                depth_b = max(0, depth_b - 1)
            elif c == "{":
                depth_c += 1
            elif c == "}":
                depth_c = max(0, depth_c - 1)
            elif c in (";", ",") and depth_p == 0 and depth_a == 0 and depth_b == 0 and depth_c == 0:
                break
            k += 1
        field_ty = expr[ft_start:k].strip()
        base_ty, is_deref = _infer_place_base_type(base_expr, cur_locals)
        if base_ty:
            out.append((base_ty, field_idx, field_ty, is_deref))
        i = j + 2
    return out


def _infer_place_base_type(base_expr: str, cur_locals: dict[str, str]) -> tuple[str, bool]:
    """Infer the Rust type of a MIR place base expression like `*_1` or `((*_1).222: ClientState)`."""
    s = base_expr.strip()
    is_deref = False
    while s.startswith("(") and s.endswith(")"):
        # Ensure outer parens match each other
        depth = 0
        matches_end = True
        for idx, ch in enumerate(s):
            if ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
                if depth == 0 and idx != len(s) - 1:
                    matches_end = False
                    break
        if matches_end:
            s = s[1:-1].strip()
        else:
            break
    if s.startswith("*"):
        is_deref = True
        inner = s[1:].strip()
        if re.match(r"^_\d+$", inner):
            return (cur_locals.get(inner, ""), True)
        inner_ty, _ = _infer_place_base_type(inner, cur_locals)
        return (inner_ty, True)
    if re.match(r"^_\d+$", s):
        return (cur_locals.get(s, ""), False)
    # Nested projection `((... ).N: Type)` -> extract trailing `: Type` at depth 0
    depth_p = depth_a = depth_b = depth_c = 0
    colon_pos = -1
    for idx, ch in enumerate(s):
        if ch == "(":
            depth_p += 1
        elif ch == ")":
            depth_p = max(0, depth_p - 1)
        elif ch == "<":
            depth_a += 1
        elif ch == ">":
            if idx > 0 and s[idx - 1] == "-":
                pass
            elif depth_a > 0:
                depth_a -= 1
        elif ch == "[":
            depth_b += 1
        elif ch == "]":
            depth_b = max(0, depth_b - 1)
        elif ch == "{":
            depth_c += 1
        elif ch == "}":
            depth_c = max(0, depth_c - 1)
        elif ch == ":" and depth_p == 0 and depth_a == 0 and depth_b == 0 and depth_c == 0:
            if idx + 1 < len(s) and s[idx + 1] == " ":
                colon_pos = idx
    if colon_pos != -1:
        return (s[colon_pos + 2 :].strip(), is_deref or "(*" in s)
    return ("", is_deref)


def _split_top_level_assign(stmt: str) -> tuple[str, str] | None:
    """Split a MIR assignment statement into `(lhs, rhs)` around top-level ` = `."""
    depth_p = depth_a = depth_b = depth_c = 0
    n = len(stmt)
    i = 0
    while i < n:
        ch = stmt[i]
        if ch in ('"', "'"):
            nxt = _skip_quoted_literal(stmt, i)
            if nxt > i:
                i = nxt
                continue
        if ch == "(":
            depth_p += 1
        elif ch == ")":
            depth_p = max(0, depth_p - 1)
        elif ch == "<":
            depth_a += 1
        elif ch == ">":
            if i > 0 and stmt[i - 1] == "-":
                pass
            elif depth_a > 0:
                depth_a -= 1
        elif ch == "[":
            depth_b += 1
        elif ch == "]":
            depth_b = max(0, depth_b - 1)
        elif ch == "{":
            depth_c += 1
        elif ch == "}":
            depth_c = max(0, depth_c - 1)
        elif (
            depth_p == 0
            and depth_a == 0
            and depth_b == 0
            and depth_c == 0
            and stmt[i : i + 3] == " = "
        ):
            return (stmt[:i].strip(), stmt[i + 3 :].strip())
        i += 1
    return None


def _rhs_access_mode(rhs: str) -> str:
    """Classify the MIR access mode of projections on an assignment RHS."""
    r = rhs.lstrip()
    if r.startswith("&mut ") or r.startswith("&raw mut "):
        return "borrow-mut"
    if r.startswith("move "):
        return "write"
    return "read"


def _parse_call_terminator(stmt: str, cur_locals: dict[str, str]) -> dict[str, Any] | None:
    """Parse a MIR call terminator `dest = callee(args...) -> [return: ..., unwind: ...]`."""
    m_ct = CALL_TERM_SUFFIX_RE.search(stmt)
    if m_ct is None:
        return None
    lhs_rhs = _split_top_level_assign(stmt[: m_ct.start()])
    if lhs_rhs is None:
        return None
    _, call_expr = lhs_rhs
    if not call_expr.endswith(")"):
        return None
    # Match outer argument paren `(...)` scanning forward while skipping string/char literals
    depth_p = 0
    depth_a = 0
    depth_b = 0
    depth_c = 0
    open_p = -1
    i = 0
    n = len(call_expr)
    while i < n:
        ch = call_expr[i]
        if ch in ('"', "'"):
            nxt = _skip_quoted_literal(call_expr, i)
            if nxt > i:
                i = nxt
                continue
        if ch == "<":
            depth_a += 1
        elif ch == ">":
            if i > 0 and call_expr[i - 1] == "-":
                pass
            elif depth_a > 0:
                depth_a -= 1
        elif ch == "[":
            depth_b += 1
        elif ch == "]":
            depth_b = max(0, depth_b - 1)
        elif ch == "{":
            depth_c += 1
        elif ch == "}":
            depth_c = max(0, depth_c - 1)
        elif ch == "(":
            if depth_p == 0 and depth_a == 0 and depth_b == 0 and depth_c == 0:
                open_p = i
            depth_p += 1
        elif ch == ")":
            depth_p -= 1
            if depth_p == 0 and depth_a == 0 and depth_b == 0 and depth_c == 0 and i == n - 1:
                break
        i += 1
    if open_p <= 0 or depth_p != 0:
        return None
    callee_raw = call_expr[:open_p].strip()
    args_raw = call_expr[open_p + 1 : -1].strip()
    arg_exprs = [a for a in split_top_level(args_raw, ",") if a] if args_raw else []
    arg_types: list[str] = []
    arg_closures: list[str] = []
    arg_fn_items: list[str] = []
    for ae in arg_exprs:
        ae_s = ae.strip()
        m_loc = re.search(r"\b(_\d+)\b", ae_s)
        if m_loc:
            lty = cur_locals.get(m_loc.group(1), "")
            arg_types.append(lty)
            for cm in CLOSURE_TY_RE.finditer(lty):
                arg_closures.append(cm.group(0))
        else:
            arg_types.append(ae_s)
            for cm in CLOSURE_TY_RE.finditer(ae_s):
                arg_closures.append(cm.group(0))
            # Bare function-item constant passed as value (e.g. `iter.map(Boat::bounds)`)
            if (
                "::" in ae_s
                and not ae_s.startswith(("const ", "move ", "copy ", "&", "(", "[", "{"))
                and not CLOSURE_TY_RE.search(ae_s)
            ):
                arg_fn_items.append(ae_s)

    # Classify call kind
    m_local_callee = re.match(r"^(?:move |copy )?(_\d+)$", callee_raw)
    if m_local_callee:
        loc_ty = cur_locals.get(m_local_callee.group(1), "")
        if CLOSURE_TY_RE.search(loc_ty):
            return {"kind": "closure_call", "callee": loc_ty, "arg_types": arg_types, "arg_closures": arg_closures, "arg_fn_items": arg_fn_items}
        if is_generic_type_param(loc_ty) or "impl Fn" in loc_ty:
            return {"kind": "generic_fn_upvar", "callee": loc_ty, "arg_types": arg_types, "arg_closures": arg_closures, "arg_fn_items": arg_fn_items}
        return {"kind": "fn_ptr", "callee": loc_ty or callee_raw, "arg_types": arg_types, "arg_closures": arg_closures, "arg_fn_items": arg_fn_items}

    if callee_raw.startswith("<dyn ") or callee_raw.startswith("<(dyn "):
        return {"kind": "dyn", "callee": callee_raw, "arg_types": arg_types, "arg_closures": arg_closures, "arg_fn_items": arg_fn_items}
    if CLOSURE_TY_RE.search(callee_raw) and callee_raw.startswith("<{closure@"):
        return {"kind": "closure_call", "callee": callee_raw, "arg_types": arg_types, "arg_closures": arg_closures, "arg_fn_items": arg_fn_items}
    return {"kind": "static", "callee": callee_raw, "arg_types": arg_types, "arg_closures": arg_closures, "arg_fn_items": arg_fn_items}


def parse_mir_file(
    mir_path: Path,
    root: Path,
    source_cache: SourceCache,
) -> dict[str, Any]:
    """Stream-parse a `-Zunpretty=mir` dump, failing closed on unrecognized headers or calls."""
    if not mir_path.is_file():
        raise LedgerError(f"MIR dump not found: {mir_path}")
    if mir_path.stat().st_size == 0:
        raise LedgerError(f"MIR dump is empty: {mir_path}")

    bodies: list[dict[str, Any]] = []
    ctor_aggregates: dict[str, list[str]] = {}
    skipped_headers: list[tuple[int, str]] = []
    unparsed_call_terms: list[tuple[int, str, str]] = []

    cur: dict[str, Any] | None = None
    in_alloc = False
    brace_depth = 0
    in_ctor_name: str | None = None
    in_ctor_fields: list[str] = []

    def finish_body() -> None:
        nonlocal cur, in_ctor_name, in_ctor_fields
        if cur is not None:
            # Extract fn_items_named from local declarations `fn(...) {path}`
            called_static = {c["callee"] for c in cur["calls"] if c["kind"] == "static"}
            reified_set = {p for p, _ in cur["reified_fn_ptrs"]}
            for lname, lty in cur["locals"].items():
                fm = FN_ITEM_LOCAL_RE.search(lty)
                if fm:
                    fpath = fm.group(1).strip()
                    if fpath not in called_static and fpath not in reified_set:
                        loc = cur["local_locs"].get(lname, f"{cur['source_file']}:{cur['line_start']}")
                        cur["fn_items_named"].append((fpath, loc))
            bodies.append(cur)
        cur = None
        in_ctor_name = None
        in_ctor_fields = []

    with mir_path.open("r", encoding="utf-8", errors="replace") as fp:
        for lineno, raw_line in enumerate(fp, 1):
            line = raw_line.rstrip("\r\n")
            if not line:
                continue
            if in_alloc:
                if line.startswith("}"):
                    in_alloc = False
                continue
            if not line[0].isspace():
                if line.startswith("//"):
                    continue
                if line == "}":
                    finish_body()
                    brace_depth = 0
                    continue
                if ALLOC_START_RE.match(line):
                    finish_body()
                    if line.rstrip().endswith("{"):
                        in_alloc = True
                    continue
                fn_hdr = _parse_top_fn_header(line)
                if fn_hdr is not None:
                    finish_body()
                    dp, args_str, ret_ty = fn_hdr
                    args: list[tuple[str, str]] = []
                    locals_map: dict[str, str] = {"_0": ret_ty}
                    closure_ty = ""
                    if args_str:
                        for part in split_top_level(args_str, ","):
                            if ":" in part:
                                an, at = part.split(":", 1)
                                an, at = an.strip(), at.strip()
                                args.append((an, at))
                                locals_map[an] = at
                                if not closure_ty:
                                    cm = CLOSURE_TY_RE.search(at)
                                    if cm and "::{closure#" in dp:
                                        closure_ty = cm.group(0)
                    bkind = "closure" if "::{closure#" in dp else (
                        "coroutine" if "::{coroutine#" in dp else (
                            "method" if "::<impl at " in dp else "fn"
                        )
                    )
                    impl_info = None
                    impl_spans = list(IMPL_AT_RE.finditer(dp))
                    if impl_spans:
                        last_m = impl_spans[-1]
                        impl_info = source_cache.resolve_impl_span(
                            last_m.group(1),
                            int(last_m.group(2)),
                            int(last_m.group(3)),
                            int(last_m.group(4)),
                            int(last_m.group(5)),
                        )
                    cur = {
                        "def_path": dp,
                        "kind": bkind,
                        "args": args,
                        "ret_type": ret_ty,
                        "locals": locals_map,
                        "local_locs": {},
                        "closure_ty": closure_ty,
                        "impl_info": impl_info,
                        "source_file": "",
                        "line_start": 0,
                        "line_end": 0,
                        "projections": [],
                        "calls": [],
                        "reified_fn_ptrs": [],
                        "fn_items_named": [],
                        "closure_unsizes": [],
                        "closures_named": [],
                        "drops": [],
                    }
                    brace_depth = 1
                    continue
                m_prom = PROMOTED_HEADER_RE.match(line)
                if m_prom:
                    finish_body()
                    dp = f"{m_prom.group(1).strip()}::promoted[{m_prom.group(2)}]"
                    cur = {
                        "def_path": dp,
                        "kind": "promoted",
                        "args": [],
                        "ret_type": m_prom.group(3).strip(),
                        "locals": {"_0": m_prom.group(3).strip()},
                        "local_locs": {},
                        "closure_ty": "",
                        "impl_info": None,
                        "source_file": "",
                        "line_start": 0,
                        "line_end": 0,
                        "projections": [],
                        "calls": [],
                        "reified_fn_ptrs": [],
                        "fn_items_named": [],
                        "closure_unsizes": [],
                        "closures_named": [],
                        "drops": [],
                    }
                    brace_depth = 1
                    continue
                m_cs = TOP_CONST_STATIC_RE.match(line)
                if m_cs:
                    finish_body()
                    bkind = m_cs.group(1)
                    dp = m_cs.group(2).strip()
                    ret_ty = m_cs.group(3).strip()
                    cur = {
                        "def_path": dp,
                        "kind": bkind,
                        "args": [],
                        "ret_type": ret_ty,
                        "locals": {"_0": ret_ty},
                        "local_locs": {},
                        "closure_ty": "",
                        "impl_info": None,
                        "source_file": "",
                        "line_start": 0,
                        "line_end": 0,
                        "projections": [],
                        "calls": [],
                        "reified_fn_ptrs": [],
                        "fn_items_named": [],
                        "closure_unsizes": [],
                        "closures_named": [],
                        "drops": [],
                    }
                    brace_depth = 1
                    continue
                m_ac = ANON_CONST_RE.match(line)
                if m_ac:
                    finish_body()
                    dp = m_ac.group(1).strip()
                    ret_ty = m_ac.group(2).strip()
                    cur = {
                        "def_path": dp,
                        "kind": "const",
                        "args": [],
                        "ret_type": ret_ty,
                        "locals": {"_0": ret_ty},
                        "local_locs": {},
                        "closure_ty": "",
                        "impl_info": None,
                        "source_file": "",
                        "line_start": 0,
                        "line_end": 0,
                        "projections": [],
                        "calls": [],
                        "reified_fn_ptrs": [],
                        "fn_items_named": [],
                        "closure_unsizes": [],
                        "closures_named": [],
                        "drops": [],
                    }
                    brace_depth = 1
                    continue
                # Inline single-line `const ... = ...;` or `static ... = ...;` without body block
                if re.match(r"^(?:pub(?:\([^)]*\))?\s+)?(?:const|static)\b.*=\s*[^\{]+;\s*$", line):
                    finish_body()
                    continue
                skipped_headers.append((lineno, line[:160]))
                continue

            if cur is None:
                continue

            # Track multi-line struct constructor aggregates for Game/ClientState/ServerState
            stripped = line.strip()
            if in_ctor_name is not None:
                for fm in re.finditer(r"(?:^|,)\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*:", stripped):
                    in_ctor_fields.append(fm.group(1))
                if "};" in stripped or stripped.endswith("}"):
                    if in_ctor_name not in ctor_aggregates or len(in_ctor_fields) > len(ctor_aggregates[in_ctor_name]):
                        ctor_aggregates[in_ctor_name] = list(in_ctor_fields)
                    in_ctor_name = None
                    in_ctor_fields = []

            # Extract scope source location if present
            loc_str = ""
            m_loc = SCOPE_LOC_RE.search(line)
            if m_loc:
                rel_f = normalize_loc_path(m_loc.group(1), root)
                ln_num = int(m_loc.group(2))
                loc_str = f"{rel_f}:{ln_num}"
                if not cur["source_file"]:
                    cur["source_file"] = rel_f
                    cur["line_start"] = ln_num
                    cur["line_end"] = ln_num
                elif rel_f == cur["source_file"]:
                    if ln_num < cur["line_start"]:
                        cur["line_start"] = ln_num
                    if ln_num > cur["line_end"]:
                        cur["line_end"] = ln_num

            if stripped.startswith("scope ") and "at " in stripped and not cur["source_file"]:
                ms = re.search(r"at\s+([^:]+):(\d+):\d+:\s*(\d+):\d+", stripped)
                if ms:
                    cur["source_file"] = normalize_loc_path(ms.group(1), root)
                    cur["line_start"] = int(ms.group(2))
                    cur["line_end"] = int(ms.group(3))
                continue

            # Local declarations
            m_ld = LOCAL_DECL_RE.match(line)
            if m_ld:
                lname, lty = m_ld.group(1), m_ld.group(2).strip()
                cur["locals"][lname] = lty
                if loc_str:
                    cur["local_locs"][lname] = loc_str
                for cm in CLOSURE_TY_RE.finditer(lty):
                    ct = cm.group(0)
                    if ct != cur["closure_ty"]:
                        cur["closures_named"].append(ct)
                continue

            if DEBUG_CLOSURE_RE.match(line) or stripped.startswith("scope ") or stripped.startswith("bb"):
                continue

            stmt_no_comment = line.split("//", 1)[0].strip()
            if not stmt_no_comment or stmt_no_comment in ("{", "}"):
                continue

            # Detect struct constructor aggregate for Game / ClientState / ServerState
            if "{" in stmt_no_comment and ":" in stmt_no_comment:
                m_ag = CTOR_AGG_RE.search(stmt_no_comment)
                if m_ag:
                    short_ctor = m_ag.group(1).rsplit("::", 1)[-1]
                    if short_ctor in ("Game", "ClientState", "ServerState"):
                        after_brace = stmt_no_comment[stmt_no_comment.find("{", m_ag.start()) + 1 :]
                        fields_here = [fm.group(1) for fm in re.finditer(r"(?:^|,)\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*:", after_brace)]
                        if "};" in after_brace or after_brace.rstrip().endswith("}"):
                            if short_ctor not in ctor_aggregates or len(fields_here) > len(ctor_aggregates[short_ctor]):
                                ctor_aggregates[short_ctor] = fields_here
                        else:
                            in_ctor_name = short_ctor
                            in_ctor_fields = fields_here

            # ReifyFnPointer coercion
            if "ReifyFnPointer" in stmt_no_comment:
                m_rf = REIFY_FN_RE.search(stmt_no_comment)
                if m_rf:
                    cur["reified_fn_ptrs"].append((m_rf.group(1).strip(), loc_str or f"{cur['source_file']}:{cur['line_start']}"))

            # Unsize coercion of closure to dyn Fn*
            if "PointerCoercion(Unsize)" in stmt_no_comment:
                m_us = UNSIZE_RE.search(stmt_no_comment)
                if m_us:
                    src_expr = m_us.group(1).strip()
                    dst_ty = m_us.group(2).strip()
                    src_ty = cur["locals"].get(src_expr, "")
                    for cm in CLOSURE_TY_RE.finditer(src_ty):
                        cur["closure_unsizes"].append((cm.group(0), dst_ty, loc_str or f"{cur['source_file']}:{cur['line_start']}"))

            # Drop terminators
            m_dr = DROP_TERM_RE.match(stmt_no_comment)
            if m_dr:
                drop_place = m_dr.group(1).strip()
                dty, _ = _infer_place_base_type(drop_place, cur["locals"])
                if dty:
                    cur["drops"].append((dty, loc_str or f"{cur['source_file']}:{cur['line_start']}"))
                if ")." in drop_place:
                    for bt, fidx, fty, is_deref in extract_projections_from_expr(drop_place, cur["locals"]):
                        cur["projections"].append((bt, fidx, fty, "write", is_deref, True, loc_str or f"{cur['source_file']}:{cur['line_start']}"))
                continue

            # Call terminators
            m_ct = CALL_TERM_SUFFIX_RE.search(stmt_no_comment)
            if m_ct is not None:
                if not stmt_no_comment.startswith(NON_CALL_TERM_PREFIXES):
                    call = _parse_call_terminator(stmt_no_comment, cur["locals"])
                    if call is None:
                        unparsed_call_terms.append((lineno, cur["def_path"], stmt_no_comment[:160]))
                    else:
                        call["loc"] = loc_str or f"{cur['source_file']}:{cur['line_start']}"
                        cur["calls"].append(call)
                        for fitem in call.get("arg_fn_items", ()):
                            cur["fn_items_named"].append((fitem, call["loc"]))
                        if ")." in stmt_no_comment:
                            lhs_rhs = _split_top_level_assign(stmt_no_comment[: m_ct.start()])
                            if lhs_rhs:
                                lhs_s, rhs_s = lhs_rhs
                                for bt, fidx, fty, is_deref in extract_projections_from_expr(lhs_s, cur["locals"]):
                                    cur["projections"].append((bt, fidx, fty, "write", is_deref, False, call["loc"]))
                                for bt, fidx, fty, is_deref in extract_projections_from_expr(rhs_s, cur["locals"]):
                                    cur["projections"].append((bt, fidx, fty, "read", is_deref, False, call["loc"]))
                else:
                    if ")." in stmt_no_comment:
                        for bt, fidx, fty, is_deref in extract_projections_from_expr(stmt_no_comment, cur["locals"]):
                            cur["projections"].append((bt, fidx, fty, "read", is_deref, False, loc_str or f"{cur['source_file']}:{cur['line_start']}"))
                continue

            # Regular statements with field projections
            if ")." in stmt_no_comment:
                lhs_rhs = _split_top_level_assign(stmt_no_comment)
                use_loc = loc_str or f"{cur['source_file']}:{cur['line_start']}"
                if lhs_rhs is not None:
                    lhs_s, rhs_s = lhs_rhs
                    for bt, fidx, fty, is_deref in extract_projections_from_expr(lhs_s, cur["locals"]):
                        cur["projections"].append((bt, fidx, fty, "write", is_deref, False, use_loc))
                    rmode = _rhs_access_mode(rhs_s)
                    for bt, fidx, fty, is_deref in extract_projections_from_expr(rhs_s, cur["locals"]):
                        cur["projections"].append((bt, fidx, fty, rmode, is_deref, False, use_loc))
                else:
                    mode = "write" if stmt_no_comment.startswith(("SetDiscriminant", "Deinit")) else "read"
                    for bt, fidx, fty, is_deref in extract_projections_from_expr(stmt_no_comment, cur["locals"]):
                        cur["projections"].append((bt, fidx, fty, mode, is_deref, False, use_loc))

    finish_body()

    if skipped_headers:
        sample = "; ".join(f"line {ln}: {txt}" for ln, txt in skipped_headers[:5])
        raise LedgerError(f"{mir_path}: refused due to {len(skipped_headers)} unrecognized top-level MIR header(s): {sample}")
    if unparsed_call_terms:
        sample = "; ".join(f"line {ln} ({dp}): {txt}" for ln, dp, txt in unparsed_call_terms[:5])
        raise LedgerError(f"{mir_path}: refused due to {len(unparsed_call_terms)} unparsed call terminator(s): {sample}")

    return {
        "bodies": bodies,
        "ctor_aggregates": ctor_aggregates,
        "skipped_headers": len(skipped_headers),
        "unparsed_call_terms": len(unparsed_call_terms),
    }


def dedupe_const_fn_bodies(bodies: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Collapse duplicate MIR bodies rustc emits for `const fn` (CTFE + runtime)."""
    out: list[dict[str, Any]] = []
    seen: dict[tuple[str, str, int, int, tuple[tuple[str, str], ...], str], int] = {}
    for b in bodies:
        key = (b["def_path"], b["source_file"], b["line_start"], b["line_end"], tuple(b["args"]), b["ret_type"])
        if key in seen and b["kind"] in ("fn", "method"):
            continue
        seen[key] = len(out)
        out.append(b)
    return out


class MirIndex:
    """Resolve static calls, closures, drop impls, and stable canonical identities over one MIR dump."""

    def __init__(self, bodies: list[dict[str, Any]], source_cache: SourceCache, module_sides: dict[str, str]):
        self.bodies = dedupe_const_fn_bodies(bodies)
        self.source_cache = source_cache
        self.module_sides = module_sides
        self.local_prefixes = self._discover_local_prefixes()
        self.def_path_to_idx: dict[str, int] = {}
        self.closure_to_idx: dict[str, int] = {}
        self.inherent_by_mod_ty_meth: dict[tuple[str, str, str], list[int]] = defaultdict(list)
        self.inherent_by_mod_shortty_meth: dict[tuple[str, str, str], list[int]] = defaultdict(list)
        self.inherent_by_ty_meth: dict[tuple[str, str], list[int]] = defaultdict(list)
        self.trait_impls_by_ty_trait_meth: dict[tuple[str, str, str], list[int]] = defaultdict(list)
        self.trait_impls_by_ty_shorttrait_meth: dict[tuple[str, str, str], list[int]] = defaultdict(list)
        self.trait_impls_by_shortty_shorttrait_meth: dict[tuple[str, str, str], list[int]] = defaultdict(list)
        self.drop_impls_by_ty: dict[str, list[int]] = defaultdict(list)
        self.nested_fn_index: dict[tuple[str, str, str], list[int]] = defaultdict(list)
        self.nested_fn_by_mod_tail: dict[tuple[str, str], list[int]] = defaultdict(list)
        self.span_to_canon: dict[str, str] = {}
        self.canon_ids: list[str] = [""] * len(self.bodies)
        self.enclosing_idx: list[int] = list(range(len(self.bodies)))
        self._build()

    def _discover_local_prefixes(self) -> set[str]:
        prefixes = {"game", "entry", "orb_limits"}
        for b in self.bodies:
            dp = b["def_path"]
            if "::" in dp:
                top = dp.split("::", 1)[0]
                if top and not top.startswith("<"):
                    prefixes.add(top)
        return prefixes

    def body_label(self, def_path: str) -> str:
        """Compute `client`, `server`, or `common` from the body's logical module path."""
        if def_path == "game::client_side" or def_path.startswith("game::client_side::"):
            return "client"
        if def_path == "game::server_side" or def_path.startswith("game::server_side::"):
            return "server"
        if def_path.startswith("game::"):
            rest = def_path[len("game::") :]
            top_seg = rest.split("::", 1)[0]
            side = self.module_sides.get(top_seg)
            if side in ("client", "server"):
                return side
        return "common"

    def _infer_self_type(self, b: dict[str, Any]) -> tuple[str, str]:
        """Infer `(bare_self_ty, sig_self_ty)` for an impl method from MIR args/return and source."""
        ii = b["impl_info"]
        src_self = ii.get("self_ty", "") if ii else ""
        src_has_meta = "$" in src_self or not src_self
        src_short = "" if src_has_meta else short_type_name(src_self)

        if b["args"]:
            arg1_raw = b["args"][0][1]
            arg1_bare = norm_full_type(arg1_raw)
            arg1_sig = norm_sig_type(arg1_raw)
            if not src_short or short_type_name(arg1_bare) == src_short:
                return (arg1_bare, arg1_sig)

        ret_raw = b["ret_type"]
        ret_bare = norm_full_type(ret_raw)
        ret_sig = norm_sig_type(ret_raw)
        if src_has_meta and ret_bare and ret_bare != "()":
            return (ret_bare, ret_sig)
        if src_short and short_type_name(ret_bare) == src_short:
            return (ret_bare, ret_sig)

        if src_short == "LevelView":
            return ("game::mod_api_host::LevelTicks", "game::mod_api_host::LevelTicks")
        if src_short == "Game":
            return ("game::Game", "game::Game")
        if src_short == "GameLevel":
            if "natural_spawning" in b["def_path"] and "super::GameLevel" not in src_self and "crate::game::GameLevel" not in src_self:
                return ("game::natural_spawning::GameLevel", "game::natural_spawning::GameLevel")
            return ("game::GameLevel", "game::GameLevel")
        if src_short == "ServerState":
            t = "game::server_side::dedicated::ServerState" if "dedicated" in b["def_path"] else "game::server_side::ServerState"
            return (t, t)
        if src_short == "ClientState":
            return ("game::client_side::ClientState", "game::client_side::ClientState")
        if src_short in ("ClientOnly", "ClientShared"):
            t = f"game::client_side::{src_short}"
            return (t, t)
        if src_short in ("ServerOnly", "ServerShared"):
            t = f"game::server_side::{src_short}"
            return (t, t)
        prefix = b["def_path"].split("::<impl at ", 1)[0] if "::<impl at " in b["def_path"] else ""
        if src_short:
            full = f"{prefix}::{src_short}" if prefix and "::" not in src_self else norm_full_type(src_self)
            sig = f"{prefix}::{src_self}" if prefix and "::" not in src_self else src_self
            return (full, sig)
        return ("", "")

    def _build(self) -> None:
        dp_to_indices: dict[str, list[int]] = defaultdict(list)
        for idx, b in enumerate(self.bodies):
            dp = b["def_path"]
            self.def_path_to_idx[dp] = idx
            dp_to_indices[dp].append(idx)
            if b["closure_ty"]:
                self.closure_to_idx[b["closure_ty"]] = idx

            if b["kind"] not in ("method", "fn"):
                continue
            ii = b["impl_info"]
            if not ii or "::<impl at " not in dp:
                continue
            last_impl_pos = dp.rfind("::<impl at ")
            mod_prefix = dp[:last_impl_pos]
            after_impl = dp[last_impl_pos:]
            end_impl = after_impl.find(">::")
            if end_impl == -1:
                continue
            span_tok = after_impl[2 : end_impl + 1]
            tail = after_impl[end_impl + 3 :]
            bare_self, sig_self = self._infer_self_type(b)
            b["resolved_self_ty"] = bare_self
            b["resolved_self_sig"] = sig_self or bare_self

            if "::" in tail:
                parts = tail.split("::")
                self.nested_fn_index[(bare_self, parts[0], parts[-1])].append(idx)
                self.nested_fn_by_mod_tail[(mod_prefix, tail)].append(idx)
                continue

            meth = tail
            short_self = short_type_name(bare_self)
            ikind = ii.get("kind", "")
            if ikind == "inherent":
                self.inherent_by_mod_ty_meth[(mod_prefix, bare_self, meth)].append(idx)
                self.inherent_by_mod_shortty_meth[(mod_prefix, short_self, meth)].append(idx)
                self.inherent_by_ty_meth[(bare_self, meth)].append(idx)
                if "::<impl at " in mod_prefix:
                    outer_mod = mod_prefix.split("::<impl at ", 1)[0]
                    outer_fn = mod_prefix.rsplit(">::", 1)[-1]
                    self.inherent_by_mod_ty_meth[(f"{outer_mod}::{outer_fn}", bare_self, meth)].append(idx)
                    self.nested_fn_by_mod_tail[(outer_mod, f"{outer_fn}::{short_self}::{meth}")].append(idx)
                self.span_to_canon.setdefault(span_tok, f"<impl {sig_self or bare_self}>")
            elif ikind in ("trait", "derive"):
                tr_norm = normalize_trait_name(ii.get("trait", ""))
                tr_short = short_trait_key(tr_norm)
                b["resolved_trait"] = tr_norm
                b["resolved_trait_short"] = tr_short
                self.trait_impls_by_ty_trait_meth[(bare_self, tr_norm, meth)].append(idx)
                self.trait_impls_by_ty_shorttrait_meth[(bare_self, tr_short, meth)].append(idx)
                self.trait_impls_by_shortty_shorttrait_meth[(short_self, tr_short, meth)].append(idx)
                if tr_short == "Drop" and meth == "drop":
                    self.drop_impls_by_ty[bare_self].append(idx)
                self.span_to_canon.setdefault(span_tok, f"<impl {tr_norm} for {sig_self or bare_self}>")
            else:
                self.inherent_by_mod_ty_meth[(mod_prefix, bare_self, meth)].append(idx)
                self.inherent_by_ty_meth[(bare_self, meth)].append(idx)
                self.span_to_canon.setdefault(span_tok, f"<impl {sig_self or bare_self}>")

        # Resolve enclosing function index for closures/coroutines/constants/promoted
        for idx, b in enumerate(self.bodies):
            if b["kind"] in ("fn", "method"):
                self.enclosing_idx[idx] = idx
                continue
            cur_dp = b["def_path"]
            found = idx
            while True:
                m = ENCLOSING_SUFFIX_RE.match(cur_dp)
                if not m:
                    break
                cur_dp = m.group(1)
                cands = dp_to_indices.get(cur_dp)
                if cands:
                    best = cands[0]
                    for c_idx in cands:
                        if c_idx <= idx:
                            best = c_idx
                    found = best
                    if self.bodies[best]["kind"] in ("fn", "method"):
                        break
            self.enclosing_idx[idx] = found

        # Build stable canonical identity for every body (no file paths, no line numbers, closures folded)
        for idx, b in enumerate(self.bodies):
            self.canon_ids[idx] = self._canonicalize_def_path(b["def_path"], b)

    def _canonicalize_def_path(self, dp: str, b: dict[str, Any] | None = None, fold_enclosing: bool = False) -> str:
        raw = dp
        if fold_enclosing:
            while True:
                m_en = ENCLOSING_SUFFIX_RE.match(raw)
                if not m_en:
                    break
                raw = m_en.group(1)

        def repl(m: re.Match[str]) -> str:
            tok = m.group(0)
            if tok in self.span_to_canon:
                return self.span_to_canon[tok]
            ii = self.source_cache.resolve_impl_span(
                m.group(1), int(m.group(2)), int(m.group(3)), int(m.group(4)), int(m.group(5))
            )
            ikind = ii.get("kind", "")
            sty = ii.get("self_ty", "")
            if (not sty or "$" in sty) and b is not None:
                sty = b.get("resolved_self_sig") or b.get("resolved_self_ty") or ""
            if ikind in ("trait", "derive"):
                tr = normalize_trait_name(ii.get("trait", ""))
                lbl = f"<impl {tr} for {sty}>"
            else:
                lbl = f"<impl {sty}>"
            self.span_to_canon[tok] = lbl
            return lbl

        res = IMPL_AT_RE.sub(repl, raw)
        if "<impl at " in res:
            raise LedgerError(f"failed to canonicalize `<impl at ...>` span in {dp!r}")
        return res

    def is_local_crate_path(self, path: str) -> bool:
        if "::" not in path:
            return path in self.def_path_to_idx
        return path.split("::", 1)[0] in self.local_prefixes

    def resolve_static_call(self, callee: str) -> tuple[str, list[int]]:
        """Resolve a MIR static call path into `(status, body_indices)`."""
        if "::_::_serde::" in callee:
            return ("external", [])
        self_ty, trait_ty, base, _ = split_qualified_path(callee)

        # Case 1: `<SelfTy as Trait>::method`
        if self_ty is not None and trait_ty is not None:
            if is_generic_type_param(self_ty):
                return ("generic_param", [])
            if self_ty.startswith("{closure@"):
                cm = CLOSURE_TY_RE.search(self_ty)
                if cm and cm.group(0) in self.closure_to_idx:
                    return ("local", [self.closure_to_idx[cm.group(0)]])
                return ("missing_local", [])
            if "dyn " in self_ty and short_trait_key(trait_ty) in ("Fn", "FnMut", "FnOnce"):
                return ("dyn_fn_box", [])

            st_norm = norm_full_type(self_ty)
            tr_norm = normalize_trait_name(trait_ty)
            tr_short = short_trait_key(tr_norm)
            meth = base
            for lookup in (
                self.trait_impls_by_ty_trait_meth.get((st_norm, tr_norm, meth)),
                self.trait_impls_by_ty_shorttrait_meth.get((st_norm, tr_short, meth)),
                self.trait_impls_by_shortty_shorttrait_meth.get((short_type_name(st_norm), tr_short, meth)),
            ):
                if lookup:
                    return ("local", list(lookup))
            default_cand = f"{strip_all_generics(tr_norm)}::{meth}"
            if default_cand in self.def_path_to_idx:
                return ("local", [self.def_path_to_idx[default_cand]])
            if self.is_local_crate_path(st_norm) and self.is_local_crate_path(strip_all_generics(tr_norm)):
                return ("missing_local", [])
            return ("external", [])

        # Case 2: `mod::<impl Type>::method`
        top_impl_pos = -1
        depth = 0
        for idx_c, ch in enumerate(base):
            if ch == "<":
                if depth == 0 and idx_c >= 2 and base[idx_c - 2 : idx_c + 6] == "::<impl ":
                    top_impl_pos = idx_c - 2
                    break
                depth += 1
            elif ch == ">":
                if idx_c > 0 and base[idx_c - 1] == "-":
                    pass
                elif depth > 0:
                    depth -= 1

        if top_impl_pos != -1:
            before = base[:top_impl_pos]
            if before.startswith(("std::", "core::", "alloc::")):
                return ("external", [])
            after = base[top_impl_pos + 8 :]
            d2 = 1
            end_i = -1
            for idx_c, ch in enumerate(after):
                if ch == "<":
                    d2 += 1
                elif ch == ">":
                    if idx_c > 0 and after[idx_c - 1] == "-":
                        pass
                    else:
                        d2 -= 1
                        if d2 == 0:
                            end_i = idx_c
                            break
            if end_i != -1 and after[end_i + 1 :].startswith("::"):
                impl_ty = norm_full_type(after[:end_i])
                tail = strip_all_generics(after[end_i + 3 :])
                if "::" in tail:
                    hits = self.nested_fn_by_mod_tail.get((before, tail))
                    if hits:
                        return ("local", list(hits))
                else:
                    meth = tail
                    before_parts = strip_all_generics(before).split("::")
                    alt_mod = f"{before_parts[0]}::{before_parts[-1]}" if len(before_parts) >= 3 else ""
                    for hits in (
                        self.inherent_by_mod_ty_meth.get((before, impl_ty, meth)),
                        self.inherent_by_mod_shortty_meth.get((before, short_type_name(impl_ty), meth)),
                        self.inherent_by_mod_ty_meth.get((alt_mod, impl_ty, meth)) if alt_mod else None,
                        self.inherent_by_ty_meth.get((impl_ty, meth)),
                    ):
                        if hits:
                            return ("local", list(hits))
                if self.is_local_crate_path(before) or self.is_local_crate_path(impl_ty):
                    return ("missing_local", [])
                return ("external", [])

        # Case 3: Plain path `a::b::c` or `Type::method`
        clean = strip_all_generics(base)
        if clean in self.def_path_to_idx:
            return ("local", [self.def_path_to_idx[clean]])
        if "::" in clean:
            ty_part, meth = clean.rsplit("::", 1)
            hits = self.inherent_by_ty_meth.get((ty_part, meth))
            if hits:
                return ("local", list(hits))
            if "::" in ty_part:
                t2, outer_fn = ty_part.rsplit("::", 1)
                hits = self.nested_fn_index.get((t2, outer_fn, meth))
                if hits:
                    return ("local", list(hits))
        if self.is_local_crate_path(clean):
            return ("missing_local", [])
        return ("external", [])


def verify_struct_fields(
    root: Path,
    config_name: str,
    ctor_aggregates: dict[str, list[str]],
    field_owners: dict[str, dict[str, str]],
    host_env: dict[str, Any],
) -> tuple[list[tuple[str, str]], list[tuple[str, str]], list[tuple[str, str]], dict[int, tuple[str, str, str]]]:
    """Parse Game/ClientState/ServerState fields for `config_name` and verify MIR constructors."""
    features = CONFIG_SPECS.get(config_name, {}).get("features", frozenset())
    game_rs = root / "crates/sunrise-client/src/game.rs"
    client_rs = root / "crates/sunrise-client/src/game/client_side.rs"
    server_rs = root / "crates/sunrise-client/src/game/server_side.rs"

    g_fields = parse_struct_fields_cfg(game_rs, "Game", features, host_env) if game_rs.is_file() else []
    c_fields = parse_struct_fields_cfg(client_rs, "ClientState", features, host_env) if client_rs.is_file() else []
    s_fields = parse_struct_fields_cfg(server_rs, "ServerState", features, host_env) if server_rs.is_file() else []

    for sname, parsed_list in (("Game", g_fields), ("ClientState", c_fields), ("ServerState", s_fields)):
        if sname in ctor_aggregates and parsed_list:
            expected_names = [f[0] for f in parsed_list]
            if ctor_aggregates[sname] != expected_names:
                raise LedgerError(
                    f"config {config_name}: MIR constructor field order for `{sname}` does not match source "
                    f"(MIR has {len(ctor_aggregates[sname])} fields, source has {len(expected_names)})"
                )

    wrapped_by_gidx: dict[int, tuple[str, str, str]] = {}
    for idx, (fname, fty) in enumerate(g_fields):
        for wprefix, (wside, wkind) in WRAPPER_KIND_MAP.items():
            if f"{wprefix}<" in fty:
                if fname in field_owners and field_owners[fname]["wrapper"] != wkind:
                    raise LedgerError(
                        f"field `{fname}` wrapper mismatch: source has {wkind}, table has {field_owners[fname]['wrapper']}"
                    )
                wrapped_by_gidx[idx] = (fname, wside, wkind)
                break
    return g_fields, c_fields, s_fields, wrapped_by_gidx


def normalize_dyn_target(callee: str) -> tuple[str, str]:
    """Return `(target_id, detail)` for a `dyn` call terminator."""
    self_ty, tr_ty, meth, _ = split_qualified_path(callee)
    if tr_ty:
        tr_norm = normalize_trait_name(tr_ty)
        return (f"dyn {tr_norm}::{meth}", f"trait={tr_norm};method={meth}")
    if self_ty:
        st_norm = normalize_trait_name(self_ty)
        return (f"dyn {st_norm}::{meth}", f"trait={st_norm};method={meth}")
    clean = strip_lifetimes(callee)
    return (f"dyn {clean}", f"trait={clean};method=?")


def normalize_generic_target(callee: str, kind: str) -> tuple[str, str]:
    """Return `(target_id, detail)` for a generic-parameter call terminator."""
    if kind == "generic_fn_upvar":
        clean = strip_lifetimes(callee)
        return (f"generic:{clean}", f"param={clean}")
    self_ty, tr_ty, meth, _ = split_qualified_path(callee)
    if tr_ty:
        tr_norm = normalize_trait_name(tr_ty)
        p = strip_lifetimes(self_ty or "T")
        return (f"<{p} as {tr_norm}>::{meth}", f"param={p};trait={tr_norm};method={meth}")
    clean = strip_lifetimes(callee)
    return (f"generic:{clean}", f"param={clean}")


def analyze_mir_dump(
    config_name: str,
    mir_path: Path,
    root: Path,
    module_sides: dict[str, str],
    field_owners: dict[str, dict[str, str]],
    source_cache: SourceCache,
    host_env: dict[str, Any],
) -> tuple[dict[tuple[str, str, str], dict[str, Any]], dict[str, Any]]:
    """Extract all MIR-derived ledger rows for one configuration dump."""
    parsed = parse_mir_file(mir_path, root, source_cache)
    _, c_fields, s_fields, wrapped_by_gidx = verify_struct_fields(
        root, config_name, parsed["ctor_aggregates"], field_owners, host_env
    )
    idx = MirIndex(parsed["bodies"], source_cache, module_sides)
    bodies = idx.bodies
    n = len(bodies)

    # First pass: direct side uses per body (needed for closures, trait-impls, and Drop impls)
    direct_body_sides: list[set[str]] = [set() for _ in range(n)]
    direct_edges: list[set[int]] = [set() for _ in range(n)]
    missing_locals: list[tuple[str, str, str]] = []

    for bi, b in enumerate(bodies):
        for cl in b["closures_named"]:
            cidx = idx.closure_to_idx.get(cl)
            if cidx is not None and cidx != bi:
                direct_edges[bi].add(cidx)

        projs = b["projections"]
        for ip, (bt, fidx, fty, _mode, is_deref, is_drop, _loc) in enumerate(projs):
            btc = strip_refs_and_generics(bt)
            fty_bare = strip_refs_and_generics(fty)
            if btc == "game::client_side::ClientState" and fidx < len(c_fields):
                direct_body_sides[bi].add("client")
            elif btc == "game::server_side::ServerState" and fidx < len(s_fields):
                direct_body_sides[bi].add("server")
            elif btc == "game::Game" and fty_bare == "game::client_side::ClientState" and (is_deref or not is_drop):
                direct_body_sides[bi].add("client")
            elif btc == "game::Game" and fty_bare == "game::server_side::ServerState" and (is_deref or not is_drop):
                direct_body_sides[bi].add("server")
            elif btc == "game::Game" and fidx in wrapped_by_gidx:
                has_inner = ip + 1 < len(projs) and strip_refs_and_generics(projs[ip + 1][0]) == fty_bare and projs[ip + 1][1] == 0
                if has_inner or (not is_drop) or is_deref:
                    direct_body_sides[bi].add(wrapped_by_gidx[fidx][1])

        for c in b["calls"]:
            ck = c["kind"]
            if ck == "static":
                st, targets = idx.resolve_static_call(c["callee"])
                if st == "missing_local":
                    missing_locals.append((idx.canon_ids[bi], c["callee"], c["loc"]))
                elif st == "local":
                    for tidx in targets:
                        direct_edges[bi].add(tidx)
                        tlbl = idx.body_label(bodies[tidx]["def_path"])
                        if tlbl in ("client", "server"):
                            direct_body_sides[bi].add(tlbl)
            elif ck == "closure_call":
                cm = CLOSURE_TY_RE.search(c["callee"])
                if cm and cm.group(0) in idx.closure_to_idx:
                    direct_edges[bi].add(idx.closure_to_idx[cm.group(0)])

    if missing_locals:
        sample = "; ".join(f"{u} -> {c} ({loc})" for u, c, loc in missing_locals[:5])
        raise LedgerError(
            f"{mir_path}: refused due to {len(missing_locals)} unresolved local static call(s): {sample}"
        )

    # Propagate side uses into closures (from nested closures) and Drop impls (transitively within crate)
    closure_sides: dict[int, set[str]] = {}
    for bi, b in enumerate(bodies):
        if b["kind"] == "closure":
            seen_s: set[str] = set()
            visited: set[int] = set()
            stack = [bi]
            while stack:
                cur_i = stack.pop()
                if cur_i in visited:
                    continue
                visited.add(cur_i)
                seen_s |= direct_body_sides[cur_i]
                for cl in bodies[cur_i]["closures_named"]:
                    cidx = idx.closure_to_idx.get(cl)
                    if cidx is not None and cidx not in visited:
                        stack.append(cidx)
            closure_sides[bi] = seen_s

    def transitive_sides(start_idx: int) -> set[str]:
        seen_s: set[str] = set()
        visited: set[int] = set()
        stack = [start_idx]
        while stack:
            cur_i = stack.pop()
            if cur_i in visited:
                continue
            visited.add(cur_i)
            seen_s |= direct_body_sides[cur_i]
            for nxt in direct_edges[cur_i]:
                if nxt not in visited:
                    stack.append(nxt)
        return seen_s

    rows_acc: dict[tuple[str, str, str], dict[str, Any]] = {}

    def add_row(
        user_id: str,
        user_label: str,
        kind: str,
        target_id: str,
        target_side: str,
        loc: str,
        *,
        modes: set[str] | None = None,
        forms: set[str] | None = None,
        tags: dict[str, str] | None = None,
        side_set: set[str] | None = None,
    ) -> None:
        key = (user_id, kind, target_id)
        entry = rows_acc.get(key)
        if entry is None:
            entry = {
                "user_id": user_id,
                "user_label": user_label,
                "kind": kind,
                "target_id": target_id,
                "target_sides": set(),
                "modes": set(),
                "forms": set(),
                "tags": {},
                "locs": set(),
                "configs": {config_name},
            }
            rows_acc[key] = entry
        if side_set:
            entry["target_sides"] |= side_set
        else:
            entry["target_sides"].add(target_side)
        if modes:
            entry["modes"] |= modes
        if forms:
            entry["forms"] |= forms
        if tags:
            entry["tags"].update(tags)
        entry["locs"].add(loc or "unknown:0")
        entry["configs"].add(config_name)

    # Second pass: emit call, field, dyn, generic, fnptr, closure-pass, trait-impl, drop
    side_drop_types: dict[str, set[str]] = {}
    type_field_types: dict[str, set[str]] = defaultdict(set)

    for bi, b in enumerate(bodies):
        enc_i = idx.enclosing_idx[bi]
        enc_b = bodies[enc_i]
        user_id = idx._canonicalize_def_path(enc_b["def_path"], enc_b, fold_enclosing=True)
        user_label = idx.body_label(enc_b["def_path"])
        in_game = enc_b["def_path"].startswith("game::")

        # Record projected struct field types to detect types holding side-drop fields
        for bt, _fidx, fty, _mode, _is_deref, _is_drop, _loc in b["projections"]:
            btc = norm_full_type(bt)
            if btc.startswith("game::"):
                type_field_types[btc].add(fty)

        # 1. Field uses (`field`)
        projs = b["projections"]
        for ip, (bt, fidx, fty, mode, is_deref, is_drop, loc) in enumerate(projs):
            btc = strip_refs_and_generics(bt)
            fty_bare = strip_refs_and_generics(fty)
            if btc == "game::client_side::ClientState" and fidx < len(c_fields):
                if user_label != "client":
                    fname = c_fields[fidx][0]
                    vis = field_owners.get(fname, {}).get("visibility", "-")
                    add_row(
                        user_id,
                        user_label,
                        "field",
                        f"ClientState.{fname}",
                        "client",
                        loc,
                        modes={mode},
                        tags={"storage": "grouped", "vis": vis},
                    )
            elif btc == "game::server_side::ServerState" and fidx < len(s_fields):
                if user_label != "server":
                    fname = s_fields[fidx][0]
                    vis = field_owners.get(fname, {}).get("visibility", "-")
                    add_row(
                        user_id,
                        user_label,
                        "field",
                        f"ServerState.{fname}",
                        "server",
                        loc,
                        modes={mode},
                        tags={"storage": "grouped", "vis": vis},
                    )
            elif btc == "game::Game" and fty_bare in ("game::client_side::ClientState", "game::server_side::ServerState"):
                # Whole-container borrow of `Game.client` or `Game.server` (not immediately followed by a subfield)
                has_sub = ip + 1 < len(projs) and strip_refs_and_generics(projs[ip + 1][0]) == fty_bare
                if not has_sub and (is_deref or not is_drop):
                    cside = "client" if fty_bare.endswith("::ClientState") else "server"
                    if user_label != cside:
                        add_row(
                            user_id,
                            user_label,
                            "field",
                            f"Game.{cside}",
                            cside,
                            loc,
                            modes={mode},
                            tags={"storage": "state-container", "vis": "game"},
                        )
            elif btc == "game::Game" and fidx in wrapped_by_gidx:
                fname, wside, wkind = wrapped_by_gidx[fidx]
                if user_label != wside:
                    has_inner = (
                        ip + 1 < len(projs)
                        and strip_refs_and_generics(projs[ip + 1][0]) == fty_bare
                        and projs[ip + 1][1] == 0
                    )
                    if has_inner or (not is_drop) or is_deref:
                        use_mode = projs[ip + 1][3] if has_inner else mode
                        vis = field_owners.get(fname, {}).get("visibility", "-")
                        add_row(
                            user_id,
                            user_label,
                            "field",
                            f"Game.{fname}",
                            wside,
                            loc,
                            modes={use_mode},
                            tags={"storage": wkind, "vis": vis},
                        )

        # 2. Reified fn pointers and passed fn items (`fnptr`)
        for fn_path, loc in b["reified_fn_ptrs"]:
            st, targets = idx.resolve_static_call(fn_path)
            if st == "local":
                for tidx in targets:
                    t_enc = idx.enclosing_idx[tidx]
                    tlbl = idx.body_label(bodies[t_enc]["def_path"])
                    if tlbl in ("client", "server"):
                        add_row(user_id, user_label, "fnptr", idx.canon_ids[t_enc], tlbl, loc, forms={"reify"})

        for fn_path, loc in b["fn_items_named"]:
            st, targets = idx.resolve_static_call(fn_path)
            if st == "local":
                for tidx in targets:
                    t_enc = idx.enclosing_idx[tidx]
                    tlbl = idx.body_label(bodies[t_enc]["def_path"])
                    if tlbl in ("client", "server"):
                        add_row(user_id, user_label, "fnptr", idx.canon_ids[t_enc], tlbl, loc, forms={"fn-value"})

        # 3. Closure unsizing to `dyn Fn*` (`closure-pass`)
        for cl_ty, dst_ty, loc in b["closure_unsizes"]:
            cidx = idx.closure_to_idx.get(cl_ty)
            if cidx is not None and closure_sides.get(cidx):
                csides = closure_sides[cidx]
                dst_clean = normalize_trait_name(dst_ty)
                add_row(
                    user_id,
                    user_label,
                    "closure-pass",
                    f"unsize:{dst_clean}",
                    "both" if len(csides) > 1 else next(iter(csides)),
                    loc,
                    tags={"callee_label": "unresolved"},
                    side_set=csides,
                )

        # 4. Call terminators (`call`, `dyn`, `generic`, `fnptr`, `closure-pass`)
        for c in b["calls"]:
            ck = c["kind"]
            callee = c["callee"]
            loc = c["loc"]

            callee_id = ""
            callee_label = "external"

            if ck == "dyn":
                if in_game:
                    tid, det = normalize_dyn_target(callee)
                    add_row(user_id, user_label, "dyn", tid, "unresolved", loc, tags={"raw": det})
                callee_id = normalize_dyn_target(callee)[0]
                callee_label = "unresolved"
            elif ck == "generic_fn_upvar":
                if in_game:
                    tid, det = normalize_generic_target(callee, ck)
                    add_row(user_id, user_label, "generic", tid, "unresolved", loc, tags={"raw": det})
                callee_id = normalize_generic_target(callee, ck)[0]
                callee_label = "unresolved"
            elif ck == "fn_ptr":
                sig_norm = strip_lifetimes(callee)
                if in_game:
                    add_row(user_id, user_label, "fnptr", f"fnptr:{sig_norm}", "unresolved", loc, forms={"indirect-call"})
                callee_id = f"fnptr:{sig_norm}"
                callee_label = "unresolved"
            elif ck == "static":
                st, targets = idx.resolve_static_call(callee)
                if st == "local":
                    for tidx in targets:
                        t_enc = idx.enclosing_idx[tidx]
                        tb = bodies[t_enc]
                        tlbl = idx.body_label(tb["def_path"])
                        t_cid = idx.canon_ids[t_enc]
                        callee_id = t_cid
                        callee_label = tlbl
                        if tlbl in ("client", "server") and user_label != tlbl:
                            ii = tb["impl_info"]
                            dkind = (
                                f"trait:{tb.get('resolved_trait', ii.get('trait', ''))}"
                                if ii and ii.get("kind") in ("trait", "derive")
                                else ("inherent" if ii else "free")
                            )
                            add_row(user_id, user_label, "call", t_cid, tlbl, loc, tags={"dispatch": dkind})
                elif st == "dyn_fn_box":
                    if in_game:
                        tid, det = normalize_dyn_target(callee)
                        add_row(user_id, user_label, "dyn", tid, "unresolved", loc, tags={"raw": det})
                    callee_id = normalize_dyn_target(callee)[0]
                    callee_label = "unresolved"
                elif st == "generic_param":
                    if in_game:
                        tid, det = normalize_generic_target(callee, ck)
                        add_row(user_id, user_label, "generic", tid, "unresolved", loc, tags={"raw": det})
                    callee_id = normalize_generic_target(callee, ck)[0]
                    callee_label = "unresolved"
                elif st == "external":
                    _, tr_q, base_q, _ = split_qualified_path(callee)
                    ext_root = (strip_all_generics(tr_q) if tr_q else strip_all_generics(base_q)).split("::", 1)[0]
                    callee_id = strip_all_generics(callee)
                    callee_label = "std" if ext_root in STD_CRATES else "external"

            # Check closure arguments passed to a callee of another label (`closure-pass`)
            if c["arg_closures"] and callee_label != "std" and callee_id:
                for cl_ty in c["arg_closures"]:
                    cidx = idx.closure_to_idx.get(cl_ty)
                    if cidx is None:
                        continue
                    csides = closure_sides.get(cidx, set())
                    crossed = {s for s in csides if s != callee_label}
                    if crossed:
                        add_row(
                            user_id,
                            user_label,
                            "closure-pass",
                            callee_id,
                            "both" if len(crossed) > 1 else next(iter(crossed)),
                            loc,
                            tags={"callee_label": callee_label},
                            side_set=crossed,
                        )

        # 5. Hand-written trait impl methods (`trait-impl`) and `Drop` impls (`drop`)
        if b["kind"] == "method" and b["def_path"].startswith("game::"):
            ii = b["impl_info"]
            if ii and ii.get("kind") == "trait":
                tr_norm = b.get("resolved_trait") or normalize_trait_name(ii.get("trait", ""))
                tr_short = short_trait_key(tr_norm)
                self_ty = b.get("resolved_self_ty", "")
                body_sides = set(direct_body_sides[bi])
                for cl in b["closures_named"]:
                    cidx = idx.closure_to_idx.get(cl)
                    if cidx is not None:
                        body_sides |= closure_sides.get(cidx, set())
                self_sides: set[str] = set()
                if self_ty.startswith("game::client_side::") or "ClientOnly" in self_ty or "ClientShared" in self_ty:
                    self_sides.add("client")
                elif self_ty.startswith("game::server_side::") or "ServerOnly" in self_ty or "ServerShared" in self_ty:
                    self_sides.add("server")
                trait_sides: set[str] = set()
                if tr_short in ("ClientHost", "ClientBackendMut") or tr_norm.startswith("game::client_side::"):
                    trait_sides.add("client")
                elif tr_short in ("ServerHost", "BackendMut") or tr_norm.startswith("game::server_side::"):
                    trait_sides.add("server")

                combined = body_sides | self_sides | trait_sides
                reasons: list[str] = []
                if body_sides:
                    reasons.append("body")
                if self_sides:
                    reasons.append("self")
                if trait_sides:
                    reasons.append("trait")
                if user_label in ("client", "server") and not combined:
                    combined.add(user_label)
                    reasons.append("module")

                def_loc = f"{b['source_file']}:{b['line_start']}" if b["source_file"] else "unknown:0"
                if combined:
                    add_row(
                        user_id,
                        user_label,
                        "trait-impl",
                        tr_norm,
                        "both" if len(combined) > 1 else next(iter(combined)),
                        def_loc,
                        tags={"self": self_ty or "-", "reason": "+".join(reasons)},
                        side_set=combined,
                    )

                if tr_short == "Drop" and b["def_path"].rsplit(">::", 1)[-1] == "drop":
                    drop_sides = transitive_sides(bi) | self_sides
                    if drop_sides:
                        side_drop_types[self_ty] = set(drop_sides)
                        add_row(
                            user_id,
                            user_label,
                            "drop",
                            self_ty or user_id,
                            "both" if len(drop_sides) > 1 else next(iter(drop_sides)),
                            def_loc,
                            tags={"role": "drop-impl"},
                            side_set=drop_sides,
                        )

    # 6. Types under `game::` holding side state with a side-using Drop destructor
    holder_sides: dict[str, set[tuple[str, str]]] = defaultdict(set)
    for owner_ty, ftys in type_field_types.items():
        for fty in ftys:
            for dty, dsides in side_drop_types.items():
                if dty and dty in fty:
                    for ds in dsides:
                        holder_sides[owner_ty].add((dty, ds))
    # Also check wrapped fields on `game::Game` whose inner type has a Drop terminator in MIR
    dropped_types_in_mir = {norm_full_type(dty) for b in bodies for dty, _ in b["drops"]}
    game_rs = root / "crates/sunrise-client/src/game.rs"
    if game_rs.is_file() and wrapped_by_gidx:
        g_fields, _, _, _ = verify_struct_fields(root, config_name, parsed["ctor_aggregates"], field_owners, host_env)
        for gidx, (fname, wside, wkind) in wrapped_by_gidx.items():
            fty = g_fields[gidx][1]
            # Check if this wrapped field's inner type is a known side-drop type or dropped with side Drop
            for dty, dsides in side_drop_types.items():
                if dty and (dty in fty or short_type_name(dty) in fty):
                    holder_sides["game::Game"].add((f"Game.{fname}", wside))
            if any(marker in fty for marker in ("Audio", "ParticleEngine", "Autosaves", "SessionLock", "WorldRestLoader", "JoinHandle", "PackReload")):
                holder_sides["game::Game"].add((f"Game.{fname}", wside))

    for holder_ty, pairs in holder_sides.items():
        h_label = idx.body_label(holder_ty)
        by_target: dict[str, set[str]] = defaultdict(set)
        for held, side in pairs:
            by_target[held].add(side)
        for held, sides in by_target.items():
            add_row(
                holder_ty,
                h_label,
                "drop",
                held,
                "both" if len(sides) > 1 else next(iter(sides)),
                f"type:{holder_ty}",
                tags={"role": "drop-holder"},
                side_set=sides,
            )

    stats = {
        "config": config_name,
        "bodies": len(bodies),
        "skipped_headers": parsed["skipped_headers"],
        "unparsed_call_terms": parsed["unparsed_call_terms"],
        "missing_local_calls": len(missing_locals),
        "rows": len(rows_acc),
    }
    return rows_acc, stats


def _extract_callable_hits(sig_text: str, self_ty: str, type_aliases: dict[str, str]) -> list[tuple[str, str]]:
    """Return `(direction, summary)` hits for game-taking or unsure callables in a fn signature."""
    # Expand known callable type aliases in signature text
    expanded = sig_text
    for alias, rhs in type_aliases.items():
        if re.search(rf"\b{re.escape(alias)}\b", expanded):
            expanded = re.sub(rf"\b{re.escape(alias)}\b", rhs, expanded)

    if not re.search(r"\b(?:FnOnce|FnMut|Fn)\b|\bfn\s*\(", expanded):
        return []

    # Split into params, return type, and where clause
    p_start = expanded.find("(")
    if p_start == -1:
        return []
    depth = 0
    p_end = -1
    for i in range(p_start, len(expanded)):
        if expanded[i] == "(":
            depth += 1
        elif expanded[i] == ")":
            depth -= 1
            if depth == 0:
                p_end = i
                break
    if p_end == -1:
        return [("accepts", "unparseable-params")]

    header_before_params = expanded[:p_start]
    params_text = expanded[p_start + 1 : p_end]
    rest = expanded[p_end + 1 :].strip()
    where_pos = _find_top_level_keyword(rest, "where")
    if where_pos != -1:
        ret_text = rest[:where_pos].strip()
        where_text = rest[where_pos + 5 :].strip()
    else:
        ret_text = rest
        where_text = ""
    if ret_text.startswith("->"):
        ret_text = ret_text[2:].strip()

    # Collect generic param names declared on fn `<T, F: ...>`
    gen_params: set[str] = set()
    gm = re.search(r"<([^>]*)>", header_before_params)
    if gm:
        for gp in split_top_level(gm.group(1), ","):
            m_id = re.match(r"^(?:const\s+)?([A-Z][A-Za-z0-9_]*)", gp.strip())
            if m_id:
                gen_params.add(m_id.group(1))

    self_is_game = (not self_ty) or any(m in self_ty for m in ("Game", "ClientState", "ServerState", "Host", "Level"))

    def callable_concerns_game(kind_tag: str, inner_args: str) -> bool:
        if kind_tag == "dyn":
            return True
        idents = set(re.findall(r"\b[A-Za-z_][A-Za-z0-9_]*\b", inner_args))
        if idents & CALLBACK_GAME_MARKERS:
            return True
        if "Self" in idents and self_is_game:
            return True
        if "dyn" in idents:
            return True
        if idents & gen_params:
            return True
        # Fail closed: if any capitalized identifier is NOT in SAFE_NON_GAME_TYPES, list it
        cap_idents = {w for w in idents if w[0].isupper() and w != "Self"}
        unknown_caps = cap_idents - SAFE_NON_GAME_TYPES
        if unknown_caps:
            return True
        return False

    hits: list[tuple[str, str]] = []
    # Check `impl/dyn Fn*(...)` and `fn(...)` in params, return, generic bounds, and where bounds
    sections = [
        ("accepts", params_text),
        ("returns", ret_text),
        ("accepts", header_before_params),
        ("accepts", where_text),
    ]
    call_pat = re.compile(r"\b(dyn\s+(?:for<[^>]*>\s*)?Fn(?:Mut|Once)?|impl\s+(?:for<[^>]*>\s*)?Fn(?:Mut|Once)?|Fn(?:Mut|Once)?|\bfn)\s*\(")
    for direction, sec in sections:
        if not sec:
            continue
        for m in call_pat.finditer(sec):
            head = " ".join(m.group(1).split())
            # Match balanced `(...)` arguments of the callable
            open_i = m.end() - 1
            d = 0
            close_i = -1
            for k in range(open_i, len(sec)):
                if sec[k] == "(":
                    d += 1
                elif sec[k] == ")":
                    d -= 1
                    if d == 0:
                        close_i = k
                        break
            if close_i == -1:
                hits.append((direction, f"{head}(?)"))
                continue
            inner = sec[open_i + 1 : close_i].strip()
            ktag = "dyn" if head.startswith("dyn ") else ("fnptr" if head == "fn" else "fn-trait")
            if callable_concerns_game(ktag, inner):
                compact_inner = " ".join(inner.split())
                if len(compact_inner) > 64:
                    compact_inner = compact_inner[:61] + "..."
                hits.append((direction, f"{head}({compact_inner})"))
    return hits


def scan_callback_params(
    root: Path,
    module_sides: dict[str, str],
) -> dict[tuple[str, str, str], dict[str, Any]]:
    """Scan non-test Rust source under `game.rs` and `game/` for game-taking callback signatures."""
    game_rs = root / "crates/sunrise-client/src/game.rs"
    if not game_rs.is_file():
        return {}

    rows_acc: dict[tuple[str, str, str], dict[str, Any]] = {}
    visited_files: set[tuple[Path, str]] = set()

    def mod_label(mod_path: str) -> str:
        if mod_path == "game::client_side" or mod_path.startswith("game::client_side::"):
            return "client"
        if mod_path == "game::server_side" or mod_path.startswith("game::server_side::"):
            return "server"
        if mod_path.startswith("game::"):
            top = mod_path[len("game::") :].split("::", 1)[0]
            s = module_sides.get(top)
            if s in ("client", "server"):
                return s
        return "common"

    def resolve_submod_file(parent_file: Path, mod_name: str, path_attr: str | None) -> Path | None:
        base_dir = parent_file.parent if parent_file.name in ("mod.rs", "lib.rs", "main.rs") else parent_file.with_suffix("")
        if path_attr:
            c1 = parent_file.parent / path_attr
            if c1.is_file():
                return c1
            c2 = base_dir / path_attr
            if c2.is_file():
                return c2
            return None
        for cand in (base_dir / f"{mod_name}.rs", base_dir / mod_name / "mod.rs"):
            if cand.is_file():
                return cand
        return None

    def scan_file(path: Path, mod_path: str) -> None:
        key = (path.resolve(), mod_path)
        if key in visited_files or not path.is_file():
            return
        visited_files.add(key)
        src = path.read_text(encoding="utf-8")
        toks = lex_rust(src)
        rel = normalize_loc_path(str(path), root)
        _scan_block(src, toks, 0, len(toks), path, rel, mod_path, "", {})

    def _scan_block(
        src: str,
        toks: list[Token],
        start: int,
        end: int,
        cur_file: Path,
        rel_file: str,
        mod_path: str,
        impl_prefix: str,
        aliases: dict[str, str],
    ) -> None:
        # First pass inside this block: gather local `type Alias = ...Fn.../fn(...);`
        local_aliases = dict(aliases)
        i = start
        while i < end:
            if toks[i].kind == "IDENT" and toks[i].text == "type" and i + 2 < end and toks[i + 1].kind == "IDENT":
                aname = toks[i + 1].text
                j = i + 2
                while j < end and toks[j].text not in ("=", ";", "{"):
                    j += 1
                if j < end and toks[j].text == "=":
                    rhs_start = toks[j + 1].start if j + 1 < end else len(src)
                    k = j + 1
                    while k < end and toks[k].text != ";":
                        k += 1
                    rhs_txt = src[rhs_start : toks[k].start if k < end else len(src)].strip()
                    if re.search(r"\b(?:FnOnce|FnMut|Fn)\b|\bfn\s*\(", rhs_txt):
                        local_aliases[aname] = rhs_txt
                    i = k + 1
                    continue
            i += 1

        i = start
        while i < end:
            # Collect outer attributes
            is_test = False
            path_attr: str | None = None
            while i < end and toks[i].text == "#" and i + 1 < end and toks[i + 1].text in ("[", "!"):
                if toks[i + 1].text == "!":
                    # Inner attribute `#![...]`
                    j = i + 2
                    if j < end and toks[j].text == "[":
                        d = 1
                        j += 1
                        while j < end and d > 0:
                            if toks[j].text == "[":
                                d += 1
                            elif toks[j].text == "]":
                                d -= 1
                            j += 1
                    i = j
                    continue
                attr_start = toks[i + 1].start + 1
                d = 1
                j = i + 2
                while j < end and d > 0:
                    if toks[j].text == "[":
                        d += 1
                    elif toks[j].text == "]":
                        d -= 1
                    j += 1
                attr_txt = src[attr_start : toks[j - 1].start].strip()
                if re.search(r"\btest\b", attr_txt) and ("cfg(" in attr_txt or attr_txt == "test"):
                    if "not(test)" not in attr_txt:
                        is_test = True
                pm = re.match(r'^path\s*=\s*"([^"]+)"$', attr_txt)
                if pm:
                    path_attr = pm.group(1)
                i = j

            if i >= end:
                break

            # Skip visibility and qualifiers (`pub(...)`, `unsafe`, `async`, `const`, `extern "..."`)
            while i < end:
                t = toks[i].text
                if t == "pub":
                    i += 1
                    if i < end and toks[i].text == "(":
                        i = _skip_delim(toks, i, end, "(", ")")
                elif t in ("unsafe", "async", "const", "default"):
                    i += 1
                elif t == "extern":
                    i += 1
                    if i < end and toks[i].kind == "STR":
                        i += 1
                else:
                    break
            if i >= end:
                break

            kw = toks[i].text
            if kw == "include" and i + 2 < end and toks[i + 1].text == "!" and toks[i + 2].text == "(":
                close_i = _skip_delim(toks, i + 2, end, "(", ")")
                if not is_test and i + 3 < close_i and toks[i + 3].kind == "STR":
                    inc_rel = toks[i + 3].text[1:-1]
                    inc_path = cur_file.parent / inc_rel
                    if inc_path.is_file():
                        scan_file(inc_path, mod_path)
                i = close_i
                if i < end and toks[i].text == ";":
                    i += 1
                continue

            if kw == "mod" and i + 1 < end and toks[i + 1].kind == "IDENT":
                mname = toks[i + 1].text
                j = i + 2
                if j < end and toks[j].text == ";":
                    if not is_test:
                        subf = resolve_submod_file(cur_file, mname, path_attr)
                        if subf is not None:
                            scan_file(subf, f"{mod_path}::{mname}")
                    i = j + 1
                    continue
                if j < end and toks[j].text == "{":
                    close_i = _skip_delim(toks, j, end, "{", "}")
                    if not is_test:
                        _scan_block(src, toks, j + 1, close_i - 1, cur_file, rel_file, f"{mod_path}::{mname}", "", local_aliases)
                    i = close_i
                    continue

            if kw == "impl":
                hdr_start = toks[i].start
                j = i + 1
                while j < end and toks[j].text != "{":
                    if toks[j].text == ";":
                        break
                    j += 1
                if j < end and toks[j].text == "{":
                    close_i = _skip_delim(toks, j, end, "{", "}")
                    if not is_test:
                        raw_hdr = " ".join(src[hdr_start : toks[j].start].split())
                        canon_impl = _canonicalize_source_impl_header(raw_hdr, mod_path)
                        _scan_block(src, toks, j + 1, close_i - 1, cur_file, rel_file, mod_path, canon_impl, local_aliases)
                    i = close_i
                    continue

            if kw == "fn" and i + 1 < end and toks[i + 1].kind == "IDENT":
                fname = toks[i + 1].text
                fn_line = toks[i].line
                sig_start = toks[i].start
                j = i + 2
                while j < end and toks[j].text not in ("{", ";"):
                    j += 1
                sig_end = toks[j].start if j < end else len(src)
                if not is_test:
                    sig_text = src[sig_start:sig_end]
                    self_ty = impl_prefix.split(" for ")[-1].strip("<> ") if impl_prefix else ""
                    hits = _extract_callable_hits(sig_text, self_ty, local_aliases)
                    if hits:
                        uid = f"{mod_path}::{impl_prefix}::{fname}" if impl_prefix else f"{mod_path}::{fname}"
                        ulbl = mod_label(mod_path)
                        dirs = sorted({h[0] for h in hits})
                        for _, target_pat in hits:
                            key = (uid, "callback-param", target_pat)
                            entry = rows_acc.get(key)
                            if entry is None:
                                entry = {
                                    "user_id": uid,
                                    "user_label": ulbl,
                                    "kind": "callback-param",
                                    "target_id": target_pat,
                                    "target_sides": {"unresolved"},
                                    "modes": set(),
                                    "forms": set(),
                                    "tags": {"direction": "+".join(dirs)},
                                    "locs": {f"{rel_file}:{fn_line}"},
                                    "configs": {"source"},
                                }
                                rows_acc[key] = entry
                            else:
                                entry["locs"].add(f"{rel_file}:{fn_line}")
                if j < end and toks[j].text == "{":
                    i = _skip_delim(toks, j, end, "{", "}")
                else:
                    i = j + 1
                continue

            # Skip any other braced block or statement cleanly
            if toks[i].text == "{":
                i = _skip_delim(toks, i, end, "{", "}")
            elif toks[i].text == "(":
                i = _skip_delim(toks, i, end, "(", ")")
            elif toks[i].text == "[":
                i = _skip_delim(toks, i, end, "[", "]")
            else:
                i += 1

    scan_file(game_rs, "game")
    return rows_acc


def _skip_delim(toks: list[Token], open_idx: int, end: int, open_ch: str, close_ch: str) -> int:
    depth = 0
    i = open_idx
    while i < end:
        if toks[i].text == open_ch:
            depth += 1
        elif toks[i].text == close_ch:
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return end


def _canonicalize_source_impl_header(raw_hdr: str, mod_path: str) -> str:
    """Normalize a source `impl ...` header into `<impl Self>` or `<impl Trait for Self>`."""
    s = re.split(r"\bwhere\b", raw_hdr, maxsplit=1)[0].strip()
    if s.startswith("impl"):
        s = s[4:].strip()
    if s.startswith("<"):
        d = 0
        for idx_c, ch in enumerate(s):
            if ch == "<":
                d += 1
            elif ch == ">":
                if idx_c > 0 and s[idx_c - 1] == "-":
                    continue
                d -= 1
                if d == 0:
                    s = s[idx_c + 1 :].strip()
                    break
    for_pos = _find_top_level_keyword(s, "for")
    if for_pos != -1:
        tr = normalize_trait_name(s[:for_pos])
        st = norm_sig_type(s[for_pos + 3 :])
        if st == "Game":
            st = "game::Game"
        elif st == "GameLevel":
            st = "game::GameLevel"
        elif st == "ClientState":
            st = "game::client_side::ClientState"
        elif st == "ServerState":
            st = "game::server_side::ServerState"
        return f"<impl {tr} for {st}>"
    st = norm_sig_type(s)
    if st == "Game":
        st = "game::Game"
    elif st == "GameLevel":
        st = "game::GameLevel"
    elif st == "ClientState":
        st = "game::client_side::ClientState"
    elif st == "ServerState":
        st = "game::server_side::ServerState"
    elif "::" not in st and st:
        st = f"{mod_path}::{st}"
    return f"<impl {st}>"


def scan_uncovered_cfgs(root: Path, configs: tuple[str, ...]) -> list[tuple[str, int, str]]:
    """List every `#[cfg(...)]` / `#[cfg_attr(...)]` predicate in `game` source never active in `configs`."""
    host_env = current_host_cfg_env()
    files: list[Path] = []
    game_rs = root / "crates/sunrise-client/src/game.rs"
    if game_rs.is_file():
        files.append(game_rs)
    game_dir = root / "crates/sunrise-client/src/game"
    if game_dir.is_dir():
        files.extend(sorted(game_dir.rglob("*.rs")))

    pred_counts: Counter[str] = Counter()
    for fpath in files:
        src = fpath.read_text(encoding="utf-8")
        toks = lex_rust(src)
        n = len(toks)
        i = 0
        while i < n:
            if toks[i].text == "#" and i + 1 < n and toks[i + 1].text == "[":
                attr_start = toks[i + 1].start + 1
                j = _skip_delim(toks, i + 1, n, "[", "]")
                attr_txt = " ".join(src[attr_start : toks[j - 1].start].split())
                attr_txt = re.sub(r"\(\s+", "(", attr_txt)
                attr_txt = re.sub(r"\s+\)", ")", attr_txt)
                attr_txt = re.sub(r"\s+,", ",", attr_txt)
                if attr_txt.startswith("cfg(") and attr_txt.endswith(")"):
                    pred_counts[attr_txt[4:-1].strip()] += 1
                elif attr_txt.startswith("cfg_attr(") and attr_txt.endswith(")"):
                    parts = split_top_level(attr_txt[9:-1], ",")
                    if parts:
                        pred_counts[parts[0].strip()] += 1
                i = j
            else:
                i += 1

    uncovered: list[tuple[str, int, str]] = []
    for pred, count in sorted(pred_counts.items(), key=lambda kv: (-kv[1], kv[0])):
        active_any = any(
            eval_cfg_expr(pred, CONFIG_SPECS[c]["features"], host_env) for c in configs if c in CONFIG_SPECS
        )
        if not active_any:
            cat = "test" if re.search(r"\btest\b", pred) and "not(test)" not in pred else "platform_or_feature"
            uncovered.append((pred, count, cat))
    return uncovered


def finalize_rows(
    merged: dict[tuple[str, str, str], dict[str, Any]],
    existing_hand: dict[tuple[str, str, str], tuple[str, str, str]],
) -> list[LedgerRow]:
    """Convert accumulated row dicts into canonical sorted `LedgerRow` records."""
    out: list[LedgerRow] = []
    for key, e in sorted(merged.items(), key=lambda kv: (kv[1]["kind"], kv[1]["user_id"], kv[1]["target_id"])):
        sides = e["target_sides"]
        if sides == {"client", "server"} or "both" in sides:
            target_side = "both"
        elif "client" in sides:
            target_side = "client"
        elif "server" in sides:
            target_side = "server"
        elif "unresolved" in sides:
            target_side = "unresolved"
        else:
            target_side = "common"

        kind = e["kind"]
        tags = e["tags"]
        if kind == "field":
            mode_s = ",".join(sorted(e["modes"])) if e["modes"] else "read"
            detail = f"mode={mode_s};storage={tags.get('storage', '-')};vis={tags.get('vis', '-')}"
        elif kind == "call":
            detail = f"dispatch={tags.get('dispatch', 'static')}"
        elif kind in ("dyn", "generic"):
            detail = tags.get("raw", kind)
        elif kind == "fnptr":
            detail = f"form={','.join(sorted(e['forms']))}"
        elif kind == "closure-pass":
            detail = f"callee_label={tags.get('callee_label', '-')};closure_uses={target_side}"
        elif kind == "trait-impl":
            detail = f"self={tags.get('self', '-')};reason={tags.get('reason', '-')}"
        elif kind == "drop":
            detail = f"role={tags.get('role', 'drop-impl')}"
        elif kind == "callback-param":
            detail = f"direction={tags.get('direction', 'accepts')}"
        else:
            detail = "-"

        locs = sorted(e["locs"])
        status, group, note = existing_hand.get(key, ("unclassified", "", ""))
        out.append(
            LedgerRow(
                user_id=e["user_id"],
                user_label=e["user_label"],
                kind=kind,
                target_id=e["target_id"],
                target_side=target_side,
                detail=detail,
                configs=",".join(sorted(e["configs"])),
                sites=max(1, len(locs)),
                example_loc=locs[0] if locs else "unknown:0",
                status=status,
                group=group,
                note=note,
            )
        )
    return out


def _analyze_one_config_worker(
    args_tuple: tuple[str, str, str, dict[str, str], dict[str, dict[str, str]], list[str], dict[str, Any]],
) -> tuple[str, dict[tuple[str, str, str], dict[str, Any]], dict[str, Any]]:
    """Top-level worker for parallel MIR dump analysis across configurations."""
    cfg_name, mir_path_s, root_s, module_sides, field_owners, out_dir_strs, host_env = args_tuple
    root = Path(root_s)
    sc = SourceCache(root, [Path(p) for p in out_dir_strs])
    cfg_rows, st = analyze_mir_dump(
        cfg_name, Path(mir_path_s), root, module_sides, field_owners, sc, host_env
    )
    return cfg_name, cfg_rows, st


def build_ledger(
    root: Path,
    dump_paths: list[tuple[str, Path]],
    module_table: Path,
    fn_table: Path,
    field_table: Path,
    existing_hand: dict[tuple[str, str, str], tuple[str, str, str]],
    include_source_callbacks: bool = True,
    out_dirs: list[Path] | None = None,
    jobs: int = 0,
) -> tuple[list[LedgerRow], list[dict[str, Any]]]:
    """Generate the full merged ledger across all configured MIR dumps and source callbacks."""
    module_sides = load_module_sides(module_table)
    load_fn_sides(fn_table)
    field_owners = load_field_owners(field_table)
    host_env = current_host_cfg_env()

    # Discover build `out/` dirs automatically under `root/target` if none passed
    auto_out_dirs = list(out_dirs or [])
    if not auto_out_dirs:
        tdir = root / "target"
        if tdir.is_dir():
            for cand in sorted(tdir.glob("*/build/*/out")):
                if cand.is_dir():
                    auto_out_dirs.append(cand)

    sorted_dumps = sorted(dump_paths, key=lambda kv: kv[0])
    total_bytes = sum(p.stat().st_size for _, p in sorted_dumps)
    worker_count = jobs if jobs > 0 else min(len(sorted_dumps), os.cpu_count() or 1, 3)

    per_cfg_results: list[tuple[str, dict[tuple[str, str, str], dict[str, Any]], dict[str, Any]]] = []
    if worker_count > 1 and len(sorted_dumps) > 1 and total_bytes > 1_000_000:
        out_dir_strs = [str(p) for p in auto_out_dirs]
        payloads = [
            (cfg_name, str(mir_path), str(root), module_sides, field_owners, out_dir_strs, host_env)
            for cfg_name, mir_path in sorted_dumps
        ]
        with concurrent.futures.ProcessPoolExecutor(max_workers=worker_count) as pool:
            per_cfg_results = list(pool.map(_analyze_one_config_worker, payloads))
    else:
        source_cache = SourceCache(root, auto_out_dirs)
        for cfg_name, mir_path in sorted_dumps:
            cfg_rows, st = analyze_mir_dump(
                cfg_name, mir_path, root, module_sides, field_owners, source_cache, host_env
            )
            per_cfg_results.append((cfg_name, cfg_rows, st))

    merged: dict[tuple[str, str, str], dict[str, Any]] = {}
    dump_stats: list[dict[str, Any]] = []
    for _cfg_name, cfg_rows, st in sorted(per_cfg_results, key=lambda x: x[0]):
        dump_stats.append(st)
        for k, v in cfg_rows.items():
            if k not in merged:
                merged[k] = v
            else:
                m = merged[k]
                m["target_sides"] |= v["target_sides"]
                m["modes"] |= v["modes"]
                m["forms"] |= v["forms"]
                m["tags"].update(v["tags"])
                m["locs"] |= v["locs"]
                m["configs"] |= v["configs"]

    if include_source_callbacks:
        cb_rows = scan_callback_params(root, module_sides)
        for k, v in cb_rows.items():
            if k not in merged:
                merged[k] = v
            else:
                merged[k]["locs"] |= v["locs"]
                merged[k]["configs"] |= v["configs"]

    return finalize_rows(merged, existing_hand), dump_stats


def resolve_cargo_cmd(cargo_override: str | None) -> list[str]:
    """Choose the `hz cargo` wrapper when present on this host, else plain `cargo`."""
    if cargo_override:
        return cargo_override.split()
    if HZ_WRAPPER.is_file() and os.access(HZ_WRAPPER, os.X_OK):
        return [str(HZ_WRAPPER), "cargo"]
    return ["cargo"]


def run_dump_mode(
    root: Path,
    dump_dir: Path,
    configs: tuple[str, ...],
    cargo_override: str | None,
    skip_warm_check: bool,
) -> int:
    """Produce MIR dumps for every requested configuration atomically into `dump_dir`."""
    dump_dir.mkdir(parents=True, exist_ok=True)
    cargo_cmd = resolve_cargo_cmd(cargo_override)
    manifest: dict[str, Any] = {"configs": {}, "root": str(root.resolve())}
    head_proc = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True)
    if head_proc.returncode == 0:
        manifest["git_head"] = head_proc.stdout.strip()

    total_t0 = time.monotonic()
    for cfg in configs:
        if cfg not in CONFIG_SPECS:
            raise LedgerError(f"unknown config {cfg!r}; expected one of {list(CONFIG_SPECS)}")
        extra = list(CONFIG_SPECS[cfg]["cargo_args"])
        t0 = time.monotonic()
        if not skip_warm_check:
            warm_cmd = ["nice", "-n", "10", *cargo_cmd, "check", "-p", "sunrise-client", "--lib", *extra]
            env_warm = dict(os.environ)
            env_warm.pop("RUSTC_BOOTSTRAP", None)
            proc_w = subprocess.run(warm_cmd, cwd=root, env=env_warm)
            if proc_w.returncode != 0:
                raise LedgerError(f"warm cargo check failed for config `{cfg}` (exit {proc_w.returncode})")
        t_warm = time.monotonic() - t0

        out_path = dump_dir / f"{cfg}.mir"
        tmp_path = dump_dir / f".{cfg}.mir.tmp.{os.getpid()}"
        env_mir = dict(os.environ)
        env_mir["RUSTC_BOOTSTRAP"] = "sunrise_client"
        mir_cmd = [
            "nice",
            "-n",
            "10",
            *cargo_cmd,
            "rustc",
            "-p",
            "sunrise-client",
            "--lib",
            "--profile",
            "check",
            *extra,
            "--",
            "-Zunpretty=mir",
            "-Ztrim-diagnostic-paths=no",
            "-Zmir-include-spans=yes",
            "-o",
            str(tmp_path),
        ]
        t1 = time.monotonic()
        try:
            proc_m = subprocess.run(mir_cmd, cwd=root, env=env_mir)
            if proc_m.returncode != 0:
                raise LedgerError(f"MIR dump failed for config `{cfg}` (exit {proc_m.returncode})")
            if not tmp_path.is_file() or tmp_path.stat().st_size == 0:
                raise LedgerError(f"MIR dump produced missing or empty file for config `{cfg}`: {tmp_path}")
            os.replace(tmp_path, out_path)
        except Exception:
            tmp_path.unlink(missing_ok=True)
            raise
        t_mir = time.monotonic() - t1
        manifest["configs"][cfg] = {
            "mir": out_path.name,
            "size_bytes": out_path.stat().st_size,
            "warm_check_s": round(t_warm, 2),
            "mir_dump_s": round(t_mir, 2),
        }
        print(
            f"[dump] {cfg}: warm={t_warm:.2f}s mir={t_mir:.2f}s size={out_path.stat().st_size} bytes -> {out_path}",
            flush=True,
        )

    manifest["total_s"] = round(time.monotonic() - total_t0, 2)
    atomic_write_text(dump_dir / "manifest.json", json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    print(f"[dump] completed {len(configs)} config(s) in {manifest['total_s']:.2f}s", flush=True)
    return 0


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
            raise LedgerError(f"cannot inspect HEAD parents: {parents_proc.stderr.strip()}")
        parents = parents_proc.stdout.strip().split()[1:]
        if len(parents) < 2:
            raise LedgerError("--pull-request-merge needs HEAD to be a merge commit.")
        anc = subprocess.run(
            ["git", "merge-base", "--is-ancestor", against, parents[0]],
            cwd=root,
            capture_output=True,
            text=True,
        )
        if anc.returncode != 0:
            raise LedgerError("--pull-request-merge needs HEAD to merge into a descendant of --against.")
        return parents[0]
    proc = subprocess.run(["git", "merge-base", against, "HEAD"], cwd=root, capture_output=True, text=True)
    if proc.returncode != 0:
        raise LedgerError(f"cannot resolve --against {against!r}: {proc.stderr.strip()}")
    return proc.stdout.strip()


def check_against_ref(
    root: Path,
    table_path: Path,
    current_rows: list[LedgerRow],
    current_lines: dict[tuple[str, str, str], int],
    against: str,
    pull_request_merge: bool,
    allow_missing_baseline: bool,
) -> list[str]:
    """Check that `current_rows` adds no row identity absent at baseline `against`."""
    base_rev = resolve_baseline_ref(root, against, pull_request_merge)
    rel_table = normalize_loc_path(str(table_path.resolve()), root)
    ls_proc = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", base_rev, "--", rel_table],
        cwd=root,
        capture_output=True,
        text=True,
    )
    if ls_proc.returncode != 0:
        raise LedgerError(f"git ls-tree failed for {base_rev}: {ls_proc.stderr.strip()}")
    if not ls_proc.stdout.strip():
        if allow_missing_baseline:
            return []
        return [
            f"baseline {against!r} ({base_rev[:10]}) has no `{rel_table}`; "
            f"pass `--allow-missing-baseline` when introducing the table for the first time."
        ]

    show_proc = subprocess.run(
        ["git", "show", f"{base_rev}:{rel_table}"],
        cwd=root,
        capture_output=True,
        text=True,
    )
    if show_proc.returncode != 0:
        raise LedgerError(f"git show {base_rev}:{rel_table} failed: {show_proc.stderr.strip()}")
    base_rows, _ = load_ledger_tsv_text(show_proc.stdout, f"{base_rev[:10]}:{rel_table}")
    base_ids = {r.identity for r in base_rows}

    errors: list[str] = []
    for r in current_rows:
        if r.identity not in base_ids:
            ln = current_lines.get(r.identity, 0)
            errors.append(
                f"ADDED vs {against} ({base_rev[:10]}): {r.kind} {r.user_id} -> {r.target_id} "
                f"(table {table_path}:{ln}, example {r.example_loc}); remove the new cross-side use or update the reviewed baseline."
            )
    return errors


def _trailing_symbol(target_id: str) -> str:
    """Extract the trailing method/function/field name from a canonical target_id."""
    if "::" in target_id:
        return target_id.rsplit("::", 1)[-1]
    if "." in target_id:
        return target_id.rsplit(".", 1)[-1]
    return target_id


def compute_diff(old_rows: list[LedgerRow], new_rows: list[LedgerRow]) -> dict[str, Any]:
    """Compare two ledgers, reporting added, removed, and rebound targets for the same user/site."""
    old_by_id = {r.identity: r for r in old_rows}
    new_by_id = {r.identity: r for r in new_rows}

    removed = [old_by_id[k] for k in sorted(old_by_id.keys() - new_by_id.keys(), key=lambda x: (x[1], x[0], x[2]))]
    added = [new_by_id[k] for k in sorted(new_by_id.keys() - old_by_id.keys(), key=lambda x: (x[1], x[0], x[2]))]

    # Detect rebound targets where one user_id loses an old target and gains a matching new target
    rebound: list[dict[str, Any]] = []
    added_by_user: dict[str, list[LedgerRow]] = defaultdict(list)
    for r in added:
        added_by_user[r.user_id].append(r)

    used_added: set[tuple[str, str, str]] = set()
    for old_r in removed:
        cands = [a for a in added_by_user.get(old_r.user_id, []) if a.identity not in used_added]
        if not cands:
            continue
        old_sym = _trailing_symbol(old_r.target_id)
        best: tuple[int, str, LedgerRow] | None = None
        for cand in cands:
            cand_sym = _trailing_symbol(cand.target_id)
            if old_r.example_loc == cand.example_loc and old_sym == cand_sym:
                score = (0, "same_loc+symbol", cand)
            elif old_r.example_loc == cand.example_loc:
                score = (1, "same_loc", cand)
            elif old_sym == cand_sym and old_sym and old_r.kind in ("call", "dyn", "generic", "fnptr", "trait-impl"):
                score = (2, "same_symbol", cand)
            else:
                continue
            if best is None or score[0] < best[0]:
                best = score
        if best is not None:
            _, reason, new_r = best
            used_added.add(new_r.identity)
            rebound.append(
                {
                    "user_id": old_r.user_id,
                    "reason": reason,
                    "old_kind": old_r.kind,
                    "old_target": old_r.target_id,
                    "old_loc": old_r.example_loc,
                    "new_kind": new_r.kind,
                    "new_target": new_r.target_id,
                    "new_loc": new_r.example_loc,
                }
            )
    return {"added": added, "removed": removed, "rebound": rebound}


def resolve_dump_paths(
    dump_dir: Path | None,
    explicit_dumps: list[str],
    configs: tuple[str, ...],
) -> list[tuple[str, Path]]:
    """Resolve `(config_name, mir_path)` pairs from `--dump` and/or `--dump-dir`, failing closed."""
    out: dict[str, Path] = {}
    if dump_dir is not None:
        for cfg in configs:
            p = dump_dir / f"{cfg}.mir"
            if not p.is_file():
                raise LedgerError(f"missing MIR dump for config `{cfg}` in {dump_dir}: expected {p}")
            out[cfg] = p
    for item in explicit_dumps:
        if "=" not in item:
            raise LedgerError(f"--dump expects CONFIG=PATH, got {item!r}")
        cfg, raw_p = item.split("=", 1)
        cfg = cfg.strip()
        p = Path(raw_p.strip())
        if not cfg:
            raise LedgerError(f"--dump has empty config name in {item!r}")
        if not p.is_file():
            raise LedgerError(f"--dump path does not exist for `{cfg}`: {p}")
        out[cfg] = p
    if not out:
        raise LedgerError("no MIR dumps provided; pass `--dump-dir DIR` or `--dump CONFIG=PATH`")
    return sorted(out.items(), key=lambda kv: kv[0])


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="game_side_ledger.py",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "subcommand",
        nargs="?",
        choices=("dump",),
        help="run `dump` to produce MIR dumps for the configured feature sets",
    )
    parser.add_argument("--write", action="store_true", help="regenerate table from dumps, preserving status/group/note")
    parser.add_argument("--check", action="store_true", help="verify committed table matches regenerated ledger")
    parser.add_argument("--against", default=None, help="fail if committed table adds any row absent at git REF")
    parser.add_argument(
        "--pull-request-merge",
        action="store_true",
        help="HEAD is a PR merge commit; compare `--against` using its first parent",
    )
    parser.add_argument(
        "--allow-missing-baseline",
        action="store_true",
        help="allow `--against REF` when REF does not yet contain the ledger table",
    )
    parser.add_argument(
        "--allow-unclassified",
        action="store_true",
        help="allow `status=unclassified` rows during `--check`",
    )
    parser.add_argument(
        "--diff",
        nargs=2,
        metavar=("OLD_TSV", "NEW_TSV"),
        help="compare two ledger TSVs for added, removed, and rebound targets",
    )
    parser.add_argument("--fail-on-rebound", action="store_true", help="exit 1 if `--diff` finds any rebound target")
    parser.add_argument("--fail-on-added", action="store_true", help="exit 1 if `--diff` finds any added row")
    parser.add_argument("--list-uncovered-cfgs", action="store_true", help="list source #[cfg(...)] never active in dumps")
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root directory")
    parser.add_argument("--table", type=Path, default=None, help="path to game_side_ledger.tsv")
    parser.add_argument("--module-table", type=Path, default=None, help="path to game_module_sides.tsv")
    parser.add_argument("--fn-table", type=Path, default=None, help="path to game_fn_sides.tsv")
    parser.add_argument("--field-table", type=Path, default=None, help="path to game_field_owners.tsv")
    parser.add_argument("--dump-dir", type=Path, default=None, help="directory containing <config>.mir dumps")
    parser.add_argument("--dump", action="append", default=[], metavar="CONFIG=PATH", help="explicit MIR dump path")
    parser.add_argument(
        "--configs",
        default=",".join(DEFAULT_CONFIGS),
        help="comma-separated feature configs (default: default,no-default-features,state-digest)",
    )
    parser.add_argument("--cargo", default=None, help="override cargo command (default: hz cargo if present, else cargo)")
    parser.add_argument("--skip-warm-check", action="store_true", help="skip warm `cargo check` before MIR dump")
    parser.add_argument("--no-source-callbacks", action="store_true", help="skip source callback-param scan (tests)")
    parser.add_argument("--out-dir", action="append", type=Path, default=[], help="extra OUT_DIR path for span resolution")

    args = parser.parse_args(argv)
    root = args.root.resolve()
    table_path = args.table if args.table is not None else (root / "tools/game_side_ledger.tsv")
    module_table = args.module_table if args.module_table is not None else (root / "tools/game_module_sides.tsv")
    fn_table = args.fn_table if args.fn_table is not None else (root / "tools/game_fn_sides.tsv")
    field_table = args.field_table if args.field_table is not None else (root / "tools/game_field_owners.tsv")
    configs = tuple(c.strip() for c in args.configs.split(",") if c.strip())

    try:
        if args.subcommand == "dump":
            if args.dump_dir is None:
                raise LedgerError("`dump` requires `--dump-dir DIR`")
            return run_dump_mode(root, args.dump_dir, configs, args.cargo, args.skip_warm_check)

        if args.list_uncovered_cfgs:
            uncovered = scan_uncovered_cfgs(root, configs)
            print(f"Uncovered #[cfg(...)] predicates across configs={list(configs)}: {len(uncovered)}")
            for pred, count, cat in uncovered:
                print(f"  [{cat}] ({count}x) #[cfg({pred})]")
            return 0

        if args.diff is not None:
            old_rows, _ = load_ledger_tsv(Path(args.diff[0]))
            new_rows, _ = load_ledger_tsv(Path(args.diff[1]))
            res = compute_diff(old_rows, new_rows)
            print(f"added={len(res['added'])} removed={len(res['removed'])} rebound={len(res['rebound'])}")
            for rb in res["rebound"]:
                print(
                    f"REBOUND ({rb['reason']}): {rb['user_id']} | "
                    f"{rb['old_kind']} {rb['old_target']} ({rb['old_loc']}) -> "
                    f"{rb['new_kind']} {rb['new_target']} ({rb['new_loc']})"
                )
            for r in res["added"]:
                print(f"ADDED: {r.kind} {r.user_id} -> {r.target_id} ({r.example_loc})")
            for r in res["removed"]:
                print(f"REMOVED: {r.kind} {r.user_id} -> {r.target_id} ({r.example_loc})")
            if (args.fail_on_rebound and res["rebound"]) or (args.fail_on_added and res["added"]):
                return 1
            return 0

        if args.write and args.check:
            raise LedgerError("cannot combine `--write` and `--check`")

        if args.write:
            existing_hand: dict[tuple[str, str, str], tuple[str, str, str]] = {}
            if table_path.is_file():
                prev_rows, _ = load_ledger_tsv(table_path)
                existing_hand = {r.identity: (r.status, r.group, r.note) for r in prev_rows}
            dump_paths = resolve_dump_paths(args.dump_dir, args.dump, configs)
            rows, stats = build_ledger(
                root,
                dump_paths,
                module_table,
                fn_table,
                field_table,
                existing_hand,
                include_source_callbacks=not args.no_source_callbacks,
                out_dirs=args.out_dir,
            )
            tsv_text = format_ledger_tsv(rows)
            # Round-trip validate before touching disk
            load_ledger_tsv_text(tsv_text, str(table_path))
            changed = atomic_write_text(table_path, tsv_text)
            verb = "wrote" if changed else "unchanged"
            print(f"[write] {verb} {len(rows)} rows to {table_path} ({len(stats)} config dumps)")
            return 0

        if args.check:
            t0 = time.monotonic()
            committed_rows, committed_lines = load_ledger_tsv(table_path)
            existing_hand = {r.identity: (r.status, r.group, r.note) for r in committed_rows}
            dump_paths = resolve_dump_paths(args.dump_dir, args.dump, configs)
            expected_rows, _ = build_ledger(
                root,
                dump_paths,
                module_table,
                fn_table,
                field_table,
                existing_hand,
                include_source_callbacks=not args.no_source_callbacks,
                out_dirs=args.out_dir,
            )

            errors: list[str] = []
            comm_map = {r.identity: r for r in committed_rows}
            exp_map = {r.identity: r for r in expected_rows}

            for ident in sorted(exp_map.keys() - comm_map.keys(), key=lambda x: (x[1], x[0], x[2])):
                r = exp_map[ident]
                errors.append(
                    f"MISSING in {table_path}: {r.kind} {r.user_id} -> {r.target_id} "
                    f"(example {r.example_loc}); run `--write` or eliminate the use."
                )
            for ident in sorted(comm_map.keys() - exp_map.keys(), key=lambda x: (x[1], x[0], x[2])):
                r = comm_map[ident]
                ln = committed_lines[ident]
                errors.append(
                    f"STALE in {table_path}:{ln}: {r.kind} {r.user_id} -> {r.target_id} "
                    f"(example {r.example_loc}); run `--write` to remove and ratchet down."
                )
            for ident in sorted(comm_map.keys() & exp_map.keys(), key=lambda x: (x[1], x[0], x[2])):
                c_r, e_r = comm_map[ident], exp_map[ident]
                if c_r != e_r:
                    ln = committed_lines[ident]
                    errors.append(
                        f"DRIFT in {table_path}:{ln}: {c_r.kind} {c_r.user_id} -> {c_r.target_id} "
                        f"metadata differs (committed detail={c_r.detail!r} configs={c_r.configs!r} sites={c_r.sites}, "
                        f"expected detail={e_r.detail!r} configs={e_r.configs!r} sites={e_r.sites}); run `--write`."
                    )

            if not args.allow_unclassified:
                for r in committed_rows:
                    if r.status == "unclassified":
                        ln = committed_lines[r.identity]
                        errors.append(
                            f"UNCLASSIFIED in {table_path}:{ln}: {r.kind} {r.user_id} -> {r.target_id} "
                            f"(example {r.example_loc}); set status to route/bridge/debt or pass `--allow-unclassified`."
                        )

            if format_ledger_tsv(committed_rows) != table_path.read_text(encoding="utf-8"):
                if not errors:
                    errors.append(f"ORDER in {table_path}: rows are not in canonical sorted order; run `--write`.")

            if args.against is not None:
                errors.extend(
                    check_against_ref(
                        root,
                        table_path,
                        committed_rows,
                        committed_lines,
                        args.against,
                        args.pull_request_merge,
                        args.allow_missing_baseline,
                    )
                )

            elapsed = time.monotonic() - t0
            if errors:
                print(f"[check] FAILED with {len(errors)} error(s) in {elapsed:.2f}s:", file=sys.stderr)
                for msg in errors[:50]:
                    print(f"  - {msg}", file=sys.stderr)
                if len(errors) > 50:
                    print(f"  ... and {len(errors) - 50} more error(s)", file=sys.stderr)
                return 1
            print(f"[check] OK: {len(committed_rows)} rows verified in {elapsed:.2f}s")
            return 0

        if args.against is not None:
            committed_rows, committed_lines = load_ledger_tsv(table_path)
            errors = check_against_ref(
                root,
                table_path,
                committed_rows,
                committed_lines,
                args.against,
                args.pull_request_merge,
                args.allow_missing_baseline,
            )
            if errors:
                print(f"[against] FAILED with {len(errors)} error(s):", file=sys.stderr)
                for msg in errors[:50]:
                    print(f"  - {msg}", file=sys.stderr)
                return 1
            print(f"[against] OK: {len(committed_rows)} rows conform to baseline {args.against}")
            return 0

        parser.print_help()
        return 2
    except LedgerError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
