#!/usr/bin/env python3
"""Rewrite and check field accesses for fields moved out of `struct Game<'a>`.

Rewrites only accesses whose receiver it can prove is Sunrise's `Game`, and
reports unresolved candidates for manual review. The Rust compiler is the final
check after `--fix`: a clean `--check` means no access the scanner can see is
stale, not that every access in the tree was seen.

Reads `tools/game_field_owners.tsv` next to this script (or `--table PATH`).
Fields with owner `server` live on `Game.server` (`ServerState`); fields with
owner `client` live on `Game.client` (`ClientState`); fields with owner `game`
live directly on `Game`. Normalises accesses in both directions.

Usage:
    python3 tools/game_field_owners.py --fix [--allow-ambiguous] [paths...]
    python3 tools/game_field_owners.py --check [--allow-ambiguous] [paths...]
    python3 tools/game_field_owners.py --list [--allow-ambiguous] [paths...]
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

# Token-only macros exported by `std` / `core` / `alloc`.
STD_TOKEN_ONLY_MACROS: set[str] = {
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
}

# Macros whose arguments are token trees or non-expressions (never rewrite inside).
TOKEN_ONLY_MACROS: set[str] = STD_TOKEN_ONLY_MACROS | {
    "quote",
    "quote_spanned",
    "parse_quote",
    "parse_quote_spanned",
}

# Expression macros exported by `std` / `core` / `alloc`.
STD_EXPR_MACROS: set[str] = {
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
}

# Macros known to evaluate arguments as ordinary Rust expressions.
EXPR_MACROS: set[str] = set(GAME_MACROS) | STD_EXPR_MACROS | {
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

# Standard crates whose 2-segment qualified macros are trusted when unshadowed.
TRUSTED_MACRO_CRATES: set[str] = {"std", "core", "alloc"}

# External crate roots whose qualified calls cannot return Sunrise's `Game`.
EXTERNAL_NON_GAME_CRATES: set[str] = {
    "std",
    "core",
    "alloc",
    "serde_json",
    "winit",
    "wgpu",
    "log",
    "tracing",
    "anyhow",
}

# Audited real-tree qualified macros allowed without loosening the general rule.
KNOWN_GOOD_QUALIFIED_MACROS: dict[tuple[str, str], str] = {
    ("crates/sunrise-client/src/game.rs", "log::info"): "external log::info! crate macro in game.rs",
    ("crates/sunrise-client/src/game/network/lan_entities.rs", "log::error"): "external log::error! crate macro",
    ("crates/sunrise-client/src/game/administration/commands/registered_damage_tests/creaking_borrowed.rs", "serde_json::json"): "external serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/interface/pad_menu.rs", "serde_json::json"): "external serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/interface/pad/mod.rs", "serde_json::json"): "imported serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/mobs/dragon.rs", "serde_json::json"): "external serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/network/remote/status.rs", "serde_json::json"): "external serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/runtime/agent/mod.rs", "serde_json::json"): "imported serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/runtime/control.rs", "serde_json::json"): "imported serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/runtime/resource_loader.rs", "serde_json::json"): "external serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/runtime/station_control.rs", "serde_json::json"): "imported serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/blocks/nether_portal.rs", "super::entity_residency::assign_resident_id"): "repo assign_resident_id! macro in entity_residency.rs",
    ("crates/sunrise-client/src/game/modding/native_entities.rs", "super::entity_residency::assign_resident_id"): "repo assign_resident_id! macro in entity_residency.rs",
    ("crates/sunrise-client/src/game/mobs/dragon/residency.rs", "crate::game::entity_residency::assign_resident_ids"): "repo assign_resident_ids! macro in entity_residency.rs",
    ("crates/sunrise-client/src/game/world/persist/lookup_order_tests.rs", "crate::game::entity_residency::assign_resident_ids"): "repo assign_resident_ids! macro in entity_residency.rs",
    ("crates/sunrise-client/src/game/mobs/creatures/mod.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/interface/dedicated/report.rs", "super::super::levels::entity_count_view"): "repo entity_count_view! macro in levels.rs",
    ("crates/sunrise-client/src/game/administration/commands/world_cmds.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/administration/commands/world_cmds.rs", "serde_json::json"): "external serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/administration/functions.rs", "serde_json::json"): "external serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/entities/armor_stands.rs", "super::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/entities/boats/mod.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/entities/explosion.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/entities/item_hurt.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/interface/pad/radial_play.rs", "serde_json::json"): "imported serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/interface/render_control.rs", "serde_json::json"): "imported serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/interface/wind.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/items/potions.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/items/special_attacks.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/mobs/creatures/death_weapon.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/mobs/creatures/projectile_targets.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/mobs/dragon.rs", "super::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/mobs/raids/mod.rs", "serde_json::json"): "imported serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/mobs/spawner_client_tick_tests.rs", "serde_json::json"): "external serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/mobs/wither.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/modding/native_entity_invulnerable.rs", "crate::game::entity::damage_level_of"): "repo damage_level_of! macro in entity.rs",
    ("crates/sunrise-client/src/game/network/lan/player_packet_tests.rs", "serde_json::json"): "imported serde_json::json! crate macro",
    ("crates/sunrise-client/src/game/runtime/repro.rs", "serde_json::json"): "imported serde_json::json! crate macro",
}

# Audited file-local `macro_rules!` taking ordinary expressions as arguments.
KNOWN_GOOD_LOCAL_EXPR_MACROS: set[tuple[str, str]] = {
    ("crates/sunrise-client/src/game/entities/entity/taken_pass_tests.rs", "lookup"),
    ("crates/sunrise-client/src/game/runtime/transactions/entities.rs", "apart"),
}

# Audited real-tree relative qualified calls resolving to indexed `crate::game::...` fns.
KNOWN_GOOD_QUALIFIED_FNS: dict[tuple[str, str], tuple[str, ...]] = {
    ("crates/sunrise-client/src/game/administration/commands/test_summary_context_tests.rs", "tests::server_game"): ("crate", "game", "game_tests", "tests", "server_game"),
    ("crates/sunrise-client/src/game/administration/game_tests.rs", "tests::server_game"): ("crate", "game", "game_tests", "tests", "server_game"),
    ("crates/sunrise-client/src/game/entities/boat_passengers.rs", "super::super::game_tests::tests::server_game"): ("crate", "game", "game_tests", "tests", "server_game"),
    ("crates/sunrise-client/src/game/world/storage.rs", "super::super::game_tests::tests::server_game"): ("crate", "game", "game_tests", "tests", "server_game"),
}

# Repo-defined expression macros valid only inside `crates/sunrise-client/src/game*`.
REPO_EXPR_MACROS: set[str] = set(GAME_MACROS) | {
    "assign_resident_id",
    "assign_resident_ids",
    "with_entity",
}

# Tokens that immediately precede a block expression `{ ... }` rather than a control-flow body `{`.
BLOCK_EXPR_PREV_TOKENS: set[str] = {
    "unsafe",
    "async",
    "const",
    "move",
    "try",
    "loop",
    "else",
    "mut",
    "ref",
    "!",
    "=",
    "==",
    "!=",
    "<",
    "<=",
    ">=",
    "+",
    "-",
    "*",
    "/",
    "%",
    "&",
    "&&",
    "|",
    "||",
    "^",
    ",",
    ";",
    "=>",
    "return",
    "break",
    "in",
}

Token = tuple[str, str, int, int]
ClosureArgMap = dict[int, dict[int, str]]
VarBinding = tuple[str, str]


def norm_ident(text: str) -> str:
    return text[2:] if text.startswith("r#") else text


def is_in_game_tree(rel_posix: str) -> bool:
    return rel_posix == "crates/sunrise-client/src/game.rs" or rel_posix.startswith("crates/sunrise-client/src/game/")


@dataclass(frozen=True)
class GamePathContext:
    rel_posix: str
    in_game_tree: bool
    in_client_crate: bool
    local_game_defined: bool
    foreign_game_imported: bool
    sunrise_game_imported: bool
    sunrise_game_aliases: frozenset[str] = frozenset()


@dataclass
class ScopeDecls:
    declares_game: bool = False
    foreign_game_import: bool = False
    sunrise_game_import: bool = False
    sunrise_game_aliases: set[str] = field(default_factory=set)
    shadowed_macro_roots: set[str] = field(default_factory=set)
    imported_roots: dict[str, list[str]] = field(default_factory=dict)
    glob_imports: list[list[str]] = field(default_factory=list)


@dataclass
class FileLexicalLayout:
    # Opening `{` token index -> direct declarations (`-1` is the file root module).
    scope_decls: dict[int, ScopeDecls] = field(default_factory=dict)
    # Opening `{` token index of an inline `mod <name> { ... }` -> module name.
    mod_braces: dict[int, str] = field(default_factory=dict)
    # Shadowed macro crate roots declared in enclosing ancestor module files on disk.
    enclosing_file_shadowed_roots: frozenset[str] = frozenset()


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
    # Struct name -> field name -> {closure_param_idx: kind} for `fn(...)` / `Fn*(...)` fields
    struct_closure_fields: dict[str, dict[str, dict[int, str]]] = field(default_factory=dict)
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
    # Canonical `crate::game::...` path -> return positions carrying `Game`.
    indexed_fn_returns: dict[tuple[str, ...], set[int | None]] = field(default_factory=dict)
    # Canonical `crate::game::...` paths of free fns proven not to return `Game`.
    indexed_non_game_fns: set[tuple[str, ...]] = field(default_factory=set)
    # Associated fns (no `self`) on `impl Game` returning `Game` / `Self`.
    game_assoc_returning_fns: dict[str, set[int | None]] = field(default_factory=dict)
    # File rel_posix -> canonical module path derived from `game.rs` `#[path]` declarations.
    file_mod_map: dict[str, tuple[str, ...]] = field(default_factory=dict)
    # Union of all leaf free-fn names returning `Game` anywhere in the workspace.
    all_game_return_fn_names: set[str] = field(default_factory=set)


@dataclass
class FileLocalIndex:
    defined_structs: set[str] = field(default_factory=set)
    struct_game_fields: dict[str, set[str]] = field(default_factory=dict)
    struct_closure_fields: dict[str, dict[str, dict[int, str]]] = field(default_factory=dict)
    defined_free_fns: set[str] = field(default_factory=set)
    game_returning_fns: dict[str, set[int | None]] = field(default_factory=dict)
    fn_closure_params: dict[str, ClosureArgMap] = field(default_factory=dict)
    mod_defined_fns: dict[tuple[str, ...], set[str]] = field(default_factory=dict)
    mod_game_returning_fns: dict[tuple[str, ...], dict[str, set[int | None]]] = field(default_factory=dict)
    mod_fn_closure_params: dict[tuple[str, ...], dict[str, ClosureArgMap]] = field(default_factory=dict)
    indexed_fn_returns: dict[tuple[str, ...], set[int | None]] = field(default_factory=dict)
    indexed_non_game_fns: set[tuple[str, ...]] = field(default_factory=set)
    game_assoc_returning_fns: dict[str, set[int | None]] = field(default_factory=dict)
    all_game_return_fn_names: set[str] = field(default_factory=set)


WRAPPER_TYPES: dict[str, tuple[str, str]] = {
    "client_only": ("client_side", "ClientOnly"),
    "client_shared": ("client_side", "ClientShared"),
    "server_only": ("server_side", "ServerOnly"),
    "server_shared": ("server_side", "ServerShared"),
}
WRAPPER_KEY_BY_NAME: dict[str, str] = {name: key for key, (_, name) in WRAPPER_TYPES.items()}
SIDE_WRAPPER_DEFS: dict[str, str] = {
    "client": (
        "\n#[allow(dead_code)]\n"
        "#[repr(transparent)]\n"
        "pub(in crate::game) struct ClientOnly<T>(pub(in crate::game::client_side) T);\n\n"
        "#[allow(dead_code)]\n"
        "impl<T> ClientOnly<T> {\n"
        "    #[inline(always)]\n"
        "    pub(in crate::game) fn new(value: T) -> Self {\n"
        "        Self(value)\n"
        "    }\n"
        "}\n\n"
        "#[allow(dead_code)]\n"
        "#[repr(transparent)]\n"
        "pub(in crate::game) struct ClientShared<T>(pub(in crate::game) T);\n\n"
        "#[allow(dead_code)]\n"
        "impl<T> ClientShared<T> {\n"
        "    #[inline(always)]\n"
        "    pub(in crate::game) fn new(value: T) -> Self {\n"
        "        Self(value)\n"
        "    }\n"
        "}\n"
    ),
    "server": (
        "\n#[allow(dead_code)]\n"
        "#[repr(transparent)]\n"
        "pub(in crate::game) struct ServerOnly<T>(pub(in crate::game::server_side) T);\n\n"
        "#[allow(dead_code)]\n"
        "impl<T> ServerOnly<T> {\n"
        "    #[inline(always)]\n"
        "    pub(in crate::game) fn new(value: T) -> Self {\n"
        "        Self(value)\n"
        "    }\n"
        "}\n\n"
        "#[allow(dead_code)]\n"
        "#[repr(transparent)]\n"
        "pub(in crate::game) struct ServerShared<T>(pub(in crate::game) T);\n\n"
        "#[allow(dead_code)]\n"
        "impl<T> ServerShared<T> {\n"
        "    #[inline(always)]\n"
        "    pub(in crate::game) fn new(value: T) -> Self {\n"
        "        Self(value)\n"
        "    }\n"
        "}\n"
    ),
}


def load_field_owners(table_path: Path) -> dict[str, str]:
    """Load `field -> owner` (`game` | `server` | `client`) from the TSV table."""
    owners: dict[str, str] = {}
    with table_path.open(newline="", encoding="utf-8") as fp:
        for row in csv.DictReader(fp, delimiter="\t"):
            owner = row["owner"].strip()
            if owner in ("game", "server", "client"):
                owners[row["field"].strip()] = owner
    return owners


def load_field_wrappers(table_path: Path) -> dict[str, str]:
    """Load `field -> wrapper` (`client_only` | `client_shared` | `server_only` | `server_shared`)."""
    wrappers: dict[str, str] = {}
    with table_path.open(newline="", encoding="utf-8") as fp:
        reader = csv.DictReader(fp, delimiter="\t")
        if not reader.fieldnames or "wrapper" not in reader.fieldnames:
            return wrappers
        for lineno, row in enumerate(reader, 2):
            fld = row["field"].strip()
            owner = row["owner"].strip()
            reason = row["reason"].strip()
            wrap = row["wrapper"].strip()
            if owner in ("client", "server"):
                if wrap != "-":
                    raise SystemExit(
                        f"{table_path}:{lineno}: {owner} field `{fld}` must have wrapper `-`, got {wrap!r}"
                    )
            elif owner == "game":
                if reason.startswith("drop-order (client)"):
                    if wrap not in ("client_only", "client_shared"):
                        raise SystemExit(
                            f"{table_path}:{lineno}: client drop-order field `{fld}` must have wrapper "
                            f"`client_only` or `client_shared`, got {wrap!r}"
                        )
                    wrappers[fld] = wrap
                elif reason.startswith("drop-order (server)"):
                    if wrap not in ("server_only", "server_shared"):
                        raise SystemExit(
                            f"{table_path}:{lineno}: server drop-order field `{fld}` must have wrapper "
                            f"`server_only` or `server_shared`, got {wrap!r}"
                        )
                    wrappers[fld] = wrap
                else:
                    if wrap != "-":
                        raise SystemExit(
                            f"{table_path}:{lineno}: shared/mode field `{fld}` must have wrapper `-`, got {wrap!r}"
                        )
    return wrappers


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


def extract_pattern_bound_names(pat_toks: list[Token]) -> tuple[set[str], set[str]]:
    """Return `(definite_bound_names, uncertain_upper_names)` introduced by a Rust pattern."""
    if not pat_toks:
        return set(), set()

    # Or-patterns `p1 | p2` (including leading `|`)
    or_parts = split_top_level_no_angles(pat_toks, "|")
    if len(or_parts) > 1:
        definite: set[str] = set()
        uncertain: set[str] = set()
        for part in or_parts:
            d, u = extract_pattern_bound_names(part)
            definite.update(d)
            uncertain.update(u)
        return definite, uncertain
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
        return set(), set()

    # `@` subpattern: `[ref] [mut] ident @ subpat`
    at_idx = find_top_level_token(p, "@")
    if at_idx is not None:
        definite = set()
        uncertain = set()
        left = p[:at_idx]
        if len(left) == 1 and left[0][0] == "IDENT" and left[0][1] != "_":
            definite.add(norm_ident(left[0][1]))
        d, u = extract_pattern_bound_names(p[at_idx + 1 :])
        definite.update(d)
        uncertain.update(u)
        return definite, uncertain

    # Range pattern `a..=b`, `a..b`, `..=b`, `..`
    if find_top_level_token(p, "..") is not None or find_top_level_token(p, "..=") is not None:
        return set(), set()

    # Tuple `(p1, p2, ...)` or parenthesised pattern
    if p[0][1] == "(" and p[-1][1] == ")" and skip_balanced(p, 0, "(", ")") == len(p):
        definite = set()
        uncertain = set()
        for elem in split_top_level_no_angles(p[1:-1], ","):
            d, u = extract_pattern_bound_names(elem)
            definite.update(d)
            uncertain.update(u)
        return definite, uncertain

    # Slice `[p1, p2, ...]`
    if p[0][1] == "[" and p[-1][1] == "]" and skip_balanced(p, 0, "[", "]") == len(p):
        definite = set()
        uncertain = set()
        for elem in split_top_level_no_angles(p[1:-1], ","):
            d, u = extract_pattern_bound_names(elem)
            definite.update(d)
            uncertain.update(u)
        return definite, uncertain

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
            definite = set()
            uncertain = set()
            for field_pat in split_top_level_no_angles(p[open_brace + 1 : -1], ","):
                if not field_pat or (len(field_pat) == 1 and field_pat[0][1] == ".."):
                    continue
                colon = find_top_level_token(field_pat, ":")
                if colon is not None:
                    d, u = extract_pattern_bound_names(field_pat[colon + 1 :])
                    definite.update(d)
                    uncertain.update(u)
                else:
                    # Shorthand `[ref] [mut] field`
                    q = 0
                    while q < len(field_pat) and field_pat[q][1] in ("ref", "mut"):
                        q += 1
                    if q == len(field_pat) - 1 and field_pat[q][0] == "IDENT" and field_pat[q][1] != "_":
                        definite.add(norm_ident(field_pat[q][1]))
            return definite, uncertain

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
            definite = set()
            uncertain = set()
            for arg in split_top_level_no_angles(p[open_paren + 1 : -1], ","):
                d, u = extract_pattern_bound_names(arg)
                definite.update(d)
                uncertain.update(u)
            return definite, uncertain

    # Leaf identifier
    if len(p) == 1 and p[0][0] == "IDENT":
        ident = p[0][1]
        if ident in ("_", "true", "false", "self", "Self", "crate", "super"):
            return set(), set()
        raw_name = norm_ident(ident)
        # Bare UpperCamelCase/ALL_CAPS without `mut`/`ref` may be a variant/const OR a binding.
        if not has_binding_mod and raw_name and raw_name[0].isupper():
            return set(), {raw_name}
        return {raw_name}, set()

    return set(), set()


def parse_use_tree_detailed(toks: list[Token], prefix: list[str]) -> list[tuple[list[str], str, bool]]:
    if not toks:
        return []
    k = 0
    n = len(toks)
    segs = list(prefix)
    while k < n:
        v = toks[k][1]
        if v == "::":
            k += 1
            continue
        if v == "{":
            end = skip_balanced(toks, k, "{", "}")
            out: list[tuple[list[str], str, bool]] = []
            for part in split_top_level_no_angles(toks[k + 1 : end - 1], ","):
                if part:
                    out.extend(parse_use_tree_detailed(part, segs))
            return out
        if v == "*":
            return [(segs + ["*"], "*", False)]
        if v == "as" and k + 1 < n and toks[k + 1][0] == "IDENT":
            return [(segs, norm_ident(toks[k + 1][1]), True)]
        if toks[k][0] == "IDENT":
            segs.append(norm_ident(v))
            k += 1
            continue
        break
    if not segs:
        return []
    if segs[-1] == "self":
        return [(segs[:-1], segs[-2], False)] if len(segs) >= 2 else []
    return [(segs, segs[-1], False)]


def parse_use_tree(toks: list[Token], prefix: list[str]) -> list[tuple[list[str], str]]:
    return [(segs, bound) for segs, bound, _ in parse_use_tree_detailed(toks, prefix)]


def collect_use_imports_detailed(tokens: list[Token]) -> list[tuple[list[str], str, bool]]:
    imports: list[tuple[list[str], str, bool]] = []
    n = len(tokens)
    i = 0
    while i < n:
        if tokens[i][0] == "IDENT" and tokens[i][1] == "use":
            prev = tokens[i - 1][1] if i > 0 else None
            if prev in (None, ";", "{", "}", "]", ")", "pub"):
                k = i + 1
                dc = 0
                while k < n:
                    tv = tokens[k][1]
                    if tv == "{":
                        dc += 1
                    elif tv == "}":
                        dc = max(0, dc - 1)
                    elif tv == ";" and dc == 0:
                        imports.extend(parse_use_tree_detailed(tokens[i + 1 : k], []))
                        i = k
                        break
                    k += 1
        i += 1
    return imports


def collect_use_imports(tokens: list[Token]) -> list[tuple[list[str], str]]:
    return [(segs, bound) for segs, bound, _ in collect_use_imports_detailed(tokens)]


def extract_generic_type_params(gen_toks: list[Token]) -> set[str]:
    params: set[str] = set()
    for part in split_top_level(gen_toks, ","):
        k = 0
        m = len(part)
        while k < m and part[k][1] == "#" and k + 1 < m and part[k + 1][1] == "[":
            k = skip_balanced(part, k + 1, "[", "]")
        if k >= m:
            continue
        if part[k][0] == "LIFETIME" or part[k][1] == "const":
            continue
        if part[k][0] == "IDENT":
            params.add(norm_ident(part[k][1]))
    return params


def parse_game_rs_mod_map(root: Path) -> dict[str, tuple[str, ...]]:
    mod_map: dict[str, tuple[str, ...]] = {"crates/sunrise-client/src/game.rs": ("crate", "game")}
    sources = [
        (root / "crates/sunrise-client/src/game.rs", "crates/sunrise-client/src"),
        (root / "crates/sunrise-client/src/game/client_side.rs", "crates/sunrise-client/src/game"),
        (root / "crates/sunrise-client/src/game/server_side.rs", "crates/sunrise-client/src/game"),
    ]
    for mod_file, path_base in sources:
        if not mod_file.is_file():
            continue
        toks = tokenize_rust(mod_file.read_text(encoding="utf-8"))
        n = len(toks)
        depth = 0
        pending_path: str | None = None
        i = 0
        while i < n:
            tv = toks[i][1]
            if tv == "{":
                depth += 1
                pending_path = None
                i += 1
                continue
            if tv == "}":
                depth = max(0, depth - 1)
                pending_path = None
                i += 1
                continue
            if depth > 0:
                i += 1
                continue
            if tv == "#" and i + 1 < n and toks[i + 1][1] == "[":
                end = skip_balanced(toks, i + 1, "[", "]")
                attr = toks[i + 2 : end - 1]
                if len(attr) == 3 and attr[0][1] == "path" and attr[1][1] == "=" and attr[2][0] == "LIT":
                    lit = attr[2][1]
                    if len(lit) >= 2 and lit[0] == '"' and lit[-1] == '"':
                        pending_path = lit[1:-1]
                i = end
                continue
            if toks[i][0] == "IDENT" and tv == "mod" and i + 2 < n and toks[i + 1][0] == "IDENT":
                mname = norm_ident(toks[i + 1][1])
                if toks[i + 2][1] == ";":
                    if pending_path is not None:
                        rel = f"{path_base}/{pending_path.lstrip('./')}"
                        mod_map[rel] = ("crate", "game", mname)
                    else:
                        mod_map[f"crates/sunrise-client/src/game/{mname}.rs"] = ("crate", "game", mname)
                        mod_map[f"crates/sunrise-client/src/game/{mname}/mod.rs"] = ("crate", "game", mname)
                    pending_path = None
                    i += 3
                    continue
                pending_path = None
            elif tv == ";":
                pending_path = None
            i += 1
    return mod_map


def file_canonical_mod_path(rel_posix: str, file_mod_map: dict[str, tuple[str, ...]]) -> tuple[str, ...]:
    if rel_posix == "crates/sunrise-client/src/game.rs":
        return ("crate", "game")
    if rel_posix in file_mod_map:
        return file_mod_map[rel_posix]
    best_prefix = ""
    best_mod: tuple[str, ...] = ()
    for mapped_rel, mpath in file_mod_map.items():
        if mapped_rel.endswith("/mod.rs"):
            dir_prefix = mapped_rel[: -len("mod.rs")]
            if rel_posix.startswith(dir_prefix) and len(dir_prefix) > len(best_prefix):
                best_prefix = dir_prefix
                best_mod = mpath
    if best_prefix:
        rem = rel_posix[len(best_prefix) :]
        if rem.endswith(".rs"):
            rem = rem[:-3]
        parts = [p for p in rem.split("/") if p]
        if parts and parts[-1] == "mod":
            parts = parts[:-1]
        return best_mod + tuple(parts)
    prefix = "crates/sunrise-client/src/game/"
    if rel_posix.startswith(prefix):
        rem = rel_posix[len(prefix) :]
        if rem.endswith(".rs"):
            rem = rem[:-3]
        parts = [p for p in rem.split("/") if p]
        if parts and parts[-1] == "mod":
            parts = parts[:-1]
        return ("crate", "game", *parts)
    return ()


def is_sunrise_game_path_raw(
    segs: list[str],
    in_game_tree: bool,
    in_client_crate: bool,
    rel_posix: str,
    local_game_defined: bool,
) -> bool:
    if not segs or segs[-1] != "Game":
        return False
    if len(segs) == 1:
        return in_game_tree and not local_game_defined
    prefix = segs[:-1]
    if prefix == ["sunrise_client", "game"]:
        return True
    if prefix == ["crate", "game"] and in_client_crate:
        return True
    if prefix == ["self"] and rel_posix == "crates/sunrise-client/src/game.rs" and not local_game_defined:
        return True
    if in_game_tree and all(s == "super" for s in prefix):
        return True
    if in_client_crate and len(prefix) >= 2 and prefix[-1] == "game" and all(s == "super" for s in prefix[:-1]):
        return True
    return False


def is_sunrise_game_path(segs: list[str], ctx: GamePathContext) -> bool:
    if not segs or segs[-1] != "Game":
        return False
    if len(segs) == 1:
        if ctx.local_game_defined or ctx.foreign_game_imported:
            return False
        return ctx.in_game_tree or ctx.sunrise_game_imported
    return is_sunrise_game_path_raw(
        segs,
        ctx.in_game_tree,
        ctx.in_client_crate,
        ctx.rel_posix,
        ctx.local_game_defined,
    )


def build_game_path_context(
    tokens: list[Token],
    use_imports: list[tuple[list[str], str]],
    rel_posix: str,
) -> GamePathContext:
    in_game = is_in_game_tree(rel_posix)
    in_client = rel_posix.startswith("crates/sunrise-client/src/")
    local_mock = file_defines_local_mock_game(tokens, rel_posix)
    foreign_imported = False
    sunrise_imported = False
    aliases: set[str] = set()
    for segs, bound in use_imports:
        if bound == "*" and len(segs) >= 2:
            if is_sunrise_game_path_raw(segs[:-1] + ["Game"], in_game, in_client, rel_posix, local_mock):
                sunrise_imported = True
            continue
        if bound == "Game":
            if is_sunrise_game_path_raw(segs, in_game, in_client, rel_posix, local_mock):
                sunrise_imported = True
            else:
                foreign_imported = True
            continue
        if is_sunrise_game_path_raw(segs, in_game, in_client, rel_posix, local_mock):
            aliases.add(bound)
    return GamePathContext(
        rel_posix=rel_posix,
        in_game_tree=in_game,
        in_client_crate=in_client,
        local_game_defined=local_mock,
        foreign_game_imported=foreign_imported,
        sunrise_game_imported=sunrise_imported,
        sunrise_game_aliases=frozenset(aliases),
    )


def scan_file_top_level_shadowed_roots(path: Path, cache: dict[Path, frozenset[str]]) -> frozenset[str]:
    if path in cache:
        return cache[path]
    if not path.is_file():
        cache[path] = frozenset()
        return cache[path]
    src = path.read_text(encoding="utf-8")
    if not any(r in src for r in TRUSTED_MACRO_CRATES):
        cache[path] = frozenset()
        return cache[path]
    toks = tokenize_rust(src)
    roots: set[str] = set()
    depth = 0
    n = len(toks)
    i = 0
    while i < n:
        tv = toks[i][1]
        if tv == "{":
            depth += 1
            i += 1
            continue
        if tv == "}":
            depth = max(0, depth - 1)
            i += 1
            continue
        if depth > 0:
            i += 1
            continue
        if toks[i][0] == "IDENT" and tv == "macro_rules" and i + 3 < n and toks[i + 1][1] == "!" and toks[i + 3][1] in ("(", "[", "{"):
            close = {"(": ")", "[": "]", "{": "}"}[toks[i + 3][1]]
            i = skip_balanced(toks, i + 3, toks[i + 3][1], close)
            continue
        prev = toks[i - 1][1] if i > 0 else None
        if toks[i][0] == "IDENT" and tv == "mod" and prev not in (".", "::") and i + 1 < n and toks[i + 1][0] == "IDENT":
            mname = norm_ident(toks[i + 1][1])
            if mname in TRUSTED_MACRO_CRATES:
                roots.add(mname)
        elif toks[i][0] == "IDENT" and tv == "extern" and i + 4 < n and toks[i + 1][1] == "crate" and toks[i + 3][1] == "as" and toks[i + 4][0] == "IDENT":
            alias = norm_ident(toks[i + 4][1])
            if alias in TRUSTED_MACRO_CRATES:
                roots.add(alias)
        elif toks[i][0] == "IDENT" and tv == "use" and prev in (None, ";", "{", "}", "]", ")", "pub"):
            k = i + 1
            dc = 0
            while k < n:
                ktv = toks[k][1]
                if ktv == "{":
                    dc += 1
                elif ktv == "}":
                    dc = max(0, dc - 1)
                elif ktv == ";" and dc == 0:
                    for segs, bound, is_as in parse_use_tree_detailed(toks[i + 1 : k], []):
                        if bound in TRUSTED_MACRO_CRATES and (is_as or segs != [bound]):
                            roots.add(bound)
                    i = k
                    break
                k += 1
        i += 1
    cache[path] = frozenset(roots)
    return cache[path]


def collect_enclosing_file_shadowed_roots(
    rel_posix: str,
    root: Path | None,
    cache: dict[Path, frozenset[str]],
) -> frozenset[str]:
    if root is None:
        return frozenset()
    rel_path = Path(rel_posix)
    if rel_path.is_absolute():
        return frozenset()
    cur_full = (root / rel_path).resolve()
    ancestors: list[Path] = []
    if rel_posix.startswith("crates/sunrise-client/src/game/"):
        ancestors.append((root / "crates/sunrise-client/src/game.rs").resolve())
        ancestors.append((root / "crates/sunrise-client/src/lib.rs").resolve())
    p = cur_full.parent
    root_resolved = root.resolve()
    while p != root_resolved and p.parent != p:
        for cand in (p / "mod.rs", p.with_suffix(".rs")):
            if cand != cur_full and cand.is_file():
                ancestors.append(cand)
        if p.name == "src":
            for cand in (p / "lib.rs", p / "main.rs"):
                if cand != cur_full and cand.is_file():
                    ancestors.append(cand)
            break
        p = p.parent
    out: set[str] = set()
    for anc in set(ancestors):
        if anc != cur_full:
            out.update(scan_file_top_level_shadowed_roots(anc, cache))
    return frozenset(out)


def build_file_lexical_layout(
    tokens: list[Token],
    rel_posix: str,
    root: Path | None = None,
    file_root_cache: dict[Path, frozenset[str]] | None = None,
) -> FileLexicalLayout:
    layout = FileLexicalLayout()
    layout.scope_decls[-1] = ScopeDecls()
    in_game = is_in_game_tree(rel_posix)
    in_client = rel_posix.startswith("crates/sunrise-client/src/")
    is_game_rs = rel_posix.endswith("crates/sunrise-client/src/game.rs")
    cache = file_root_cache if file_root_cache is not None else {}
    layout.enclosing_file_shadowed_roots = collect_enclosing_file_shadowed_roots(rel_posix, root, cache)

    brace_stack: list[int] = [-1]
    n = len(tokens)
    i = 0
    while i < n:
        kind, tv, _, _ = tokens[i]
        if kind == "IDENT" and tv == "macro_rules" and i + 3 < n and tokens[i + 1][1] == "!" and tokens[i + 3][1] in ("(", "[", "{"):
            close = {"(": ")", "[": "]", "{": "}"}[tokens[i + 3][1]]
            i = skip_balanced(tokens, i + 3, tokens[i + 3][1], close)
            continue
        if tv == "{":
            brace_stack.append(i)
            layout.scope_decls.setdefault(i, ScopeDecls())
            i += 1
            continue
        if tv == "}":
            if len(brace_stack) > 1:
                brace_stack.pop()
            i += 1
            continue

        cur_key = brace_stack[-1]
        cur = layout.scope_decls[cur_key]
        prev = tokens[i - 1][1] if i > 0 else None

        if kind == "IDENT" and tv == "mod" and prev not in (".", "::") and i + 1 < n and tokens[i + 1][0] == "IDENT":
            mname = norm_ident(tokens[i + 1][1])
            cur.shadowed_macro_roots.add(mname)
            if i + 2 < n and tokens[i + 2][1] == "{":
                layout.mod_braces[i + 2] = mname
        elif kind == "IDENT" and tv == "extern" and prev not in (".", "::") and i + 4 < n and tokens[i + 1][1] == "crate":
            if tokens[i + 3][1] == "as" and tokens[i + 4][0] == "IDENT":
                cur.shadowed_macro_roots.add(norm_ident(tokens[i + 4][1]))
        elif kind == "IDENT" and tv == "use" and prev in (None, ";", "{", "}", "]", ")", "pub"):
            k = i + 1
            dc = 0
            while k < n:
                ktv = tokens[k][1]
                if ktv == "{":
                    dc += 1
                elif ktv == "}":
                    dc = max(0, dc - 1)
                elif ktv == ";" and dc == 0:
                    for segs, bound, is_as in parse_use_tree_detailed(tokens[i + 1 : k], []):
                        if bound != "*":
                            cur.imported_roots[bound] = segs
                            if is_as or segs != [bound]:
                                cur.shadowed_macro_roots.add(bound)
                        if bound == "*" and len(segs) >= 2:
                            cur.glob_imports.append(segs[:-1])
                            if is_sunrise_game_path_raw(segs[:-1] + ["Game"], in_game, in_client, rel_posix, False):
                                cur.sunrise_game_import = True
                            continue
                        if bound == "Game":
                            if is_sunrise_game_path_raw(segs, in_game, in_client, rel_posix, False):
                                cur.sunrise_game_import = True
                            elif is_as:
                                cur.declares_game = True
                            else:
                                cur.foreign_game_import = True
                        elif bound != "*" and is_sunrise_game_path_raw(segs, in_game, in_client, rel_posix, False):
                            cur.sunrise_game_aliases.add(bound)
                    i = k
                    break
                k += 1
        elif kind == "IDENT" and tv in ("struct", "enum", "union", "trait", "type") and prev not in (".", "::") and i + 1 < n and tokens[i + 1][0] == "IDENT":
            iname = norm_ident(tokens[i + 1][1])
            if iname == "Game":
                if not (tv == "struct" and is_game_rs and cur_key == -1):
                    cur.declares_game = True
            elif tv == "type":
                k = i + 2
                if k < n and tokens[k][1] == "<":
                    k = skip_angle_generics(tokens, k)
                if k < n and tokens[k][1] == "=":
                    rs = k + 1
                    re = rs
                    da = dp = db = 0
                    while re < n:
                        rtv = tokens[re][1]
                        if rtv == "<":
                            da += 1
                        elif rtv == ">":
                            da = max(0, da - 1)
                        elif rtv == "(":
                            dp += 1
                        elif rtv == ")":
                            dp = max(0, dp - 1)
                        elif rtv == "[":
                            db += 1
                        elif rtv == "]":
                            db = max(0, db - 1)
                        elif rtv == ";" and da == 0 and dp == 0 and db == 0:
                            break
                        re += 1
                    parsed = extract_type_path_segments(tokens[rs:re], allow_dyn=False)
                    if parsed is not None:
                        rsegs, rj = parsed
                        rhs_slice = tokens[rs:re]
                        if rj < len(rhs_slice) and rhs_slice[rj][1] == "<":
                            rj = skip_angle_generics(rhs_slice, rj)
                        if rj == len(rhs_slice) and is_sunrise_game_path_raw(rsegs, in_game, in_client, rel_posix, False):
                            cur.sunrise_game_aliases.add(iname)
        i += 1

    # Propagate `Game` and shadowed macro roots from local modules imported via `use mod::*;`.
    mods_with_game: set[str] = set()
    mod_shadowed_roots: dict[str, set[str]] = {}
    for brace_idx, mname in layout.mod_braces.items():
        mscope = layout.scope_decls.get(brace_idx)
        if mscope is None:
            continue
        if mscope.declares_game:
            mods_with_game.add(mname)
        if mscope.shadowed_macro_roots:
            mod_shadowed_roots.setdefault(mname, set()).update(mscope.shadowed_macro_roots)
    for scope in layout.scope_decls.values():
        for gsegs in scope.glob_imports:
            if not gsegs:
                continue
            leaf = gsegs[-1]
            if leaf in mods_with_game:
                scope.declares_game = True
            if leaf in mod_shadowed_roots:
                scope.shadowed_macro_roots.update(mod_shadowed_roots[leaf])
    return layout


@dataclass(frozen=True)
class LexicalState:
    enclosing_declares_game: bool = False
    foreign_game_import: bool = False
    sunrise_game_import: bool = False
    sunrise_game_aliases: frozenset[str] = frozenset()
    generic_params: frozenset[str] = frozenset()
    at_game_rs_root: bool = False


def compute_lexical_state(
    scope_stack: list[ScopeDecls],
    generic_stack: list[tuple[int, set[str]]],
    extra_generics: set[str] | None,
    rel_posix: str,
    in_inline_mod: bool,
) -> LexicalState:
    declares_game = any(s.declares_game for s in scope_stack)
    foreign_imp = False
    sunrise_imp = False
    aliases: set[str] = set()
    for s in scope_stack:
        if s.foreign_game_import:
            foreign_imp = True
            sunrise_imp = False
        if s.sunrise_game_import:
            sunrise_imp = True
            foreign_imp = False
        aliases.update(s.sunrise_game_aliases)
    gparams: set[str] = set()
    for _, gp in generic_stack:
        gparams.update(gp)
    if extra_generics:
        gparams.update(extra_generics)
    at_root = rel_posix.endswith("crates/sunrise-client/src/game.rs") and not in_inline_mod
    return LexicalState(
        enclosing_declares_game=declares_game,
        foreign_game_import=foreign_imp,
        sunrise_game_import=sunrise_imp,
        sunrise_game_aliases=frozenset(aliases),
        generic_params=frozenset(gparams),
        at_game_rs_root=at_root,
    )


def collect_shadowed_macros(
    tokens: list[Token],
    use_imports: list[tuple[list[str], str]],
    in_game_tree: bool,
    rel_posix: str = "",
) -> set[str]:
    shadowed: set[str] = set()
    n = len(tokens)
    for i in range(n - 2):
        if tokens[i][0] == "IDENT" and tokens[i][1] == "macro_rules" and tokens[i + 1][1] == "!" and tokens[i + 2][0] == "IDENT":
            mname = norm_ident(tokens[i + 2][1])
            if not (in_game_tree and (mname in GAME_MACROS or (rel_posix, mname) in KNOWN_GOOD_LOCAL_EXPR_MACROS)):
                shadowed.add(mname)
    known_macros = EXPR_MACROS | TOKEN_ONLY_MACROS
    for segs, bound in use_imports:
        if bound not in known_macros:
            continue
        if segs and segs[0] in TRUSTED_MACRO_CRATES and segs[-1] == bound:
            continue
        if segs and segs[-1] == bound and (rel_posix, "::".join(segs)) in KNOWN_GOOD_QUALIFIED_MACROS:
            continue
        if in_game_tree and bound in REPO_EXPR_MACROS and segs and segs[0] in ("crate", "super", "self") and segs[-1] == bound:
            continue
        shadowed.add(bound)
    return shadowed


def extract_type_path_segments(toks: list[Token], allow_dyn: bool = False) -> tuple[list[str], int] | None:
    if not toks:
        return None
    j = 0
    n = len(toks)
    skip_words = ("&", "mut", "*", "const", "dyn") if allow_dyn else ("&", "mut", "*", "const")
    while j < n and (toks[j][1] in skip_words or toks[j][0] == "LIFETIME"):
        j += 1
    if j >= n:
        return None
    segs: list[str] = []
    while j < n:
        k, v, _, _ = toks[j]
        if k == "IDENT":
            segs.append(norm_ident(v))
            j += 1
            if j < n and toks[j][1] == "::":
                j += 1
                continue
            break
        elif v == "::":
            j += 1
        else:
            return None
    return (segs, j) if segs else None


def extract_type_base_name(toks: list[Token]) -> str | None:
    parsed = extract_type_path_segments(toks, allow_dyn=True)
    return parsed[0][-1] if parsed is not None else None


def is_exact_type_name(toks: list[Token], target_name: str) -> bool:
    """Return True if `toks` is `[& ['a] [mut]] [path::]TargetName[<...>]`."""
    parsed = extract_type_path_segments(toks, allow_dyn=False)
    if parsed is None:
        return False
    segs, j = parsed
    if segs[-1] != target_name:
        return False
    if j < len(toks) and toks[j][1] == "<":
        j = skip_angle_generics(toks, j)
    return j == len(toks)


def classify_type_tokens(
    toks: list[Token],
    wrapper_structs: set[str],
    self_type: str | None = None,
    ctx: GamePathContext | None = None,
    lex: LexicalState | None = None,
) -> tuple[str | None, str]:
    parsed = extract_type_path_segments(toks, allow_dyn=False)
    if parsed is None:
        return None, ""
    segs, j = parsed
    if j < len(toks) and toks[j][1] == "<":
        j = skip_angle_generics(toks, j)
    if j != len(toks):
        return None, ""

    if segs == ["Self"]:
        if self_type == "Game":
            return "Game", ""
        if self_type == AMBIGUOUS:
            return AMBIGUOUS, "unresolved `Self` in shadowed `impl Game`"
        if self_type is not None and self_type in wrapper_structs:
            return self_type, ""
        return None, ""

    aliases = lex.sunrise_game_aliases if lex is not None else (ctx.sunrise_game_aliases if ctx is not None else frozenset())
    if len(segs) == 1 and segs[0] in aliases:
        return AMBIGUOUS, f"aliased `Game` type `{segs[0]}`"

    if segs[-1] == "Game":
        if ctx is None:
            return "Game", ""
        declares_game = lex.enclosing_declares_game if lex is not None else ctx.local_game_defined
        gparams = lex.generic_params if lex is not None else frozenset()
        foreign_imp = lex.foreign_game_import if lex is not None else ctx.foreign_game_imported
        sunrise_imp = lex.sunrise_game_import if lex is not None else ctx.sunrise_game_imported
        at_game_root = lex.at_game_rs_root if lex is not None else ctx.rel_posix.endswith("crates/sunrise-client/src/game.rs")

        if len(segs) == 1:
            if "Game" in gparams:
                return AMBIGUOUS, "unresolved `Game` (shadowed by generic parameter `Game`)"
            if declares_game:
                return AMBIGUOUS, "unresolved `Game` (enclosing module declares `Game`)"
            if foreign_imp:
                return None, ""
            if ctx.in_game_tree or sunrise_imp:
                return "Game", ""
            return None, ""

        prefix = segs[:-1]
        if prefix == ["sunrise_client", "game"]:
            return "Game", ""
        if prefix == ["crate", "game"] and ctx.in_client_crate:
            return "Game", ""
        if ctx.in_client_crate and len(prefix) >= 2 and prefix[-1] == "game" and all(s == "super" for s in prefix[:-1]):
            return "Game", ""
        if prefix == ["self"]:
            if declares_game:
                return AMBIGUOUS, "unresolved `self::Game` (enclosing module declares `Game`)"
            if at_game_root:
                return "Game", ""
            return AMBIGUOUS, "unresolved `self::Game`"
        if all(s == "super" for s in prefix):
            if declares_game:
                return AMBIGUOUS, f"unresolved `{'::'.join(segs)}` (enclosing module declares `Game`)"
            if ctx.in_game_tree:
                return "Game", ""
            return AMBIGUOUS, f"unresolved `{'::'.join(segs)}`"
        return None, ""

    base = segs[-1]
    if base in wrapper_structs:
        return base, ""
    return None, ""


def is_game_type_tokens(
    toks: list[Token],
    self_type: str | None = None,
    ctx: GamePathContext | None = None,
    lex: LexicalState | None = None,
) -> bool:
    kind, _ = classify_type_tokens(toks, set(), self_type, ctx, lex)
    return kind == "Game"


def resolve_type_kind(
    toks: list[Token],
    wrapper_structs: set[str],
    self_type: str | None = None,
    ctx: GamePathContext | None = None,
    lex: LexicalState | None = None,
) -> str | None:
    kind, _ = classify_type_tokens(toks, wrapper_structs, self_type, ctx, lex)
    return None if kind == AMBIGUOUS else kind


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


def extract_game_return_positions(
    ret_toks: list[Token],
    self_type: str | None = None,
    ctx: GamePathContext | None = None,
    lex: LexicalState | None = None,
) -> set[int | None]:
    """Return positions where a function's return type carries `Game`."""
    inner = unwrap_result_or_option(ret_toks)
    if is_game_type_tokens(inner, self_type, ctx, lex):
        return {None}
    if len(inner) >= 2 and inner[0][1] == "(" and inner[-1][1] == ")":
        elems = split_top_level(inner[1:-1], ",")
        out: set[int | None] = set()
        for idx, elem in enumerate(elems):
            if is_game_type_tokens(unwrap_result_or_option(elem), self_type, ctx, lex):
                out.add(idx)
        return out
    return set()


def parse_impl_target(
    tokens: list[Token],
    impl_idx: int,
    ctx: GamePathContext | None = None,
    lex: LexicalState | None = None,
) -> tuple[str | None, int, set[str]] | None:
    n = len(tokens)
    j = impl_idx + 1
    if j < n and tokens[j][1] == "const":
        j += 1
    impl_generics: set[str] = set()
    if j < n and tokens[j][1] == "<":
        gend = skip_angle_generics(tokens, j)
        impl_generics = extract_generic_type_params(tokens[j + 1 : gend - 1])
        j = gend
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
                type_toks = tokens[type_start:type_end]
                eff_lex = lex
                if lex is not None and impl_generics:
                    eff_lex = LexicalState(
                        enclosing_declares_game=lex.enclosing_declares_game,
                        foreign_game_import=lex.foreign_game_import,
                        sunrise_game_import=lex.sunrise_game_import,
                        sunrise_game_aliases=lex.sunrise_game_aliases,
                        generic_params=lex.generic_params | impl_generics,
                        at_game_rs_root=lex.at_game_rs_root,
                    )
                if ctx is not None:
                    kind, _ = classify_type_tokens(type_toks, set(), None, ctx, eff_lex)
                    if kind == "Game":
                        target: str | None = "Game"
                    elif kind == AMBIGUOUS:
                        target = AMBIGUOUS
                    else:
                        base = extract_type_base_name(type_toks)
                        target = None if base == "Game" else base
                else:
                    target = extract_type_base_name(type_toks)
                return target, j, impl_generics
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
    is_game_rs = rel_posix.endswith("crates/sunrise-client/src/game.rs")
    n = len(tokens)
    for i in range(n - 1):
        if tokens[i][0] == "IDENT" and tokens[i + 1][0] == "IDENT" and norm_ident(tokens[i + 1][1]) == "Game":
            kw = tokens[i][1]
            prev = tokens[i - 1][1] if i > 0 else None
            if prev in (".", "::"):
                continue
            if kw == "struct" and is_game_rs:
                continue
            if kw in ("struct", "enum", "trait", "union", "type"):
                return True
    return False


def scan_structs(
    tokens: list[Token],
    ctx: GamePathContext | None = None,
    layout: FileLexicalLayout | None = None,
) -> tuple[set[str], dict[str, set[str]], dict[str, dict[str, dict[int, str]]]]:
    """Return `(all_defined_structs, struct_name -> set of fields holding Game, struct_name -> field -> closure_params)`."""
    defined: set[str] = set()
    game_fields: dict[str, set[str]] = {}
    closure_fields: dict[str, dict[str, dict[int, str]]] = {}
    n = len(tokens)
    scope_stack: list[ScopeDecls] = [layout.scope_decls[-1]] if layout is not None else []
    mod_stack: list[tuple[int, str]] = []
    brace_depth = 0
    i = 0
    while i < n:
        tv = tokens[i][1]
        if tv == "{":
            brace_depth += 1
            if layout is not None:
                scope_stack.append(layout.scope_decls.get(i, ScopeDecls()))
                if i in layout.mod_braces:
                    mod_stack.append((brace_depth, layout.mod_braces[i]))
            i += 1
            continue
        if tv == "}":
            while mod_stack and mod_stack[-1][0] >= brace_depth:
                mod_stack.pop()
            if layout is not None and len(scope_stack) > 1:
                scope_stack.pop()
            brace_depth = max(0, brace_depth - 1)
            i += 1
            continue
        if tokens[i][0] == "IDENT" and tv == "struct" and i + 1 < n and tokens[i + 1][0] == "IDENT":
            sname = norm_ident(tokens[i + 1][1])
            defined.add(sname)
            j = i + 2
            s_generics: set[str] = set()
            if j < n and tokens[j][1] == "<":
                gend = skip_angle_generics(tokens, j)
                s_generics = extract_generic_type_params(tokens[j + 1 : gend - 1])
                j = gend
            lex = (
                compute_lexical_state(scope_stack, [], s_generics, ctx.rel_posix if ctx else "", bool(mod_stack))
                if layout is not None
                else None
            )
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
                        fname = norm_ident(part[k][1])
                        ftype = part[k + 2 :]
                        if is_game_type_tokens(ftype, None, ctx, lex):
                            game_fields.setdefault(sname, set()).add(fname)
                        cparams = extract_fn_trait_params(ftype, set(), None, ctx, lex)
                        if cparams:
                            closure_fields.setdefault(sname, {})[fname] = cparams
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
                    if is_game_type_tokens(part[k:], None, ctx, lex):
                        game_fields.setdefault(sname, set()).add(str(tidx))
                i = end
                continue
        i += 1
    return defined, game_fields, closure_fields


def extract_fn_trait_params(
    ty_toks: list[Token],
    wrapper_structs: set[str],
    self_type: str | None,
    ctx: GamePathContext | None = None,
    lex: LexicalState | None = None,
) -> dict[int, str]:
    """Extract `{closure_param_idx: kind}` from `FnOnce(...)` / `FnMut(...)` / `Fn(...)` / `fn(...)`."""
    found: dict[int, str] = {}
    m = len(ty_toks)
    for idx, tok in enumerate(ty_toks):
        if tok[0] == "IDENT" and tok[1] in ("FnOnce", "FnMut", "Fn", "fn") and idx + 1 < m and ty_toks[idx + 1][1] == "(":
            cend = skip_balanced(ty_toks, idx + 1, "(", ")")
            cargs = split_top_level(ty_toks[idx + 2 : cend - 1], ",")
            for pidx, arg in enumerate(cargs):
                kind = resolve_type_kind(arg, wrapper_structs, self_type, ctx, lex)
                if kind:
                    found[pidx] = kind
    return found


def scan_fn_signatures(
    tokens: list[Token],
    wrapper_structs: set[str],
    ctx: GamePathContext | None = None,
    layout: FileLexicalLayout | None = None,
    base_mod_path: tuple[str, ...] = (),
):
    """Yield `(fname, mod_path, is_pub, impl_target, has_self, non_self_param_count, ret_positions, arg_closure_map)`."""
    n = len(tokens)
    rel_posix = ctx.rel_posix if ctx is not None else ""
    eff_layout = layout if layout is not None else build_file_lexical_layout(tokens, rel_posix)

    impl_opens: dict[int, tuple[str | None, set[str]]] = {}
    scope_stack: list[ScopeDecls] = [eff_layout.scope_decls[-1]]
    mod_stack: list[tuple[int, str]] = []
    generic_stack: list[tuple[int, set[str]]] = []
    impl_stack: list[tuple[int, str | None]] = []
    brace_depth = 0

    i = 0
    while i < n:
        tv = tokens[i][1]
        if tv == "{":
            brace_depth += 1
            scope_stack.append(eff_layout.scope_decls.get(i, ScopeDecls()))
            if i in eff_layout.mod_braces:
                mod_stack.append((brace_depth, eff_layout.mod_braces[i]))
            if i in impl_opens:
                itarget, igen = impl_opens[i]
                impl_stack.append((brace_depth, itarget))
                if igen:
                    generic_stack.append((brace_depth, igen))
            i += 1
            continue
        if tv == "}":
            while impl_stack and impl_stack[-1][0] >= brace_depth:
                impl_stack.pop()
            while generic_stack and generic_stack[-1][0] >= brace_depth:
                generic_stack.pop()
            while mod_stack and mod_stack[-1][0] >= brace_depth:
                mod_stack.pop()
            if len(scope_stack) > 1:
                scope_stack.pop()
            brace_depth = max(0, brace_depth - 1)
            i += 1
            continue

        if tokens[i][0] == "IDENT" and tv == "impl":
            prev = tokens[i - 1][1] if i > 0 else None
            if prev in (None, ";", "{", "}", "]", "unsafe", "default"):
                lex = compute_lexical_state(scope_stack, generic_stack, None, rel_posix, bool(mod_stack))
                parsed = parse_impl_target(tokens, i, ctx, lex)
                if parsed is not None:
                    target, brace_idx, impl_generics = parsed
                    impl_opens[brace_idx] = (target, impl_generics)

        if tokens[i][0] == "IDENT" and tv == "fn" and i + 1 < n and tokens[i + 1][0] == "IDENT":
            prev = tokens[i - 1][1] if i > 0 else None
            if prev in (":", "->", "<", ",", "(", "dyn", "impl", "+"):
                i += 1
                continue
            fname = norm_ident(tokens[i + 1][1])
            is_pub = is_pub_before_fn(tokens, i)
            impl_target = impl_stack[-1][1] if impl_stack else None
            cur_mod_path = base_mod_path + tuple(m for _, m in mod_stack)

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

            fn_generics = extract_generic_type_params(gen_toks)
            lex = compute_lexical_state(scope_stack, generic_stack, fn_generics, rel_posix, bool(mod_stack))

            params = split_top_level(param_toks, ",")
            has_self = bool(params) and any(t[1] == "self" for t in params[0])
            non_self_params = params[1:] if has_self else params

            ret_positions = extract_game_return_positions(ret_toks, impl_target, ctx, lex) if ret_toks else set()

            # Build generic type parameter -> closure param types from `<...>` and `where ...`
            generic_bounds: dict[str, dict[int, str]] = {}
            for gpart in split_top_level(gen_toks, ","):
                colon = next((idx_p for idx_p, tk in enumerate(gpart) if tk[1] == ":"), None)
                if colon is not None and colon >= 1 and gpart[0][0] == "IDENT":
                    cparams = extract_fn_trait_params(gpart[colon + 1 :], wrapper_structs, impl_target, ctx, lex)
                    if cparams:
                        generic_bounds.setdefault(gpart[0][1], {}).update(cparams)
            for wpart in split_top_level(where_toks, ","):
                colon = next((idx_p for idx_p, tk in enumerate(wpart) if tk[1] == ":"), None)
                if colon is not None:
                    gname = extract_type_base_name(wpart[:colon])
                    if gname:
                        cparams = extract_fn_trait_params(wpart[colon + 1 :], wrapper_structs, impl_target, ctx, lex)
                        if cparams:
                            generic_bounds.setdefault(gname, {}).update(cparams)

            arg_closure_map: ClosureArgMap = {}
            for arg_idx, p_slice in enumerate(non_self_params):
                colon = next((idx_p for idx_p, tk in enumerate(p_slice) if tk[1] == ":"), None)
                if colon is None:
                    continue
                ty = p_slice[colon + 1 :]
                cparams = extract_fn_trait_params(ty, wrapper_structs, impl_target, ctx, lex)
                if not cparams:
                    gbase = extract_type_base_name(ty)
                    if gbase and gbase in generic_bounds:
                        cparams = dict(generic_bounds[gbase])
                if cparams:
                    arg_closure_map[arg_idx] = cparams

            yield fname, cur_mod_path, is_pub, impl_target, has_self, len(non_self_params), ret_positions, arg_closure_map
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
    index.file_mod_map = parse_game_rs_mod_map(ROOT)
    scan_paths: list[Path] = []
    seen_paths: set[Path] = set()
    root_resolved = ROOT.resolve()
    for p in list(DEFAULT_PATHS) + extra_files:
        full = p if p.is_absolute() else (ROOT / p)
        candidates: list[Path] = []
        if full.is_file() and full.suffix == ".rs":
            candidates.append(full.resolve())
        elif full.is_dir():
            candidates.extend(child.resolve() for child in sorted(full.rglob("*.rs")))
        for rp in candidates:
            if rp in seen_paths:
                continue
            try:
                rel_posix = rp.relative_to(root_resolved).as_posix()
            except ValueError:
                continue
            if not is_in_game_tree(rel_posix):
                continue
            seen_paths.add(rp)
            scan_paths.append(rp)

    tokenized: list[tuple[list[Token], GamePathContext, FileLexicalLayout, tuple[str, ...]]] = []
    for path in scan_paths:
        rel_posix = path.relative_to(root_resolved).as_posix()
        src = path.read_text(encoding="utf-8")
        if "Game" not in src:
            continue
        toks = tokenize_rust(src)
        use_imports = collect_use_imports(toks)
        ctx = build_game_path_context(toks, use_imports, rel_posix)
        layout = build_file_lexical_layout(toks, rel_posix)
        if layout.scope_decls[-1].declares_game or layout.scope_decls[-1].foreign_game_import:
            continue
        _, sfields, scfields = scan_structs(toks, ctx, layout)
        for sname, fset in sfields.items():
            index.struct_game_fields.setdefault(sname, set()).update(fset)
        for sname, cmap in scfields.items():
            index.struct_closure_fields.setdefault(sname, {}).update(cmap)
        base_mod = file_canonical_mod_path(rel_posix, index.file_mod_map)
        tokenized.append((toks, ctx, layout, base_mod))

    wrapper_structs = set(index.struct_game_fields)

    for toks, ctx, layout, base_mod in tokenized:
        for fname, mod_path, is_pub, impl_target, has_self, non_self_count, ret_positions, arg_closure_map in scan_fn_signatures(
            toks, wrapper_structs, ctx, layout, base_mod
        ):
            if has_self:
                if ret_positions and None in ret_positions and non_self_count == 0 and impl_target != "Game":
                    index.game_returning_methods.add(fname)
                    if impl_target and impl_target != AMBIGUOUS:
                        index.wrapper_game_returning_methods.setdefault(impl_target, set()).add(fname)
                if arg_closure_map:
                    if impl_target == "Game":
                        merge_closure_map(index.game_method_closure_params.setdefault(fname, {}), arg_closure_map)
                    elif impl_target and impl_target in wrapper_structs:
                        wmap = index.wrapper_method_closure_params.setdefault(impl_target, {})
                        merge_closure_map(wmap.setdefault(fname, {}), arg_closure_map)
            else:
                if impl_target == "Game":
                    if ret_positions:
                        index.game_assoc_returning_fns.setdefault(fname, set()).update(ret_positions)
                    continue
                if impl_target is not None:
                    continue
                full_path = mod_path + (fname,) if mod_path else ()
                if ret_positions:
                    target_ret = index.pub_game_returning_fns if is_pub else index.priv_game_returning_fns
                    target_ret.setdefault(fname, set()).update(ret_positions)
                    index.all_game_return_fn_names.add(fname)
                    if full_path:
                        index.indexed_fn_returns.setdefault(full_path, set()).update(ret_positions)
                else:
                    index.non_game_return_free_fns.add(fname)
                    if full_path:
                        index.indexed_non_game_fns.add(full_path)
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


def build_file_local_index(
    tokens: list[Token],
    global_wrapper_structs: set[str],
    ctx: GamePathContext,
    layout: FileLexicalLayout,
    base_mod_path: tuple[str, ...],
) -> FileLocalIndex:
    local = FileLocalIndex()
    local.defined_structs, local.struct_game_fields, local.struct_closure_fields = scan_structs(tokens, ctx, layout)
    wrapper_structs = (global_wrapper_structs - local.defined_structs) | set(local.struct_game_fields)

    for fname, mod_path, _is_pub, impl_target, has_self, _non_self_count, ret_positions, arg_closure_map in scan_fn_signatures(
        tokens, wrapper_structs, ctx, layout, base_mod_path
    ):
        if has_self:
            continue
        if impl_target == "Game":
            if ret_positions:
                local.game_assoc_returning_fns.setdefault(fname, set()).update(ret_positions)
            continue
        if impl_target is not None:
            continue
        local.defined_free_fns.add(fname)
        local.mod_defined_fns.setdefault(mod_path, set()).add(fname)
        full_path = mod_path + (fname,) if mod_path else ()
        if ret_positions:
            local.game_returning_fns.setdefault(fname, set()).update(ret_positions)
            local.mod_game_returning_fns.setdefault(mod_path, {}).setdefault(fname, set()).update(ret_positions)
            local.all_game_return_fn_names.add(fname)
            if full_path:
                local.indexed_fn_returns.setdefault(full_path, set()).update(ret_positions)
        elif full_path:
            local.indexed_non_game_fns.add(full_path)
        if arg_closure_map:
            merge_closure_map(local.fn_closure_params.setdefault(fname, {}), arg_closure_map)
            merge_closure_map(local.mod_fn_closure_params.setdefault(mod_path, {}).setdefault(fname, {}), arg_closure_map)
    return local


def parse_fn_typed_params(
    tokens: list[Token],
    fn_idx: int,
    wrapper_structs: set[str],
    self_type: str | None,
    ctx: GamePathContext,
    lex: LexicalState | None = None,
) -> tuple[dict[str, VarBinding], int, set[str]] | None:
    """Given index of `fn`, return `(dict[param_name -> (kind, origin)], open_brace_idx, fn_generics)`."""
    n = len(tokens)
    j = fn_idx + 1
    if j >= n or tokens[j][0] != "IDENT":
        return None
    j += 1
    fn_generics: set[str] = set()
    if j < n and tokens[j][1] == "<":
        gend = skip_angle_generics(tokens, j)
        fn_generics = extract_generic_type_params(tokens[j + 1 : gend - 1])
        j = gend
    if j >= n or tokens[j][1] != "(":
        return None
    eff_lex = lex
    if lex is not None and fn_generics:
        eff_lex = LexicalState(
            enclosing_declares_game=lex.enclosing_declares_game,
            foreign_game_import=lex.foreign_game_import,
            sunrise_game_import=lex.sunrise_game_import,
            sunrise_game_aliases=lex.sunrise_game_aliases,
            generic_params=lex.generic_params | fn_generics,
            at_game_rs_root=lex.at_game_rs_root,
        )
    params_end = skip_balanced(tokens, j, "(", ")")
    typed_params: dict[str, VarBinding] = {}
    for p_slice in split_top_level(tokens[j + 1 : params_end - 1], ","):
        colon = find_top_level_token(p_slice, ":")
        if colon is None:
            continue
        pat = p_slice[:colon]
        ty = p_slice[colon + 1 :]
        simple_name = parse_simple_pat_ident(pat)
        if simple_name and simple_name != "self":
            kind, detail = classify_type_tokens(ty, wrapper_structs, self_type, ctx, eff_lex)
            if kind == "Game":
                typed_params[simple_name] = ("Game", "explicit-param")
            elif kind == AMBIGUOUS:
                typed_params[simple_name] = (AMBIGUOUS, detail)
            elif kind is not None:
                typed_params[simple_name] = (kind, "wrapper-param")
            else:
                typed_params[simple_name] = (NON_GAME, "")
        else:
            # Destructured parameters shadow any outer tracked names.
            definite, uncertain = extract_pattern_bound_names(pat)
            for bname in definite | uncertain:
                typed_params[bname] = (NON_GAME, "")
    m = params_end
    while m < n:
        tv = tokens[m][1]
        if tv == "{":
            return typed_params, m, fn_generics
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
        return norm_ident(toks[0][1])
    if len(toks) == 2 and toks[0][1] == "mut" and toks[1][0] in ("IDENT", "MACRO_VAR") and toks[1][1] != "_":
        return norm_ident(toks[1][1])
    if len(toks) == 2 and toks[0][1] == "ref" and toks[1][0] in ("IDENT", "MACRO_VAR") and toks[1][1] != "_":
        return norm_ident(toks[1][1])
    if (
        len(toks) == 3
        and toks[0][1] == "ref"
        and toks[1][1] == "mut"
        and toks[2][0] in ("IDENT", "MACRO_VAR")
        and toks[2][1] != "_"
    ):
        return norm_ident(toks[2][1])
    return None


def find_header_body_brace(
    tokens: list[Token],
    start_idx: int,
    record_amp: bool = False,
) -> tuple[int | None, int | None]:
    """Find the body `{` after a `match`/`for`/`if let`/`while let` header expression."""
    n = len(tokens)
    k = start_idx
    dp = db = 0
    first_amp: int | None = None
    while k < n:
        tv = tokens[k][1]
        if tv == "(":
            dp += 1
            k += 1
            continue
        if tv == ")":
            if dp == 0:
                return None, first_amp
            dp -= 1
            k += 1
            continue
        if tv == "[":
            db += 1
            k += 1
            continue
        if tv == "]":
            if db == 0:
                return None, first_amp
            db -= 1
            k += 1
            continue
        if dp > 0 or db > 0:
            k += 1
            continue
        if tv == ";":
            return None, first_amp
        if record_amp and tv == "&&" and first_amp is None:
            first_amp = k
            k += 1
            continue
        if tokens[k][0] == "IDENT" and tv == "let" and (k == start_idx or tokens[k - 1][1] == "&&"):
            # Skip `let <pat> =` in a let-chain so struct patterns `{ ... }` are not treated as the body brace.
            k += 1
            pdp = pdb = pdc = 0
            while k < n:
                ptv = tokens[k][1]
                if ptv == "(":
                    pdp += 1
                elif ptv == ")":
                    pdp = max(0, pdp - 1)
                elif ptv == "[":
                    pdb += 1
                elif ptv == "]":
                    pdb = max(0, pdb - 1)
                elif ptv == "{":
                    pdc += 1
                elif ptv == "}":
                    pdc = max(0, pdc - 1)
                elif ptv in ("=", ";") and pdp == 0 and pdb == 0 and pdc == 0:
                    k += 1
                    break
                k += 1
            continue
        if tokens[k][0] == "IDENT" and tv in ("if", "while", "match", "for", "loop"):
            prev = tokens[k - 1][1] if k > 0 else None
            if prev not in (".", "::"):
                if tv == "loop":
                    if k + 1 < n and tokens[k + 1][1] == "{":
                        k = skip_balanced(tokens, k + 1, "{", "}")
                        continue
                elif tv == "for":
                    q = k + 1
                    qdp = qdb = qdc = 0
                    while q < n:
                        qtv = tokens[q][1]
                        if qtv == "(":
                            qdp += 1
                        elif qtv == ")":
                            qdp = max(0, qdp - 1)
                        elif qtv == "[":
                            qdb += 1
                        elif qtv == "]":
                            qdb = max(0, qdb - 1)
                        elif qtv == "{":
                            qdc += 1
                        elif qtv == "}":
                            qdc = max(0, qdc - 1)
                        elif qtv == "in" and qdp == 0 and qdb == 0 and qdc == 0:
                            q += 1
                            break
                        q += 1
                    nested_brace, _ = find_header_body_brace(tokens, q, False)
                    if nested_brace is None:
                        return None, first_amp
                    k = skip_balanced(tokens, nested_brace, "{", "}")
                    continue
                elif tv == "if":
                    nested_brace, _ = find_header_body_brace(tokens, k + 1, False)
                    if nested_brace is None:
                        return None, first_amp
                    k = skip_balanced(tokens, nested_brace, "{", "}")
                    while k < n and tokens[k][0] == "IDENT" and tokens[k][1] == "else":
                        if k + 1 < n and tokens[k + 1][1] == "{":
                            k = skip_balanced(tokens, k + 1, "{", "}")
                        elif k + 1 < n and tokens[k + 1][0] == "IDENT" and tokens[k + 1][1] == "if":
                            nested_brace, _ = find_header_body_brace(tokens, k + 2, False)
                            if nested_brace is None:
                                return None, first_amp
                            k = skip_balanced(tokens, nested_brace, "{", "}")
                        else:
                            break
                    continue
                else:
                    nested_brace, _ = find_header_body_brace(tokens, k + 1, False)
                    if nested_brace is None:
                        return None, first_amp
                    k = skip_balanced(tokens, nested_brace, "{", "}")
                    continue
        if tv == "{":
            is_block_expr = False
            if k == start_idx:
                is_block_expr = True
            else:
                prev = tokens[k - 1][1]
                if prev in BLOCK_EXPR_PREV_TOKENS:
                    is_block_expr = True
                elif prev == ":" and k >= 2 and tokens[k - 2][0] == "LIFETIME":
                    is_block_expr = True
                elif prev in ("..", "..=", ">"):
                    after_blk = skip_balanced(tokens, k, "{", "}")
                    if after_blk < n and tokens[after_blk][1] in (
                        "{",
                        ".",
                        "?",
                        "&&",
                        "||",
                        "+",
                        "-",
                        "*",
                        "/",
                        "%",
                        "==",
                        "!=",
                        "<",
                        ">",
                        "<=",
                        ">=",
                        "as",
                    ):
                        is_block_expr = True
            if is_block_expr:
                k = skip_balanced(tokens, k, "{", "}")
                continue
            return k, first_amp
        k += 1
    return None, first_amp


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
    file_root_cache: dict[Path, frozenset[str]] | None = None,
    wrappers: dict[str, str] | None = None,
) -> tuple[list[Edit], list[AmbiguousSite]]:
    """Scan one Rust file and return `(sorted_edits, sorted_ambiguous_sites)`."""
    active_wrappers = wrappers or {}
    tokens = tokenize_rust(src)
    use_imports = collect_use_imports(tokens)
    ctx = build_game_path_context(tokens, use_imports, rel_posix)
    layout = build_file_lexical_layout(tokens, rel_posix, ROOT, file_root_cache)
    shadowed_macros = collect_shadowed_macros(tokens, use_imports, ctx.in_game_tree, rel_posix)
    effective_index = index if ctx.in_game_tree else WorkspaceIndex()
    base_mod_path = file_canonical_mod_path(rel_posix, effective_index.file_mod_map)

    local = build_file_local_index(tokens, set(effective_index.struct_game_fields), ctx, layout, base_mod_path)
    struct_game_fields: dict[str, set[str]] = dict(effective_index.struct_game_fields)
    struct_closure_fields: dict[str, dict[str, dict[int, str]]] = {
        k: dict(v) for k, v in effective_index.struct_closure_fields.items()
    }
    for sname in local.defined_structs:
        if sname in local.struct_game_fields:
            struct_game_fields[sname] = local.struct_game_fields[sname]
        else:
            struct_game_fields.pop(sname, None)
        if sname in local.struct_closure_fields:
            struct_closure_fields[sname] = dict(local.struct_closure_fields[sname])
        else:
            struct_closure_fields.pop(sname, None)
    wrapper_structs = set(struct_game_fields) if ctx.in_game_tree else set()

    def lookup_free_fn_returns(fname: str, cur_mod_path: tuple[str, ...]) -> tuple[set[int | None] | None, bool]:
        if not ctx.in_game_tree:
            return None, False
        if fname in local.mod_defined_fns.get(cur_mod_path, ()):
            return local.mod_game_returning_fns.get(cur_mod_path, {}).get(fname), False
        # Check parent inline module when inside a nested `mod tests` using `use super::*`.
        if len(cur_mod_path) > len(base_mod_path):
            parent_mod = cur_mod_path[:-1]
            if fname in local.mod_defined_fns.get(parent_mod, ()):
                return local.mod_game_returning_fns.get(parent_mod, {}).get(fname), False
        if fname in effective_index.ambiguous_return_fns and fname not in effective_index.pub_game_returning_fns:
            return None, True
        res = effective_index.pub_game_returning_fns.get(fname) or effective_index.priv_game_returning_fns.get(fname)
        return res, False

    def lookup_free_fn_closures(fname: str, cur_mod_path: tuple[str, ...]) -> tuple[ClosureArgMap | None, bool]:
        if not ctx.in_game_tree:
            return None, False
        if fname in local.mod_defined_fns.get(cur_mod_path, ()):
            return local.mod_fn_closure_params.get(cur_mod_path, {}).get(fname), False
        if len(cur_mod_path) > len(base_mod_path):
            parent_mod = cur_mod_path[:-1]
            if fname in local.mod_defined_fns.get(parent_mod, ()):
                return local.mod_fn_closure_params.get(parent_mod, {}).get(fname), False
        if fname in local.defined_free_fns:
            return local.fn_closure_params.get(fname), False
        if fname in effective_index.ambiguous_closure_fns and fname not in effective_index.pub_fn_closure_params:
            return None, True
        res = effective_index.pub_fn_closure_params.get(fname) or effective_index.priv_fn_closure_params.get(fname)
        return res, False

    line_starts = compute_line_starts(src)
    n = len(tokens)
    edits: dict[tuple[int, int], Edit] = {}
    ambiguous: dict[int, AmbiguousSite] = {}

    impl_opens: dict[int, str | None] = {}
    impl_header_ranges: list[tuple[int, int]] = []
    fn_opens: dict[int, dict[str, VarBinding]] = {}
    fn_item_braces: set[int] = set()
    generic_opens: dict[int, set[str]] = {}
    macro_opens: dict[int, set[str]] = {}

    # Pre-scan `impl`, `trait`, `fn`, and `macro_rules!` blocks with lexical module and generic scopes.
    pre_depth = 0
    pre_scopes: list[ScopeDecls] = [layout.scope_decls[-1]]
    pre_mods: list[tuple[int, str]] = []
    pre_impl: list[tuple[int, str | None]] = []
    pre_generics: list[tuple[int, set[str]]] = []
    for i in range(n):
        tv = tokens[i][1]
        if tv == "{":
            pre_depth += 1
            pre_scopes.append(layout.scope_decls.get(i, ScopeDecls()))
            if i in layout.mod_braces:
                pre_mods.append((pre_depth, layout.mod_braces[i]))
            if i in impl_opens:
                pre_impl.append((pre_depth, impl_opens[i]))
            if i in generic_opens:
                pre_generics.append((pre_depth, generic_opens[i]))
            continue
        if tv == "}":
            while pre_impl and pre_impl[-1][0] >= pre_depth:
                pre_impl.pop()
            while pre_generics and pre_generics[-1][0] >= pre_depth:
                pre_generics.pop()
            while pre_mods and pre_mods[-1][0] >= pre_depth:
                pre_mods.pop()
            if len(pre_scopes) > 1:
                pre_scopes.pop()
            pre_depth = max(0, pre_depth - 1)
            continue
        if tokens[i][0] != "IDENT":
            continue
        if tv == "impl":
            prev = tokens[i - 1][1] if i > 0 else None
            if prev in (None, ";", "{", "}", "]", "unsafe", "default"):
                lex = compute_lexical_state(pre_scopes, pre_generics, None, rel_posix, bool(pre_mods))
                parsed_impl = parse_impl_target(tokens, i, ctx, lex)
                if parsed_impl is not None:
                    target, brace_idx, impl_gens = parsed_impl
                    impl_opens[brace_idx] = target
                    impl_header_ranges.append((i, brace_idx))
                    if impl_gens:
                        generic_opens.setdefault(brace_idx, set()).update(impl_gens)
        elif tv == "trait" and i + 2 < n and tokens[i + 1][0] == "IDENT":
            prev = tokens[i - 1][1] if i > 0 else None
            if prev not in (".", "::"):
                tj = i + 2
                if tj < n and tokens[tj][1] == "<":
                    tgend = skip_angle_generics(tokens, tj)
                    tgens = extract_generic_type_params(tokens[tj + 1 : tgend - 1])
                    if tgens:
                        tk = tgend
                        while tk < n and tokens[tk][1] not in ("{", ";"):
                            tk += 1
                        if tk < n and tokens[tk][1] == "{":
                            generic_opens.setdefault(tk, set()).update(tgens)
        elif tv == "macro_rules" and i + 3 < n and tokens[i + 1][1] == "!" and tokens[i + 2][0] == "IDENT":
            mname = norm_ident(tokens[i + 2][1])
            if ctx.in_game_tree and mname in GAME_MACROS and tokens[i + 3][1] == "{":
                macro_opens[i + 3] = GAME_MACROS[mname]
        elif tv == "fn":
            prev = tokens[i - 1][1] if i > 0 else None
            if prev not in (":", "->", "<", ",", "(", "dyn", "impl", "+"):
                cur_self = pre_impl[-1][1] if pre_impl else None
                lex = compute_lexical_state(pre_scopes, pre_generics, None, rel_posix, bool(pre_mods))
                parsed_fn = parse_fn_typed_params(tokens, i, wrapper_structs, cur_self, ctx, lex)
                if parsed_fn is not None:
                    tparams, brace_idx, fn_gens = parsed_fn
                    fn_item_braces.add(brace_idx)
                    if tparams:
                        fn_opens.setdefault(brace_idx, {}).update(tparams)
                    if fn_gens:
                        generic_opens.setdefault(brace_idx, set()).update(fn_gens)

    def in_impl_header(idx: int) -> bool:
        return any(s <= idx < e for s, e in impl_header_ranges)

    brace_depth = 0
    scope_stack: list[ScopeDecls] = [layout.scope_decls[-1]]
    mod_stack: list[tuple[int, str]] = []
    generic_stack: list[tuple[int, set[str]]] = []
    impl_stack: list[tuple[int, str | None]] = []
    # Scopes map normalized var name -> `(kind, origin)`; `is_fn_barrier` blocks outer local capture.
    var_scopes: list[tuple[int, dict[str, VarBinding], bool]] = [(0, {}, False)]
    expr_closure_scopes: list[tuple[int, int, int, dict[str, VarBinding]]] = []
    # Range-scoped bindings for `match` arm guards/bodies, `matches!` guards, and `if let ... && ...` chains.
    range_scopes: list[tuple[int, dict[str, VarBinding]]] = []
    # Scheduled range scope activations keyed by token index: `start_idx -> list[(end_idx, vars)]`
    scheduled_ranges: dict[int, list[tuple[int, dict[str, VarBinding]]]] = {}
    # Active `match` guard intervals `(guard_start, arrow_idx, arm_end)` for `if let` inside match guards.
    match_guard_arms: list[tuple[int, int, int]] = []
    # Deferred `let` statement bindings applied at the statement's terminating `;`
    deferred_let_bindings: dict[int, dict[str, VarBinding]] = {}
    # Token index ranges inside `match` / `matches!` patterns so `|` is not parsed as a closure.
    match_pat_ranges: list[tuple[int, int]] = []
    macro_stack: list[tuple[int, set[str]]] = []
    # Active unknown macro invocations `(end_idx, macro_name)`
    unknown_macro_stack: list[tuple[int, str]] = []

    paren_depth = 0
    bracket_depth = 0

    def active_impl_target() -> str | None:
        return impl_stack[-1][1] if impl_stack else None

    def current_mod_path() -> tuple[str, ...]:
        return base_mod_path + tuple(m for _, m in mod_stack)

    def current_lexical_state() -> LexicalState:
        return compute_lexical_state(scope_stack, generic_stack, None, rel_posix, bool(mod_stack))

    def active_shadowed_macro_roots() -> set[str]:
        out = set(layout.enclosing_file_shadowed_roots)
        for s in scope_stack:
            out.update(s.shadowed_macro_roots)
        return out

    def lookup_imported_root(name: str) -> list[str] | None:
        for s in reversed(scope_stack):
            if name in s.imported_roots:
                return s.imported_roots[name]
        return None

    def var_info(name: str) -> tuple[str | None, str]:
        key = norm_ident(name)
        for _, rvars in reversed(range_scopes):
            if key in rvars:
                k, orig = rvars[key]
                return (k or None), orig
        for _, _, _, cvars in reversed(expr_closure_scopes):
            if key in cvars:
                k, orig = cvars[key]
                return (k or None), orig
        for _, scope, is_barrier in reversed(var_scopes):
            if key in scope:
                k, orig = scope[key]
                return (k or None), orig
            if is_barrier:
                break
        return None, ""

    def var_type(name: str) -> str | None:
        return var_info(name)[0]

    def is_game_macro_var(name: str) -> bool:
        return any(name in mvars for _, mvars in macro_stack)

    def in_match_pattern(idx: int) -> bool:
        return any(s <= idx < e for s, e in match_pat_ranges)

    def record_access(dot_idx: int, recv_kind: str, reason: str) -> None:
        """Check `.field` or `.server.field` / `.client.field` after a Game or ambiguous receiver."""
        if dot_idx + 1 >= n or tokens[dot_idx + 1][0] != "IDENT":
            return
        first_idx = dot_idx + 1
        first_name = norm_ident(tokens[first_idx][1])

        # Case 1: already tail-qualified `<recv>.server.<field>` or `<recv>.client.<field>`
        if (
            first_name in ("server", "client")
            and first_idx + 2 < n
            and tokens[first_idx + 1][1] == "."
            and tokens[first_idx + 2][0] == "IDENT"
        ):
            field_idx = first_idx + 2
            fname = norm_ident(tokens[field_idx][1])
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
            if not ctx.in_game_tree:
                off = tokens[first_idx][2]
                ambiguous.setdefault(
                    off,
                    AmbiguousSite(off, fname, f"outside crates/sunrise-client/src/game* ({reason})"),
                )
                return
            if expected == "game":
                already_dot0 = (
                    field_idx + 2 < n
                    and tokens[field_idx + 1][1] == "."
                    and tokens[field_idx + 2][0] == "LIT"
                    and tokens[field_idx + 2][1] == "0"
                )
                if fname in active_wrappers and not already_dot0:
                    span = (tokens[first_idx][2], tokens[field_idx][3])
                    edits.setdefault(
                        span,
                        Edit(
                            span[0],
                            span[1],
                            f"{fname}.0",
                            fname,
                            f"{reason}:unmove-wrap",
                            f"{first_name}.{fname}",
                            f"{fname}.0",
                        ),
                    )
                else:
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
        is_wrapped = expected == "game" and first_name in active_wrappers
        if expected not in ("server", "client") and not is_wrapped:
            return
        nxt = tokens[first_idx + 1][1] if first_idx + 1 < n else None
        if nxt in ("(", "::"):
            return
        if is_wrapped:
            if (
                nxt == "."
                and first_idx + 2 < n
                and tokens[first_idx + 2][0] == "LIT"
                and tokens[first_idx + 2][1] == "0"
            ):
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
        if not ctx.in_game_tree:
            off = tokens[first_idx][2]
            ambiguous.setdefault(
                off,
                AmbiguousSite(off, first_name, f"outside crates/sunrise-client/src/game* ({reason})"),
            )
            return
        if is_wrapped:
            off = tokens[first_idx][3]
            span = (off, off)
            edits.setdefault(
                span,
                Edit(off, off, ".0", first_name, f"{reason}:wrap", first_name, f"{first_name}.0"),
            )
        else:
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
            if t == AMBIGUOUS:
                return AMBIGUOUS, "unresolved `impl Game` target"
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
                if t == AMBIGUOUS:
                    return AMBIGUOUS, "unresolved `impl Game` target"
                if t and t in wrapper_structs:
                    return t, None
                return None, None
            if inner_tok[0] in ("IDENT", "MACRO_VAR"):
                vt, orig = var_info(inner_tok[1])
                if vt == "Game":
                    return "Game", f"deref-{orig or 'typed-game-var'}"
                if vt == AMBIGUOUS:
                    return AMBIGUOUS, orig or f"ambiguous dereferenced variable `{inner_tok[1]}`"
                if vt:
                    return vt, None
                return None, None

        # `self.<game_method>().<...>`
        if (
            ctx.in_game_tree
            and prev_text == ")"
            and dot_idx >= 5
            and tokens[dot_idx - 2][1] == "("
            and tokens[dot_idx - 3][0] == "IDENT"
            and tokens[dot_idx - 4][1] == "."
            and tokens[dot_idx - 5][1] == "self"
        ):
            mname = norm_ident(tokens[dot_idx - 3][1])
            impl_t = active_impl_target()
            if impl_t and mname in effective_index.wrapper_game_returning_methods.get(impl_t, ()):
                return "Game", "wrapper-self-method"
            if impl_t is None and mname in effective_index.game_returning_methods:
                return AMBIGUOUS, f"unresolved self method `{mname}()`"
            return None, None

        # `self.<game_field>.<...>` or `self.0.<...>`
        if (
            ctx.in_game_tree
            and prev_kind in ("IDENT", "LIT")
            and prev2 == "."
            and dot_idx >= 3
            and tokens[dot_idx - 3][1] == "self"
            and (dot_idx < 4 or tokens[dot_idx - 4][1] not in (".", "::"))
        ):
            impl_t = active_impl_target()
            if impl_t and norm_ident(prev_text) in struct_game_fields.get(impl_t, ()):
                return "Game", "wrapper-self-field"
            return None, None

        # `<wrapper_var>.<game_field>.<...>`
        if (
            ctx.in_game_tree
            and prev_kind in ("IDENT", "LIT")
            and prev2 == "."
            and dot_idx >= 3
            and tokens[dot_idx - 3][0] in ("IDENT", "MACRO_VAR")
            and tokens[dot_idx - 3][1] != "self"
            and (dot_idx < 4 or tokens[dot_idx - 4][1] not in (".", "::"))
        ):
            wt = var_type(tokens[dot_idx - 3][1])
            if wt and wt not in ("Game", AMBIGUOUS) and norm_ident(prev_text) in struct_game_fields.get(wt, ()):
                return "Game", "wrapper-var-field"
            return None, None

        # `<game_var>.<...>` or `<wrapper_var>.<...>` or ambiguous var
        if prev_kind in ("IDENT", "MACRO_VAR") and prev_text != "self" and prev2 not in (".", "::"):
            vt, orig = var_info(prev_text)
            if vt == "Game":
                return "Game", orig or "typed-game-var"
            if vt == AMBIGUOUS:
                return AMBIGUOUS, orig or f"ambiguous variable `{prev_text}`"
            if vt:
                return vt, None
            if prev_kind == "MACRO_VAR" and is_game_macro_var(prev_text):
                return "Game", "game-macro"

        return None, None

    def find_block_tail_start(brace_idx: int, blk_end: int) -> int | None:
        last_semi: int | None = None
        dp = db = dc = 0
        for p in range(brace_idx + 1, blk_end - 1):
            pv = tokens[p][1]
            if pv == "(":
                dp += 1
            elif pv == ")":
                dp = max(0, dp - 1)
            elif pv == "[":
                db += 1
            elif pv == "]":
                db = max(0, db - 1)
            elif pv == "{":
                dc += 1
            elif pv == "}":
                dc = max(0, dc - 1)
            elif pv == ";" and dp == 0 and db == 0 and dc == 0:
                last_semi = p
        tail = (last_semi + 1) if last_semi is not None else (brace_idx + 1)
        return tail if tail < blk_end - 1 else None

    def classify_let_rhs(rhs_idx: int, depth: int = 0) -> tuple[str | None, dict[int, str], bool, str]:
        """Classify a `let` RHS starting at `rhs_idx` as `(whole, tuple_types, is_ambig, origin)`."""
        if depth > 4:
            return None, {}, False, ""
        r = rhs_idx
        while r < n and tokens[r][1] in ("&", "mut", "*"):
            r += 1
        if r >= n:
            return None, {}, False, ""
        tk, tv, _, _ = tokens[r]

        # Compound block / parenthesized expressions (`let alias = { game };`, `let alias = (game);`).
        if tv == "{":
            blk_end = skip_balanced(tokens, r, "{", "}")
            if blk_end >= n or tokens[blk_end][1] in (";", ",", ")", "}", "else"):
                tail = find_block_tail_start(r, blk_end)
                if tail is not None:
                    w, tup, amb, _ = classify_let_rhs(tail, depth + 1)
                    if w == "Game" or tup or amb:
                        return None, {}, True, "unresolved block expression may alias `Game`"
            return None, {}, False, ""
        if tv == "(":
            p_end = skip_balanced(tokens, r, "(", ")")
            if p_end >= n or tokens[p_end][1] in (";", ",", ")", "}", "else"):
                inner = tokens[r + 1 : p_end - 1]
                if inner and find_top_level_token(inner, ",") is None:
                    w, tup, amb, _ = classify_let_rhs(r + 1, depth + 1)
                    if w == "Game" or tup or amb:
                        return None, {}, True, "unresolved parenthesized expression may alias `Game`"
            return None, {}, False, ""
        if tk == "IDENT" and tv in ("unsafe", "async", "const", "try") and r + 1 < n and tokens[r + 1][1] == "{":
            return classify_let_rhs(r + 1, depth + 1)
        if tk != "IDENT":
            return None, {}, False, ""

        # Case A: `self` or `self.<game_field>` or `self.<game_method>()`
        if tv == "self" and (r + 1 >= n or tokens[r + 1][1] != "::"):
            if active_impl_target() == "Game" and (r + 1 >= n or tokens[r + 1][1] in (";", ",", ")", "}", "else")):
                return "Game", {}, False, "let-from-self"
            if active_impl_target() == AMBIGUOUS and (r + 1 >= n or tokens[r + 1][1] in (";", ",", ")", "}", "else")):
                return None, {}, True, "unresolved `impl Game` target"
            if ctx.in_game_tree and r + 2 < n and tokens[r + 1][1] == "." and tokens[r + 2][0] in ("IDENT", "LIT"):
                impl_t = active_impl_target()
                if impl_t and norm_ident(tokens[r + 2][1]) in struct_game_fields.get(impl_t, ()):
                    after = r + 3
                    if after >= n or tokens[after][1] in (";", ",", ")", "}", "else"):
                        return "Game", {}, False, "let-from-wrapper-field"
                if (
                    impl_t
                    and norm_ident(tokens[r + 2][1]) in effective_index.wrapper_game_returning_methods.get(impl_t, ())
                    and r + 3 < n
                    and tokens[r + 3][1] == "("
                ):
                    if rhs_preserves_game_after_call(tokens, r + 3):
                        return "Game", {}, False, "let-from-wrapper-method"
            return None, {}, False, ""

        # Case B: `<var>` or `<wrapper_var>.<game_field>`
        vt, vorig = var_info(tv)
        if vt == "Game":
            if r + 1 >= n or tokens[r + 1][1] in (";", ",", ")", "}", "else"):
                return "Game", {}, False, "let-from-game-var"
        elif vt == AMBIGUOUS:
            if r + 1 >= n or tokens[r + 1][1] in (";", ",", ")", "}", "else"):
                return None, {}, True, vorig
        elif ctx.in_game_tree and vt and vt in struct_game_fields:
            if r + 2 < n and tokens[r + 1][1] == "." and norm_ident(tokens[r + 2][1]) in struct_game_fields[vt]:
                after = r + 3
                if after >= n or tokens[after][1] in (";", ",", ")", "}", "else"):
                    return "Game", {}, False, "let-from-wrapper-field"

        # Case C: `<wrapper_struct> { ... }`
        norm_tv = norm_ident(tv)
        if ctx.in_game_tree and norm_tv in wrapper_structs and r + 1 < n and tokens[r + 1][1] == "{":
            return norm_tv, {}, False, "wrapper-ctor"

        # Case D: path call `a::b::func(...)` or `Game::new(...)` or `Game { ... }`
        q = r
        segments = [norm_tv]
        while q + 2 < n and tokens[q + 1][1] == "::" and tokens[q + 2][0] == "IDENT":
            q += 2
            segments.append(norm_ident(tokens[q][1]))
        q_call = q + 1
        if q_call + 1 < n and tokens[q_call][1] == "::" and tokens[q_call + 1][1] == "<":
            q_call = skip_angle_generics(tokens, q_call + 1)

        lex = current_lexical_state()
        last_seg = segments[-1]
        seg_toks = tokens[r : q + 1]
        if q_call < n and tokens[q_call][1] == "{":
            ckind, cdet = classify_type_tokens(seg_toks, wrapper_structs, active_impl_target(), ctx, lex)
            if ckind == "Game":
                return "Game", {}, False, "game-ctor"
            if ckind == AMBIGUOUS:
                return None, {}, True, cdet
            if ctx.in_game_tree and last_seg in wrapper_structs:
                return last_seg, {}, False, "wrapper-ctor"
            return None, {}, False, ""

        if q_call >= n or tokens[q_call][1] != "(" or not rhs_preserves_game_after_call(tokens, q_call):
            return None, {}, False, ""

        cur_mod = current_mod_path()

        # Unqualified call `func(...)`: resolve against current module, explicit imports, or workspace index.
        if len(segments) == 1:
            if not ctx.in_game_tree:
                return None, {}, False, ""
            if last_seg in local.mod_defined_fns.get(cur_mod, ()):
                pos_set = local.mod_game_returning_fns.get(cur_mod, {}).get(last_seg)
                if pos_set:
                    whole = "Game" if None in pos_set else None
                    tup = {p: "Game" for p in pos_set if isinstance(p, int)}
                    return whole, tup, False, "game-fn-return"
                return None, {}, False, ""
            imp = lookup_imported_root(last_seg)
            if imp is not None:
                if imp[0] in EXTERNAL_NON_GAME_CRATES or imp[0].startswith("sunrise_"):
                    return None, {}, False, ""
                if len(imp) >= 3 and imp[:2] == ["crate", "game"]:
                    imp_path = tuple(imp)
                    pos_set = local.indexed_fn_returns.get(imp_path) or effective_index.indexed_fn_returns.get(imp_path)
                    if pos_set:
                        whole = "Game" if None in pos_set else None
                        tup = {p: "Game" for p in pos_set if isinstance(p, int)}
                        return whole, tup, False, "game-fn-return"
                    if imp_path in local.indexed_non_game_fns or imp_path in effective_index.indexed_non_game_fns:
                        return None, {}, False, ""
            pos_set, is_ambig = lookup_free_fn_returns(last_seg, cur_mod)
            if is_ambig:
                return None, {}, True, ""
            if pos_set:
                whole = "Game" if None in pos_set else None
                tup = {p: "Game" for p in pos_set if isinstance(p, int)}
                return whole, tup, False, "game-fn-return"
            return None, {}, False, ""

        # Qualified call `a::b::func(...)`: check `Game::*`, `Self::*`, `Wrapper::*`, `self::*`, `crate::game::*`.
        prefix_toks = tokens[r : q - 1]
        pkind, pdet = classify_type_tokens(prefix_toks, wrapper_structs, active_impl_target(), ctx, lex)
        if pkind == "Game":
            if last_seg in ("new", "construct") or last_seg.startswith(("new_", "test", "fixture")):
                return "Game", {}, False, "game-ctor"
            pos_set = local.game_assoc_returning_fns.get(last_seg) or effective_index.game_assoc_returning_fns.get(last_seg)
            if pos_set:
                whole = "Game" if None in pos_set else None
                tup = {p: "Game" for p in pos_set if isinstance(p, int)}
                return whole, tup, False, "game-fn-return"
            return None, {}, False, ""
        if pkind == AMBIGUOUS:
            return None, {}, True, pdet
        if ctx.in_game_tree and pkind and pkind in wrapper_structs:
            if last_seg in ("new", "of", "default"):
                return pkind, {}, False, "wrapper-ctor"
            return None, {}, False, ""

        if not ctx.in_game_tree:
            return None, {}, False, ""

        # `self::func(...)`: resolve strictly in the current lexical module.
        if segments[0] == "self" and len(segments) == 2:
            if last_seg in local.mod_defined_fns.get(cur_mod, ()):
                pos_set = local.mod_game_returning_fns.get(cur_mod, {}).get(last_seg)
                if pos_set:
                    whole = "Game" if None in pos_set else None
                    tup = {p: "Game" for p in pos_set if isinstance(p, int)}
                    return whole, tup, False, "game-fn-return"
                return None, {}, False, ""
            return None, {}, True, f"unresolved qualified call `{'::'.join(segments)}`"

        # Full `crate::game::...::func(...)`: resolve strictly against indexed canonical paths.
        if len(segments) >= 3 and segments[:2] == ["crate", "game"]:
            full_path = tuple(segments)
            pos_set = local.indexed_fn_returns.get(full_path) or effective_index.indexed_fn_returns.get(full_path)
            if pos_set:
                whole = "Game" if None in pos_set else None
                tup = {p: "Game" for p in pos_set if isinstance(p, int)}
                return whole, tup, False, "game-fn-return"
            if full_path in local.indexed_non_game_fns or full_path in effective_index.indexed_non_game_fns:
                return None, {}, False, ""
            return None, {}, True, f"unresolved qualified call `{'::'.join(segments)}`"

        # Audited real-tree relative qualified calls mapped explicitly to canonical indexed paths.
        qual_str = "::".join(segments)
        if (rel_posix, qual_str) in KNOWN_GOOD_QUALIFIED_FNS:
            canonical = KNOWN_GOOD_QUALIFIED_FNS[(rel_posix, qual_str)]
            pos_set = local.indexed_fn_returns.get(canonical) or effective_index.indexed_fn_returns.get(canonical)
            if pos_set:
                whole = "Game" if None in pos_set else None
                tup = {p: "Game" for p in pos_set if isinstance(p, int)}
                return whole, tup, False, "game-fn-return"

        # Any other qualified call: never infer `Game`; fail closed for unresolved module-qualified calls.
        if segments[-2] and segments[-2][0].isupper():
            return None, {}, False, ""
        if segments[0] in EXTERNAL_NON_GAME_CRATES or segments[0].startswith("sunrise_"):
            return None, {}, False, ""
        imp_root = lookup_imported_root(segments[0])
        if imp_root is not None and (imp_root[0] in EXTERNAL_NON_GAME_CRATES or imp_root[0].startswith("sunrise_")):
            return None, {}, False, ""
        return None, {}, True, f"unresolved qualified call `{qual_str}`"

    def build_pattern_bindings(
        pat_toks: list[Token],
        whole: str | None,
        tup_types: dict[int, str],
        is_ambig: bool = False,
        origin: str = "typed-game-var",
    ) -> dict[str, VarBinding]:
        """Compute `name -> (kind, origin)` for every variable bound by `pat_toks`."""
        bindings: dict[str, VarBinding] = {}
        ambig_origin = origin if (is_ambig and origin and origin != "explicit-let") else ""
        default_binding: VarBinding = (AMBIGUOUS, ambig_origin) if is_ambig else (NON_GAME, "")
        definite, uncertain_upper = extract_pattern_bound_names(pat_toks)
        # Every definite bound identifier shadows any outer tracked name by default.
        for bname in definite:
            bindings[bname] = default_binding
        # Unresolved UpperCamelCase pattern names colliding with a tracked outer var fail closed.
        for uname in uncertain_upper:
            if var_type(uname) is not None:
                bindings[uname] = (
                    AMBIGUOUS,
                    f"unresolved UpperCamelCase pattern `{uname}` may shadow outer binding",
                )

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
                    bindings[vname] = (tup_types[tidx], origin)
            return bindings
        vname = parse_simple_pat_ident(p)
        if vname and whole is not None:
            bindings[vname] = (whole, origin)
        return bindings

    def schedule_matches_macro_guard(open_idx: int, mac_end: int) -> bool:
        inner_start = open_idx + 1
        inner_end = mac_end - 1
        comma_rel = find_top_level_token(tokens[inner_start:inner_end], ",")
        if comma_rel is None:
            return False
        rest_start = inner_start + comma_rel + 1
        guard_rel = find_top_level_token(tokens[rest_start:inner_end], "if")
        if guard_rel is None:
            match_pat_ranges.append((rest_start, inner_end))
            return True
        guard_if_idx = rest_start + guard_rel
        match_pat_ranges.append((rest_start, guard_if_idx))
        pat_toks = tokens[rest_start:guard_if_idx]
        definite, uncertain = extract_pattern_bound_names(pat_toks)
        whole_s, tup_s, scrut_ambig, _ = classify_let_rhs(inner_start)
        scrut_may_be_game = whole_s is not None or bool(tup_s) or scrut_ambig
        guard_vars: dict[str, VarBinding] = {}
        for bname in definite | uncertain:
            if var_type(bname) is not None or scrut_may_be_game:
                guard_vars[bname] = (AMBIGUOUS, f"pattern binding `{bname}` in `matches!` guard")
            else:
                guard_vars[bname] = (NON_GAME, "")
        if guard_vars:
            scheduled_ranges.setdefault(guard_if_idx + 1, []).append((inner_end, guard_vars))
        return True

    def schedule_with_entity_macro_binding(open_idx: int, mac_end: int) -> None:
        inner_start = open_idx + 1
        inner_end = mac_end - 1
        commas: list[int] = []
        dp = db = dc = 0
        for p in range(inner_start, inner_end):
            pv = tokens[p][1]
            if pv == "(":
                dp += 1
            elif pv == ")":
                dp = max(0, dp - 1)
            elif pv == "[":
                db += 1
            elif pv == "]":
                db = max(0, db - 1)
            elif pv == "{":
                dc += 1
            elif pv == "}":
                dc = max(0, dc - 1)
            elif pv == "," and dp == 0 and db == 0 and dc == 0:
                commas.append(p)
        if len(commas) < 2:
            return
        bind_start = commas[1] + 1
        arr_rel = find_top_level_token(tokens[bind_start:inner_end], "=>")
        if arr_rel is None:
            return
        arr_idx = bind_start + arr_rel
        definite, uncertain = extract_pattern_bound_names(tokens[bind_start:arr_idx])
        body_vars: dict[str, VarBinding] = {}
        for bname in definite | uncertain:
            if var_type(bname) is not None:
                body_vars[bname] = (AMBIGUOUS, f"binding `{bname}` in `with_entity!` body")
            else:
                body_vars[bname] = (NON_GAME, "")
        if body_vars:
            scheduled_ranges.setdefault(arr_idx + 1, []).append((inner_end, body_vars))

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

        # Skip token-only macros and track unknown, qualified, shadowed, or pattern-binding macros.
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
            close_for = {"(": ")", "[": "]", "{": "}"}[open_ch]
            mac_end = skip_balanced(tokens, i + 2, open_ch, close_for)
            mname = norm_ident(text)
            qual_segs: list[str] = [mname]
            q = i
            while q >= 2 and tokens[q - 1][1] == "::" and tokens[q - 2][0] == "IDENT":
                q -= 2
                qual_segs.append(norm_ident(tokens[q][1]))
            qual_segs.reverse()
            has_leading_colons = q >= 1 and tokens[q - 1][1] == "::"
            shadowed_roots = active_shadowed_macro_roots()
            if len(qual_segs) >= 2 or has_leading_colons:
                qual_path = "::".join(qual_segs)
                root_seg = qual_segs[0] if len(qual_segs) >= 2 else ""
                trusted_std = (
                    len(qual_segs) == 2
                    and root_seg in TRUSTED_MACRO_CRATES
                    and root_seg not in shadowed_roots
                )
                audited_qual = (
                    (rel_posix, qual_path) in KNOWN_GOOD_QUALIFIED_MACROS
                    and root_seg not in shadowed_roots
                )
                if trusted_std and mname in STD_TOKEN_ONLY_MACROS:
                    i = mac_end
                    continue
                if trusted_std and mname in STD_EXPR_MACROS:
                    if mname == "matches" and not schedule_matches_macro_guard(i + 2, mac_end):
                        unknown_macro_stack.append((mac_end, qual_path))
                elif audited_qual:
                    if mname == "with_entity":
                        schedule_with_entity_macro_binding(i + 2, mac_end)
                else:
                    unknown_macro_stack.append((mac_end, qual_path))
            elif mname in shadowed_macros:
                unknown_macro_stack.append((mac_end, f"shadowed {mname}"))
            elif mname in TOKEN_ONLY_MACROS:
                i = mac_end
                continue
            elif mname not in EXPR_MACROS and (rel_posix, mname) not in KNOWN_GOOD_LOCAL_EXPR_MACROS:
                unknown_macro_stack.append((mac_end, mname))
            elif mname == "matches":
                if not schedule_matches_macro_guard(i + 2, mac_end):
                    unknown_macro_stack.append((mac_end, mname))
            elif mname == "with_entity":
                schedule_with_entity_macro_binding(i + 2, mac_end)

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
            scope_stack.append(layout.scope_decls.get(i, ScopeDecls()))
            if i in layout.mod_braces:
                mod_stack.append((brace_depth, layout.mod_braces[i]))
            if i in generic_opens:
                generic_stack.append((brace_depth, generic_opens[i]))
            scope_vars: dict[str, VarBinding] = {}
            is_fn_barrier = i in fn_item_braces or i in layout.mod_braces
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
            while generic_stack and generic_stack[-1][0] >= brace_depth:
                generic_stack.pop()
            while mod_stack and mod_stack[-1][0] >= brace_depth:
                mod_stack.pop()
            if len(scope_stack) > 1:
                scope_stack.pop()
            brace_depth = max(0, brace_depth - 1)
            i += 1
            continue

        # `match scrutinee { pat [if guard] => body, ... }`
        if kind == "IDENT" and text == "match" and (i == 0 or tokens[i - 1][1] not in (".", "::")):
            whole_scrut, tup_scrut, scrut_ambig, scrut_origin = classify_let_rhs(i + 1)
            match_brace, _ = find_header_body_brace(tokens, i + 1, False)
            if match_brace is not None:
                match_end = skip_balanced(tokens, match_brace, "{", "}")
                for pat_start, pat_end, guard_start, arrow_idx, arm_end, pat_toks in find_match_arms(
                    tokens, match_brace, match_end
                ):
                    match_pat_ranges.append((pat_start, pat_end))
                    if guard_start is not None:
                        match_guard_arms.append((guard_start, arrow_idx, arm_end))
                    arm_vars = build_pattern_bindings(
                        pat_toks,
                        whole_scrut,
                        tup_scrut,
                        scrut_ambig,
                        scrut_origin or "match-scrutinee",
                    )
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
                body_brace, _ = find_header_body_brace(tokens, in_idx + 1, False)
                if body_brace is not None:
                    for_vars = build_pattern_bindings(for_pat, None, {})
                    if for_vars:
                        fn_opens.setdefault(body_brace, {}).update(for_vars)

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
                elif tv in (";", ".") and da == 0 and dp == 0 and db == 0 and dc == 0:
                    break
                elif tv == "|" and da == 0 and dp == 0 and db == 0 and dc == 0:
                    close_bar = j
                    break
                j += 1
            if close_bar is not None:
                c_slice = tokens[i + 1 : close_bar]
                cvars: dict[str, VarBinding] = {}
                expected_params: dict[int, str] = {}
                call_is_ambig = False

                # Check struct literal field closure: `StructName { field: [move] |...| ... }`
                if ctx.in_game_tree and struct_closure_fields:
                    prev_c = i - 1
                    if prev_c >= 0 and tokens[prev_c][1] == "move":
                        prev_c -= 1
                    if prev_c >= 1 and tokens[prev_c][1] == ":" and tokens[prev_c - 1][0] == "IDENT":
                        lit_field = norm_ident(tokens[prev_c - 1][1])
                        # Walk back to enclosing `{`
                        sc = prev_c - 2
                        sdp = sdb = sdc = 0
                        while sc >= 0:
                            sv = tokens[sc][1]
                            if sv == "}":
                                sdc += 1
                            elif sv == "{":
                                if sdc == 0:
                                    if sc >= 1 and tokens[sc - 1][0] == "IDENT":
                                        lit_struct = norm_ident(tokens[sc - 1][1])
                                        if lit_field in struct_closure_fields.get(lit_struct, {}):
                                            expected_params = struct_closure_fields[lit_struct][lit_field]
                                    break
                                sdc -= 1
                            elif sv == ")":
                                sdp += 1
                            elif sv == "(":
                                sdp = max(0, sdp - 1)
                            elif sv == "]":
                                sdb += 1
                            elif sv == "[":
                                sdb = max(0, sdb - 1)
                            sc -= 1

                call_info = find_enclosing_call_and_arg(tokens, i) if (ctx.in_game_tree and not expected_params) else None
                if call_info is not None:
                    call_name, arg_idx, is_method, dot_idx = call_info
                    norm_call = norm_ident(call_name)
                    if is_method and dot_idx is not None:
                        recv_kind, _ = receiver_kind_before_dot(dot_idx)
                        if recv_kind == "Game":
                            expected_params = effective_index.game_method_closure_params.get(norm_call, {}).get(arg_idx, {})
                        elif recv_kind and recv_kind in wrapper_structs:
                            expected_params = (
                                effective_index.wrapper_method_closure_params.get(recv_kind, {})
                                .get(norm_call, {})
                                .get(arg_idx, {})
                            )
                    elif not is_method:
                        # Special-case `Mutate::new(|tick| ...)` where `new` is noisy globally.
                        if (
                            norm_call == "new"
                            and i >= 5
                            and tokens[i - 2][1] == "new"
                            and tokens[i - 3][1] == "::"
                            and tokens[i - 4][1] == "Mutate"
                        ):
                            expected_params = {0: "BlockEntityTick"}
                        else:
                            fmap, call_is_ambig = lookup_free_fn_closures(norm_call, current_mod_path())
                            if fmap:
                                expected_params = fmap.get(arg_idx, {})

                cur_self = active_impl_target()
                lex = current_lexical_state()
                for pidx, part in enumerate(split_top_level(c_slice, ",")):
                    colon = find_top_level_token(part, ":")
                    if colon is not None:
                        pat = part[:colon]
                        ty = part[colon + 1 :]
                        simple_name = parse_simple_pat_ident(pat)
                        if simple_name:
                            ptype, pdet = classify_type_tokens(ty, wrapper_structs, cur_self, ctx, lex)
                            if ptype == "Game":
                                cvars[simple_name] = ("Game", "explicit-closure-param")
                            elif ptype == AMBIGUOUS:
                                cvars[simple_name] = (AMBIGUOUS, pdet)
                            elif ptype is not None:
                                cvars[simple_name] = (ptype, "wrapper-closure-param")
                            else:
                                cvars[simple_name] = (NON_GAME, "")
                        else:
                            definite, uncertain = extract_pattern_bound_names(pat)
                            for bname in definite | uncertain:
                                cvars[bname] = (NON_GAME, "")
                    else:
                        simple_name = parse_simple_pat_ident(part)
                        if simple_name:
                            if pidx in expected_params:
                                kind_val = expected_params[pidx]
                                orig = "game-closure-arg" if kind_val == "Game" else "wrapper-closure-arg"
                                cvars[simple_name] = (kind_val, orig)
                            elif call_is_ambig:
                                cvars[simple_name] = (AMBIGUOUS, "")
                            else:
                                cvars[simple_name] = (NON_GAME, "")
                        else:
                            definite, uncertain = extract_pattern_bound_names(part)
                            for bname in definite | uncertain:
                                cvars[bname] = (NON_GAME, "")

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
            let_origin = "explicit-let"
            has_type_ann = False
            if k < n and tokens[k][1] == ":":
                has_type_ann = True
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
                tkind, tdet = classify_type_tokens(
                    tokens[ts:te], wrapper_structs, active_impl_target(), ctx, current_lexical_state()
                )
                if tkind == AMBIGUOUS:
                    rhs_ambig = True
                    let_origin = tdet
                else:
                    whole_ty = tkind
                k = te
            if whole_ty is None and not rhs_ambig and not has_type_ann and k < n and tokens[k][1] == "=":
                whole_ty, tup_tys, rhs_ambig, let_origin = classify_let_rhs(k + 1)

            new_bindings = build_pattern_bindings(pat_toks, whole_ty, tup_tys, rhs_ambig, let_origin)

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
                # Attach `if let` / `while let` / `&& let` bindings to the condition chain, match guard, or body `{`.
                guard_arm = next(((gs, arr, ae) for gs, arr, ae in reversed(match_guard_arms) if gs <= i < arr), None)
                if guard_arm is not None:
                    _, arr_idx, arm_end = guard_arm
                    amp_rel = find_top_level_token(tokens[k + 1 : arr_idx], "&&")
                    if amp_rel is not None:
                        scheduled_ranges.setdefault(k + 1 + amp_rel + 1, []).append((arr_idx, new_bindings))
                    scheduled_ranges.setdefault(arr_idx + 1, []).append((arm_end, new_bindings))
                else:
                    body_brace, first_amp = find_header_body_brace(tokens, k + 1, True)
                    if body_brace is not None:
                        if first_amp is not None:
                            scheduled_ranges.setdefault(first_amp + 1, []).append((body_brace, new_bindings))
                        fn_opens.setdefault(body_brace, {}).update(new_bindings)

        # Match field access `.field` or `.server.field` / `.client.field`
        if text == "." and i > 0 and i + 1 < n and tokens[i + 1][0] == "IDENT":
            recv_kind, reason = receiver_kind_before_dot(i)
            if recv_kind in ("Game", AMBIGUOUS) and reason is not None:
                record_access(i, recv_kind, reason)
            elif EXTRA_SITES:
                fname = norm_ident(tokens[i + 1][1])
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
    wrappers: dict[str, str] | None = None,
) -> list[Edit]:
    edits, _ = scan_file(src, rel_posix, owners, index, wrappers=wrappers)
    return edits


def unwrap_wrapper_type(src: str, ty_toks: list[Token]) -> tuple[str | None, str]:
    """Return `(wrapper_key_or_None, inner_type_str)` for a `struct Game` field type."""
    if not ty_toks:
        return None, ""
    full = src[ty_toks[0][2] : ty_toks[-1][3]].strip()
    m = len(ty_toks)
    # Match `[client_side::|server_side::](ClientOnly|ClientShared|ServerOnly|ServerShared)<...>`
    idx = 0
    if m >= 3 and ty_toks[0][0] == "IDENT" and ty_toks[0][1] in ("client_side", "server_side") and ty_toks[1][1] == "::":
        idx = 2
    if (
        idx + 2 < m
        and ty_toks[idx][0] == "IDENT"
        and ty_toks[idx][1] in WRAPPER_KEY_BY_NAME
        and ty_toks[idx + 1][1] == "<"
        and skip_angle_generics(ty_toks, idx + 1) == m
    ):
        wkey = WRAPPER_KEY_BY_NAME[ty_toks[idx][1]]
        inner_toks = ty_toks[idx + 2 : m - 1]
        if inner_toks:
            return wkey, src[inner_toks[0][2] : inner_toks[-1][3]].strip()
    return None, full


def unwrap_wrapper_ctor(src: str, expr_toks: list[Token]) -> tuple[str | None, str]:
    """Return `(wrapper_key_or_None, inner_expr_str)` for a `Game { ... }` field initialiser."""
    if not expr_toks:
        return None, ""
    full = src[expr_toks[0][2] : expr_toks[-1][3]].strip()
    m = len(expr_toks)
    idx = 0
    if m >= 3 and expr_toks[0][0] == "IDENT" and expr_toks[0][1] in ("client_side", "server_side") and expr_toks[1][1] == "::":
        idx = 2
    if (
        idx + 4 < m
        and expr_toks[idx][0] == "IDENT"
        and expr_toks[idx][1] in WRAPPER_KEY_BY_NAME
        and expr_toks[idx + 1][1] == "::"
        and expr_toks[idx + 2][1] == "new"
        and expr_toks[idx + 3][1] == "("
        and skip_balanced(expr_toks, idx + 3, "(", ")") == m
    ):
        wkey = WRAPPER_KEY_BY_NAME[expr_toks[idx][1]]
        inner_toks = expr_toks[idx + 4 : m - 1]
        if inner_toks:
            return wkey, src[inner_toks[0][2] : inner_toks[-1][3]].strip()
    return None, full


def plan_game_struct_and_ctor_edits(
    src: str,
    owners: dict[str, str],
    wrappers: dict[str, str],
) -> tuple[list[Edit], list[str]]:
    """Plan wrapper type and constructor edits for `struct Game` and `Game::finish_construct`."""
    tokens = tokenize_rust(src)
    n = len(tokens)
    edits: list[Edit] = []
    problems: list[str] = []

    # 1. Locate top-level `struct Game<'a> { ... }`
    depth = 0
    struct_body: tuple[int, int] | None = None
    i = 0
    while i < n:
        tv = tokens[i][1]
        if tv == "{":
            depth += 1
        elif tv == "}":
            depth = max(0, depth - 1)
        elif depth == 0 and tokens[i][0] == "IDENT" and tv == "struct" and i + 1 < n and tokens[i + 1][1] == "Game":
            j = i + 2
            if j < n and tokens[j][1] == "<":
                j = skip_angle_generics(tokens, j)
            while j < n and tokens[j][1] not in ("{", ";"):
                j += 1
            if j < n and tokens[j][1] == "{":
                end = skip_balanced(tokens, j, "{", "}")
                struct_body = (j, end)
                break
        i += 1

    if struct_body is None:
        problems.append("crates/sunrise-client/src/game.rs: `struct Game` declaration not found")
        return edits, problems

    seen_struct_fields: set[str] = set()
    for part in split_top_level(tokens[struct_body[0] + 1 : struct_body[1] - 1], ","):
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
        if k + 2 >= len(part) or part[k][0] != "IDENT" or part[k + 1][1] != ":":
            continue
        fname = norm_ident(part[k][1])
        seen_struct_fields.add(fname)
        ty_toks = part[k + 2 :]
        actual_wrap, inner_ty = unwrap_wrapper_type(src, ty_toks)
        expected_wrap = wrappers.get(fname)
        if fname not in ("server", "client") and fname not in owners:
            problems.append(f"crates/sunrise-client/src/game.rs: `Game::{fname}` missing from game_field_owners.tsv")
            continue
        if actual_wrap != expected_wrap:
            ty_start, ty_end = ty_toks[0][2], ty_toks[-1][3]
            if expected_wrap is not None:
                smod, wname = WRAPPER_TYPES[expected_wrap]
                new_ty = f"{smod}::{wname}<{inner_ty}>"
            else:
                new_ty = inner_ty
            old_ty = src[ty_start:ty_end]
            edits.append(
                Edit(
                    ty_start,
                    ty_end,
                    new_ty,
                    fname,
                    "struct-game-wrapper",
                    f"{fname}: {old_ty}",
                    f"{fname}: {new_ty}",
                )
            )
            problems.append(
                f"crates/sunrise-client/src/game.rs: `Game::{fname}` wrapper is `{actual_wrap or '-'}`, "
                f"expected `{expected_wrap or '-'}` per game_field_owners.tsv"
            )

    for missing in sorted(set(wrappers) - seen_struct_fields):
        problems.append(f"crates/sunrise-client/src/game.rs: wrapped field `{missing}` missing from `struct Game`")

    # 2. Locate `Game { ... }` and optional out-of-line wrapped init helpers inside `fn finish_construct`
    ctor_body: tuple[int, int] | None = None
    helper_specs = (
        ("WrappedClientInit", "wc", ("client_",), False),
        ("WrappedServerInit", "ws", ("server_",), False),
        ("WrappedRendererInit", "wr", ("client_",), False),
        ("WrappedPrepInit", "wp", ("client_", "server_"), True),
    )
    helper_names = {spec[0] for spec in helper_specs}
    helper_struct_bodies: dict[str, tuple[int, int]] = {}
    helper_lit_bodies: dict[str, tuple[int, int]] = {}
    i = 0
    while i < n:
        if (
            tokens[i][0] == "IDENT"
            and tokens[i][1] == "struct"
            and i + 2 < n
            and tokens[i + 1][1] in helper_names
            and tokens[i + 2][1] == "{"
        ):
            sname = tokens[i + 1][1]
            send = skip_balanced(tokens, i + 2, "{", "}")
            helper_struct_bodies[sname] = (i + 2, send)
            i = send
            continue
        if tokens[i][0] == "IDENT" and tokens[i][1] == "fn" and i + 1 < n and tokens[i + 1][1] == "finish_construct":
            j = i + 2
            while j < n and tokens[j][1] != "{":
                j += 1
            if j < n:
                fn_end = skip_balanced(tokens, j, "{", "}")
                k = j + 1
                while k < fn_end - 1:
                    if (
                        tokens[k][0] == "IDENT"
                        and tokens[k][1] == "struct"
                        and k + 2 < fn_end
                        and tokens[k + 1][1] in helper_names
                        and tokens[k + 2][1] == "{"
                    ):
                        sname = tokens[k + 1][1]
                        send = skip_balanced(tokens, k + 2, "{", "}")
                        helper_struct_bodies[sname] = (k + 2, send)
                        k = send
                        continue
                    if (
                        tokens[k][0] == "IDENT"
                        and tokens[k][1] in helper_names
                        and tokens[k + 1][1] == "{"
                        and (k == j + 1 or tokens[k - 1][1] != "struct")
                    ):
                        sname = tokens[k][1]
                        lend = skip_balanced(tokens, k + 1, "{", "}")
                        helper_lit_bodies[sname] = (k + 1, lend)
                        k = lend
                        continue
                    if tokens[k][0] == "IDENT" and tokens[k][1] == "Game" and tokens[k + 1][1] == "{":
                        lit_end = skip_balanced(tokens, k + 1, "{", "}")
                        ctor_body = (k + 1, lit_end)
                        break
                    k += 1
                break
        i += 1

    if ctor_body is None:
        problems.append("crates/sunrise-client/src/game.rs: `Game { ... }` literal in `finish_construct` not found")
        return edits, problems

    helper_field_vars: dict[str, str] = {}
    for sname, var_name, expected_prefixes, allow_unwrapped in helper_specs:
        sbody = helper_struct_bodies.get(sname)
        lbody = helper_lit_bodies.get(sname)
        if sbody is None and lbody is None:
            continue
        if sbody is None or lbody is None:
            problems.append(f"crates/sunrise-client/src/game.rs: `{sname}` struct and literal must both exist")
            continue
        struct_fnames: set[str] = set()
        for part in split_top_level(tokens[sbody[0] + 1 : sbody[1] - 1], ","):
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
            if k + 2 >= len(part) or part[k][0] != "IDENT" or part[k + 1][1] != ":":
                continue
            fname = norm_ident(part[k][1])
            struct_fnames.add(fname)
            ty_toks = part[k + 2 :]
            actual_wrap, inner_ty = unwrap_wrapper_type(src, ty_toks)
            expected_wrap = wrappers.get(fname)
            if expected_wrap is None:
                if allow_unwrapped:
                    if actual_wrap is not None:
                        problems.append(
                            f"crates/sunrise-client/src/game.rs: `{sname}::{fname}` has unexpected wrapper `{actual_wrap}`"
                        )
                    continue
                problems.append(
                    f"crates/sunrise-client/src/game.rs: `{sname}::{fname}` has wrapper `-`, "
                    f"expected `{expected_prefixes}`"
                )
                continue
            if not any(expected_wrap.startswith(p) for p in expected_prefixes):
                problems.append(
                    f"crates/sunrise-client/src/game.rs: `{sname}::{fname}` has wrapper `{expected_wrap}`, "
                    f"expected `{expected_prefixes}`"
                )
                continue
            helper_field_vars[fname] = var_name
            if actual_wrap != expected_wrap:
                ty_start, ty_end = ty_toks[0][2], ty_toks[-1][3]
                smod, wname = WRAPPER_TYPES[expected_wrap]
                new_ty = f"{smod}::{wname}<{inner_ty}>"
                old_ty = src[ty_start:ty_end]
                edits.append(
                    Edit(
                        ty_start,
                        ty_end,
                        new_ty,
                        fname,
                        "ctor-helper-struct-wrapper",
                        f"{fname}: {old_ty}",
                        f"{fname}: {new_ty}",
                    )
                )
                problems.append(
                    f"crates/sunrise-client/src/game.rs: `{sname}::{fname}` wrapper is `{actual_wrap or '-'}`, "
                    f"expected `{expected_wrap}`"
                )
        lit_fnames: set[str] = set()
        for part in split_top_level(tokens[lbody[0] + 1 : lbody[1] - 1], ","):
            k = 0
            while k < len(part) and part[k][1] == "#" and k + 1 < len(part) and part[k + 1][1] == "[":
                k = skip_balanced(part, k + 1, "[", "]")
            if k >= len(part) or part[k][0] != "IDENT":
                continue
            fname = norm_ident(part[k][1])
            lit_fnames.add(fname)
            expected_wrap = wrappers.get(fname)
            if expected_wrap is None or not any(expected_wrap.startswith(p) for p in expected_prefixes):
                continue
            if k + 1 < len(part) and part[k + 1][1] == ":":
                expr_toks = part[k + 2 :]
                actual_wrap, inner_expr = unwrap_wrapper_ctor(src, expr_toks)
                rep_start, rep_end = expr_toks[0][2], expr_toks[-1][3]
                is_shorthand = False
            elif k + 1 == len(part):
                actual_wrap = None
                inner_expr = part[k][1]
                rep_start, rep_end = part[k][2], part[k][3]
                is_shorthand = True
            else:
                continue
            if actual_wrap != expected_wrap:
                smod, wname = WRAPPER_TYPES[expected_wrap]
                wrapped_expr = f"{smod}::{wname}::new({inner_expr})"
                replacement = f"{fname}: {wrapped_expr}" if is_shorthand else wrapped_expr
                old_expr = src[rep_start:rep_end]
                edits.append(
                    Edit(
                        rep_start,
                        rep_end,
                        replacement,
                        fname,
                        "ctor-helper-wrapper",
                        f"{fname}: {old_expr}",
                        f"{fname}: {replacement}",
                    )
                )
                problems.append(
                    f"crates/sunrise-client/src/game.rs: `{sname}` literal `{fname}` uses wrapper "
                    f"`{actual_wrap or '-'}`, expected `{expected_wrap}`"
                )
        if struct_fnames != lit_fnames:
            problems.append(
                f"crates/sunrise-client/src/game.rs: `{sname}` struct/literal field mismatch: "
                f"struct_only={sorted(struct_fnames - lit_fnames)}, lit_only={sorted(lit_fnames - struct_fnames)}"
            )

    seen_ctor_fields: set[str] = set()
    for part in split_top_level(tokens[ctor_body[0] + 1 : ctor_body[1] - 1], ","):
        k = 0
        while k < len(part) and part[k][1] == "#" and k + 1 < len(part) and part[k + 1][1] == "[":
            k = skip_balanced(part, k + 1, "[", "]")
        if k >= len(part) or part[k][0] != "IDENT":
            continue
        fname = norm_ident(part[k][1])
        seen_ctor_fields.add(fname)
        expected_wrap = wrappers.get(fname)
        if fname in helper_field_vars:
            var_name = helper_field_vars[fname]
            expected_move = f"{var_name}.{fname}"
            if k + 1 < len(part) and part[k + 1][1] == ":":
                expr_toks = part[k + 2 :]
                is_move = (
                    len(expr_toks) == 3
                    and expr_toks[0][0] == "IDENT"
                    and expr_toks[0][1] == var_name
                    and expr_toks[1][1] == "."
                    and expr_toks[2][0] == "IDENT"
                    and norm_ident(expr_toks[2][1]) == fname
                )
                if not is_move:
                    rep_start, rep_end = expr_toks[0][2], expr_toks[-1][3]
                    old_expr = src[rep_start:rep_end]
                    edits.append(
                        Edit(
                            rep_start,
                            rep_end,
                            expected_move,
                            fname,
                            "ctor-helper-move",
                            f"{fname}: {old_expr}",
                            f"{fname}: {expected_move}",
                        )
                    )
                    problems.append(
                        f"crates/sunrise-client/src/game.rs: `finish_construct` `{fname}` must move `{expected_move}`"
                    )
            else:
                problems.append(
                    f"crates/sunrise-client/src/game.rs: `finish_construct` `{fname}` must move `{expected_move}`"
                )
            continue
        if k + 1 < len(part) and part[k + 1][1] == ":":
            expr_toks = part[k + 2 :]
            actual_wrap, inner_expr = unwrap_wrapper_ctor(src, expr_toks)
            rep_start, rep_end = expr_toks[0][2], expr_toks[-1][3]
            is_shorthand = False
        elif k + 1 == len(part):
            actual_wrap = None
            inner_expr = part[k][1]
            rep_start, rep_end = part[k][2], part[k][3]
            is_shorthand = True
        else:
            continue
        if actual_wrap != expected_wrap:
            if expected_wrap is not None:
                smod, wname = WRAPPER_TYPES[expected_wrap]
                wrapped_expr = f"{smod}::{wname}::new({inner_expr})"
                replacement = f"{fname}: {wrapped_expr}" if is_shorthand else wrapped_expr
            else:
                replacement = inner_expr
            old_expr = src[rep_start:rep_end]
            edits.append(
                Edit(
                    rep_start,
                    rep_end,
                    replacement,
                    fname,
                    "ctor-game-wrapper",
                    f"{fname}: {old_expr}",
                    f"{fname}: {replacement}",
                )
            )
            problems.append(
                f"crates/sunrise-client/src/game.rs: `finish_construct` `{fname}` uses wrapper "
                f"`{actual_wrap or '-'}`, expected `{expected_wrap or '-'}`"
            )

    for missing in sorted(set(wrappers) - seen_ctor_fields):
        problems.append(
            f"crates/sunrise-client/src/game.rs: wrapped field `{missing}` missing from `Game::finish_construct`"
        )

    edits.sort(key=lambda e: (e.start, e.end))
    return edits, problems


def check_side_wrapper_defs(side_path: Path, side: str) -> tuple[bool, list[str]]:
    """Verify wrapper structs in `{client,server}_side.rs`. Returns `(needs_append, problems)`."""
    rel = side_path.as_posix()
    if not side_path.is_file():
        return False, [f"{rel}: side module file not found"]
    src = side_path.read_text(encoding="utf-8")
    tokens = tokenize_rust(src)
    n = len(tokens)
    smod = "client_side" if side == "client" else "server_side"
    expected_structs = (
        ("ClientOnly", ("pub", "(", "in", "crate", "::", "game", "::", "client_side", ")"))
        if side == "client"
        else ("ServerOnly", ("pub", "(", "in", "crate", "::", "game", "::", "server_side", ")"))
    ), (
        ("ClientShared", ("pub", "(", "in", "crate", "::", "game", ")"))
        if side == "client"
        else ("ServerShared", ("pub", "(", "in", "crate", "::", "game", ")"))
    )
    expected_names = {name for name, _ in expected_structs}
    found_structs: set[str] = set()
    problems: list[str] = []

    i = 0
    while i < n:
        if tokens[i][0] == "IDENT" and tokens[i][1] == "struct" and i + 1 < n and tokens[i + 1][1] in expected_names:
            sname = tokens[i + 1][1]
            found_structs.add(sname)
            exp_vis = dict(expected_structs)[sname]
            # Collect contiguous attributes before `pub(in crate::game) struct <Name>`
            vis_start = i
            if vis_start >= 1 and tokens[vis_start - 1][1] == ")":
                # walk back over `pub(...)`
                k = vis_start - 1
                dp = 0
                while k >= 0:
                    if tokens[k][1] == ")":
                        dp += 1
                    elif tokens[k][1] == "(":
                        dp -= 1
                        if dp == 0:
                            break
                    k -= 1
                if k > 0 and tokens[k - 1][1] == "pub":
                    vis_start = k - 1
            elif vis_start >= 1 and tokens[vis_start - 1][1] == "pub":
                vis_start -= 1
            # Walk back over immediately preceding attributes `#[...]`
            attr_end = vis_start
            has_repr_transparent = False
            while attr_end >= 1 and tokens[attr_end - 1][1] == "]":
                k = attr_end - 1
                db = 0
                while k >= 0:
                    if tokens[k][1] == "]":
                        db += 1
                    elif tokens[k][1] == "[":
                        db -= 1
                        if db == 0:
                            break
                    k -= 1
                if k > 0 and tokens[k - 1][1] == "#":
                    attr_toks = [t[1] for t in tokens[k + 1 : attr_end - 1]]
                    if attr_toks == ["repr", "(", "transparent", ")"]:
                        has_repr_transparent = True
                    if attr_toks and attr_toks[0] == "derive":
                        problems.append(f"{rel}: `{sname}` must not carry `#[derive(...)]`")
                    attr_end = k - 1
                else:
                    break
            if not has_repr_transparent:
                problems.append(f"{rel}: `{sname}` must carry `#[repr(transparent)]`")
            # Check outer visibility is `pub(in crate::game)`
            outer_vis = tuple(t[1] for t in tokens[vis_start:i])
            if outer_vis != ("pub", "(", "in", "crate", "::", "game", ")"):
                problems.append(f"{rel}: `{sname}` must be `pub(in crate::game)`")
            # Check tuple body `<T>(<exp_vis> T);`
            j = i + 2
            if j < n and tokens[j][1] == "<":
                j = skip_angle_generics(tokens, j)
            if j >= n or tokens[j][1] != "(":
                problems.append(f"{rel}: `{sname}` must be a single-field tuple struct")
            else:
                pend = skip_balanced(tokens, j, "(", ")")
                inner = tuple(t[1] for t in tokens[j + 1 : pend - 1])
                if inner != exp_vis + ("T",):
                    problems.append(
                        f"{rel}: `{sname}` inner field must be `{' '.join(exp_vis)} T` (got `{' '.join(inner)}`)"
                    )
        # Check `impl` blocks for wrapper types: forbid Deref/AsRef and any wide method other than `new`.
        elif tokens[i][0] == "IDENT" and tokens[i][1] == "impl":
            j = i + 1
            if j < n and tokens[j][1] == "<":
                j = skip_angle_generics(tokens, j)
            for_idx = None
            brace_idx = None
            while j < n and tokens[j][1] not in ("{", ";"):
                if tokens[j][1] == "for" and for_idx is None:
                    for_idx = j
                j += 1
            if j < n and tokens[j][1] == "{":
                brace_idx = j
                end = skip_balanced(tokens, j, "{", "}")
                hdr_toks = [t[1] for t in tokens[i + 1 : brace_idx]]
                target_toks = [t[1] for t in tokens[(for_idx + 1 if for_idx is not None else i + 1) : brace_idx]]
                hit_name = next((nm for nm in expected_names if nm in target_toks), None)
                if hit_name is not None:
                    if for_idx is not None:
                        trait_toks = [t[1] for t in tokens[i + 1 : for_idx]]
                        for bad in ("Deref", "DerefMut", "AsRef", "AsMut", "Borrow", "BorrowMut", "Default", "Debug", "Clone"):
                            if bad in trait_toks:
                                problems.append(f"{rel}: `{hit_name}` must not implement `{bad}`")
                    else:
                        # Inherent impl: allow only `pub(in crate::game) fn new`
                        k = brace_idx + 1
                        while k < end - 1:
                            if tokens[k][0] == "IDENT" and tokens[k][1] == "fn" and k + 1 < end:
                                mname = norm_ident(tokens[k + 1][1])
                                if mname != "new" and is_pub_before_fn(tokens, k):
                                    # Allow only side-private methods if any; reject wider ones.
                                    # Walk back to check visibility tokens.
                                    vk = k - 1
                                    while vk > brace_idx and tokens[vk][1] in ("const", "async", "unsafe"):
                                        vk -= 1
                                    vis_toks: list[str] = []
                                    if tokens[vk][1] == ")":
                                        vstart = vk
                                        dp = 0
                                        while vstart > brace_idx:
                                            if tokens[vstart][1] == ")":
                                                dp += 1
                                            elif tokens[vstart][1] == "(":
                                                dp -= 1
                                                if dp == 0:
                                                    break
                                            vstart -= 1
                                        if vstart > brace_idx and tokens[vstart - 1][1] == "pub":
                                            vis_toks = [t[1] for t in tokens[vstart - 1 : vk + 1]]
                                    elif tokens[vk][1] == "pub":
                                        vis_toks = ["pub"]
                                    if tuple(vis_toks) != ("pub", "(", "in", "crate", "::", "game", "::", smod, ")"):
                                        problems.append(
                                            f"{rel}: `{hit_name}::{mname}` must not be visible outside `{smod}`"
                                        )
                            k += 1
        i += 1

    missing = sorted(expected_names - found_structs)
    needs_append = len(missing) == len(expected_names)
    for m in missing:
        problems.append(f"{rel}: missing wrapper struct `{m}`")
    return needs_append, problems


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
    mode.add_argument("--list", action="store_true", help="print every site that would be rewritten with its justification")
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
    wrappers = load_field_wrappers(args.table)
    paths = args.paths if args.paths else list(DEFAULT_PATHS)
    files = collect_files(paths)
    root_resolved = ROOT.resolve()
    file_rels: list[tuple[Path, str]] = []
    for path in files:
        try:
            rel_posix = path.resolve().relative_to(root_resolved).as_posix()
        except ValueError:
            rel_posix = path.as_posix()
        file_rels.append((path, rel_posix))
    index = build_workspace_index(paths) if any(is_in_game_tree(rel) for _, rel in file_rels) else WorkspaceIndex()

    total_edits = 0
    total_ambiguous = 0
    structural_problems: list[str] = []
    files_changed = 0
    changed_paths: list[Path] = []
    by_reason: dict[str, int] = {}

    # Sync / check side wrapper definitions when scanning the real game tree with a wrapper table.
    has_game_rs = any(rel == "crates/sunrise-client/src/game.rs" for _, rel in file_rels)
    if wrappers and has_game_rs:
        for side, fname in (("client", "client_side.rs"), ("server", "server_side.rs")):
            side_path = root_resolved / "crates/sunrise-client/src/game" / fname
            needs_append, probs = check_side_wrapper_defs(side_path, side)
            if args.fix and needs_append:
                side_path.write_text(side_path.read_text(encoding="utf-8") + SIDE_WRAPPER_DEFS[side], encoding="utf-8")
                changed_paths.append(side_path)
                _, probs = check_side_wrapper_defs(side_path, side)
            structural_problems.extend(probs)

    file_root_cache: dict[Path, frozenset[str]] = {}
    for path, rel_posix in file_rels:
        src = path.read_text(encoding="utf-8")
        edits, ambig = scan_file(src, rel_posix, owners, index, file_root_cache, wrappers=wrappers)
        line_starts = compute_line_starts(src) if (ambig or (edits and (args.check or args.list))) else []
        for site in ambig:
            total_ambiguous += 1
            lineno = offset_to_line(line_starts, site.offset)
            print(
                f"{rel_posix}:{lineno}: ambiguous receiver for Game field `{site.field}` ({site.detail}) — review manually",
                file=sys.stderr,
            )

        struct_edits: list[Edit] = []
        struct_probs: list[str] = []
        if wrappers and rel_posix == "crates/sunrise-client/src/game.rs":
            if args.fix:
                src_after_access = apply_edits(src, edits) if edits else src
                struct_edits, struct_probs = plan_game_struct_and_ctor_edits(src_after_access, owners, wrappers)
                if edits or struct_edits:
                    new_src = apply_edits(src_after_access, struct_edits) if struct_edits else src_after_access
                    path.write_text(new_src, encoding="utf-8")
                    changed_paths.append(path)
                    files_changed += 1
                all_e = edits + struct_edits
                total_edits += len(all_e)
                for e in all_e:
                    by_reason[e.reason] = by_reason.get(e.reason, 0) + 1
                # Re-verify structural problems after fix
                if struct_edits:
                    _, struct_probs = plan_game_struct_and_ctor_edits(
                        path.read_text(encoding="utf-8"), owners, wrappers
                    )
                structural_problems.extend(struct_probs)
                continue
            else:
                struct_edits, struct_probs = plan_game_struct_and_ctor_edits(src, owners, wrappers)
                if args.check:
                    structural_problems.extend(struct_probs)

        all_e = edits + (struct_edits if args.list else [])
        if not all_e:
            continue
        files_changed += 1
        total_edits += len(all_e)
        for e in all_e:
            by_reason[e.reason] = by_reason.get(e.reason, 0) + 1
        if args.fix:
            path.write_text(apply_edits(src, edits), encoding="utf-8")
            changed_paths.append(path)
        elif args.list:
            if not line_starts:
                line_starts = compute_line_starts(src)
            for e in all_e:
                lineno = offset_to_line(line_starts, e.start)
                print(f"{rel_posix}:{lineno}: `{e.old_expr}` -> `{e.new_expr}` [{e.reason}]")
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
    elif args.list:
        print(f"Listed {total_edits} rewrite sites across {files_changed} files.")
    else:
        print(f"Found {total_edits} stale sites across {files_changed} files.")

    for prob in structural_problems:
        print(f"error: {prob}", file=sys.stderr)

    if total_ambiguous > 0:
        print(f"Reported {total_ambiguous} ambiguous site(s) for manual review.", file=sys.stderr)

    if args.stats and by_reason:
        for reason, count in sorted(by_reason.items()):
            print(f"  {reason}: {count}")

    if args.check and (total_edits > 0 or structural_problems):
        return 1
    if args.fix and structural_problems:
        return 1
    if total_ambiguous > 0 and not args.allow_ambiguous:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
