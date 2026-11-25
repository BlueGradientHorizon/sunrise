#!/usr/bin/env python3
"""Role-reachability analyzer and re-homing tool for `sunrise-client` gameplay modules/functions.

Computes client/server role reachability from production entry points over MIR dumps, flags
role-vs-state contradictions, generates a safe re-homing table, and applies module/function moves
across `game`, `client_side`, and `server_side` (including moving functions out of side modules or
mirror files back to common modules and dissolving empty `*_client.rs` / `*_server.rs` mirrors).

Usage:
    python3 tools/game_rehome.py --analyze --dump-dir DIR [--configs default,...]
                                 [--fire-sides JSON] [--out-rehome TSV]
                                 [--out-contradictions TSV] [--out-reach TSV] [--out-json JSON]
    python3 tools/game_rehome.py --dry-run --table tools/game_rehome.tsv [--report TSV]
    python3 tools/game_rehome.py --apply --table tools/game_rehome.tsv
                                 [--dump-dir DIR] [--report TSV] [--strict] [--no-fmt]
    python3 tools/game_rehome.py --check --table tools/game_rehome.tsv
"""

from __future__ import annotations

import argparse
import csv
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
from collections import Counter, defaultdict, deque
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_REHOME_TABLE = Path(__file__).resolve().with_name("game_rehome.tsv")
REHOME_COLUMNS = ("kind", "module", "impl", "name", "cfg", "current_side", "target_side", "reason")
VALID_KINDS = frozenset({"module", "fn"})
VALID_SIDES = frozenset({"client", "server", "game"})
SIDE_MOD_NAME = {"client": "client_side", "server": "server_side"}

UNSIZE_ANY_RE = re.compile(
    r"=\s*(?:move|copy)\s+([^\s;]+)\s+as\s+(.+?)\s+\(PointerCoercion\(Unsize(?:,\s*[^)]*)?\)\);"
)
DERIVE_TRAITS = frozenset(
    {"Debug", "Clone", "Copy", "PartialEq", "Eq", "Hash", "Default", "PartialOrd", "Ord", "Serialize", "Deserialize"}
)

# Production client roots under `game::`.
CLIENT_CORE_ROOTS = frozenset(
    {
        "game::client_side::window_handler::<impl ApplicationHandler for game::Game>::resumed",
        "game::client_side::window_handler::<impl ApplicationHandler for game::Game>::window_event",
        "game::client_side::window_handler::<impl ApplicationHandler for game::Game>::device_event",
        "game::client_side::window_handler::<impl ApplicationHandler for game::Game>::about_to_wait",
        "game::client_side::window_handler::<impl ApplicationHandler for game::Game>::exiting",
        "game::client_side::window_handler::<impl game::Game>::shut_down",
        "game::headless::<impl game::Game>::run_headless",
        "game::remote::tick::<impl game::Game>::tick_remote",
        "game::client_side::game_client::<impl game::Game>::render",
        "game::remote::<impl game::Game>::poll_remote",
        "game::client_side::remote_client::<impl game::Game>::apply_packet",
        "game::client_side::remote_client::raw_ingress::<impl game::Game>::handle_remote_event",
    }
)

# Production server roots under `game::`.
SERVER_CORE_ROOTS = frozenset(
    {
        "game::server_side::integrated::serve",
        "game::server_side::integrated::<impl game::Game>::integrated_tick_loop",
        "game::server_side::integrated::<impl game::Game>::run_integrated",
        "game::server_side::dedicated::<impl game::Game>::run_dedicated",
        "game::server_side::levels::<impl game::Game>::tick_server",
        "game::server_side::levels::<impl game::Game>::tick_server_first",
        "game::server_side::lan::<impl game::Game>::tick_lan",
        "game::server_side::lan::<impl game::Game>::tick_lan_packets",
        "game::server_side::lan::<impl game::Game>::lan_process_events",
        "game::server_side::lan_actions_server::turns::<impl game::Game>::guest_turn",
        "game::lan_actions::turns::<impl game::Game>::as_guest",
        "game::lan_actions::turns::<impl game::Game>::as_guest_at",
        "game::server_side::lan_actions_server::turns::<impl game::Game>::as_guest_messaged",
    }
)

# Startup / loading screen roots (used in startup sensitivity mode).
STARTUP_CLIENT_ROOTS = frozenset(
    {
        "game::client_side::loading_window::start_game",
        "game::client_side::loading_window::start_client_game",
        "game::client_side::loading_window::<impl game::Game>::finish_start_game",
        "game::client_side::loading_window::<impl game::Game>::apply_pending_client_start",
        "game::client_side::loading_screen::<impl ApplicationHandler for game::client_side::loading_screen::App>::resumed",
        "game::client_side::loading_screen::<impl ApplicationHandler for game::client_side::loading_screen::App>::window_event",
        "game::client_side::loading_screen::<impl ApplicationHandler for game::client_side::loading_screen::App>::device_event",
        "game::client_side::loading_screen::<impl ApplicationHandler for game::client_side::loading_screen::App>::about_to_wait",
        "game::client_side::loading_screen::<impl ApplicationHandler for game::client_side::loading_screen::App>::exiting",
        "game::no_resources::<impl ApplicationHandler for game::no_resources::Missing>::resumed",
        "game::no_resources::<impl ApplicationHandler for game::no_resources::Missing>::window_event",
        "game::no_resources::<impl ApplicationHandler for game::no_resources::Missing>::about_to_wait",
    }
)

# Launch and second-Game construction cuts on client traversal.
LAUNCH_CUT_NODES = frozenset(
    {
        "game::client_side::loading_window::start_dedicated_game",
        "game::server_side::integrated::serve",
        "game::server_side::integrated::server_game",
        "game::server_side::integrated::<impl game::Game>::run_component_harness",
        "game::server_side::dedicated::<impl game::Game>::run_dedicated",
        "game::<impl game::Game>::construct",
        "game::<impl game::Game>::new",
    }
)

# Fixture and harness entries that construct synthetic games or probe both roles.
FIXTURE_CUT_TAILS = frozenset(
    {
        "::run_boat_rider_fixture",
        "::run_native_client_particles_fixture",
        "::run_net_harness",
        "::screen_build_fixture",
        "::run_boat_authority_fixture",
        "::wait_command_harness",
        "::hot_paths",
        "::dispense_fixture",
        "::sculk_fixture",
        "::run_explosion_items_fixture",
    }
)

# Functions in `crates/sunrise-client/src/game.rs` inspected by `tools/game_field_owners.py`.
PINNED_GAME_RS_FNS = frozenset(
    {
        "finish_construct",
        "build_wrapped_client",
        "build_wrapped_server",
        "build_wrapped_renderer",
        "wrap_prep_and_effects",
        "assemble_game",
    }
)

# Modules whose role placement is architecturally fixed and must never be re-homed.
PINNED_MODULES = frozenset(
    {
        "side_ops",
        "tables",
        "server_effects",
        "modapi_client",
        "modapi_server",
        "native_hooks",
        "native_hooks_client",
        "game_client",
        "game_server",
    }
)


def _load_sibling(name: str):
    existing = sys.modules.get(name)
    if existing is not None:
        return existing
    path = Path(__file__).resolve().with_name(f"{name}.py")
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


gsl = _load_sibling("game_side_ledger")
gfs = _load_sibling("game_fn_sides")
gms = _load_sibling("game_module_sides")
gsc = _load_sibling("game_side_calls")


@dataclass(frozen=True)
class RehomeRow:
    kind: str  # "module" | "fn"
    module: str
    impl: str
    name: str
    cfg: str
    current_side: str
    target_side: str
    reason: str

    def sort_key(self) -> tuple[str, str, str, str, str]:
        return (self.kind, self.module, self.impl, self.name, self.cfg)

    def as_tsv_fields(self) -> list[str]:
        return [
            self.kind,
            self.module,
            self.impl,
            self.name,
            self.cfg,
            self.current_side,
            self.target_side,
            self.reason,
        ]


@dataclass(frozen=True)
class Refusal:
    kind: str
    module: str
    impl: str
    name: str
    cfg: str
    target_side: str
    code: str
    detail: str


def _rel_str(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def load_rehome_table(path: Path) -> list[RehomeRow]:
    """Parse a re-homing TSV table and fail closed on schema or duplicate errors."""
    if not path.is_file():
        raise SystemExit(f"{path}: rehome table does not exist")
    rows: list[RehomeRow] = []
    seen: set[tuple[str, str, str, str, str]] = set()
    with path.open(newline="", encoding="utf-8") as fp:
        reader = csv.reader(fp, delimiter="\t")
        header = next(reader, None)
        if not header or tuple(h.strip() for h in header) != REHOME_COLUMNS:
            raise SystemExit(f"{path}: expected columns {list(REHOME_COLUMNS)}, got {header}")
        for lineno, raw in enumerate(reader, 2):
            if not any(c.strip() for c in raw):
                continue
            if len(raw) != len(REHOME_COLUMNS):
                raise SystemExit(f"{path}:{lineno}: expected {len(REHOME_COLUMNS)} columns, got {len(raw)}")
            kind, mod, impl, name, cfg, cur, tgt, reason = (c.strip() for c in raw)
            if kind not in VALID_KINDS:
                raise SystemExit(f"{path}:{lineno}: invalid kind {kind!r}")
            if cur not in VALID_SIDES or tgt not in VALID_SIDES:
                raise SystemExit(f"{path}:{lineno}: invalid side transition {cur!r} -> {tgt!r}")
            if not mod or not impl or not name or not cfg or not reason:
                raise SystemExit(f"{path}:{lineno}: empty required field")
            if kind == "module" and (impl != "-" or name != "-" or cfg != "-"):
                raise SystemExit(f"{path}:{lineno}: module row must have impl=name=cfg='-'")
            key = (kind, mod, impl, name, cfg)
            if key in seen:
                raise SystemExit(f"{path}:{lineno}: duplicate row {key}")
            seen.add(key)
            rows.append(RehomeRow(kind, mod, impl, name, cfg, cur, tgt, reason))
    return rows


def format_rehome_table(rows: list[RehomeRow]) -> str:
    out = ["\t".join(REHOME_COLUMNS) + "\n"]
    for r in sorted(rows, key=lambda x: x.sort_key()):
        out.append("\t".join(r.as_tsv_fields()) + "\n")
    return "".join(out)


def _canonical_orig_module_from_cid(cid: str, regular_mod_names: set[str] | frozenset[str] | None = None) -> str:
    """Strip `game::`, `client_side::`, `server_side::`, and mirror-only `_client`/`_server` suffixes."""
    if not cid.startswith("game::"):
        return ""
    reg = regular_mod_names or ()
    rest = cid[len("game::") :]
    if rest.startswith("client_side::"):
        rest = rest[len("client_side::") :]
        parts = rest.split("::")
        if parts[0].endswith("_client") and parts[0] not in reg:
            parts[0] = parts[0][: -len("_client")]
    elif rest.startswith("server_side::"):
        rest = rest[len("server_side::") :]
        parts = rest.split("::")
        if parts[0].endswith("_server") and parts[0] not in reg:
            parts[0] = parts[0][: -len("_server")]
    else:
        parts = rest.split("::")
    # Strip trailing impl/fn/nested segments to find module prefix.
    mod_parts: list[str] = []
    for seg in parts:
        if seg.startswith("<impl ") or seg == "_":
            break
        mod_parts.append(seg)
    if len(mod_parts) >= 1 and not any(p.startswith("<impl ") for p in parts):
        # Free fn: drop final fn name segment (and nested inner fn segments if any).
        mod_parts = mod_parts[:-1]
    return "::".join(mod_parts) if mod_parts else "game"


def _top_module_from_cid(cid: str, regular_mod_names: set[str] | frozenset[str] | None = None) -> str:
    """Return top-level regular module name (`game` for `game.rs`, else module in `game_module_sides.tsv`)."""
    orig_mod = _canonical_orig_module_from_cid(cid, regular_mod_names)
    return orig_mod.split("::", 1)[0] if orig_mod else "game"


def collect_side_ops_spec(root: Path) -> dict[str, Any]:
    """Parse `crates/sunrise-client/src/game/side_ops/*.rs` dispatchers and their server/client arms.

    Returns:
        dispatcher_fns: set[str] - all methods defined in common `side_ops` files (pinned common)
        server_arms: set[str] - all `self.<fn>(...)` methods called on the logical-server branch
        client_arms: set[str] - all `self.<fn>(...)` methods called on the logical-client branch
        server_arms_by_disp: dict[str, set[str]] - dispatcher fn -> server arm fn names
        client_arms_by_disp: dict[str, set[str]] - dispatcher fn -> client arm fn names
    """
    side_ops_dir = root / "crates/sunrise-client/src/game/side_ops"
    dispatcher_fns: set[str] = set()
    server_arms: set[str] = set()
    client_arms: set[str] = set()
    server_arms_by_disp: dict[str, set[str]] = defaultdict(set)
    client_arms_by_disp: dict[str, set[str]] = defaultdict(set)
    if not side_ops_dir.is_dir():
        return {
            "dispatcher_fns": dispatcher_fns,
            "server_arms": server_arms,
            "client_arms": client_arms,
            "server_arms_by_disp": server_arms_by_disp,
            "client_arms_by_disp": client_arms_by_disp,
        }

    self_call_re = re.compile(r"\bself\.([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:::<[^>]*>)?\s*\(")

    def _skip_block(toks: list[Any], idx: int) -> int:
        depth = 0
        n = len(toks)
        while idx < n:
            t = toks[idx].text
            if t == "{":
                depth += 1
            elif t == "}":
                depth -= 1
                if depth == 0:
                    return idx + 1
            idx += 1
        return n

    for fp in sorted(side_ops_dir.rglob("*.rs")):
        rel_name = fp.name
        if (
            rel_name == "mod.rs"
            or rel_name.endswith(("_client.rs", "_server.rs", "_tests.rs", "_test.rs"))
            or "/tests/" in fp.as_posix()
        ):
            continue
        txt = fp.read_text(encoding="utf-8")
        try:
            items = gfs.parse_rust_items(txt)
        except ValueError:
            continue
        for it in items:
            if it.kind != "impl" or it.is_trait_impl or "Game" not in it.impl_header:
                continue
            for m in it.children:
                if m.kind not in ("fn", "method"):
                    continue
                dispatcher_fns.add(m.name)
                body = txt[m.start : m.end]
                toks = gfs.lex_rust(body)
                n = len(toks)
                # Locate `if self.is_logical_server() { ... } else { ... }` (or negated `!self.is_logical_server()`).
                for i in range(n):
                    if toks[i].kind == "ident" and toks[i].text == "if":
                        # Find first `{` after `if`
                        j = i + 1
                        while j < n and toks[j].text != "{":
                            j += 1
                        if j >= n:
                            break
                        cond = body[toks[i + 1].start : toks[j].start]
                        if "is_logical_server" not in cond:
                            continue
                        negated = cond.lstrip().startswith("!")
                        then_end = _skip_block(toks, j)
                        then_src = body[toks[j].start : toks[then_end - 1].end]
                        else_src = ""
                        if then_end < n and toks[then_end].kind == "ident" and toks[then_end].text == "else":
                            k = then_end + 1
                            if k < n and toks[k].text == "{":
                                else_end = _skip_block(toks, k)
                                else_src = body[toks[k].start : toks[else_end - 1].end]
                        srv_src, cli_src = (else_src, then_src) if negated else (then_src, else_src)
                        for cm in self_call_re.finditer(srv_src):
                            cname = cm.group(1)
                            if cname != "is_logical_server":
                                server_arms.add(cname)
                                server_arms_by_disp[m.name].add(cname)
                        for cm in self_call_re.finditer(cli_src):
                            cname = cm.group(1)
                            if cname != "is_logical_server":
                                client_arms.add(cname)
                                client_arms_by_disp[m.name].add(cname)
                        break

    return {
        "dispatcher_fns": dispatcher_fns,
        "server_arms": server_arms,
        "client_arms": client_arms,
        "server_arms_by_disp": server_arms_by_disp,
        "client_arms_by_disp": client_arms_by_disp,
    }


def collect_role_guarded_calls(
    root: Path,
) -> tuple[dict[tuple[str, str], set[str]], dict[tuple[str, str], set[str]]]:
    """Scan `impl Game` methods for strictly role-guarded `self.<callee>(` calls.

    A callee is recorded under `(rel_file, method_name)` ONLY when 100% of `self.<callee>(`
    occurrences in that method's body lie inside a matching `is_logical_server()` /
    `!is_logical_server()` branch, top-level unconditional early-return guard, or
    client-only `self.remote.is_some()` / `self.integrated.is_some()` guard.
    """
    srv_by_file_fn: dict[tuple[str, str], set[str]] = defaultdict(set)
    cli_by_file_fn: dict[tuple[str, str], set[str]] = defaultdict(set)
    game_dir = root / "crates/sunrise-client/src/game"
    files: list[Path] = sorted(game_dir.rglob("*.rs")) if game_dir.is_dir() else []
    game_rs = root / "crates/sunrise-client/src/game.rs"
    if game_rs.is_file():
        files.append(game_rs)

    self_call_re = re.compile(r"\bself\.([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:::<[^>]*>)?\s*\(")

    def _skip_block(toks: list[Any], idx: int) -> int:
        depth = 0
        n = len(toks)
        while idx < n:
            t = toks[idx].text
            if t == "{":
                depth += 1
            elif t == "}":
                depth -= 1
                if depth == 0:
                    return idx + 1
            idx += 1
        return n

    for fp in files:
        if fp.name.endswith(("_tests.rs", "_test.rs")) or "/tests/" in fp.as_posix():
            continue
        txt = fp.read_text(encoding="utf-8", errors="replace")
        if (
            "is_logical_server" not in txt
            and "self.remote.is_some" not in txt
            and "self.integrated.is_some" not in txt
        ):
            continue
        try:
            items = gfs.parse_rust_items(txt)
        except ValueError:
            continue
        rel = _rel_str(fp, root)
        for it in items:
            if it.kind != "impl" or it.is_trait_impl or "Game" not in it.impl_header:
                continue
            for m in it.children:
                if m.kind not in ("fn", "method"):
                    continue
                body = txt[m.start : m.end]
                if (
                    "is_logical_server" not in body
                    and "self.remote.is_some" not in body
                    and "self.integrated.is_some" not in body
                ):
                    continue
                toks = gfs.lex_rust(body)
                n = len(toks)
                b0 = 0
                while b0 < n and toks[b0].text != "{":
                    b0 += 1
                if b0 >= n:
                    continue
                srv_spans: list[tuple[int, int]] = []
                cli_spans: list[tuple[int, int]] = []
                depth = 0
                i = b0
                while i < n:
                    t = toks[i].text
                    if t == "{":
                        depth += 1
                        i += 1
                        continue
                    if t == "}":
                        depth -= 1
                        i += 1
                        continue
                    if toks[i].kind == "ident" and t == "if":
                        j = i + 1
                        while j < n and toks[j].text != "{":
                            j += 1
                        if j >= n:
                            break
                        cond = body[toks[i + 1].start : toks[j].start].strip()
                        then_end = _skip_block(toks, j)
                        t_s, t_e = toks[j].start, toks[then_end - 1].end
                        then_src = body[t_s:t_e]
                        has_else = False
                        e_s = e_e = 0
                        if then_end < n and toks[then_end].kind == "ident" and toks[then_end].text == "else":
                            k = then_end + 1
                            if k < n and toks[k].text == "{":
                                has_else = True
                                else_end = _skip_block(toks, k)
                                e_s, e_e = toks[k].start, toks[else_end - 1].end
                        cond_norm = re.sub(r"\s+", " ", cond)
                        is_or = "||" in cond_norm
                        has_pos_srv = bool(
                            re.search(r"(?<![!\w])self(?:\.options\.execution_role)?\.is_logical_server\(\)", cond_norm)
                        )
                        has_neg_srv = bool(
                            re.search(r"!self(?:\.options\.execution_role)?\.is_logical_server\(\)", cond_norm)
                        )
                        has_cli_opt = bool(
                            re.search(r"(?<![!\w])self\.(?:remote|integrated)\.is_some\(\)", cond_norm)
                        )
                        if not is_or and has_pos_srv and not has_neg_srv:
                            srv_spans.append((t_s, t_e))
                            if has_else and cond_norm in (
                                "self.is_logical_server()",
                                "self.options.execution_role.is_logical_server()",
                            ):
                                cli_spans.append((e_s, e_e))
                        elif not is_or and has_neg_srv and not has_pos_srv:
                            cli_spans.append((t_s, t_e))
                            if has_else and cond_norm in (
                                "!self.is_logical_server()",
                                "!self.options.execution_role.is_logical_server()",
                            ):
                                srv_spans.append((e_s, e_e))
                        if not is_or and has_cli_opt:
                            cli_spans.append((t_s, t_e))
                        if depth == 1 and not has_else and re.search(r"\breturn\b", then_src):
                            inner_toks = [x.text for x in toks[j + 1 : then_end - 1]]
                            if "if" not in inner_toks and "match" not in inner_toks and "return" in inner_toks:
                                r_s, r_e = t_e, len(body)
                                if has_neg_srv and not has_pos_srv and "&&" not in cond_norm:
                                    srv_spans.append((r_s, r_e))
                                elif has_pos_srv and not has_neg_srv and "&&" not in cond_norm:
                                    cli_spans.append((r_s, r_e))
                    i += 1
                if not srv_spans and not cli_spans:
                    continue
                occ_roles: dict[str, set[str]] = defaultdict(set)
                for cm in self_call_re.finditer(body):
                    cname = cm.group(1)
                    if cname == "is_logical_server":
                        continue
                    pos = cm.start()
                    in_s = any(a <= pos < b for a, b in srv_spans)
                    in_c = any(a <= pos < b for a, b in cli_spans)
                    if in_s and not in_c:
                        occ_roles[cname].add("server")
                    elif in_c and not in_s:
                        occ_roles[cname].add("client")
                    else:
                        occ_roles[cname].add("unguarded")
                for cname, roles in occ_roles.items():
                    if roles == {"server"}:
                        srv_by_file_fn[(rel, m.name)].add(cname)
                    elif roles == {"client"}:
                        cli_by_file_fn[(rel, m.name)].add(cname)

    return srv_by_file_fn, cli_by_file_fn


def _compute_live_fire_sides(root: Path) -> dict[str, str]:
    """Compute `dispatch_<hook> -> server|client|both` directly from `crates/sunrise-mod-vocab/src/hooks/*.rs`."""
    hooks_dir = root / "crates/sunrise-mod-vocab/src/hooks"
    if not hooks_dir.is_dir():
        return {}
    live: dict[str, str] = {}
    for p in sorted(hooks_dir.rglob("*.rs")):
        txt = p.read_text(encoding="utf-8", errors="replace")
        for m in re.finditer(
            r"#\[hook\((.*?)\)\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(",
            txt,
            re.S,
        ):
            attr, fn_name = m.group(1), m.group(2)
            m_side = re.search(r'\bside\s*=\s*"([^"]+)"', attr)
            side = m_side.group(1) if m_side else "both"
            m_recv = re.search(r'\breceiver\s*=\s*"([^"]+)"', attr)
            recv = m_recv.group(1) if m_recv else "write"
            m_host = re.search(r'\bhost\s*=\s*"([^"]+)"', attr)
            host = m_host.group(1) if m_host else None
            m_ctx = re.search(r'\bcontext\s*=\s*"([^"]+)"', attr)
            ctx = m_ctx.group(1) if m_ctx else None
            m_tgt = re.search(r'\btarget\s*=\s*"([^"]+)"', attr)
            tgt = m_tgt.group(1) if m_tgt else ""
            is_lvl = host in ("level", "level_random_side", "hit") or "::GameLevel<" in tgt or "::LevelView<" in tgt
            if ctx is not None:
                res = "client" if ctx in ("client_setup", "client_reload", "client_reload_listener", "client_identity") else "both"
            elif recv == "associated" or is_lvl:
                res = "both"
            elif host == "read" and side in ("server", "both"):
                res = "both"
            elif side == "server":
                res = "server"
            elif side == "client":
                res = "client"
            else:
                res = "both"
            live["dispatch_" + fn_name] = res
            if fn_name == "add_pack_finders":
                live["dispatch_add_pack_finders_for"] = res
    return live


def _parse_non_closure_unsizes(mir_path: Path, idx: Any) -> dict[str, set[tuple[str, str]]]:
    """Extract `enclosing_cid -> {(bare_self_ty, short_dyn_trait)}` for non-closure Unsize coercions."""
    out: dict[str, set[tuple[str, str]]] = defaultdict(set)
    cur_locals: dict[str, str] = {}
    cur_enc_cid = ""
    in_alloc = False
    with mir_path.open("r", encoding="utf-8", errors="replace") as fp:
        for line in fp:
            if not line or line == "\n":
                continue
            if in_alloc:
                if line.startswith("}"):
                    in_alloc = False
                continue
            if not line[0].isspace():
                if line.startswith("//") or line == "}\n":
                    cur_enc_cid = ""
                    continue
                if gsl.ALLOC_START_RE.match(line):
                    cur_enc_cid = ""
                    if line.rstrip().endswith("{"):
                        in_alloc = True
                    continue
                h = gsl._parse_top_fn_header(line)
                if h:
                    bi = idx.def_path_to_idx.get(h[0])
                    if bi is not None:
                        ei = idx.enclosing_idx[bi]
                        eb = idx.bodies[ei]
                        cur_enc_cid = idx._canonicalize_def_path(eb["def_path"], eb, fold_enclosing=True)
                    else:
                        cur_enc_cid = ""
                    cur_locals = {"_0": h[2]}
                    for part in gsl.split_top_level(h[1], ","):
                        if ":" in part:
                            a, t = part.split(":", 1)
                            cur_locals[a.strip()] = t.strip()
                else:
                    cur_enc_cid = ""
                continue
            if not cur_enc_cid:
                continue
            m_ld = gsl.LOCAL_DECL_RE.match(line)
            if m_ld:
                cur_locals[m_ld.group(1)] = m_ld.group(2).strip()
                continue
            if "PointerCoercion(Unsize" in line and "dyn " in line:
                stmt = line.split("//", 1)[0].strip()
                mu = UNSIZE_ANY_RE.search(stmt)
                if mu:
                    src_ty = cur_locals.get(mu.group(1).strip(), "")
                    if src_ty and "{closure@" not in src_ty:
                        src_bare = gsl.norm_full_type(src_ty)
                        dst_short = gsl.short_trait_key(gsl.normalize_trait_name(mu.group(2).strip()))
                        out[cur_enc_cid].add((src_bare, dst_short))
    return out


def build_mir_graph(
    root: Path,
    dump_paths: dict[str, Path],
    mod_sides: dict[str, str],
    field_owners: dict[str, dict[str, str]],
) -> dict[str, Any]:
    """Parse MIR dump(s) and return unified enclosing-function graph, field uses, and unresolved metadata."""
    host_env = gsl.current_host_cfg_env()
    sc = gsl.SourceCache(root)

    adj: dict[str, set[str]] = defaultdict(set)
    rev: dict[str, set[str]] = defaultdict(set)
    edge_loc: dict[tuple[str, str], str] = {}
    cid_meta: dict[str, dict[str, Any]] = {}
    cid_client_fields: dict[str, set[str]] = defaultdict(set)
    cid_server_fields: dict[str, set[str]] = defaultdict(set)
    cid_client_priv_fields: dict[str, set[str]] = defaultdict(set)
    cid_server_priv_fields: dict[str, set[str]] = defaultdict(set)
    unsizes_by_cid: dict[str, set[tuple[str, str]]] = defaultdict(set)
    cid_dyn_calls: dict[str, set[tuple[str, str]]] = defaultdict(set)
    cid_gen_calls: dict[str, set[tuple[str, str]]] = defaultdict(set)
    cid_has_fnptr_call: set[str] = set()
    all_reified_targets: set[str] = set()
    trait_impls_by_self_trait: dict[tuple[str, str], set[str]] = defaultdict(set)
    trait_impls_by_trait_meth: dict[tuple[str, str], set[str]] = defaultdict(set)

    for cfg_name, mir_path in dump_paths.items():
        parsed = gsl.parse_mir_file(mir_path, root, sc)
        _, c_fields, s_fields, wrapped_by_gidx = gsl.verify_struct_fields(
            root, cfg_name, parsed["ctor_aggregates"], field_owners, host_env
        )
        idx = gsl.MirIndex(parsed["bodies"], sc, mod_sides)
        bodies = idx.bodies

        for u_cid, pairs in _parse_non_closure_unsizes(mir_path, idx).items():
            unsizes_by_cid[u_cid] |= pairs

        for bi, b in enumerate(bodies):
            ei = idx.enclosing_idx[bi]
            eb = bodies[ei]
            src_cid = idx._canonicalize_def_path(eb["def_path"], eb, fold_enclosing=True)
            if src_cid not in cid_meta:
                lbl = idx.body_label(eb["def_path"])
                cid_meta[src_cid] = {
                    "def_path": eb["def_path"],
                    "label": lbl,  # client | server | common
                    "source_file": eb["source_file"],
                    "line_start": eb["line_start"],
                    "line_end": eb["line_end"],
                    "kind": eb["kind"],
                    "impl_info": eb["impl_info"],
                    "configs": set(),
                }
            cid_meta[src_cid]["configs"].add(cfg_name)

            if b["kind"] == "method" and b["impl_info"] and b["impl_info"].get("kind") in ("trait", "derive"):
                tr_short = b.get("resolved_trait_short", "")
                self_bare = b.get("resolved_self_ty", "")
                meth = b["def_path"].rsplit(">::", 1)[-1]
                if tr_short:
                    trait_impls_by_self_trait[(self_bare, tr_short)].add(src_cid)
                    trait_impls_by_trait_meth[(tr_short, meth)].add(src_cid)

            projs = b["projections"]
            for ip, (bt, fidx, fty, _mode, is_deref, is_drop, _loc) in enumerate(projs):
                btc = gsl.strip_refs_and_generics(bt)
                fty_bare = gsl.strip_refs_and_generics(fty)
                if btc == "game::client_side::ClientState" and fidx < len(c_fields):
                    fname = c_fields[fidx][0]
                    cid_client_fields[src_cid].add(f"ClientState.{fname}")
                    if field_owners.get(fname, {}).get("visibility", "side") == "side":
                        cid_client_priv_fields[src_cid].add(f"ClientState.{fname}")
                elif btc == "game::server_side::ServerState" and fidx < len(s_fields):
                    fname = s_fields[fidx][0]
                    cid_server_fields[src_cid].add(f"ServerState.{fname}")
                    if field_owners.get(fname, {}).get("visibility", "side") == "side":
                        cid_server_priv_fields[src_cid].add(f"ServerState.{fname}")
                elif btc == "game::Game" and fty_bare in (
                    "game::client_side::ClientState",
                    "game::server_side::ServerState",
                ):
                    has_sub = ip + 1 < len(projs) and gsl.strip_refs_and_generics(projs[ip + 1][0]) == fty_bare
                    if not has_sub and (is_deref or not is_drop):
                        if fty_bare.endswith("::ClientState"):
                            cid_client_fields[src_cid].add("Game.client")
                        else:
                            cid_server_fields[src_cid].add("Game.server")
                elif btc == "game::Game" and fidx in wrapped_by_gidx:
                    fname, wside, wkind = wrapped_by_gidx[fidx]
                    has_inner = (
                        ip + 1 < len(projs)
                        and gsl.strip_refs_and_generics(projs[ip + 1][0]) == fty_bare
                        and projs[ip + 1][1] == 0
                    )
                    if has_inner or (not is_drop) or is_deref:
                        if wside == "client":
                            cid_client_fields[src_cid].add(f"Game.{fname}")
                            if wkind == "client_only":
                                cid_client_priv_fields[src_cid].add(f"Game.{fname}")
                        else:
                            cid_server_fields[src_cid].add(f"Game.{fname}")
                            if wkind == "server_only":
                                cid_server_priv_fields[src_cid].add(f"Game.{fname}")

            for c in b["calls"]:
                ck = c["kind"]
                callee = c["callee"]
                if ck == "static":
                    st, targets = idx.resolve_static_call(callee)
                    if st == "local":
                        for tidx in targets:
                            te = idx.enclosing_idx[tidx]
                            tb = bodies[te]
                            dst_cid = idx._canonicalize_def_path(tb["def_path"], tb, fold_enclosing=True)
                            if dst_cid != src_cid:
                                adj[src_cid].add(dst_cid)
                                rev[dst_cid].add(src_cid)
                                edge_loc.setdefault((src_cid, dst_cid), c["loc"])
                    elif st == "dyn_fn_box":
                        _, tr_ty, meth, _ = gsl.split_qualified_path(callee)
                        if tr_ty:
                            cid_dyn_calls[src_cid].add((gsl.short_trait_key(gsl.normalize_trait_name(tr_ty)), meth))
                    elif st == "generic_param":
                        _, tr_ty, meth, _ = gsl.split_qualified_path(callee)
                        if tr_ty:
                            cid_gen_calls[src_cid].add((gsl.short_trait_key(gsl.normalize_trait_name(tr_ty)), meth))
                elif ck == "dyn":
                    self_ty, tr_ty, meth, _ = gsl.split_qualified_path(callee)
                    tr = tr_ty or self_ty or ""
                    if tr:
                        cid_dyn_calls[src_cid].add((gsl.short_trait_key(gsl.normalize_trait_name(tr)), meth))
                elif ck == "fn_ptr":
                    cid_has_fnptr_call.add(src_cid)

            for fn_path, loc in b["reified_fn_ptrs"]:
                st, targets = idx.resolve_static_call(fn_path)
                if st == "local":
                    for tidx in targets:
                        te = idx.enclosing_idx[tidx]
                        tb = bodies[te]
                        dst_cid = idx._canonicalize_def_path(tb["def_path"], tb, fold_enclosing=True)
                        all_reified_targets.add(dst_cid)
                        if dst_cid != src_cid:
                            adj[src_cid].add(dst_cid)
                            rev[dst_cid].add(src_cid)
                            edge_loc.setdefault((src_cid, dst_cid), f"reify:{loc}")

            for fn_path, loc in b["fn_items_named"]:
                st, targets = idx.resolve_static_call(fn_path)
                if st == "local":
                    for tidx in targets:
                        te = idx.enclosing_idx[tidx]
                        tb = bodies[te]
                        dst_cid = idx._canonicalize_def_path(tb["def_path"], tb, fold_enclosing=True)
                        if dst_cid != src_cid:
                            adj[src_cid].add(dst_cid)
                            rev[dst_cid].add(src_cid)
                            edge_loc.setdefault((src_cid, dst_cid), f"fn_item:{loc}")

            for dty, loc in b["drops"]:
                dnorm = gsl.norm_full_type(dty)
                for tidx in idx.drop_impls_by_ty.get(dnorm, ()):
                    te = idx.enclosing_idx[tidx]
                    tb = bodies[te]
                    dst_cid = idx._canonicalize_def_path(tb["def_path"], tb, fold_enclosing=True)
                    if dst_cid != src_cid:
                        adj[src_cid].add(dst_cid)
                        rev[dst_cid].add(src_cid)
                        edge_loc.setdefault((src_cid, dst_cid), f"drop:{loc}")

    return {
        "adj": adj,
        "rev": rev,
        "edge_loc": edge_loc,
        "cid_meta": cid_meta,
        "cid_client_fields": cid_client_fields,
        "cid_server_fields": cid_server_fields,
        "cid_client_priv_fields": cid_client_priv_fields,
        "cid_server_priv_fields": cid_server_priv_fields,
        "unsizes_by_cid": unsizes_by_cid,
        "cid_dyn_calls": cid_dyn_calls,
        "cid_gen_calls": cid_gen_calls,
        "cid_has_fnptr_call": cid_has_fnptr_call,
        "all_reified_targets": all_reified_targets,
        "trait_impls_by_self_trait": trait_impls_by_self_trait,
        "trait_impls_by_trait_meth": trait_impls_by_trait_meth,
    }


def _resolve_root_or_cut(cid_meta: dict[str, Any], candidates: set[str] | frozenset[str]) -> set[str]:
    """Match root/cut IDs even if modules moved between `game::`, `client_side::`, and `server_side::`."""
    present = {c for c in candidates if c in cid_meta}
    missing = candidates - present
    if not missing:
        return present
    by_norm: dict[str, list[str]] = defaultdict(list)
    for c in cid_meta:
        norm = c.replace("::client_side::", "::").replace("::server_side::", "::")
        norm = re.sub(r"^(game::[A-Za-z0-9_]+)_(?:client|server)::", r"\1::", norm)
        by_norm[norm].append(c)
    for m in missing:
        norm = m.replace("::client_side::", "::").replace("::server_side::", "::")
        norm = re.sub(r"^(game::[A-Za-z0-9_]+)_(?:client|server)::", r"\1::", norm)
        present.update(by_norm.get(norm, ()))
    return present


def compute_role_reachability(
    graph: dict[str, Any],
    *,
    fire_sides_path: Path | None = None,
    root: Path | None = None,
    mod_sides: dict[str, str] | None = None,
    side_ops_spec: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Compute role reachability across baseline and sensitivity modes."""
    cid_meta = graph["cid_meta"]
    adj = graph["adj"]
    rev = graph["rev"]
    unsizes_by_cid = graph["unsizes_by_cid"]
    cid_dyn_calls = graph["cid_dyn_calls"]
    cid_gen_calls = graph["cid_gen_calls"]
    cid_has_fnptr_call = graph["cid_has_fnptr_call"]
    all_reified_targets = graph["all_reified_targets"]
    trait_impls_by_self_trait = graph["trait_impls_by_self_trait"]
    trait_impls_by_trait_meth = graph["trait_impls_by_trait_meth"]

    reg_mods = set(mod_sides) if mod_sides else set()
    if side_ops_spec is None:
        side_ops_spec = collect_side_ops_spec(root) if root is not None else {
            "dispatcher_fns": set(),
            "server_arms": set(),
            "client_arms": set(),
            "server_arms_by_disp": {},
            "client_arms_by_disp": {},
        }

    game_nodes = {c for c in cid_meta if c.startswith("game::")}
    client_core = _resolve_root_or_cut(cid_meta, CLIENT_CORE_ROOTS)
    server_core = _resolve_root_or_cut(cid_meta, SERVER_CORE_ROOTS)
    startup_client = _resolve_root_or_cut(cid_meta, STARTUP_CLIENT_ROOTS)
    launch_cuts = _resolve_root_or_cut(cid_meta, LAUNCH_CUT_NODES)
    fixture_cuts = {c for c in game_nodes if any(c.endswith(t) for t in FIXTURE_CUT_TAILS)}
    component_server = {c for c in game_nodes if c.endswith(("::dispense_fixture", "::sculk_fixture", "::hot_paths"))}

    # Explicit role-dispatch cuts (`match (self.is_logical_server(), ...)` / `if self.is_logical_server()`).
    role_edges_cut_client: set[tuple[str, str]] = set()
    role_edges_cut_server: set[tuple[str, str]] = set()
    tick_body_ids = _resolve_root_or_cut(cid_meta, {"game::<impl game::Game>::tick_body"})
    tick_srv_first_ids = _resolve_root_or_cut(cid_meta, {"game::server_side::levels::<impl game::Game>::tick_server_first"})
    start_game_ids = _resolve_root_or_cut(
        cid_meta,
        {"game::client_side::loading_window::start_game", "game::client_side::loading_window::start_client_game"},
    )
    run_ded_ids = _resolve_root_or_cut(
        cid_meta,
        {"game::server_side::dedicated::<impl game::Game>::run_dedicated", "game::client_side::loading_window::start_dedicated_game"},
    )
    spawn_player_ids = _resolve_root_or_cut(cid_meta, {"game::server_side::player_spawn::<impl game::Game>::spawn_player"})
    for u in tick_body_ids:
        for v in tick_srv_first_ids:
            role_edges_cut_client.add((u, v))
    for u in start_game_ids:
        for v in run_ded_ids:
            role_edges_cut_client.add((u, v))
    for sp in spawn_player_ids:
        for u in rev.get(sp, ()):
            if "game_client" in u or u.endswith("::redraw"):
                role_edges_cut_client.add((u, sp))

    # Cut `game::side_ops::*` role-dispatcher arms by role so `side_<op>` never leaks across roles.
    srv_arms_by_disp = side_ops_spec.get("server_arms_by_disp", {})
    cli_arms_by_disp = side_ops_spec.get("client_arms_by_disp", {})
    all_srv_arms = side_ops_spec.get("server_arms", set())
    all_cli_arms = side_ops_spec.get("client_arms", set())
    for u in game_nodes:
        if _top_module_from_cid(u, reg_mods) != "side_ops":
            continue
        u_tail = u.rsplit("::", 1)[-1]
        u_srv = srv_arms_by_disp.get(u_tail, all_srv_arms)
        u_cli = cli_arms_by_disp.get(u_tail, all_cli_arms)
        for v in adj.get(u, ()):
            v_tail = v.rsplit("::", 1)[-1]
            if v_tail in u_srv:
                role_edges_cut_client.add((u, v))
            if v_tail in u_cli:
                role_edges_cut_server.add((u, v))

    # Cut strictly role-guarded `self.<callee>(` edges (`is_logical_server()` / `remote.is_some()` / `integrated.is_some()`).
    if root is not None:
        guarded_srv, guarded_cli = collect_role_guarded_calls(root)
        for u in game_nodes:
            u_key = (cid_meta[u]["source_file"], u.rsplit("::", 1)[-1])
            u_srv_g = guarded_srv.get(u_key)
            u_cli_g = guarded_cli.get(u_key)
            if not u_srv_g and not u_cli_g:
                continue
            for v in adj.get(u, ()):
                v_tail = v.rsplit("::", 1)[-1]
                if u_srv_g and v_tail in u_srv_g:
                    role_edges_cut_client.add((u, v))
                if u_cli_g and v_tail in u_cli_g:
                    role_edges_cut_server.add((u, v))

    poll_remote_cut_server: set[tuple[str, str]] = set()
    for u in _resolve_root_or_cut(
        cid_meta,
        {
            "game::client_side::world_update::<impl game::Game>::update_world",
            "game::client_side::world_update::<impl game::Game>::client_update_world",
            "game::server_side::world_update_server::<impl game::Game>::server_update_world",
        },
    ):
        for v in _resolve_root_or_cut(cid_meta, {"game::remote::<impl game::Game>::poll_remote"}):
            poll_remote_cut_server.add((u, v))

    fire_c_roots: set[str] = set()
    fire_s_roots: set[str] = set()
    fire_sides: dict[str, str] = {}
    if fire_sides_path is not None and fire_sides_path.is_file():
        fire_sides = json.loads(fire_sides_path.read_text(encoding="utf-8"))
    elif root is not None:
        fire_sides = _compute_live_fire_sides(root)
    if fire_sides:
        for c, meta in cid_meta.items():
            if "mod_api_fire_generated.rs" in meta["source_file"]:
                fn_name = c.rsplit("::", 1)[-1]
                fs = fire_sides.get(fn_name) or fire_sides.get(f"dispatch_{fn_name}")
                if fs is None and fn_name.startswith("fire_"):
                    fs = fire_sides.get(f"dispatch_{fn_name[5:]}")
                if fs == "client":
                    fire_c_roots.add(c)
                elif fs == "server":
                    fire_s_roots.add(c)
                elif fs == "both":
                    fire_c_roots.add(c)
                    fire_s_roots.add(c)

    def run_bfs(
        roots: set[str],
        cut_nodes: set[str],
        cut_edges: set[tuple[str, str]],
        *,
        expand_unsize: bool = False,
        expand_dyn: bool = False,
        expand_gen: bool = False,
        expand_fnptr: bool = False,
    ) -> set[str]:
        vis: set[str] = set()
        q: deque[str] = deque()
        for r in roots:
            if r in cid_meta and r not in vis:
                vis.add(r)
                q.append(r)
        while q:
            u = q.popleft()
            nexts = set(adj.get(u, ()))
            if expand_unsize:
                for self_bare, tr_short in unsizes_by_cid.get(u, ()):
                    nexts |= trait_impls_by_self_trait.get((self_bare, tr_short), set())
            if expand_dyn:
                for tr_short, meth in cid_dyn_calls.get(u, ()):
                    nexts |= trait_impls_by_trait_meth.get((tr_short, meth), set())
            if expand_gen:
                for tr_short, meth in cid_gen_calls.get(u, ()):
                    nexts |= trait_impls_by_trait_meth.get((tr_short, meth), set())
            if expand_fnptr and u in cid_has_fnptr_call:
                nexts |= all_reified_targets
            for v in nexts:
                if v in cut_nodes or (u, v) in cut_edges:
                    continue
                if v not in vis:
                    vis.add(v)
                    q.append(v)
        return vis

    c_cut = launch_cuts | fixture_cuts
    s_cut = fixture_cuts

    modes: dict[str, tuple[set[str], set[str]]] = {
        "S0_static": (
            run_bfs(client_core, c_cut, set()),
            run_bfs(server_core, s_cut, set()),
        ),
        "S1_role_branch": (
            run_bfs(client_core, c_cut, role_edges_cut_client),
            run_bfs(server_core, s_cut, role_edges_cut_server),
        ),
        "S2_poll_remote_cut": (
            run_bfs(client_core, c_cut, role_edges_cut_client),
            run_bfs(server_core, s_cut, role_edges_cut_server | poll_remote_cut_server),
        ),
        "S3_startup_harness": (
            run_bfs(client_core | startup_client, c_cut, role_edges_cut_client),
            run_bfs(server_core | component_server, fixture_cuts - component_server, role_edges_cut_server),
        ),
        "S4a_unsize": (
            run_bfs(client_core, c_cut, role_edges_cut_client, expand_unsize=True),
            run_bfs(server_core, s_cut, role_edges_cut_server, expand_unsize=True),
        ),
        "S4b_dyn": (
            run_bfs(client_core, c_cut, role_edges_cut_client, expand_dyn=True),
            run_bfs(server_core, s_cut, role_edges_cut_server, expand_dyn=True),
        ),
        "S4c_generic": (
            run_bfs(client_core, c_cut, role_edges_cut_client, expand_gen=True),
            run_bfs(server_core, s_cut, role_edges_cut_server, expand_gen=True),
        ),
        "S4d_hooks": (
            run_bfs(client_core | fire_c_roots, c_cut, role_edges_cut_client),
            run_bfs(server_core | fire_s_roots, s_cut, role_edges_cut_server),
        ),
        "S4_all_unresolved": (
            run_bfs(
                client_core | fire_c_roots,
                c_cut,
                role_edges_cut_client,
                expand_unsize=True,
                expand_dyn=True,
                expand_gen=True,
                expand_fnptr=True,
            ),
            run_bfs(
                server_core | fire_s_roots,
                s_cut,
                role_edges_cut_server,
                expand_unsize=True,
                expand_dyn=True,
                expand_gen=True,
                expand_fnptr=True,
            ),
        ),
    }

    summary: dict[str, dict[str, int]] = {}
    for mname, (cv, sv) in modes.items():
        cg = cv & game_nodes
        sg = sv & game_nodes
        summary[mname] = {
            "client_only": len(cg - sg),
            "server_only": len(sg - cg),
            "both": len(cg & sg),
            "neither": len(game_nodes - (cg | sg)),
            "client_total": len(cg),
            "server_total": len(sg),
        }

    return {
        "game_nodes": game_nodes,
        "modes": modes,
        "summary": summary,
        "client_core": client_core,
        "server_core": server_core,
        "launch_cuts": launch_cuts,
        "fixture_cuts": fixture_cuts,
        "role_edges_cut_client": role_edges_cut_client,
        "role_edges_cut_server": role_edges_cut_server,
        "side_ops_spec": side_ops_spec,
        "fire_c_roots": fire_c_roots,
        "fire_s_roots": fire_s_roots,
    }


def classify_unreachable(cid: str, meta: dict[str, Any]) -> str:
    sf = meta["source_file"]
    if "OUT_DIR" in sf or "generated" in sf:
        return "generated"
    if any(k in sf for k in ("fixture", "harness", "hot_paths", "parity_fixtures")):
        return "fixture_harness"
    if "_tests" in sf or "/tests" in sf or "::tests::" in cid:
        return "test"
    if meta["kind"] in ("const", "static", "promoted"):
        return "const_static"
    ii = meta.get("impl_info")
    if ii and ii.get("kind") == "derive":
        return "derive"
    if ii and ii.get("kind") == "trait":
        tr = (ii.get("trait") or "").rsplit("::", 1)[-1].split("<", 1)[0].strip()
        if tr in DERIVE_TRAITS:
            return "std_serde_trait"
        return "trait_impl"
    return "unreferenced_fn"


def compute_labels_and_contradictions(
    graph: dict[str, Any],
    reach_res: dict[str, Any],
    *,
    primary_mode: str = "S1_role_branch",
) -> dict[str, Any]:
    """Classify every `game::` node into target label and record field + callee contradictions."""
    cid_meta = graph["cid_meta"]
    adj = graph["adj"]
    cid_cf = graph["cid_client_fields"]
    cid_sf = graph["cid_server_fields"]
    game_nodes = reach_res["game_nodes"]
    cv, sv = reach_res["modes"][primary_mode]
    cg = cv & game_nodes
    sg = sv & game_nodes

    reach_of: dict[str, str] = {}
    for n in game_nodes:
        in_c, in_s = n in cg, n in sg
        reach_of[n] = "both" if (in_c and in_s) else ("client_only" if in_c else ("server_only" if in_s else "neither"))

    # First pass: direct field contradictions & target label per node.
    target_label: dict[str, str] = {}
    node_records: dict[str, dict[str, Any]] = {}
    contradictions: list[dict[str, Any]] = []

    for n in sorted(game_nodes):
        m = cid_meta[n]
        r = reach_of[n]
        cf = sorted(cid_cf.get(n, ()))
        sf = sorted(cid_sf.get(n, ()))
        cur = m["label"]  # client | server | common

        if r == "neither":
            tgt = f"neither:{classify_unreachable(n, m)}"
        elif r == "client_only":
            tgt = "client" if not sf else "contradiction:client_only_uses_server_state"
        elif r == "server_only":
            tgt = "server" if not cf else "contradiction:server_only_uses_client_state"
        else:
            tgt = "common"

        target_label[n] = tgt
        node_records[n] = {
            "cid": n,
            "current_side": cur,
            "reach": r,
            "target_label": tgt,
            "client_fields": cf,
            "server_fields": sf,
            "source_file": m["source_file"],
            "line_start": m["line_start"],
            "kind": m["kind"],
        }

    # Second pass: compute contradictions including BOTH direct side fields AND direct side-contradicting callees.
    role_dispatched_edges = reach_res.get("role_edges_cut_client", set()) | reach_res.get("role_edges_cut_server", set())
    for n in sorted(game_nodes):
        rec = node_records[n]
        r = rec["reach"]
        if r == "neither":
            continue
        cf = rec["client_fields"]
        sf = rec["server_fields"]

        bad_client_callees: list[str] = []
        bad_server_callees: list[str] = []
        for g in sorted(adj.get(n, ())):
            if g not in node_records or (n, g) in role_dispatched_edges:
                continue
            gr = node_records[g]
            g_has_c = bool(gr["client_fields"])
            g_has_s = bool(gr["server_fields"])
            g_reach = gr["reach"]
            # A callee is a client thing if it touches client fields or is client_only.
            if r == "server_only" and (g_has_c or g_reach == "client_only"):
                bad_client_callees.append(g)
            elif r == "client_only" and (g_has_s or g_reach == "server_only"):
                bad_server_callees.append(g)
            elif r == "both":
                if g_has_c or g_reach == "client_only":
                    bad_client_callees.append(g)
                if g_has_s or g_reach == "server_only":
                    bad_server_callees.append(g)

        rec["bad_client_callees"] = bad_client_callees
        rec["bad_server_callees"] = bad_server_callees

        ckind = ""
        if r == "server_only" and (cf or bad_client_callees):
            ckind = "server_only_uses_client"
        elif r == "client_only" and (sf or bad_server_callees):
            ckind = "client_only_uses_server"
        elif r == "both" and (cf or sf or bad_client_callees or bad_server_callees):
            has_c = bool(cf or bad_client_callees)
            has_s = bool(sf or bad_server_callees)
            if has_c and has_s:
                ckind = "both_uses_both"
            elif has_c:
                ckind = "both_uses_client"
            else:
                ckind = "both_uses_server"

        if ckind:
            direct_field_flag = bool(
                (r == "server_only" and cf)
                or (r == "client_only" and sf)
                or (r == "both" and (cf or sf))
            )
            contradictions.append(
                {
                    "cid": n,
                    "current_side": rec["current_side"],
                    "reach": r,
                    "contradiction_kind": ckind,
                    "has_direct_field": "yes" if direct_field_flag else "no",
                    "source_file": rec["source_file"],
                    "line_start": rec["line_start"],
                    "client_fields": cf if r in ("server_only", "both") else [],
                    "server_fields": sf if r in ("client_only", "both") else [],
                    "client_callees": bad_client_callees,
                    "server_callees": bad_server_callees,
                }
            )

    return {
        "primary_mode": primary_mode,
        "reach_of": reach_of,
        "target_label": target_label,
        "node_records": node_records,
        "contradictions": contradictions,
    }


def index_source_functions(
    root: Path, mod_sides: dict[str, str]
) -> tuple[
    dict[str, tuple[str, Path, tuple[str, ...]]],
    dict[str, dict[str, tuple[Path, tuple[str, ...]]]],
    dict[str, dict[Any, tuple[str, Path, Any, Any | None]]],
    dict[tuple[str, str], list[tuple[str, str, Any, str, int, int, bool, bool]]],
    dict[str, list[tuple[str, str, Any, str, int, int, bool, bool]]],
]:
    """Index all source `FnKey` items across `game.rs`, regular modules, and mirrors."""
    errs: list[str] = []
    regular_mods, existing_mirrors = gfs.parse_top_mod_decls(root, set(mod_sides), errors=errs)
    by_top_fnkey: dict[str, dict[Any, tuple[str, Path, Any, Any | None]]] = defaultdict(dict)
    by_mod_name: dict[tuple[str, str], list[tuple[str, str, Any, str, int, int, bool, bool]]] = defaultdict(list)
    by_file: dict[str, list[tuple[str, str, Any, str, int, int, bool, bool]]] = defaultdict(list)

    def _index(top_name: str, loc_side: str, tree: Any) -> None:
        for k, (_node, fpath, _fsrc, it, pimpl, _fcfgs) in gfs.collect_tree_functions(tree).items():
            rel = _rel_str(fpath, root)
            is_trait = pimpl is not None and pimpl.is_trait_impl
            rec = (top_name, loc_side, k, rel, it.start_line, it.end_line, is_trait, it.is_test_attr)
            by_top_fnkey[top_name][k] = (loc_side, fpath, it, pimpl)
            by_mod_name[(k.module, k.name)].append(rec)
            by_file[rel].append(rec)

    game_rs = (root / "crates/sunrise-client/src/game.rs").resolve()
    if game_rs.is_file():
        _index("game", "game", gfs.build_mod_tree("game", game_rs))
    for mname, (pside, entry, cfg) in regular_mods.items():
        if entry.is_file():
            _index(mname, pside, gfs.build_mod_tree(mname, entry, cfg))
    for side in ("client", "server"):
        for mname, (mentry, mcfg) in existing_mirrors[side].items():
            top = mname[: -len(f"_{side}")]
            if mentry.is_file():
                _index(top, f"mirror_{side}", gfs.build_mod_tree(top, mentry, mcfg, is_mirror=True))

    for rel in by_file:
        by_file[rel].sort(key=lambda r: (r[4], r[5]))
    return regular_mods, existing_mirrors, by_top_fnkey, by_mod_name, by_file


def map_cid_to_source_fn(
    cid: str,
    meta: dict[str, Any],
    by_file: dict[str, list[tuple[str, str, Any, str, int, int, bool, bool]]],
    by_mod_name: dict[tuple[str, str], list[tuple[str, str, Any, str, int, int, bool, bool]]],
    regular_mod_names: set[str] | frozenset[str] | None = None,
) -> tuple[str, tuple[str, str, Any, str, int, int, bool, bool] | None]:
    """Match a MIR canonical ID to `(status, source_record_or_None)`."""
    sf = meta["source_file"]
    ln = meta["line_start"]
    ii = meta.get("impl_info")
    if meta["kind"] in ("const", "static", "promoted"):
        return ("const_static", None)
    if ii and ii.get("kind") == "derive":
        return ("derive", None)
    if "OUT_DIR" in sf or "generated" in sf:
        return ("generated", None)

    fn_tail = cid.rsplit("::", 1)[-1]
    # 1. Exact file + span match.
    for rec in by_file.get(sf, ()):
        if rec[4] <= ln <= rec[5] and rec[2].name == fn_tail:
            return ("matched", rec)
    for rec in by_file.get(sf, ()):
        if abs(rec[4] - ln) <= 5 and rec[2].name == fn_tail:
            return ("matched", rec)

    # 2. Macro-expanded first statement (`${RUST_SRC}`, `${CARGO_REG}`, or another file): match by (orig_mod, name).
    orig_mod = _canonical_orig_module_from_cid(cid, regular_mod_names)
    cands = by_mod_name.get((orig_mod, fn_tail), ())
    if len(cands) == 1:
        return ("matched", cands[0])
    if len(cands) > 1:
        # Disambiguate by impl kind (free vs inherent vs trait).
        is_method = "::<impl " in cid
        is_trait_cid = bool(ii and ii.get("kind") == "trait")
        filt = [
            r
            for r in cands
            if (r[2].impl != "-") == is_method and (r[6] == is_trait_cid)
        ]
        if len(filt) == 1:
            return ("matched", filt[0])
    # 3. Nested inner function inside an enclosing source function (`a::outer::inner`).
    if sf in by_file:
        enclosing = [r for r in by_file[sf] if r[4] <= ln <= r[5]]
        if enclosing:
            return ("nested_inner_fn", enclosing[-1])
    return ("unmatched_macro_or_trait_default", None)


def _check_root_super_super(root: Path, entry_files: list[Path]) -> bool:
    """Return True if any root-depth entry file uses `super::super` outside nested `mod { ... }`."""
    for fp in entry_files:
        if not fp.is_file():
            continue
        txt = fp.read_text(encoding="utf-8")
        if "super::super" not in txt:
            continue
        toks = gfs.lex_rust(txt)
        depth = 0
        mod_depths: list[int] = []
        pending_mod = False
        n = len(toks)
        for i, t in enumerate(toks):
            if t.text == "{":
                depth += 1
                if pending_mod:
                    mod_depths.append(depth)
                    pending_mod = False
            elif t.text == "}":
                if mod_depths and mod_depths[-1] == depth:
                    mod_depths.pop()
                depth = max(0, depth - 1)
            elif t.text == ";" and pending_mod:
                pending_mod = False
            elif t.kind == "ident" and t.text == "mod" and (i == 0 or toks[i - 1].text not in ("::", ".")):
                pending_mod = True
            elif (
                not mod_depths
                and t.kind == "ident"
                and t.text == "super"
                and i + 2 < n
                and toks[i + 1].text == "::"
                and toks[i + 2].kind == "ident"
                and toks[i + 2].text == "super"
            ):
                return True
    return False


def _precheck_fn_move_constraints(
    root: Path,
    top_name: str,
    regular_mods: dict[str, tuple[str, Path, tuple[str, ...]]],
    existing_mirrors: dict[str, dict[str, tuple[Path, tuple[str, ...]]]],
    desired_side: dict[Any, str],
    game_root_tree: Any | None = None,
) -> set[Any]:
    """Return set of `FnKey`s in `top_name` that fail `game_fn_sides` mechanical move constraints."""
    bad: set[Any] = set()
    resolved = gfs.resolve_top_entry(root, top_name, regular_mods)
    if resolved is None:
        return set(desired_side)
    _pside, entry_file, top_cfg = resolved
    errs: list[str] = []
    orig_tree = gfs.build_mod_tree(top_name, entry_file, top_cfg, errors=errs, root=root)
    orig_fns = gfs.collect_tree_functions(orig_tree, dup_errors=errs, root=root)
    mirror_trees: dict[str, Any] = {"client": None, "server": None}
    mirror_fns: dict[str, dict[Any, Any]] = {"client": {}, "server": {}}
    for side in ("client", "server"):
        mname = f"{top_name}_{side}"
        if mname in existing_mirrors[side]:
            mentry, mcfg = existing_mirrors[side][mname]
            if mentry.is_file():
                mt = gfs.build_mod_tree(top_name, mentry, mcfg, errors=errs, root=root, is_mirror=True)
                mirror_trees[side] = mt
                mirror_fns[side] = gfs.collect_tree_functions(mt, dup_errors=errs, root=root)

    if errs:
        return set(desired_side)

    macro_trees = [orig_tree, *(t for t in mirror_trees.values() if t is not None)]
    if top_name != "game":
        if game_root_tree is not None:
            macro_trees.append(game_root_tree)
        else:
            game_rs = (root / "crates/sunrise-client/src/game.rs").resolve()
            if game_rs.is_file():
                macro_trees.append(gfs.build_mod_tree("game", game_rs))
    sensitive_macros = gfs.collect_location_sensitive_macros(macro_trees)

    orig_node_by_nid: dict[tuple[str, tuple[str, ...]], Any] = {}

    def _idx(n: Any) -> None:
        orig_node_by_nid[(n.orig_mod, gfs.node_cfg_chain(n))] = n
        for ch in n.children:
            _idx(ch)

    _idx(orig_tree)

    all_entries: dict[Any, tuple[Any, Path, str, Any, Any | None, tuple[str, ...]]] = {}
    for d in (orig_fns, mirror_fns["client"], mirror_fns["server"]):
        all_entries.update(d)

    # Check overlaps & cfg_attr per fn
    by_overlap: dict[tuple[str, str, str], list[Any]] = defaultdict(list)
    trait_groups: dict[tuple[str, tuple[str, ...], str, tuple[str, ...]], list[Any]] = defaultdict(list)
    for k, (node, fp, fsrc, it, pimpl, fcfgs) in all_entries.items():
        is_trait = pimpl is not None and pimpl.is_trait_impl
        grp = (k.module, gfs.owner_overlap_group(k.impl, is_trait), k.name)
        by_overlap[grp].append(k)
        if is_trait and pimpl is not None:
            nid = (node.orig_mod, gfs.node_cfg_chain(node))
            eff_iattrs = gfs.effective_attrs(pimpl.norm_attrs, fcfgs)
            trait_groups[(nid[0], nid[1], pimpl.impl_header, eff_iattrs)].append(k)

    # Collect private `const`/`static` (including `thread_local! { static NAME: ... }`) in `orig_tree`:
    # moving a function referencing a private static/const into a sibling mirror file breaks resolution (`E0425`).
    private_val_names: dict[tuple[str, tuple[str, ...]], set[str]] = defaultdict(set)
    tl_static_re = re.compile(r"\bstatic\s+([A-Z_][A-Z0-9_]*)\s*:")

    def _collect_priv_vals(n: Any) -> None:
        nid = (n.orig_mod, gfs.node_cfg_chain(n))
        for _fp, fsrc, fitems, _fcfgs in gfs.node_all_fragments(n):
            for it in fitems:
                if it.kind in ("const", "static") and re.sub(r"\s+", "", it.vis) in ("", "pub(self)"):
                    private_val_names[nid].add(it.name)
                elif it.kind == "macro_invoc" and it.name.rsplit("::", 1)[-1] == "thread_local":
                    txt_m = fsrc[it.start : it.end]
                    for m in tl_static_re.finditer(txt_m):
                        # Only non-pub thread_local statics
                        before = txt_m[max(0, m.start() - 32) : m.start()]
                        if "pub" not in before:
                            private_val_names[nid].add(m.group(1))
        for ch in n.children:
            _collect_priv_vals(ch)

    _collect_priv_vals(orig_tree)

    for k, tgt in desired_side.items():
        if k not in all_entries:
            bad.add(k)
            continue
        node, fp, fsrc, it, pimpl, fcfgs = all_entries[k]
        if gfs.has_cfg_attr(it.attrs) or (pimpl is not None and gfs.has_cfg_attr(pimpl.attrs)):
            bad.add(k)
            continue
        if tgt in ("client", "server"):
            fn_txt = fsrc[it.start : it.end]
            if gfs.find_location_macro_uses(fn_txt, sensitive_macros):
                bad.add(k)
                continue
            nid = (node.orig_mod, gfs.node_cfg_chain(node))
            privs = private_val_names.get(nid)
            if privs and any(re.search(rf"\b{re.escape(nm)}\b", fn_txt) for nm in privs):
                bad.add(k)
                continue
            onode = orig_node_by_nid.get(nid)
            if onode is not None:
                dest_file = gfs.mirror_file_path(onode.file_path, tgt).resolve()
                if fp.resolve().parent != dest_file.parent and gfs.find_relative_includes_in_src(fn_txt):
                    bad.add(k)
                    continue
        # Overlap check
        is_trait = pimpl is not None and pimpl.is_trait_impl
        grp = (k.module, gfs.owner_overlap_group(k.impl, is_trait), k.name)
        for k2 in by_overlap.get(grp, ()):
            if k2 != k and not gfs.cfgs_proven_disjoint(k.cfg, k2.cfg):
                bad.add(k)
                break

    # Trait impl consistency: all methods of a trait impl must map to the same side.
    for _tkey, tmembers in trait_groups.items():
        sides = {desired_side.get(k, "game") for k in tmembers}
        if len(sides) > 1:
            bad.update(k for k in tmembers if k in desired_side)

    return bad


def _subtree_touches_side_fields(
    files: list[Path],
    leaving_side: str,
    field_owners: dict[str, dict[str, str]],
) -> bool:
    """Return True if any file (including `#[cfg(test)]` blocks) in `files` accesses private `leaving_side` fields."""
    if leaving_side not in ("client", "server"):
        return False
    priv_state_fields = sorted(
        f for f, spec in field_owners.items() if spec.get("owner") == leaving_side and spec.get("visibility", "side") == "side"
    )
    only_wrappers = sorted(
        f for f, spec in field_owners.items() if spec.get("wrapper") == f"{leaving_side}_only"
    )
    pats: list[re.Pattern[str]] = []
    if priv_state_fields:
        pats.append(re.compile(rf"\.{leaving_side}\.(?:{'|'.join(re.escape(f) for f in priv_state_fields)})\b"))
    if only_wrappers:
        pats.append(re.compile(rf"\.(?:{'|'.join(re.escape(f) for f in only_wrappers)})\.0\b"))
    if not pats:
        return False
    for fp in files:
        if not fp.is_file():
            continue
        txt = fp.read_text(encoding="utf-8", errors="replace")
        if any(p.search(txt) for p in pats):
            return True
    return False


def _collect_test_and_external_method_calls(root: Path) -> set[str]:
    """Collect method/associated-fn names called via `.name(` or `::name(` in tests, bins, or benches."""
    called: set[str] = set()
    call_re = re.compile(r"(?:\.|::)([a-z_][a-z0-9_]*)\s*(?:::<[^>]*>)?\s*\(")
    crate_root = root / "crates/sunrise-client"
    for sub in ("tests", "benches", "src/bin"):
        d = crate_root / sub
        if d.is_dir():
            for fp in d.rglob("*.rs"):
                called.update(call_re.findall(fp.read_text(encoding="utf-8", errors="replace")))
    src_dir = crate_root / "src"
    if src_dir.is_dir():
        for fp in src_dir.rglob("*.rs"):
            rel = fp.relative_to(src_dir).as_posix()
            txt = fp.read_text(encoding="utf-8", errors="replace")
            if rel.endswith(("_tests.rs", "_test.rs", "_tests_client.rs", "_tests_server.rs")) or "/tests/" in rel or rel.endswith("/tests.rs"):
                called.update(call_re.findall(txt))
            elif "#[cfg(test)]" in txt:
                # Scan everything after the first `#[cfg(test)]` conservatively.
                idx = txt.find("#[cfg(test)]")
                called.update(call_re.findall(txt[idx:]))
    return called


def _subtree_calls_narrow_side_methods(
    files: list[Path],
    narrow_only_names: set[str],
) -> bool:
    """Return True if any test file or `#[cfg(test)]` section in `files` calls a side-narrow method."""
    if not narrow_only_names:
        return False
    call_re = re.compile(r"(?:\.|::)([a-z_][a-z0-9_]*)\s*(?:::<[^>]*>)?\s*\(")
    for fp in files:
        if not fp.is_file():
            continue
        name = fp.name
        txt = fp.read_text(encoding="utf-8", errors="replace")
        is_test_file = name.endswith(("_tests.rs", "_test.rs", "_tests_client.rs", "_tests_server.rs")) or "/tests/" in fp.as_posix() or name == "tests.rs"
        if is_test_file:
            scan_txt = txt
        elif "#[cfg(test)]" in txt:
            scan_txt = txt[txt.find("#[cfg(test)]") :]
        else:
            continue
        for m in call_re.finditer(scan_txt):
            if m.group(1) in narrow_only_names:
                return True
    return False


def plan_safe_rehome(
    root: Path,
    graph: dict[str, Any],
    reach_res: dict[str, Any],
    analysis: dict[str, Any],
    mod_sides: dict[str, str],
    *,
    field_owners: dict[str, dict[str, str]] | None = None,
    conservative_modes: tuple[str, ...] = ("S1_role_branch", "S3_startup_harness", "S4a_unsize"),
    promote_side_modules: bool = True,
) -> tuple[list[RehomeRow], list[Refusal], dict[str, Any]]:
    """Compute maximal compiling re-homing table for items free of leaving-side fields & narrow calls."""
    cid_meta = graph["cid_meta"]
    adj = graph["adj"]
    cid_cf = graph["cid_client_fields"]
    cid_sf = graph["cid_server_fields"]
    game_nodes = reach_res["game_nodes"]
    reach_of = analysis["reach_of"]
    reg_mod_names = set(mod_sides)
    side_ops_spec = reach_res.get("side_ops_spec") or collect_side_ops_spec(root)
    side_ops_srv_arms: set[str] = side_ops_spec.get("server_arms", set())
    side_ops_cli_arms: set[str] = side_ops_spec.get("client_arms", set())
    if field_owners is None:
        field_owners = gsl.load_field_owners(root / "tools/game_field_owners.tsv")

    # Conservative reachability union across `conservative_modes` so startup/dyn-Host targets are never mis-narrowed.
    cons_c: set[str] = set()
    cons_s: set[str] = set()
    for mname in conservative_modes:
        if mname in reach_res["modes"]:
            cv, sv = reach_res["modes"][mname]
            cons_c |= cv & game_nodes
            cons_s |= sv & game_nodes

    # Collect currently narrow side functions & macro-invoking top modules from `game_side_calls`.
    sc_errors: list[str] = []
    sc_found, _ = gsc.collect_side_scope(root, mod_sides, sc_errors)
    narrow_by_loc: dict[tuple[str, int], bool] = {}
    narrow_by_fnkey: dict[tuple[str, str, str, str], bool] = {}
    macro_invoc_tops: set[str] = set()

    def _strip_mirror_prefix(mod: str, side: str) -> str:
        top, *rest = mod.split("::")
        suf = f"_{side}"
        if top.endswith(suf) and top not in reg_mod_names:
            top = top[: -len(suf)]
        return "::".join([top, *rest])

    for k, fi in sc_found.items():
        rel = _rel_str(fi.file_path, root)
        if k.kind in gsc.FN_LIKE_KINDS:
            is_n = fi.vis_class == "narrow"
            narrow_by_loc[(rel, fi.line)] = is_n
            orig_m = _strip_mirror_prefix(k.module, k.side)
            narrow_by_fnkey[(orig_m, k.impl, k.name, k.cfg)] = is_n
        if k.kind in ("macro_invoc", "macro_method"):
            top_m = _strip_mirror_prefix(k.module, k.side).split("::", 1)[0]
            macro_invoc_tops.add(top_m)

    regular_mods, existing_mirrors, by_top_fnkey, by_mod_name, by_file = index_source_functions(root, mod_sides)

    # Map MIR cids to source FnKeys and group MIR cids by FnKey (folding nested inner fns into enclosing FnKey).
    fnkey_to_cids: dict[tuple[str, Any], list[str]] = defaultdict(list)
    cid_to_fnkey: dict[str, tuple[str, Any]] = {}
    unmatched_cids: dict[str, str] = {}

    for cid in sorted(game_nodes):
        st, rec = map_cid_to_source_fn(cid, cid_meta[cid], by_file, by_mod_name, reg_mod_names)
        if st in ("matched", "nested_inner_fn") and rec is not None:
            key = (rec[0], rec[2])  # (top_name, FnKey)
            fnkey_to_cids[key].append(cid)
            cid_to_fnkey[cid] = key
        else:
            unmatched_cids[cid] = st

    def _vis_class_of_fnkey(top: str, fk: Any) -> str:
        """Return `wide` | `side_client` | `side_server` | `private` for a source FnKey."""
        entry = by_top_fnkey.get(top, {}).get(fk)
        if entry is None:
            return "wide"
        loc_s, _fp, it, pimpl = entry
        if pimpl is not None and pimpl.is_trait_impl:
            return "wide"
        v = re.sub(r"\s+", "", it.vis)
        if v in ("pub", "pub(crate)", "pub(incrate::game)"):
            return "wide"
        if v == "pub(incrate::game::client_side)":
            return "side_client"
        if v == "pub(incrate::game::server_side)":
            return "side_server"
        if v == "pub(super)":
            if "::" not in fk.module:
                if loc_s == "game":
                    return "wide"
                if loc_s in ("client", "mirror_client"):
                    return "side_client"
                if loc_s in ("server", "mirror_server"):
                    return "side_server"
            return "private"
        return "private"

    def is_currently_narrow(cid: str) -> bool:
        m = cid_meta[cid]
        if m["label"] == "common":
            return False
        fk_pair = cid_to_fnkey.get(cid)
        if fk_pair is not None:
            top_g, fk_g = fk_pair
            vc = _vis_class_of_fnkey(top_g, fk_g)
            if vc in ("side_client", "side_server", "private"):
                return True
            if (fk_g.module, fk_g.impl, fk_g.name, fk_g.cfg) in narrow_by_fnkey:
                return narrow_by_fnkey[(fk_g.module, fk_g.impl, fk_g.name, fk_g.cfg)]
        sf, ln = m["source_file"], m["line_start"]
        for d in range(-8, 9):
            if (sf, ln + d) in narrow_by_loc:
                return narrow_by_loc[(sf, ln + d)]
        return False

    def is_module_pinned_callee(cid: str) -> bool:
        """Return True if `cid` is an OUT_DIR/generated `pub(super)` callee (e.g. `native_hooks::fire::*`) or module-private."""
        m = cid_meta[cid]
        if "OUT_DIR" in m["source_file"] or "::fire::fire_" in cid:
            return True
        fk_pair = cid_to_fnkey.get(cid)
        if fk_pair is not None:
            return _vis_class_of_fnkey(fk_pair[0], fk_pair[1]) == "private"
        return False

    # Desired role target per MIR cid (using conservative modes for single-role safety!).
    # Note: "game" denotes common in TSV rows; "common" is used internally for MIR labels.
    def _to_tsv_side(lbl: str) -> str:
        return "game" if lbl == "common" else lbl

    cid_desired: dict[str, str] = {}
    for cid in game_nodes:
        in_c = cid in cons_c
        in_s = cid in cons_s
        r = "both" if (in_c and in_s) else ("client_only" if in_c else ("server_only" if in_s else "neither"))
        if r == "neither":
            continue
        cf = cid_cf.get(cid, set())
        sf = cid_sf.get(cid, set())
        if r == "client_only" and not sf:
            cid_desired[cid] = "client"
        elif r == "server_only" and not cf:
            cid_desired[cid] = "server"
        elif r == "both" and not cf and not sf:
            cid_desired[cid] = "common"

    # Compute per-FnKey field usage and target label across all folded MIR cids.
    fn_cf: dict[tuple[str, Any], set[str]] = defaultdict(set)
    fn_sf: dict[tuple[str, Any], set[str]] = defaultdict(set)
    fn_cur_side: dict[tuple[str, Any], str] = {}
    fn_desired: dict[tuple[str, Any], str] = {}
    fn_movable_cands: dict[tuple[str, Any], tuple[str, str]] = {}  # (top, FnKey) -> (cur_lbl, tgt_lbl)

    for (top, fk), cids in fnkey_to_cids.items():
        loc_side = by_top_fnkey[top][fk][0]
        if loc_side == "mirror_client":
            cur_lbl = "client"
        elif loc_side == "mirror_server":
            cur_lbl = "server"
        elif loc_side == "game":
            cur_lbl = "common"
        else:
            cur_lbl = loc_side
        fn_cur_side[(top, fk)] = cur_lbl

        cf_all: set[str] = set()
        sf_all: set[str] = set()
        tgts: set[str] = set()
        has_contradiction = False
        for c in cids:
            cf_all |= cid_cf.get(c, set())
            sf_all |= cid_sf.get(c, set())
            in_c, in_s = c in cons_c, c in cons_s
            if in_c or in_s:
                if c in cid_desired:
                    tgts.add(cid_desired[c])
                else:
                    has_contradiction = True
        fn_cf[(top, fk)] = cf_all
        fn_sf[(top, fk)] = sf_all

        if has_contradiction or not tgts:
            continue
        tgt_lbl = "common" if len(tgts) > 1 else next(iter(tgts))
        fn_desired[(top, fk)] = tgt_lbl
        if tgt_lbl == cur_lbl:
            continue
        # Pin `side_ops` dispatchers (always common), `PINNED_MODULES` functions if whole module pinned to common,
        # and constructor helpers inspected by `game_field_owners.py` in `game.rs`.
        if top == "side_ops" or (top == "game" and fk.name in PINNED_GAME_RS_FNS):
            continue
        # Pin `side_ops` server/client arm implementations so they NEVER leave their required side!
        if fk.impl == "impl Game<'_>":
            if fk.name in side_ops_srv_arms and tgt_lbl != "server":
                continue
            if fk.name in side_ops_cli_arms and tgt_lbl != "client":
                continue
        # Must be free of leaving-side fields and target-forbidding fields.
        if cur_lbl == "client" and cf_all:
            continue
        if cur_lbl == "server" and sf_all:
            continue
        if tgt_lbl == "common" and (cf_all or sf_all):
            continue
        if tgt_lbl == "client" and sf_all:
            continue
        if tgt_lbl == "server" and cf_all:
            continue
        fn_movable_cands[(top, fk)] = (cur_lbl, tgt_lbl)

    # Greatest fixed point: prune any movable fn calling a narrow fn of its leaving side that is NOT moving to tgt/common.
    def _effective_cid_side(c: str) -> str:
        fk_pair = cid_to_fnkey.get(c)
        if fk_pair is not None and fk_pair in fn_movable_cands:
            return fn_movable_cands[fk_pair][1]
        return cid_meta[c]["label"]

    changed = True
    while changed:
        changed = False
        drop_keys: list[tuple[str, Any]] = []
        for fk_pair, (cur_lbl, tgt_lbl) in fn_movable_cands.items():
            if cur_lbl not in ("client", "server"):
                continue
            bad_call = False
            for c in fnkey_to_cids[fk_pair]:
                for g in adj.get(c, ()):
                    if g not in cid_meta:
                        continue
                    if cid_meta[g]["label"] != cur_lbl:
                        continue
                    if not is_currently_narrow(g):
                        continue
                    g_new = _effective_cid_side(g)
                    if g_new not in (tgt_lbl, "common"):
                        bad_call = True
                        break
                if bad_call:
                    break
            if bad_call:
                drop_keys.append(fk_pair)
        for fk_pair in drop_keys:
            del fn_movable_cands[fk_pair]
            changed = True

    # Now decide whole-module moves vs single-function moves per top-level module!
    rehome_rows: list[RehomeRow] = []
    refusals: list[Refusal] = []
    moved_modules: dict[str, tuple[str, str]] = {}  # top -> (cur_side_tsv, tgt_side_tsv)

    # Index ALL game:: cids by top module (including unmatched derives/consts/macro fns!)
    cids_by_top: dict[str, list[str]] = defaultdict(list)
    for c in game_nodes:
        cids_by_top[_top_module_from_cid(c, reg_mod_names)].append(c)

    # Track per-top-module whether any non-fn / unmatched item uses leaving-side fields or narrow calls.
    all_arms, _, resolved_entries, _ = gms.collect_all_arms(root, set(mod_sides))

    # Index side-narrow free functions by (side, top_module) so module promotions refuse if their `use` imports one.
    narrow_free_by_side_mod: dict[tuple[str, str], set[str]] = defaultdict(set)
    for top_m, fk_dict in by_top_fnkey.items():
        for fk_m in fk_dict:
            if fk_m.impl == "-":
                vc = _vis_class_of_fnkey(top_m, fk_m)
                if vc == "side_client":
                    narrow_free_by_side_mod[("client", top_m)].add(fk_m.name)
                elif vc == "side_server":
                    narrow_free_by_side_mod[("server", top_m)].add(fk_m.name)

    def _module_imports_narrow_free(top_m: str, leaving_side: str) -> bool:
        if leaving_side not in ("client", "server"):
            return False
        sub_files = gms.collect_module_subtree_files(root, resolved_entries.get(top_m, []), all_arms.get(top_m, []))
        for fp in sub_files:
            if not fp.is_file():
                continue
            txt = fp.read_text(encoding="utf-8")
            for m_use in re.finditer(r"\buse\s+[^;]*?\b([a-z_][a-z0-9_]*)::([^;]+);", txt):
                other_mod, tail = m_use.group(1), m_use.group(2)
                if other_mod == top_m:
                    continue
                n_names = narrow_free_by_side_mod.get((leaving_side, other_mod))
                if n_names and any(re.search(rf"\b{re.escape(nm)}\b", tail) for nm in n_names):
                    return True
        return False

    # Collect narrow-only method names per side (excluding names also defined wide/common) for `--all-targets` safety.
    all_wide_or_common_names: set[str] = set()
    narrow_by_side_and_top: dict[str, dict[str, set[str]]] = {"client": defaultdict(set), "server": defaultdict(set)}
    for top_m, fk_dict in by_top_fnkey.items():
        for fk_m, (loc_s, _fp, it_m, _pi) in fk_dict.items():
            if loc_s == "game" or _vis_class_of_fnkey(top_m, fk_m) == "wide":
                all_wide_or_common_names.add(fk_m.name)
    for k, fi in sc_found.items():
        if k.kind in gsc.FN_LIKE_KINDS and fi.vis_class == "narrow" and k.name not in all_wide_or_common_names:
            top_m = _strip_mirror_prefix(k.module, k.side).split("::", 1)[0]
            narrow_by_side_and_top[k.side][top_m].add(k.name)

    def _is_pinned_regular_side_mod(top_name: str, cur_side: str) -> bool:
        """Hand-split regular `*_client` / `*_server` modules stay on their explicit side."""
        return (
            top_name in PINNED_MODULES
            or (cur_side == "client" and top_name.endswith("_client"))
            or (cur_side == "server" and top_name.endswith("_server"))
        )

    for top_name in sorted(mod_sides):
        cur_mod_side = mod_sides[top_name]  # client | server | game
        if _is_pinned_regular_side_mod(top_name, cur_mod_side):
            continue
        cur_mod_lbl = "common" if cur_mod_side == "game" else cur_mod_side
        top_cids = cids_by_top.get(top_name, [])
        if not top_cids:
            continue

        # Gather FnKeys living in the regular (non-mirror) module of `top_name`.
        reg_fnkeys = [
            fk
            for fk, (loc_s, _fp, _it, _pi) in by_top_fnkey.get(top_name, {}).items()
            if not loc_s.startswith("mirror_")
        ]
        # Never move a whole regular module away from the side required by a `side_ops` arm it defines.
        has_srv_arm = any(fk.impl == "impl Game<'_>" and fk.name in side_ops_srv_arms for fk in reg_fnkeys)
        has_cli_arm = any(fk.impl == "impl Game<'_>" and fk.name in side_ops_cli_arms for fk in reg_fnkeys)
        # Reachable cids in `top_name` (regular module, not mirrors).
        reg_cids = [
            c
            for c in top_cids
            if not (cid_to_fnkey.get(c) and by_top_fnkey[top_name][cid_to_fnkey[c][1]][0].startswith("mirror_"))
        ]
        reach_reg_cids = [c for c in reg_cids if (c in cons_c or c in cons_s)]
        if not reach_reg_cids:
            continue

        # Check if ALL reachable cids in regular module `top_name` agree on one target `T != cur_mod_lbl`.
        want_targets: set[str] = set()
        any_contra = False
        for c in reach_reg_cids:
            if c in cid_desired:
                want_targets.add(cid_desired[c])
            else:
                any_contra = True

        if not any_contra and len(want_targets) == 1:
            tgt_lbl = next(iter(want_targets))
            if (has_srv_arm and tgt_lbl != "server") or (has_cli_arm and tgt_lbl != "client"):
                continue
            if tgt_lbl != cur_mod_lbl:
                # Verify EVERY `--lib` cid in `reg_cids` (including `neither`!) is free of leaving-side fields & narrow calls outside `top_name`.
                mod_cf = set().union(*(cid_cf.get(c, set()) for c in reg_cids)) if reg_cids else set()
                mod_sf = set().union(*(cid_sf.get(c, set()) for c in reg_cids)) if reg_cids else set()
                fields_ok = True
                if cur_mod_lbl == "client" and mod_cf:
                    fields_ok = False
                if cur_mod_lbl == "server" and mod_sf:
                    fields_ok = False
                if tgt_lbl == "common" and (mod_cf or mod_sf):
                    fields_ok = False
                if tgt_lbl == "client" and mod_sf:
                    fields_ok = False
                if tgt_lbl == "server" and mod_cf:
                    fields_ok = False
                if fields_ok and cur_mod_lbl in ("client", "server"):
                    sub_files = gms.collect_module_subtree_files(root, resolved_entries.get(top_name, []), all_arms.get(top_name, []))
                    other_narrow = set().union(
                        *(nms for m_o, nms in narrow_by_side_and_top[cur_mod_lbl].items() if m_o != top_name)
                    ) if narrow_by_side_and_top[cur_mod_lbl] else set()
                    if _subtree_touches_side_fields(sub_files, cur_mod_lbl, field_owners) or _subtree_calls_narrow_side_methods(sub_files, other_narrow):
                        fields_ok = False

                # Check narrow calls to OTHER modules in `cur_mod_lbl`
                narrow_ok = True
                if fields_ok and cur_mod_lbl in ("client", "server"):
                    for c in reg_cids:
                        for g in adj.get(c, ()):
                            if g not in cid_meta or cid_meta[g]["label"] != cur_mod_lbl:
                                continue
                            if _top_module_from_cid(g, reg_mod_names) == top_name:
                                continue
                            if not is_currently_narrow(g):
                                continue
                            g_new = _effective_cid_side(g)
                            if g_new not in (tgt_lbl, "common"):
                                narrow_ok = False
                                break
                        if not narrow_ok:
                            break

                # Check root-depth `super::super` if moving between `game` and side.
                super_ok = True
                if (cur_mod_lbl == "common") != (tgt_lbl == "common"):
                    if _check_root_super_super(root, resolved_entries.get(top_name, [])):
                        super_ok = False

                # Check if `top_name` has `OUT_DIR` generated includes, macro-generated side visibility, or narrow free-fn imports.
                gen_ok = not any("OUT_DIR" in cid_meta[c]["source_file"] for c in reg_cids)
                if cur_mod_lbl in ("client", "server") and top_name in macro_invoc_tops:
                    gen_ok = False
                if cur_mod_lbl in ("client", "server") and _module_imports_narrow_free(top_name, cur_mod_lbl):
                    gen_ok = False

                if fields_ok and narrow_ok and super_ok and gen_ok:
                    tgt_tsv = _to_tsv_side(tgt_lbl)
                    moved_modules[top_name] = (cur_mod_side, tgt_tsv)
                    rehome_rows.append(
                        RehomeRow(
                            kind="module",
                            module=top_name,
                            impl="-",
                            name="-",
                            cfg="-",
                            current_side=cur_mod_side,
                            target_side=tgt_tsv,
                            reason=f"role-reachability:{tgt_lbl}",
                        )
                    )

    # Re-run fixed-point check taking whole-module moves into account, then emit single-fn moves!
    existing_fn_rows = gfs.load_fn_sides(root / "tools/game_fn_sides.tsv")
    game_rs_path = (root / "crates/sunrise-client/src/game.rs").resolve()
    game_root_tree = gfs.build_mod_tree("game", game_rs_path) if game_rs_path.is_file() else None

    # Group candidate single-fn moves by top_name.
    cands_by_top: dict[str, dict[Any, tuple[str, str]]] = defaultdict(dict)
    for (top, fk), (cur_lbl, tgt_lbl) in fn_movable_cands.items():
        if top in moved_modules:
            _old_m, new_m = moved_modules[top]
            new_m_lbl = "common" if new_m == "game" else new_m
            # If the module itself is moving to `tgt_lbl`, a mirror fn dissolves automatically via the module move!
            if tgt_lbl == new_m_lbl:
                continue
        cands_by_top[top][fk] = (cur_lbl, tgt_lbl)

    # Also check if any whole-side module can be promoted to `game` (common) so its common fns move to `game`
    # while its staying side fns move into `M_<cur_side>`.
    for top_name, fk_map in sorted(cands_by_top.items()):
        cur_mod_side = moved_modules[top_name][1] if top_name in moved_modules else mod_sides.get(top_name, "game")
        # Check which candidate fns want to move to `common` while living inside a `client` or `server` regular module.
        want_common_in_side_mod = [
            fk
            for fk, (cl, tl) in fk_map.items()
            if tl == "common" and cur_mod_side in ("client", "server") and not by_top_fnkey[top_name][fk][0].startswith("mirror_")
        ]
        # Can we promote `top_name` to `game` and mirror its staying side fns into `top_name_<cur_mod_side>`?
        if want_common_in_side_mod and top_name != "game" and top_name not in moved_modules and not _is_pinned_regular_side_mod(top_name, cur_mod_side):
            if not promote_side_modules:
                for fk in want_common_in_side_mod:
                    refusals.append(
                        Refusal(
                            kind="fn",
                            module=fk.module,
                            impl=fk.impl,
                            name=fk.name,
                            cfg=fk.cfg,
                            target_side="game",
                            code="side-module-promotion-disabled",
                            detail=f"parent module `{top_name}` is `{cur_mod_side}` (--no-promote-side-modules)",
                        )
                    )
                want_common_in_side_mod = []
        if want_common_in_side_mod and top_name != "game" and top_name not in moved_modules and not _is_pinned_regular_side_mod(top_name, cur_mod_side):
            # Check if ALL non-movable `--lib` items in regular `top_name` that need `cur_mod_side` are cleanly mirrorable FnKeys!
            reg_cids = [
                c
                for c in cids_by_top.get(top_name, [])
                if not (cid_to_fnkey.get(c) and by_top_fnkey[top_name][cid_to_fnkey[c][1]][0].startswith("mirror_"))
            ]
            # Check for any unmatched/macro/derive/generated cid, narrow free-fn imports, or mirror-name collision.
            unmovable_blocker = False
            if f"{top_name}_{cur_mod_side}" in reg_mod_names:
                unmovable_blocker = True
            if top_name in macro_invoc_tops:
                unmovable_blocker = True
            if _module_imports_narrow_free(top_name, cur_mod_side):
                unmovable_blocker = True
            if _check_root_super_super(root, resolved_entries.get(top_name, [])):
                unmovable_blocker = True
            if any("OUT_DIR" in cid_meta[c]["source_file"] for c in reg_cids):
                unmovable_blocker = True
            if not unmovable_blocker:
                for c in reg_cids:
                    if c not in cid_to_fnkey:
                        # Unmatched cid (derive, macro fn, trait default, const) - check if it touches cur_mod_side fields or narrow calls
                        cf = cid_cf.get(c, set())
                        sf = cid_sf.get(c, set())
                        if (cur_mod_side == "client" and cf) or (cur_mod_side == "server" and sf):
                            unmovable_blocker = True
                            break
                        for g in adj.get(c, ()):
                            if g in cid_meta and cid_meta[g]["label"] == cur_mod_side and is_currently_narrow(g):
                                if _effective_cid_side(g) != "common":
                                    unmovable_blocker = True
                                    break
                        if unmovable_blocker:
                            break

            # Identify all regular FnKeys in `top_name` that must stay in `cur_mod_side` (into mirror `M_<cur_mod_side>`)
            stay_in_side_fks: dict[Any, str] = {}
            if not unmovable_blocker:
                for fk, (loc_s, _fp, it, _pi) in by_top_fnkey.get(top_name, {}).items():
                    if loc_s.startswith("mirror_"):
                        continue
                    # Any `side_ops` arm MUST stay on its required side!
                    if fk.impl == "impl Game<'_>" and fk.name in side_ops_srv_arms:
                        stay_in_side_fks[fk] = "server"
                        continue
                    if fk.impl == "impl Game<'_>" and fk.name in side_ops_cli_arms:
                        stay_in_side_fks[fk] = "client"
                        continue
                    if fk in fk_map and fk_map[fk][1] == "common":
                        continue
                    if fk in fk_map and fk_map[fk][1] in ("client", "server"):
                        stay_in_side_fks[fk] = fk_map[fk][1]
                        continue
                    # Also keep `#[cfg(test)]` fns in `cur_mod_side` if the module's tests access private side fields.
                    if it.is_test_attr or "#[cfg(test)]" in fk.cfg:
                        stay_in_side_fks[fk] = cur_mod_side
                        continue
                    # Otherwise keep this fn in `cur_mod_side` (if it's a test or uses side state/calls or stays on cur_mod_side)
                    cf = fn_cf.get((top_name, fk), set())
                    sf = fn_sf.get((top_name, fk), set())
                    needs_side = (cur_mod_side == "client" and bool(cf)) or (cur_mod_side == "server" and bool(sf))
                    if not needs_side:
                        for c in fnkey_to_cids.get((top_name, fk), ()):
                            for g in adj.get(c, ()):
                                if g in cid_meta and cid_meta[g]["label"] == cur_mod_side and is_currently_narrow(g):
                                    if _effective_cid_side(g) != "common":
                                        needs_side = True
                                        break
                            if needs_side:
                                break
                    # Also keep `cur_mod_side`-only reachable fns in `cur_mod_side`
                    if needs_side or fn_desired.get((top_name, fk)) == cur_mod_side:
                        stay_in_side_fks[fk] = cur_mod_side

                # Check if any retained side collides with a regular module name!
                if any(f"{top_name}_{s_tgt}" in reg_mod_names for s_tgt in stay_in_side_fks.values()):
                    unmovable_blocker = True

                # Also check any existing mirror fns in `top_name`
                for k_ex, r_ex in existing_fn_rows.items():
                    if r_ex.top_module == top_name and r_ex.side in ("client", "server"):
                        if k_ex not in fk_map:
                            stay_in_side_fks[k_ex] = r_ex.side

                # Free functions (`impl == "-"`) cannot split into mirror files without changing caller paths.
                if any(
                    fk.impl == "-" and not by_top_fnkey.get(top_name, {}).get(fk, ("",))[0].startswith("mirror_")
                    for fk in stay_in_side_fks
                ):
                    unmovable_blocker = True

                # Check private/OUT_DIR intra-module callees: a staying mirror fn cannot call a private sibling left in `game`,
                # nor can a common fn left in `game` call a private sibling moving to a mirror!
                if not unmovable_blocker:
                    for fk_s, s_tgt in stay_in_side_fks.items():
                        for c in fnkey_to_cids.get((top_name, fk_s), ()):
                            for g in adj.get(c, ()):
                                if g not in cid_meta or _top_module_from_cid(g, reg_mod_names) != top_name:
                                    continue
                                if is_module_pinned_callee(g):
                                    g_fk = cid_to_fnkey.get(g)
                                    if g_fk is None or stay_in_side_fks.get(g_fk[1]) != s_tgt:
                                        unmovable_blocker = True
                                        break
                            if unmovable_blocker:
                                break
                        if unmovable_blocker:
                            break
                if not unmovable_blocker:
                    for fk_c in want_common_in_side_mod:
                        for c in fnkey_to_cids.get((top_name, fk_c), ()):
                            for g in adj.get(c, ()):
                                if g not in cid_meta or _top_module_from_cid(g, reg_mod_names) != top_name:
                                    continue
                                g_fk = cid_to_fnkey.get(g)
                                if g_fk is not None and g_fk[1] in stay_in_side_fks:
                                    if _vis_class_of_fnkey(top_name, g_fk[1]) in ("private", f"side_{cur_mod_side}"):
                                        unmovable_blocker = True
                                        break
                            if unmovable_blocker:
                                break
                        if unmovable_blocker:
                            break

                # Precheck whether `stay_in_side_fks` passes ALL `game_fn_sides` constraints!
                if not unmovable_blocker:
                    bad_stay = _precheck_fn_move_constraints(
                        root, top_name, regular_mods, existing_mirrors, stay_in_side_fks, game_root_tree
                    )
                    if bad_stay:
                        unmovable_blocker = True

            if not unmovable_blocker:
                # Promote `top_name` to `game` and mirror `stay_in_side_fks`!
                moved_modules[top_name] = (cur_mod_side, "game")
                rehome_rows.append(
                    RehomeRow(
                        kind="module",
                        module=top_name,
                        impl="-",
                        name="-",
                        cfg="-",
                        current_side=cur_mod_side,
                        target_side="game",
                        reason=f"promote-for-common-fns:{len(want_common_in_side_mod)}",
                    )
                )
                for fk, s_tgt in sorted(stay_in_side_fks.items(), key=lambda kv: (kv[0].module, kv[0].impl, kv[0].name, kv[0].cfg)):
                    if fk in existing_fn_rows and existing_fn_rows[fk].side == s_tgt:
                        continue
                    rehome_rows.append(
                        RehomeRow(
                            kind="fn",
                            module=fk.module,
                            impl=fk.impl,
                            name=fk.name,
                            cfg=fk.cfg,
                            current_side=cur_mod_side,
                            target_side=s_tgt,
                            reason=f"retain-in-{s_tgt}-mirror",
                        )
                    )
                # Remaining mirror fns of `top_name` wanting `game` or opposite side are handled below.
                cur_mod_side = "game"
            else:
                for fk in want_common_in_side_mod:
                    refusals.append(
                        Refusal(
                            kind="fn",
                            module=fk.module,
                            impl=fk.impl,
                            name=fk.name,
                            cfg=fk.cfg,
                            target_side="game",
                            code="side-module-cannot-split-to-common",
                            detail=f"parent module `{top_name}` is `{cur_mod_side}` and has unmirrorable side-dependent items",
                        )
                    )

        # Now handle individual fn moves for `top_name` (excluding already-handled common fns inside promoted/refused side mods).
        # Build desired map for prechecking `game_fn_sides` mechanical constraints.
        test_desired: dict[Any, str] = {}
        for k_ex, r_ex in existing_fn_rows.items():
            if r_ex.top_module == top_name and r_ex.side in ("client", "server"):
                test_desired[k_ex] = r_ex.side
        fn_candidates_here: list[tuple[Any, str, str]] = []
        for fk, (cl, tl) in sorted(fk_map.items(), key=lambda kv: (kv[0].module, kv[0].impl, kv[0].name, kv[0].cfg)):
            tgt_tsv = _to_tsv_side(tl)
            cur_tsv = _to_tsv_side(cl)
            loc_s = by_top_fnkey[top_name][fk][0]
            # Splitting a free function (`impl == "-"`) into a mirror changes its module path (`M::f` -> `M_side::f`).
            is_mirror_split = (tgt_tsv != cur_mod_side) and not loc_s.startswith("mirror_")
            # Never create a `game_fn_sides` mirror `<top>_<side>` if `<top>_<side>` is already a regular module!
            if tgt_tsv in ("client", "server") and tgt_tsv != cur_mod_side and f"{top_name}_{tgt_tsv}" in reg_mod_names:
                refusals.append(
                    Refusal(
                        kind="fn",
                        module=fk.module,
                        impl=fk.impl,
                        name=fk.name,
                        cfg=fk.cfg,
                        target_side=tgt_tsv,
                        code="mirror-name-is-regular-module",
                        detail=f"regular module `{top_name}_{tgt_tsv}` already exists in game_module_sides.tsv",
                    )
                )
                continue
            if is_mirror_split and fk.impl == "-":
                refusals.append(
                    Refusal(
                        kind="fn",
                        module=fk.module,
                        impl=fk.impl,
                        name=fk.name,
                        cfg=fk.cfg,
                        target_side=tgt_tsv,
                        code="free-fn-mirror-split",
                        detail=f"free function `{fk.module}::{fk.name}` cannot split into mirror without changing callers' path",
                    )
                )
                continue
            # Splitting a function into a mirror cannot call module-private or OUT_DIR/generated `pub(super)` siblings.
            if is_mirror_split:
                pinned_hit = False
                for c in fnkey_to_cids.get((top_name, fk), ()):
                    for g in adj.get(c, ()):
                        if g not in cid_meta or _top_module_from_cid(g, reg_mod_names) != top_name:
                            continue
                        if is_module_pinned_callee(g):
                            pinned_hit = True
                            break
                    if pinned_hit:
                        break
                if pinned_hit or _vis_class_of_fnkey(top_name, fk) == "private":
                    refusals.append(
                        Refusal(
                            kind="fn",
                            module=fk.module,
                            impl=fk.impl,
                            name=fk.name,
                            cfg=fk.cfg,
                            target_side=tgt_tsv,
                            code="private-or-generated-sibling",
                            detail=f"function `{fk.module}::{fk.name}` is private or calls private/generated sibling in `{top_name}`",
                        )
                    )
                    continue
            if cur_mod_side == "game":
                if top_name in moved_modules and tgt_tsv == "game" and not loc_s.startswith("mirror_"):
                    continue  # Already moved to `game` via module promotion above.
                if top_name in moved_modules and tgt_tsv in ("client", "server") and not loc_s.startswith("mirror_"):
                    continue  # Already emitted above.
                test_desired[fk] = tgt_tsv
                fn_candidates_here.append((fk, cur_tsv, tgt_tsv))
            else:
                # Parent module is `client` or `server`!
                if tgt_tsv == "game" and not loc_s.startswith("mirror_"):
                    continue  # Already refused above.
                if tgt_tsv == "game" and loc_s.startswith("mirror_"):
                    # Mirror fn returning to `game` when parent is `client`/`server` -> cannot go to common without promoting parent!
                    refusals.append(
                        Refusal(
                            kind="fn",
                            module=fk.module,
                            impl=fk.impl,
                            name=fk.name,
                            cfg=fk.cfg,
                            target_side="game",
                            code="side-module-cannot-split-to-common",
                            detail=f"mirror fn cannot return to common while parent `{top_name}` is `{cur_mod_side}`",
                        )
                    )
                    continue
                if tgt_tsv == cur_mod_side and loc_s.startswith("mirror_"):
                    # Mirror fn returning to its parent module (dissolves mirror)!
                    test_desired[fk] = "game"  # in game_fn_sides.tsv, "game" means parent module!
                    fn_candidates_here.append((fk, cur_tsv, tgt_tsv))
                elif tgt_tsv != cur_mod_side and not loc_s.startswith("mirror_"):
                    # Fn in parent side module moving to opposite-side mirror!
                    test_desired[fk] = tgt_tsv
                    fn_candidates_here.append((fk, cur_tsv, tgt_tsv))

        if not fn_candidates_here:
            continue
        bad_fks = _precheck_fn_move_constraints(
            root, top_name, regular_mods, existing_mirrors, test_desired, game_root_tree
        )
        for fk, cur_tsv, tgt_tsv in fn_candidates_here:
            if fk in bad_fks:
                refusals.append(
                    Refusal(
                        kind="fn",
                        module=fk.module,
                        impl=fk.impl,
                        name=fk.name,
                        cfg=fk.cfg,
                        target_side=tgt_tsv,
                        code="fn-move-constraint",
                        detail="blocked by location-sensitive macro, relative include, cfg_attr, cfg overlap, or mixed trait impl",
                    )
                )
            else:
                rehome_rows.append(
                    RehomeRow(
                        kind="fn",
                        module=fk.module,
                        impl=fk.impl,
                        name=fk.name,
                        cfg=fk.cfg,
                        current_side=cur_tsv,
                        target_side=tgt_tsv,
                        reason=f"role-reachability:{'common' if tgt_tsv == 'game' else tgt_tsv}",
                    )
                )

    # Deduplicate rehome_rows, then run a final fixed-point verification against ACTUAL post-move sides
    # so no moved module/fn can call a side-narrow callee whose own move was refused.
    dedup: dict[tuple[str, str, str, str, str], RehomeRow] = {}
    for r in rehome_rows:
        dedup[r.sort_key()] = r

    def _actual_post_side(c: str, cur_dedup: dict[tuple[str, str, str, str, str], RehomeRow]) -> str:
        fk_pair = cid_to_fnkey.get(c)
        if fk_pair is not None:
            top_c, fk_c = fk_pair
            fn_row = cur_dedup.get(("fn", fk_c.module, fk_c.impl, fk_c.name, fk_c.cfg))
            if fn_row is not None:
                return "common" if fn_row.target_side == "game" else fn_row.target_side
            loc_s = by_top_fnkey[top_c][fk_c][0]
            if loc_s == "mirror_client":
                return "client"
            if loc_s == "mirror_server":
                return "server"
            mod_row = cur_dedup.get(("module", top_c, "-", "-", "-"))
            if mod_row is not None:
                return "common" if mod_row.target_side == "game" else mod_row.target_side
            return cid_meta[c]["label"]
        top_c = _top_module_from_cid(c, reg_mod_names)
        mod_row = cur_dedup.get(("module", top_c, "-", "-", "-"))
        if mod_row is not None:
            return "common" if mod_row.target_side == "game" else mod_row.target_side
        return cid_meta[c]["label"]

    fp_changed = True
    while fp_changed:
        fp_changed = False
        bad_mods: list[str] = []
        bad_fn_keys: list[tuple[str, str, str, str, str]] = []
        for sk, r in list(dedup.items()):
            if r.kind == "module":
                if r.current_side not in ("client", "server"):
                    continue
                cur_lbl = r.current_side
                tgt_lbl = "common" if r.target_side == "game" else r.target_side
                # Check every regular cid of `r.module` that actually lands in `tgt_lbl` (not retained in a `cur_lbl` mirror!)
                violated = False
                for c in cids_by_top.get(r.module, ()):
                    c_post = _actual_post_side(c, dedup)
                    if c_post == cur_lbl:
                        continue
                    for g in adj.get(c, ()):
                        if g not in cid_meta or cid_meta[g]["label"] != cur_lbl:
                            continue
                        if not is_currently_narrow(g):
                            continue
                        g_post = _actual_post_side(g, dedup)
                        if g_post not in (c_post, "common"):
                            violated = True
                            break
                    if violated:
                        break
                if violated:
                    bad_mods.append(r.module)
            else:
                if r.current_side not in ("client", "server") or r.current_side == r.target_side:
                    continue
                cur_lbl = r.current_side
                tgt_lbl = "common" if r.target_side == "game" else r.target_side
                top_f = r.module.split("::", 1)[0]
                fk_obj = gfs.FnKey(module=r.module, impl=r.impl, name=r.name, cfg=r.cfg)
                violated = False
                for c in fnkey_to_cids.get((top_f, fk_obj), ()):
                    for g in adj.get(c, ()):
                        if g not in cid_meta or cid_meta[g]["label"] != cur_lbl:
                            continue
                        if not is_currently_narrow(g):
                            continue
                        g_post = _actual_post_side(g, dedup)
                        if g_post not in (tgt_lbl, "common"):
                            violated = True
                            break
                    if violated:
                        break
                if violated:
                    bad_fn_keys.append(sk)
        for mname in bad_mods:
            r_mod = dedup.pop(("module", mname, "-", "-", "-"), None)
            if r_mod is not None:
                refusals.append(
                    Refusal("module", mname, "-", "-", "-", r_mod.target_side, "narrow-callee-stayed-on-side", f"calls narrow `{r_mod.current_side}` callee that remained on `{r_mod.current_side}`")
                )
                # Drop any retain-in-*-mirror rows emitted solely for this reverted module promotion.
                for sk2 in list(dedup):
                    r2 = dedup[sk2]
                    if r2.kind == "fn" and r2.module.split("::", 1)[0] == mname and r2.reason.startswith("retain-in-"):
                        del dedup[sk2]
                fp_changed = True
        for sk in bad_fn_keys:
            r_fn = dedup.pop(sk, None)
            if r_fn is not None:
                refusals.append(
                    Refusal("fn", r_fn.module, r_fn.impl, r_fn.name, r_fn.cfg, r_fn.target_side, "narrow-callee-stayed-on-side", f"calls narrow `{r_fn.current_side}` callee that remained on `{r_fn.current_side}`")
                )
                fp_changed = True

    final_rows = sorted(dedup.values(), key=lambda x: x.sort_key())

    stats = {
        "total_rows": len(final_rows),
        "module_rows": sum(1 for r in final_rows if r.kind == "module"),
        "fn_rows": sum(1 for r in final_rows if r.kind == "fn"),
        "refusals": len(refusals),
        "by_kind_dir": dict(
            Counter(f"{r.kind}:{r.current_side}->{r.target_side}" for r in final_rows)
        ),
        "refusals_by_code": dict(Counter(rf.code for rf in refusals)),
    }
    return final_rows, refusals, stats


# ---------------------------------------------------------------------------
# Re-homing Application Engine (--dry-run / --apply / --check)
# ---------------------------------------------------------------------------


def _rewrite_side_vis_in_subtree(
    files: list[Path], old_side: str, new_vis: str = "pub(in crate::game)"
) -> list[Path]:
    """Rewrite `pub(in crate::game::{old_side}_side)` to `new_vis` in `files`."""
    old_mod = SIDE_MOD_NAME[old_side]
    pat = re.compile(rf"pub\s*\(\s*in\s+crate::game::{old_mod}\s*\)")
    changed: list[Path] = []
    for fp in sorted(set(files)):
        if not fp.is_file():
            continue
        txt = fp.read_text(encoding="utf-8")
        new_txt = pat.sub(new_vis, txt)
        new_txt = re.sub(r"[ \t]+\n", "\n", new_txt)
        if new_txt != txt:
            fp.write_text(new_txt, encoding="utf-8")
            changed.append(fp)
    return changed


def _rewrite_explicit_mod_paths_in_mirrors(
    root: Path,
    moved_mods: dict[str, tuple[str, str]],
) -> list[Path]:
    """Update explicit `crate::game::[old_side::]<M>` paths across `game/` after module moves."""
    if not moved_mods:
        return []
    game_dir = root / "crates/sunrise-client/src/game"
    changed: list[Path] = []
    for p in sorted(game_dir.rglob("*.rs")):
        if p.name in ("client_side.rs", "server_side.rs"):
            continue
        txt = p.read_text(encoding="utf-8")
        new_txt = txt
        for mname, (old_s, new_s) in moved_mods.items():
            if old_s not in SIDE_MOD_NAME:
                continue
            old_prefix = f"crate::game::{SIDE_MOD_NAME[old_s]}::{mname}"
            new_prefix = (
                f"crate::game::{SIDE_MOD_NAME[new_s]}::{mname}"
                if new_s in SIDE_MOD_NAME
                else f"crate::game::{mname}"
            )
            if old_prefix != new_prefix and old_prefix in new_txt:
                new_txt = re.sub(rf"{re.escape(old_prefix)}(?=\b|::)", new_prefix, new_txt)
        if new_txt != txt:
            p.write_text(new_txt, encoding="utf-8")
            changed.append(p)
    return changed


def _write_module_sides_tsv(
    table_path: Path, updates: dict[str, tuple[str, str]]
) -> None:
    """Update `side` and `reason` columns in `tools/game_module_sides.tsv`, preserving `paths`."""
    with table_path.open(newline="", encoding="utf-8") as fp:
        reader = csv.reader(fp, delimiter="\t")
        header = next(reader)
        rows = list(reader)
    mi = header.index("module")
    si = header.index("side")
    ri = header.index("reason")
    for r in rows:
        if r and r[mi].strip() in updates:
            new_s, new_r = updates[r[mi].strip()]
            r[si] = new_s
            r[ri] = new_r
    with table_path.open("w", newline="", encoding="utf-8") as fp:
        fp.write("\t".join(header) + "\n")
        for r in rows:
            if any(c.strip() for c in r):
                fp.write("\t".join(r) + "\n")


def _update_module_includes_tsv(
    root: Path,
    includes_path: Path,
    mod_to_side: dict[str, str],
) -> None:
    """Sync `tools/game_module_includes.tsv` with moved module subtrees."""
    if not includes_path.is_file():
        return
    all_arms, _, resolved_entries, _ = gms.collect_all_arms(root, set(mod_to_side))
    _, _, _, moved_includes, _ = gms.collect_moved_subtrees_and_includes(
        root, mod_to_side, all_arms, resolved_entries
    )
    existing_reasons: dict[tuple[str, str], str] = {}
    with includes_path.open(newline="", encoding="utf-8") as fp:
        reader = csv.DictReader(fp, delimiter="\t", quoting=csv.QUOTE_NONE)
        for row in reader:
            existing_reasons[(row["file"].strip(), row["include_arg"].strip())] = (row.get("reason") or "moved include").strip()

    seen: dict[tuple[str, str], tuple[str, str]] = {}
    for frel, carg, scope_str, _ln in moved_includes:
        key = (frel, carg)
        seen[key] = (scope_str, existing_reasons.get(key, "rehomed module include"))

    out = ["file\tinclude_arg\tscope\treason\n"]
    for (frel, carg), (scope_str, reason) in sorted(seen.items()):
        out.append(f"{frel}\t{carg}\t{scope_str}\t{reason}\n")
    includes_path.write_text("".join(out), encoding="utf-8")


def _write_fn_sides_tsv(table_path: Path, rows: dict[Any, Any]) -> None:
    """Write `tools/game_fn_sides.tsv` in canonical sorted order."""
    out = ["module\timpl\tname\tcfg\tside\treason\n"]
    for k in sorted(rows, key=lambda x: (x.module, x.impl, x.name, x.cfg)):
        r = rows[k]
        out.append(f"{k.module}\t{k.impl}\t{k.name}\t{k.cfg}\t{r.side}\t{r.reason}\n")
    table_path.write_text("".join(out), encoding="utf-8")


def _sync_game_side_calls_tsv(
    root: Path,
    mod_sides: dict[str, str],
    table_path: Path,
    *,
    moved_mods: dict[str, tuple[str, str]],
    moved_fn_keys: set[tuple[str, str, str]],
    mir_callers_by_key: dict[tuple[str, str, str], set[str]] | None = None,
) -> dict[str, int]:
    """Reconcile `tools/game_side_calls.tsv` with the post-rehome side scope.

    Removes orphan rows (items that moved to `game` or left their old side), preserves existing
    rows for untouched items, adds required `trait_impl`/`macro_invoc` rows for newly side-placed
    modules, and keeps wide visibility for moved side functions unless exact MIR callers prove
    all callers are inside the item's own side.
    """
    old_rows, _ = gsc.load_call_table(table_path)
    errors: list[str] = []
    found, _ = gsc.collect_side_scope(root, mod_sides, errors)
    if errors:
        raise RuntimeError("collect_side_scope errors after rehome:\n" + "\n".join(errors[:20]))

    new_rows: dict[Any, Any] = {}
    removed = 0
    preserved = 0
    added_meta = 0
    added_fn_wide = 0
    pruned_own_side = 0

    reg_mod_names = set(mod_sides)
    side_ops_spec = collect_side_ops_spec(root)
    side_ops_arms: set[str] = side_ops_spec.get("server_arms", set()) | side_ops_spec.get("client_arms", set())
    test_called_methods = _collect_test_and_external_method_calls(root)

    def _strip_mirror_mod(mod: str, side: str) -> str:
        top, *rest = mod.split("::")
        suf = f"_{side}"
        if top.endswith(suf) and top not in reg_mod_names:
            top = top[: -len(suf)]
        return "::".join([top, *rest])

    old_by_orig: dict[tuple[str, str, str, str, str], Any] = {}
    for k, r in old_rows.items():
        old_by_orig[(_strip_mirror_mod(k.module, k.side), k.impl, k.name, k.cfg, k.kind)] = r

    for k, r in old_rows.items():
        if k not in found:
            removed += 1
            continue
        item = found[k]
        if k.kind in gsc.FN_LIKE_KINDS and item.vis_class != "wide":
            removed += 1
            continue
        # If exact MIR caller info is supplied, narrow ONLY methods with >=1 MIR caller all inside `k.side`
        # (never narrow `side_ops` arm implementations or methods called by tests/external targets).
        if (
            mir_callers_by_key is not None
            and k.kind == "method"
            and k.name != "new"
            and not (k.impl == "impl Game<'_>" and k.name in side_ops_arms)
        ):
            mkey = (_strip_mirror_mod(k.module, k.side), k.impl, k.name)
            ext_callers = mir_callers_by_key.get(mkey)
            if (
                ext_callers
                and not (set(r.callers) & {"test", "generated"})
                and k.name not in test_called_methods
                and (ext_callers - {k.side}) == set()
            ):
                pruned_own_side += 1
                continue
        new_rows[k] = r
        preserved += 1

    # Handle newly appearing items in `found` (from modules/fns newly moved into `client` or `server`).
    for k, item in sorted(found.items(), key=lambda kv: kv[0].sort_tuple()):
        if k in new_rows:
            continue
        if k.kind in ("trait_impl", "macro_invoc"):
            new_rows[k] = gsc.CallRow(key=k, callers=(), reason="rehomed")
            added_meta += 1
            continue
        if k.kind in gsc.FN_LIKE_KINDS and item.vis_class == "wide":
            orig_mod = _strip_mirror_mod(k.module, k.side)
            orig_key = (orig_mod, k.impl, k.name, k.cfg, k.kind)
            prev_row = old_by_orig.get(orig_key)
            outside: set[str] = set()
            can_narrow = False
            if k.impl == "impl Game<'_>" and k.name in side_ops_arms:
                outside.add("game")
            elif mir_callers_by_key is not None and k.kind == "method" and k.name != "new":
                matched_callers = mir_callers_by_key.get((orig_mod, k.impl, k.name))
                if matched_callers is not None:
                    outside |= matched_callers - {k.side}
                    if prev_row is not None:
                        outside |= (set(prev_row.callers) & {"test", "generated"})
                    if k.name in test_called_methods:
                        outside.add("test")
                    if matched_callers and not outside:
                        can_narrow = True
                else:
                    if prev_row is not None:
                        outside |= set(prev_row.callers) - {k.side}
                        if prev_row.key.side != k.side:
                            outside.add(prev_row.key.side)
                    if k.name in test_called_methods:
                        outside.add("test")
                    if not outside:
                        outside.add("game")
            else:
                if prev_row is not None:
                    outside |= set(prev_row.callers) - {k.side}
                    if prev_row.key.side != k.side:
                        outside.add(prev_row.key.side)
                    outside |= (set(prev_row.callers) & {"test", "generated"})
                    if k.name in test_called_methods:
                        outside.add("test")
                    if not outside and k.kind == "method" and k.name != "new":
                        can_narrow = True
                elif item.vis in ("pub", "pub(crate)"):
                    outside.add("game")
                elif k.kind == "method" and k.name != "new":
                    outside.add("game")
            if not can_narrow:
                if not outside:
                    outside.add("game")
                canon_callers = tuple(c for c in gsc.CALLER_ORDER if c in outside)
                new_rows[k] = gsc.CallRow(key=k, callers=canon_callers, reason="rehomed")
                added_fn_wide += 1

    # Check cfg-twin consistency: if one arm of a cfg-twin is in `new_rows`, include all wide arms.
    twin_groups: dict[tuple[str, str, str, str, str], list[Any]] = defaultdict(list)
    for k, item in found.items():
        if k.kind in ("method", "fn"):
            owner = gfs.owner_overlap_group(k.impl, False) if k.kind == "method" else "-"
            twin_groups[(k.side, k.module, owner, k.name, k.kind)].append(k)
    for _gk, keys in twin_groups.items():
        if len(keys) < 2:
            continue
        listed = [k for k in keys if k in new_rows]
        if listed and len(listed) < len(keys):
            ref_row = new_rows[listed[0]]
            for k in keys:
                if k not in new_rows and found[k].vis_class == "wide":
                    new_rows[k] = gsc.CallRow(key=k, callers=ref_row.callers, reason="rehomed-cfg-twin")
                    added_fn_wide += 1

    table_path.write_text(gsc.format_call_table(list(new_rows.values())), encoding="utf-8")
    return {
        "before_rows": len(old_rows),
        "after_rows": len(new_rows),
        "removed_orphan": removed,
        "pruned_own_side": pruned_own_side,
        "added_meta": added_meta,
        "added_fn_wide": added_fn_wide,
    }


def _compute_post_move_mir_callers_by_key(
    root: Path,
    graph: dict[str, Any],
    mod_sides_after: dict[str, str],
    fn_rows_after: dict[Any, Any],
) -> dict[tuple[str, str, str], set[str]]:
    """Estimate post-move caller labels per exact `(orig_mod, impl, name)` from pre-move MIR + new placement."""
    cid_meta = graph["cid_meta"]
    rev = graph["rev"]
    regular_mods, existing_mirrors, by_top_fnkey, by_mod_name, by_file = index_source_functions(root, mod_sides_after)

    reg_mod_names_after = set(mod_sides_after)
    cid_new_sides: dict[str, set[str]] = {}
    cid_new_key: dict[str, tuple[str, str, str]] = {}
    for cid, meta in cid_meta.items():
        if not cid.startswith("game::"):
            cid_new_sides[cid] = {"game"}
            continue
        orig_mod = _canonical_orig_module_from_cid(cid, reg_mod_names_after)
        fn_tail = cid.rsplit("::", 1)[-1]
        top = orig_mod.split("::", 1)[0]
        cands = by_mod_name.get((orig_mod, fn_tail), ())
        matched = None
        filt: list[Any] = list(cands)
        if len(cands) == 1:
            matched = cands[0]
        elif len(cands) > 1:
            ii = meta.get("impl_info")
            is_method = "::<impl " in cid
            is_trait = bool(ii and ii.get("kind") == "trait")
            filt = [r for r in cands if (r[2].impl != "-") == is_method and r[6] == is_trait]
            if is_method and not is_trait and len(filt) > 1:
                if "::<impl game::Game>::" in cid:
                    gfilt = [r for r in filt if r[2].impl == "impl Game<'_>"]
                    if gfilt:
                        filt = gfilt
                else:
                    ngfilt = [r for r in filt if r[2].impl != "impl Game<'_>"]
                    if ngfilt:
                        filt = ngfilt
            if len(filt) == 1:
                matched = filt[0]
            else:
                same_file = [r for r in (filt or cands) if r[3] == meta["source_file"]]
                if len(same_file) == 1:
                    matched = same_file[0]
                else:
                    for r in same_file or filt or cands:
                        if r[3] == meta["source_file"] and abs(r[4] - meta["line_start"]) <= 5:
                            matched = r
                            break
        if matched is not None:
            loc_s = matched[1]
            new_s = "client" if loc_s == "mirror_client" else ("server" if loc_s == "mirror_server" else loc_s)
            cid_new_sides[cid] = {new_s}
            fk = matched[2]
            cid_new_key[cid] = (fk.module, fk.impl, fk.name)
        elif filt:
            sides_u: set[str] = set()
            for r in filt:
                loc_s = r[1]
                sides_u.add("client" if loc_s == "mirror_client" else ("server" if loc_s == "mirror_server" else loc_s))
            cid_new_sides[cid] = sides_u
        else:
            cid_new_sides[cid] = {mod_sides_after.get(top, "game") if top != "game" else "game"}

    callers_by_key: dict[tuple[str, str, str], set[str]] = defaultdict(set)
    for dst_cid, mkey in cid_new_key.items():
        cs: set[str] = set()
        for src_cid in rev.get(dst_cid, ()):
            if src_cid in cid_meta and "OUT_DIR" in cid_meta[src_cid]["source_file"]:
                cs.add("generated")
            else:
                cs |= cid_new_sides.get(src_cid, {"game"})
        callers_by_key[mkey] |= cs
    return callers_by_key


def apply_rehome(
    root: Path,
    rehome_rows: list[RehomeRow],
    *,
    dry_run: bool,
    strict: bool,
    fmt: bool,
    dump_paths: dict[str, Path] | None = None,
    pre_graph: dict[str, Any] | None = None,
    report_path: Path | None = None,
) -> tuple[int, dict[str, Any]]:
    """Validate and apply (or dry-run) a re-homing table cleanly across all side tools."""
    mod_table_path = root / "tools/game_module_sides.tsv"
    fn_table_path = root / "tools/game_fn_sides.tsv"
    inc_table_path = root / "tools/game_module_includes.tsv"
    calls_table_path = root / "tools/game_side_calls.tsv"
    fields_table_path = root / "tools/game_field_owners.tsv"

    mod_table_full = gms.load_module_sides(mod_table_path)
    mod_sides = {m: s for m, (s, _, _) in mod_table_full.items()}
    fn_rows = gfs.load_fn_sides(fn_table_path)
    field_specs = gms.load_field_specs(fields_table_path)
    regular_mods, existing_mirrors, by_top_fnkey, _by_mod_name, _by_file = index_source_functions(root, mod_sides)
    all_arms, _, resolved_entries, _ = gms.collect_all_arms(root, set(mod_sides))

    applied_mods: list[RehomeRow] = []
    applied_fns: list[RehomeRow] = []
    refusals: list[Refusal] = []

    # 1. Validate module rows
    moved_mods: dict[str, tuple[str, str]] = {}
    mod_updates_tsv: dict[str, tuple[str, str]] = {}
    for r in [x for x in rehome_rows if x.kind == "module"]:
        if r.module == "game":
            refusals.append(Refusal(r.kind, r.module, r.impl, r.name, r.cfg, r.target_side, "root-module-immutable", "cannot re-declare root module `game`"))
            continue
        if r.module not in mod_sides:
            refusals.append(Refusal(r.kind, r.module, r.impl, r.name, r.cfg, r.target_side, "module-not-found", f"module `{r.module}` not in game_module_sides.tsv"))
            continue
        cur_s = mod_sides[r.module]
        if cur_s == r.target_side:
            refusals.append(Refusal(r.kind, r.module, r.impl, r.name, r.cfg, r.target_side, "no-op", f"already `{cur_s}`"))
            continue
        if (cur_s == "game") != (r.target_side == "game") and _check_root_super_super(root, resolved_entries.get(r.module, [])):
            refusals.append(Refusal(r.kind, r.module, r.impl, r.name, r.cfg, r.target_side, "root-super-super", f"module `{r.module}` uses root-depth `super::super`"))
            continue
        moved_mods[r.module] = (cur_s, r.target_side)
        mod_updates_tsv[r.module] = (r.target_side, r.reason)
        applied_mods.append(r)

    # 2. Validate fn rows
    # Group fn rows by top module and check existence + parent side compatibility
    fn_rows_by_top: dict[str, list[RehomeRow]] = defaultdict(list)
    for r in [x for x in rehome_rows if x.kind == "fn"]:
        top = r.module.split("::", 1)[0]
        fk = gfs.FnKey(module=r.module, impl=r.impl, name=r.name, cfg=r.cfg)
        if fk not in by_top_fnkey.get(top, {}):
            refusals.append(Refusal(r.kind, r.module, r.impl, r.name, r.cfg, r.target_side, "fn-not-found", f"function not found in `{top}`"))
            continue
        loc_s = by_top_fnkey[top][fk][0]
        cur_eff = "client" if loc_s == "mirror_client" else ("server" if loc_s == "mirror_server" else loc_s)
        post_parent = moved_mods[top][1] if top in moved_mods else mod_sides.get(top, "game")
        if cur_eff == r.target_side and top not in moved_mods:
            refusals.append(Refusal(r.kind, r.module, r.impl, r.name, r.cfg, r.target_side, "no-op", f"already `{cur_eff}`"))
            continue
        if r.target_side == "game" and post_parent != "game":
            refusals.append(
                Refusal(
                    r.kind,
                    r.module,
                    r.impl,
                    r.name,
                    r.cfg,
                    r.target_side,
                    "side-module-cannot-split-to-common",
                    f"parent module `{top}` stays `{post_parent}`; promote `{top}` to `game` to move fn to common",
                )
            )
            continue
        if r.target_side in ("client", "server") and r.target_side != post_parent and f"{top}_{r.target_side}" in mod_sides:
            refusals.append(
                Refusal(
                    r.kind,
                    r.module,
                    r.impl,
                    r.name,
                    r.cfg,
                    r.target_side,
                    "mirror-name-is-regular-module",
                    f"regular module `{top}_{r.target_side}` already exists in game_module_sides.tsv",
                )
            )
            continue
        fn_rows_by_top[top].append(r)

    # Precheck `game_fn_sides` constraints per top module
    game_rs_path = (root / "crates/sunrise-client/src/game.rs").resolve()
    game_root_tree = gfs.build_mod_tree("game", game_rs_path) if game_rs_path.is_file() else None
    valid_fn_moves_by_top: dict[str, list[RehomeRow]] = defaultdict(list)
    for top, rlist in sorted(fn_rows_by_top.items()):
        post_parent = moved_mods[top][1] if top in moved_mods else mod_sides.get(top, "game")
        desired_check: dict[Any, str] = {}
        for k_ex, r_ex in fn_rows.items():
            if r_ex.top_module == top and r_ex.side in ("client", "server"):
                desired_check[k_ex] = r_ex.side
        for r in rlist:
            fk = gfs.FnKey(module=r.module, impl=r.impl, name=r.name, cfg=r.cfg)
            desired_check[fk] = "game" if r.target_side == post_parent else r.target_side
        bad_fks = _precheck_fn_move_constraints(
            root, top, regular_mods, existing_mirrors, desired_check, game_root_tree
        )
        for r in rlist:
            fk = gfs.FnKey(module=r.module, impl=r.impl, name=r.name, cfg=r.cfg)
            if fk in bad_fks:
                refusals.append(
                    Refusal(
                        r.kind,
                        r.module,
                        r.impl,
                        r.name,
                        r.cfg,
                        r.target_side,
                        "fn-move-constraint",
                        "blocked by location-sensitive macro, relative include, cfg_attr, cfg overlap, or mixed trait impl",
                    )
                )
            else:
                valid_fn_moves_by_top[top].append(r)
                applied_fns.append(r)

    if report_path is not None:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        with report_path.open("w", newline="", encoding="utf-8") as fp:
            fp.write("status\tkind\tmodule\timpl\tname\tcfg\ttarget_side\tcode\tdetail\n")
            for r in applied_mods + applied_fns:
                fp.write(f"applied\t{r.kind}\t{r.module}\t{r.impl}\t{r.name}\t{r.cfg}\t{r.target_side}\tok\t{r.reason}\n")
            for rf in refusals:
                fp.write(f"refused\t{rf.kind}\t{rf.module}\t{rf.impl}\t{rf.name}\t{rf.cfg}\t{rf.target_side}\t{rf.code}\t{rf.detail}\n")

    summary: dict[str, Any] = {
        "applied_modules": len(applied_mods),
        "applied_fns": len(applied_fns),
        "refused": len(refusals),
        "refusals_by_code": dict(Counter(rf.code for rf in refusals)),
        "dissolved_mirrors": [],
    }

    if strict and refusals:
        for rf in refusals[:25]:
            print(f"refused [{rf.code}]: {rf.kind} {rf.module}::{rf.name} -> {rf.target_side}: {rf.detail}", file=sys.stderr)
        return 1, summary

    if dry_run:
        print(
            f"[dry-run] would apply {len(applied_mods)} module move(s) and {len(applied_fns)} fn move(s); "
            f"{len(refusals)} refused ({summary['refusals_by_code']})."
        )
        return 0, summary

    # Load pre-move MIR graph if provided (used to sync `game_side_calls.tsv` accurately after edits).
    if pre_graph is None and dump_paths:
        field_owners = gsl.load_field_owners(fields_table_path)
        pre_graph = build_mir_graph(root, dump_paths, mod_sides, field_owners)

    mirrors_before = {
        (s, m) for s in ("client", "server") for m in existing_mirrors[s]
    }

    # --- STAGE 1: Evacuate existing mirrors of all moving modules, plus single-fn moves on unmoved modules ---
    # Temporarily restoring every mirror of a moving module M back into M guarantees Stage 2 moves M cleanly
    # with zero stale generated `use` lines; Stage 3 then reconstructs surviving and new mirrors canonically.
    phase1_fn_rows = dict(fn_rows)
    evacuated_mirrors: dict[Any, Any] = {}
    for mname in moved_mods:
        for k, r in list(phase1_fn_rows.items()):
            if r.top_module == mname and r.side in ("client", "server"):
                evacuated_mirrors[k] = r
                phase1_fn_rows[k] = gfs.FnRow(key=k, side="game", reason="evacuated-for-module-move")

    # 1b. For any module NOT in `moved_mods`, apply its fn moves right now!
    for top, rlist in valid_fn_moves_by_top.items():
        if top in moved_mods:
            continue
        parent_s = mod_sides.get(top, "game")
        for r in rlist:
            fk = gfs.FnKey(module=r.module, impl=r.impl, name=r.name, cfg=r.cfg)
            fn_tsv_side = "game" if r.target_side == parent_s else r.target_side
            phase1_fn_rows[fk] = gfs.FnRow(key=fk, side=fn_tsv_side, reason=r.reason)

    if phase1_fn_rows != fn_rows:
        _write_fn_sides_tsv(fn_table_path, phase1_fn_rows)
        rc = gfs.run_fix(root, phase1_fn_rows, mod_sides, fmt=False)
        if rc != 0:
            raise RuntimeError("Phase 1 game_fn_sides.run_fix failed")

    # --- STAGE 2: Whole-module re-declarations + leaving-side visibility widening ---
    new_mod_sides = dict(mod_sides)
    vis_changed_files: list[Path] = []
    if moved_mods:
        # 2a. Rewrite `pub(in crate::game::{old_side}_side)` inside every module leaving `client` or `server`.
        for mname, (old_s, new_s) in moved_mods.items():
            if old_s in ("client", "server"):
                sub_files = list(
                    gms.collect_module_subtree_files(root, resolved_entries.get(mname, []), all_arms.get(mname, []))
                )
                target_vis = (
                    f"pub(in crate::game::{SIDE_MOD_NAME[new_s]})"
                    if new_s in SIDE_MOD_NAME
                    else "pub(in crate::game)"
                )
                vis_changed_files.extend(_rewrite_side_vis_in_subtree(sub_files, old_s, target_vis))
            new_mod_sides[mname] = new_s

        # 2b. Update `tools/game_module_sides.tsv` and `tools/game_module_includes.tsv`, run `game_module_sides.run_fix`.
        _write_module_sides_tsv(mod_table_path, mod_updates_tsv)
        _update_module_includes_tsv(root, inc_table_path, new_mod_sides)
        updated_mod_table = gms.load_module_sides(mod_table_path)
        rc = gms.run_fix(root, updated_mod_table, field_specs, fmt=False)
        if rc != 0:
            raise RuntimeError("Phase 2 game_module_sides.run_fix failed")

        # 2c. Rewrite explicit `crate::game::{old_side}_side::<M>::` paths in other modules (e.g. regular side modules).
        vis_changed_files.extend(_rewrite_explicit_mod_paths_in_mirrors(root, moved_mods))

    # --- STAGE 3: Reconstruct surviving & new mirrors for moved modules & clean up temporary `side=game` rows ---
    phase3_fn_rows = dict(phase1_fn_rows)
    # Restore evacuated mirror fns whose side still differs from the module's post-move parent side.
    for k, orig_r in evacuated_mirrors.items():
        post_parent = new_mod_sides.get(orig_r.top_module, "game")
        if orig_r.side != post_parent:
            phase3_fn_rows[k] = orig_r
    for top, rlist in valid_fn_moves_by_top.items():
        if top not in moved_mods:
            continue
        post_parent = new_mod_sides.get(top, "game")
        for r in rlist:
            fk = gfs.FnKey(module=r.module, impl=r.impl, name=r.name, cfg=r.cfg)
            fn_tsv_side = "game" if r.target_side == post_parent else r.target_side
            phase3_fn_rows[fk] = gfs.FnRow(key=fk, side=fn_tsv_side, reason=r.reason)

    # First run with any newly-restored `side=game` rows present so `game_fn_sides` moves them out of mirrors,
    # then drop rehome-restored `side=game` rows so `game_fn_sides.tsv` stays tidy!
    if phase3_fn_rows != phase1_fn_rows:
        _write_fn_sides_tsv(fn_table_path, phase3_fn_rows)
        rc = gfs.run_fix(root, phase3_fn_rows, new_mod_sides, fmt=False)
        if rc != 0:
            raise RuntimeError("Phase 3 game_fn_sides.run_fix failed")

    # Drop `side=game` rows that WE introduced (preserve original `side=game` rows if any, or drop dissolved ones)
    cleaned_fn_rows = {
        k: r
        for k, r in phase3_fn_rows.items()
        if r.side != "game" or (k in fn_rows and fn_rows[k].side == "game")
    }
    if cleaned_fn_rows != phase3_fn_rows:
        _write_fn_sides_tsv(fn_table_path, cleaned_fn_rows)

    # Re-sync module path selectors (e.g. `tools/test_areas.toml`) after Stage 3 mirror updates.
    if moved_mods:
        for sel_path, sel_txt in gms.selector_targets(root, new_mod_sides):
            sel_path.write_text(sel_txt, encoding="utf-8")

    # Record dissolved mirrors
    _reg_after, mirrors_after_map = gfs.parse_top_mod_decls(root, set(new_mod_sides))
    mirrors_after = {(s, m) for s in ("client", "server") for m in mirrors_after_map[s]}
    summary["dissolved_mirrors"] = sorted(f"{s}:{m}" for s, m in (mirrors_before - mirrors_after))
    summary["created_mirrors"] = sorted(f"{s}:{m}" for s, m in (mirrors_after - mirrors_before))

    # --- STAGE 4: Reconcile `tools/game_side_calls.tsv` and narrow unlisted side functions ---
    mir_callers = None
    if pre_graph is not None:
        mir_callers = _compute_post_move_mir_callers_by_key(root, pre_graph, new_mod_sides, cleaned_fn_rows)

    moved_fn_id_set = {(r.module, r.impl, r.name) for r in applied_fns}
    calls_delta = _sync_game_side_calls_tsv(
        root,
        new_mod_sides,
        calls_table_path,
        moved_mods=moved_mods,
        moved_fn_keys=moved_fn_id_set,
        mir_callers_by_key=mir_callers,
    )
    summary["side_calls_delta"] = calls_delta

    new_call_rows, call_errs = gsc.load_call_table(calls_table_path)
    rc = gsc.run_fix(root, new_call_rows, call_errs, new_mod_sides, fmt=False)
    if rc != 0:
        raise RuntimeError("Phase 4 game_side_calls.run_fix failed")

    # --- STAGE 5: Format all changed Rust files ---
    if fmt:
        rustfmt = gsc.resolve_rustfmt()
        if not rustfmt:
            raise RuntimeError("rustfmt not found (pass --no-fmt to skip formatting)")
        proc = subprocess.run(
            ["git", "status", "--porcelain", "--", "crates/sunrise-client/src"],
            cwd=root,
            capture_output=True,
            text=True,
        )
        fmt_paths: list[Path] = list(vis_changed_files)
        if proc.returncode == 0:
            for ln in proc.stdout.splitlines():
                if len(ln) >= 4:
                    p_str = ln[3:].split(" -> ")[-1].strip()
                    p = (root / p_str).resolve()
                    if p.suffix == ".rs" and p.is_file():
                        fmt_paths.append(p)
        unique_fmt = sorted(set(fmt_paths))
        vis_nl_pat = re.compile(r"(pub\s*\(\s*in\s+crate::game(?:::(?:client|server)_side)?\s*\))[ \t]*\n[ \t]+")
        for fp in unique_fmt:
            txt = fp.read_text(encoding="utf-8")
            cleaned = vis_nl_pat.sub(r"\1 ", txt)
            cleaned = re.sub(r"[ \t]+\n", "\n", cleaned)
            if cleaned != txt:
                fp.write_text(cleaned, encoding="utf-8")
        if unique_fmt:
            # Run per-chunk, catching rustfmt's internal trailing-whitespace bug on >100-char
            # tuple-struct `pub(in crate::game) ...` fields by marking the tuple struct `#[rustfmt::skip]`.
            bad_tuple_pat = re.compile(r"error\[internal\]: left behind trailing whitespace\s+-->\s+([^:\n]+):(\d+):")
            for start in range(0, len(unique_fmt), 100):
                chunk = unique_fmt[start : start + 100]
                p_fmt = subprocess.run(
                    [rustfmt, "--edition", "2024", "--config", "skip_children=true", *(str(p) for p in chunk)],
                    capture_output=True,
                    text=True,
                )
                if p_fmt.returncode != 0:
                    matches = bad_tuple_pat.findall(p_fmt.stderr + "\n" + p_fmt.stdout)
                    fixed_any = False
                    for fpath_s, line_s in matches:
                        fp = Path(fpath_s)
                        if not fp.is_file():
                            continue
                        flines = fp.read_text(encoding="utf-8").splitlines(keepends=True)
                        idx = max(0, int(line_s) - 1)
                        # Strip trailing whitespace on the offending line and put `#[rustfmt::skip]` above its struct.
                        if idx < len(flines):
                            flines[idx] = flines[idx].rstrip(" \t\r\n") + "\n"
                            for back in range(idx - 1, max(-1, idx - 6), -1):
                                if re.match(r"^\s*(?:pub(?:\([^)]*\))?\s+)?struct\s+\w+\s*\(", flines[back]):
                                    indent = re.match(r"^(\s*)", flines[back]).group(1)
                                    if back == 0 or "#[rustfmt::skip]" not in flines[back - 1]:
                                        flines.insert(back, f"{indent}#[rustfmt::skip]\n")
                                    fixed_any = True
                                    break
                        fp.write_text("".join(flines), encoding="utf-8")
                    if fixed_any:
                        gsc.run_rustfmt(rustfmt, chunk)
                    else:
                        sys.stderr.write(p_fmt.stderr)
                        raise subprocess.CalledProcessError(p_fmt.returncode, p_fmt.args, p_fmt.stdout, p_fmt.stderr)

    print(
        f"Applied rehome: {len(applied_mods)} module move(s), {len(applied_fns)} fn move(s), "
        f"{len(summary['dissolved_mirrors'])} mirror(s) dissolved, {len(refusals)} refused; "
        f"game_side_calls.tsv: {calls_delta['before_rows']} -> {calls_delta['after_rows']} rows."
    )
    return 0, summary


def sync_tables_to_source(root: Path, *, fmt: bool = True) -> int:
    """Reconcile side-enforcement TSV tables and empty mirror files to match the current Rust source tree."""
    game_dir = root / "crates/sunrise-client/src/game"
    fields_path = root / "tools/game_field_owners.tsv"
    mod_path = root / "tools/game_module_sides.tsv"
    inc_path = root / "tools/game_module_includes.tsv"
    fn_path = root / "tools/game_fn_sides.tsv"
    calls_path = root / "tools/game_side_calls.tsv"

    # 1. Sync `tools/game_field_owners.tsv` for fields present in `ClientState` / `ServerState`.
    if fields_path.is_file():
        with fields_path.open(newline="", encoding="utf-8") as fp:
            rdr = csv.reader(fp, delimiter="\t")
            f_hdr = next(rdr)
            f_rows = [r for r in rdr if any(c.strip() for c in r)]
        fi_name = f_hdr.index("field")
        fi_type = f_hdr.index("type") if "type" in f_hdr else None
        fi_owner = f_hdr.index("owner")
        fi_reason = f_hdr.index("reason")
        fi_idx = f_hdr.index("declaration_index") if "declaration_index" in f_hdr else None
        fi_vis = f_hdr.index("visibility")
        fi_wrap = f_hdr.index("wrapper")
        row_by_field = {r[fi_name].strip(): r for r in f_rows}
        max_idx = max((int(r[fi_idx]) for r in f_rows if fi_idx is not None and r[fi_idx].isdigit()), default=0)

        for side, fname_rs, sname in (
            ("client", "client_side.rs", "ClientState"),
            ("server", "server_side.rs", "ServerState"),
        ):
            sfp = game_dir / fname_rs
            if not sfp.is_file():
                continue
            src = sfp.read_text(encoding="utf-8")
            parsed = gms.parse_struct_fields(src, sname, _rel_str(sfp, root))
            side_mod = SIDE_MOD_NAME[side]
            for fld_name, vis_txt, _vs, ve in parsed:
                vis_norm = re.sub(r"\s+", "", vis_txt)
                vis_col = "side" if vis_norm == f"pub(incrate::game::{side_mod})" else "game"
                if fld_name in row_by_field:
                    r = row_by_field[fld_name]
                    if r[fi_owner].strip() != side:
                        r[fi_owner] = side
                        r[fi_vis] = vis_col
                        r[fi_wrap] = "-"
                else:
                    # Extract rough field type from `name: Type,`
                    m_ty = re.search(rf"\b{re.escape(fld_name)}\s*:\s*([^,\n]+)", src[ve : ve + 300])
                    ty_str = m_ty.group(1).strip() if m_ty else "_"
                    max_idx += 1
                    new_r = [""] * len(f_hdr)
                    new_r[fi_name] = fld_name
                    if fi_type is not None:
                        new_r[fi_type] = ty_str
                    new_r[fi_owner] = side
                    new_r[fi_reason] = f"synced from {sname}"
                    if fi_idx is not None:
                        new_r[fi_idx] = str(max_idx)
                    new_r[fi_vis] = vis_col
                    new_r[fi_wrap] = "-"
                    f_rows.append(new_r)
                    row_by_field[fld_name] = new_r

        with fields_path.open("w", newline="", encoding="utf-8") as fp:
            fp.write("\t".join(f_hdr) + "\n")
            for r in f_rows:
                fp.write("\t".join(r) + "\n")

    # 2. Remove 0-byte / whitespace-only undeclared orphan mirror files.
    old_mod_table = gms.load_module_sides(mod_path)
    old_mod_meta = {m: (s, r, p) for m, (s, r, p) in old_mod_table.items()}
    # Discover actual regular top-level modules from `game.rs`, `client_side.rs`, `server_side.rs`.
    src_root = (root / "crates/sunrise-client/src").resolve()
    raw_arms: list[tuple[str, Any]] = []
    for parent_side, decl_path, base_dir in (
        ("game", src_root / "game.rs", src_root),
        ("client", game_dir / "client_side.rs", game_dir),
        ("server", game_dir / "server_side.rs", game_dir),
    ):
        if not decl_path.is_file():
            continue
        rel = _rel_str(decl_path, root)
        items, _scan_errs = gms.scan_top_level_items(decl_path.read_text(encoding="utf-8"), rel, base_dir)
        for it in items:
            if it.kind == "mod_decl" and it.mod_arm is not None and it.mod_arm.name not in gms.SIDE_CONTAINERS:
                raw_arms.append((parent_side, it.mod_arm))

    raw_names = {arm.name for _, arm in raw_arms} | set(old_mod_table)
    actual_mods: dict[str, tuple[str, list[str]]] = {}
    for parent_side, arm in raw_arms:
        if (
            arm.name not in old_mod_table
            and parent_side in ("client", "server")
            and arm.name.endswith(f"_{parent_side}")
        ):
            base = arm.name[: -len(f"_{parent_side}")]
            if base == "game" or base in raw_names:
                continue
        entry_p, _rerr = gms.resolve_arm_entry_file(root, arm)
        rel_entry = (
            entry_p.resolve().relative_to(src_root).as_posix()
            if entry_p is not None and entry_p.is_file()
            else ""
        )
        if arm.name not in actual_mods:
            actual_mods[arm.name] = (parent_side, [])
        if rel_entry and rel_entry not in actual_mods[arm.name][1]:
            actual_mods[arm.name][1].append(rel_entry)

    # Ensure every side-resident regular module has a `use {client,server}_side::<mod>;` re-export in `game.rs`.
    game_rs_p = src_root / "game.rs"
    if game_rs_p.is_file():
        g_txt = game_rs_p.read_text(encoding="utf-8")
        missing_reexports: list[str] = []
        for mname, (pside, _) in sorted(actual_mods.items()):
            if pside not in ("client", "server"):
                continue
            smod = SIDE_MOD_NAME[pside]
            if not re.search(rf"\buse\s+{smod}::{re.escape(mname)}\s*;", g_txt) and not re.search(
                rf"\bmod\s+{re.escape(mname)}\s*;", g_txt
            ):
                missing_reexports.append(f"#[allow(unused_imports)]\npub(in crate::game) use {smod}::{mname};\n")
        if missing_reexports:
            anchor = "pub(in crate::game) use server_side as server_state;\n"
            ins = "".join(missing_reexports)
            if anchor in g_txt:
                g_txt = g_txt.replace(anchor, anchor + ins, 1)
            else:
                g_txt = g_txt + "\n" + ins
            game_rs_p.write_text(g_txt, encoding="utf-8")

    # Remove empty undeclared mirror files on disk.
    removed_empty = 0
    for p in sorted(game_dir.rglob("*.rs")):
        if not re.search(r"_(?:client|server)\.rs$", p.name):
            continue
        if not p.read_text(encoding="utf-8").strip():
            # Verify it isn't referenced by any active `#[path = "..."]` in side files
            rel_from_game = p.relative_to(game_dir).as_posix()
            c_txt = (game_dir / "client_side.rs").read_text(encoding="utf-8")
            s_txt = (game_dir / "server_side.rs").read_text(encoding="utf-8")
            if rel_from_game not in c_txt and rel_from_game not in s_txt:
                p.unlink()
                removed_empty += 1

    # 3. Write synced `tools/game_module_sides.tsv` and `tools/game_module_includes.tsv`.
    mod_out = ["module\tside\treason\tpaths\n"]
    new_mod_sides: dict[str, str] = {}
    for m in sorted(actual_mods):
        s, plist = actual_mods[m]
        new_mod_sides[m] = s
        if m in old_mod_meta:
            old_s, old_r, old_p = old_mod_meta[m]
            reason = old_r if old_s == s else f"synced from {s}_side" if s != "game" else "synced from game.rs"
            p_str = ",".join(plist) if plist else (",".join(old_p) if old_p else "-")
        else:
            reason = f"synced from {s}_side" if s != "game" else "synced from game.rs"
            p_str = ",".join(plist) if plist else "-"
        mod_out.append(f"{m}\t{s}\t{reason}\t{p_str}\n")
    mod_path.write_text("".join(mod_out), encoding="utf-8")
    _update_module_includes_tsv(root, inc_path, new_mod_sides)
    field_specs = gms.load_field_specs(fields_path)
    updated_mod_table = gms.load_module_sides(mod_path)
    rc = gms.run_fix(root, updated_mod_table, field_specs, fmt=False)
    if rc != 0:
        raise RuntimeError("sync_tables: game_module_sides.run_fix failed")

    # 4. Sync `tools/game_fn_sides.tsv` to match actual functions in declared mirrors, remove 0-fn empty mirrors,
    #    and refresh drifted mirror headers/method order + port_map selectors.
    old_fn_rows = gfs.load_fn_sides(fn_path)
    regular_mods, existing_mirrors = gfs.parse_top_mod_decls(root, set(new_mod_sides))
    synced_fn_rows: dict[Any, Any] = {}
    refreshed_mirrors = 0
    empty_declared_mirrors: list[tuple[str, str, Path]] = []
    for side in ("client", "server"):
        for mname, (mentry, mcfg) in sorted(existing_mirrors[side].items()):
            top = mname[: -len(f"_{side}")]
            if not mentry.is_file():
                continue
            mtree = gfs.build_mod_tree(top, mentry, mcfg, is_mirror=True)
            mfns = gfs.collect_tree_functions(mtree)
            if not mfns:
                empty_declared_mirrors.append((side, mname, mentry))
                continue
            for k in mfns:
                reason = old_fn_rows[k].reason if k in old_fn_rows else "synced from mirror"
                synced_fn_rows[k] = gfs.FnRow(key=k, side=side, reason=reason)

    # Dissolve declared mirror modules that have 0 functions left (only stale `use` imports).
    if empty_declared_mirrors:
        for side, mname, mentry in empty_declared_mirrors:
            for mf in gfs.collect_mirror_files(mentry):
                if mf.is_file():
                    mf.unlink()
                    removed_empty += 1
            side_fp = game_dir / f"{SIDE_MOD_NAME[side]}.rs"
            if side_fp.is_file():
                s_txt = side_fp.read_text(encoding="utf-8")
                s_txt = re.sub(
                    rf"(?m)^(?:#\[[^\n]+\]\n)*pub\(in crate::game\)\s+mod\s+{re.escape(mname)};\n*",
                    "",
                    s_txt,
                )
                side_fp.write_text(s_txt, encoding="utf-8")
        # Re-parse mirror declarations after removing empty ones.
        regular_mods, existing_mirrors = gfs.parse_top_mod_decls(root, set(new_mod_sides))

    _write_fn_sides_tsv(fn_path, synced_fn_rows)

    # Refresh any mirror file whose generated `use` header or method order drifted after hand edits.
    for top in sorted({r.top_module for r in synced_fn_rows.values()}):
        resolved = gfs.resolve_top_entry(root, top, regular_mods)
        if resolved is None:
            continue
        pside, entry_file, top_cfg = resolved
        orig_tree = gfs.build_mod_tree(top, entry_file, top_cfg, root=root)
        orig_fns = gfs.collect_tree_functions(orig_tree, root=root)
        mtrees: dict[str, Any] = {"client": None, "server": None}
        mfns_map: dict[str, dict[Any, Any]] = {"client": {}, "server": {}}
        mfiles: dict[str, set[Path]] = {"client": set(), "server": set()}
        for side in ("client", "server"):
            mn = f"{top}_{side}"
            if mn in existing_mirrors[side]:
                me, mc = existing_mirrors[side][mn]
                if me.is_file():
                    mt = gfs.build_mod_tree(top, me, mc, root=root, is_mirror=True)
                    mtrees[side] = mt
                    mfns_map[side] = gfs.collect_tree_functions(mt, root=root)
                    mfiles[side] = gfs.collect_mirror_files(me)
        cur_place: dict[Any, str] = {}
        for k in orig_fns:
            cur_place[k] = "game"
        for side in ("client", "server"):
            for k in mfns_map[side]:
                cur_place[k] = side
        errs_cap: list[str] = []
        fn_src, fn_nid, tr_src, im_pref = gfs._capture_fn_sources(
            orig_fns, mfns_map, cur_place, errs_cap, check_trait_consistency=False
        )
        if errs_cap:
            continue
        moved_mac, stay_mac, _restored_mac = gfs._collect_macros_for_placement(
            orig_tree, mtrees, cur_place, fn_src
        )
        node_moved: dict[tuple[str, tuple[str, ...], str], list[Any]] = defaultdict(list)
        for side in ("client", "server"):
            for k in mfns_map[side]:
                nid = fn_nid[k]
                node_moved[(nid[0], nid[1], side)].append(k)
        # Pass A: if any drifted root mirror file still uses an extra `use` item that was
        # removed from its parent file, carry it back to `entry_file` before re-rendering
        # BOTH sides' mirrors cleanly from the updated `orig_tree`.
        parent_uses_added = False
        extra_parent_uses_all: list[str] = []
        seen_extra_u: set[str] = set()
        for side in ("client", "server"):
            if mtrees[side] is None:
                continue
            root_mirror_file = mtrees[side].file_path
            exp_files = gfs._render_mirror_tree(
                orig_tree, side, top, pside, regular_mods, cur_place, orig_fns, mfns_map,
                fn_src, fn_nid, tr_src, im_pref, moved_mac, stay_mac, node_moved,
            )
            if root_mirror_file in exp_files:
                act_txt = root_mirror_file.read_text(encoding="utf-8")
                if gfs.diff_mirror_file(_rel_str(root_mirror_file, root), act_txt, exp_files[root_mirror_file], allow_reorder_methods=False):
                    try:
                        act_items = gfs.parse_rust_items(act_txt)
                        exp_items = gfs.parse_rust_items(exp_files[root_mirror_file])
                    except ValueError:
                        act_items, exp_items = [], []
                    exp_use_set = {gfs._canonical_item_tokens(exp_files[root_mirror_file], it) for it in exp_items if it.kind == "use"}
                    body_txt = "\n".join(act_txt[it.start : it.end] for it in act_items if it.kind != "use")
                    side_mod_s = SIDE_MOD_NAME[side]
                    for it in act_items:
                        if it.kind != "use":
                            continue
                        if gfs._canonical_item_tokens(act_txt, it) in exp_use_set:
                            continue
                        raw_u = act_txt[it.start : it.end]
                        if f"crate::game::{top}" in raw_u or f"crate::game::{SIDE_MOD_NAME.get(pside, '')}::{top}" in raw_u:
                            continue
                        u_body = re.sub(rf"^.*?use\s+", "", raw_u, flags=re.S).rstrip(";").strip()
                        idents = [w for w in re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\b", u_body) if w not in ("super", "crate", "self", "as")]
                        if any(re.search(rf"\b{re.escape(w)}\b", body_txt) for w in idents):
                            cleaned_u = re.sub(rf"pub\s*\(\s*in\s+crate::game::{side_mod_s}\s*\)\s*", "", raw_u).strip()
                            if "#[allow(unused_imports)]" not in cleaned_u:
                                cleaned_u = f"#[allow(unused_imports)]\n{cleaned_u}"
                            if cleaned_u not in seen_extra_u:
                                seen_extra_u.add(cleaned_u)
                                extra_parent_uses_all.append(cleaned_u)
        if extra_parent_uses_all and entry_file.is_file():
            p_txt = entry_file.read_text(encoding="utf-8")
            try:
                p_items = gfs.parse_rust_items(p_txt)
            except ValueError:
                p_items = []
            p_use_items = [it for it in p_items if it.kind == "use"]

            def _use_root_and_idents(u_str: str) -> tuple[str, set[str]]:
                body = re.sub(r"^.*?use\s+", "", u_str, flags=re.S).rstrip(";").strip()
                m_rt = re.match(r"([A-Za-z_][A-Za-z0-9_]*)", body)
                rt = m_rt.group(1) if m_rt else ""
                ids = {w for w in re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\b", body) if w not in ("super", "crate", "self", "as", rt)}
                return rt, ids

            # Replace an existing parent `use` in place if the mirror's `use` is a strict superset
            # with the same crate root (avoids E0252 duplicate imports), else prepend remaining uses.
            replacements: list[tuple[int, int, str]] = []
            to_prepend: list[str] = []
            p_existing_ids: set[str] = set()
            for pit in p_use_items:
                _, pids = _use_root_and_idents(p_txt[pit.start : pit.end])
                p_existing_ids |= pids
            for cu in extra_parent_uses_all:
                crt, cids = _use_root_and_idents(cu)
                if cids and cids.issubset(p_existing_ids):
                    continue
                replaced = False
                for pit in p_use_items:
                    prt, pids = _use_root_and_idents(p_txt[pit.start : pit.end])
                    if prt == crt and pids and pids.issubset(cids):
                        replacements.append((pit.start, pit.end, cu))
                        replaced = True
                        break
                if not replaced:
                    to_prepend.append(cu)
            if replacements or to_prepend:
                for st, en, rep in sorted(replacements, key=lambda x: -x[0]):
                    p_txt = p_txt[:st] + rep + p_txt[en:]
                if to_prepend:
                    m_first_u = re.search(r"(?m)^(?:#\[allow\(unused_imports\)\]\n)?use\s+", p_txt)
                    ins_u = "\n".join(to_prepend) + "\n"
                    if m_first_u:
                        p_txt = p_txt[: m_first_u.start()] + ins_u + p_txt[m_first_u.start() :]
                    else:
                        p_txt = ins_u + p_txt
                entry_file.write_text(p_txt, encoding="utf-8")
                orig_tree = gfs.build_mod_tree(top, entry_file, top_cfg, root=root)
                orig_fns = gfs.collect_tree_functions(orig_tree, root=root)
                parent_uses_added = True

        # Pass B: re-render every drifted mirror file across BOTH sides against the final `orig_tree`.
        for side in ("client", "server"):
            if mtrees[side] is None:
                continue
            exp_files = gfs._render_mirror_tree(
                orig_tree, side, top, pside, regular_mods, cur_place, orig_fns, mfns_map,
                fn_src, fn_nid, tr_src, im_pref, moved_mac, stay_mac, node_moved,
            )
            for mf in sorted(mfiles[side]):
                if mf in exp_files:
                    act_txt = mf.read_text(encoding="utf-8")
                    if parent_uses_added or gfs.diff_mirror_file(_rel_str(mf, root), act_txt, exp_files[mf], allow_reorder_methods=False):
                        mf.write_text(exp_files[mf], encoding="utf-8")
                        refreshed_mirrors += 1

    # Sync moved-function port_map / test selectors via `game_fn_sides.run_fix` (no-op on Rust bodies).
    rc = gfs.run_fix(root, synced_fn_rows, new_mod_sides, fmt=False)
    if rc != 0:
        raise RuntimeError("sync_tables: game_fn_sides.run_fix failed")

    # 5. Sync `tools/game_side_calls.tsv` conservatively and apply narrowing for resolved rows.
    calls_delta = _sync_game_side_calls_tsv(
        root,
        new_mod_sides,
        calls_path,
        moved_mods={},
        moved_fn_keys=set(),
        mir_callers_by_key=None,
    )
    new_call_rows, call_errs = gsc.load_call_table(calls_path)
    rc = gsc.run_fix(root, new_call_rows, call_errs, new_mod_sides, fmt=False)
    if rc != 0:
        raise RuntimeError("sync_tables: game_side_calls.run_fix failed")
    if fmt:
        rustfmt = gsc.resolve_rustfmt()
        if rustfmt:
            proc = subprocess.run(
                ["git", "status", "--porcelain", "--", "crates/sunrise-client/src"],
                cwd=root,
                capture_output=True,
                text=True,
            )
            fpaths = []
            if proc.returncode == 0:
                for ln in proc.stdout.splitlines():
                    if len(ln) >= 4:
                        p = (root / ln[3:].split(" -> ")[-1].strip()).resolve()
                        if p.suffix == ".rs" and p.is_file():
                            fpaths.append(p)
            if fpaths:
                gsc.run_rustfmt(rustfmt, fpaths)

    print(
        f"Synced tables: {len(new_mod_sides)} modules, {len(synced_fn_rows)} mirror fns, "
        f"{removed_empty} empty orphan mirror(s) removed, {refreshed_mirrors} mirror header(s) refreshed, "
        f"game_side_calls.tsv: {calls_delta['before_rows']} -> {calls_delta['after_rows']} rows."
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Role-reachability analyzer and re-homing tool for sunrise-client gameplay code."
    )
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--analyze", action="store_true", help="compute role reachability, contradictions, and safe rehome table")
    mode.add_argument("--dry-run", action="store_true", help="validate a rehome table and report movable vs refused items")
    mode.add_argument("--apply", action="store_true", help="apply a rehome table to the repository")
    mode.add_argument("--run", action="store_true", help="one-command pipeline: optional --sync-first, --analyze, --apply, port-map refresh, and all checks")
    mode.add_argument("--check", action="store_true", help="verify rehome table has already been applied (no pending moves)")
    mode.add_argument("--sync-tables", action="store_true", help="reconcile side-enforcement TSV tables to match current Rust source")

    parser.add_argument("--root", type=Path, default=ROOT, help="repository root directory")
    parser.add_argument("--table", type=Path, default=None, help="rehome TSV path (default: tools/game_rehome.tsv)")
    parser.add_argument("--dump-dir", type=Path, default=None, help="directory with <config>.mir dumps")
    parser.add_argument("--dump", action="append", default=[], help="explicit CONFIG=PATH MIR dump")
    parser.add_argument("--configs", default="default", help="comma-separated feature configs (default: default)")
    parser.add_argument("--fire-sides", type=Path, default=None, help="optional fire_sides.json for mod-hook sensitivity (default: live from hooks)")
    parser.add_argument("--primary-mode", default="S1_role_branch", help="reachability mode for primary labels")
    parser.add_argument("--out-rehome", type=Path, default=None, help="output path for generated safe rehome TSV")
    parser.add_argument("--out-contradictions", type=Path, default=None, help="output path for contradictions TSV")
    parser.add_argument("--out-reach", type=Path, default=None, help="output path for per-function reachability TSV")
    parser.add_argument("--out-json", type=Path, default=None, help="output path for JSON summary")
    parser.add_argument("--report", type=Path, default=None, help="output TSV for applied/refused items")
    parser.add_argument("--sync-first", action="store_true", help="with --run: run --sync-tables before analyzing")
    parser.add_argument("--strict", action="store_true", help="fail if any table row is refused")
    parser.add_argument("--no-fmt", action="store_true", help="skip rustfmt after --apply")
    parser.add_argument(
        "--no-promote-side-modules",
        action="store_true",
        help="do not promote client/server modules to game with retained mirrors during --analyze",
    )
    args = parser.parse_args(argv)

    root = args.root.resolve()
    table_path = args.table or (root / "tools/game_rehome.tsv" if args.root != ROOT else DEFAULT_REHOME_TABLE)

    dump_paths: dict[str, Path] | None = None
    if args.dump_dir is not None or args.dump:
        cfgs = tuple(c.strip() for c in args.configs.split(",") if c.strip())
        dump_paths = dict(gsl.resolve_dump_paths(args.dump_dir, args.dump, cfgs))

    if args.sync_tables:
        return sync_tables_to_source(root, fmt=not args.no_fmt)

    if args.analyze or args.run:
        if args.run and args.sync_first:
            rc_sync = sync_tables_to_source(root, fmt=not args.no_fmt)
            if rc_sync != 0:
                return rc_sync
        if not dump_paths:
            parser.error("--analyze / --run requires --dump-dir DIR or --dump CONFIG=PATH")
        mod_sides = gsl.load_module_sides(root / "tools/game_module_sides.tsv")
        field_owners = gsl.load_field_owners(root / "tools/game_field_owners.tsv")
        graph = build_mir_graph(root, dump_paths, mod_sides, field_owners)
        reach_res = compute_role_reachability(
            graph, fire_sides_path=args.fire_sides, root=root, mod_sides=mod_sides
        )
        analysis = compute_labels_and_contradictions(graph, reach_res, primary_mode=args.primary_mode)
        safe_rows, plan_refusals, plan_stats = plan_safe_rehome(
            root,
            graph,
            reach_res,
            analysis,
            mod_sides,
            field_owners=field_owners,
            promote_side_modules=not args.no_promote_side_modules,
        )

        print("=== Reachability Sensitivity ===")
        for mname, counts in reach_res["summary"].items():
            print(
                f"  {mname:22s} C_only={counts['client_only']:5d} S_only={counts['server_only']:5d} "
                f"Both={counts['both']:5d} Neither={counts['neither']:5d}"
            )
        print(f"Contradictions ({args.primary_mode}): {len(analysis['contradictions'])}")
        print(f"Safe rehome plan: {plan_stats}")

        out_rehome = args.out_rehome or (table_path if args.run else None)
        if out_rehome is not None:
            out_rehome.parent.mkdir(parents=True, exist_ok=True)
            out_rehome.write_text(format_rehome_table(safe_rows), encoding="utf-8")
            print(f"Wrote {len(safe_rows)} rows to {out_rehome}")

        if args.out_contradictions is not None:
            args.out_contradictions.parent.mkdir(parents=True, exist_ok=True)
            with args.out_contradictions.open("w", newline="", encoding="utf-8") as fp:
                fp.write(
                    "function_id\tcurrent_side\treach\tcontradiction_kind\thas_direct_field\t"
                    "source_file\tline\tclient_fields\tserver_fields\tclient_callees\tserver_callees\n"
                )
                for c in analysis["contradictions"]:
                    fp.write(
                        "\t".join(
                            [
                                c["cid"],
                                c["current_side"],
                                c["reach"],
                                c["contradiction_kind"],
                                c["has_direct_field"],
                                c["source_file"],
                                str(c["line_start"]),
                                ",".join(c["client_fields"]),
                                ",".join(c["server_fields"]),
                                ",".join(c["client_callees"]),
                                ",".join(c["server_callees"]),
                            ]
                        )
                        + "\n"
                    )
            print(f"Wrote {len(analysis['contradictions'])} contradictions to {args.out_contradictions}")

        if args.out_reach is not None:
            args.out_reach.parent.mkdir(parents=True, exist_ok=True)
            with args.out_reach.open("w", newline="", encoding="utf-8") as fp:
                fp.write("function_id\tcurrent_side\treach\ttarget_label\tsource_file\tline\tclient_fields\tserver_fields\n")
                for n, rec in sorted(analysis["node_records"].items()):
                    fp.write(
                        "\t".join(
                            [
                                n,
                                rec["current_side"],
                                rec["reach"],
                                rec["target_label"],
                                rec["source_file"],
                                str(rec["line_start"]),
                                ",".join(rec["client_fields"]),
                                ",".join(rec["server_fields"]),
                            ]
                        )
                        + "\n"
                    )
            print(f"Wrote {len(analysis['node_records'])} reachability rows to {args.out_reach}")

        if args.out_json is not None:
            args.out_json.parent.mkdir(parents=True, exist_ok=True)
            payload = {
                "configs": sorted(dump_paths),
                "primary_mode": args.primary_mode,
                "game_nodes": len(reach_res["game_nodes"]),
                "sensitivity": reach_res["summary"],
                "contradiction_count": len(analysis["contradictions"]),
                "contradictions_by_kind": dict(Counter(c["contradiction_kind"] for c in analysis["contradictions"])),
                "direct_field_contradictions_by_kind": dict(
                    Counter(c["contradiction_kind"] for c in analysis["contradictions"] if c["has_direct_field"] == "yes")
                ),
                "plan_stats": plan_stats,
                "plan_refusals": [rf.__dict__ for rf in plan_refusals],
            }
            args.out_json.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
            print(f"Wrote JSON summary to {args.out_json}")

        if not args.run:
            return 0

        rc_apply, _summ = apply_rehome(
            root,
            safe_rows,
            dry_run=False,
            strict=args.strict,
            fmt=not args.no_fmt,
            dump_paths=dump_paths,
            pre_graph=graph,
            report_path=args.report,
        )
        if rc_apply != 0:
            return rc_apply
        port_map_script = root / "tools/ci/check_port_map_paths.py"
        if port_map_script.is_file():
            subprocess.run([sys.executable, str(port_map_script), "--refresh"], cwd=root, check=False)
        for tool_name in (
            "tools/game_field_owners.py",
            "tools/game_module_sides.py",
            "tools/game_fn_sides.py",
            "tools/game_side_calls.py",
        ):
            tpath = root / tool_name
            if tpath.is_file():
                p = subprocess.run([sys.executable, str(tpath), "--check"], cwd=root)
                if p.returncode != 0:
                    return p.returncode
        if port_map_script.is_file():
            p = subprocess.run([sys.executable, str(port_map_script)], cwd=root)
            if p.returncode != 0:
                return p.returncode
        return 0

    rows = load_rehome_table(table_path)
    if args.check:
        rc, summ = apply_rehome(root, rows, dry_run=True, strict=False, fmt=False)
        pending = summ["applied_modules"] + summ["applied_fns"]
        if pending > 0:
            print(f"error: {pending} pending rehome move(s) remain for {table_path}", file=sys.stderr)
            return 1
        print(f"OK: rehome table {table_path} satisfied ({summ['refused']} no-op/refused).")
        return 0

    rc, _summ = apply_rehome(
        root,
        rows,
        dry_run=args.dry_run,
        strict=args.strict,
        fmt=not args.no_fmt,
        dump_paths=dump_paths,
        report_path=args.report,
    )
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
