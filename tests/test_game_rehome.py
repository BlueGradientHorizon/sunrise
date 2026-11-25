"""Unit tests for `tools/game_rehome.py` on synthetic gameplay trees."""

from __future__ import annotations

import importlib.util
import io
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _load_tool(name: str, rel: str):
    path = ROOT / rel
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


gr = _load_tool("game_rehome", "tools/game_rehome.py")
gms = _load_tool("game_module_sides", "tools/game_module_sides.py")
gfs = _load_tool("game_fn_sides", "tools/game_fn_sides.py")
gsc = _load_tool("game_side_calls", "tools/game_side_calls.py")


def _write_tree(root: Path, files: dict[str, str]) -> None:
    for rel, content in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")


def _make_synthetic_repo(root: Path) -> None:
    """Build a minimal valid gameplay tree with common, client, server modules and mirror files."""
    _write_tree(
        root,
        {
            "crates/sunrise-client/src/game.rs": """//! Game root.
#[path = "game/client_side.rs"]
pub(in crate::game) mod client_side;
#[allow(unused_imports)]
pub(in crate::game) use client_side as client_state;
#[path = "game/server_side.rs"]
pub(in crate::game) mod server_side;
#[allow(unused_imports)]
pub(in crate::game) use server_side as server_state;
#[allow(unused_imports)]
pub(in crate::game) use client_side::misplaced_srv;
#[path = "game/shared_mod.rs"]
mod shared_mod;
#[allow(unused_imports)]
pub(in crate::game) use server_side::srv_only;

pub struct Game<'a> {
    pub(in crate::game) client: client_side::ClientState,
    pub(in crate::game) server: server_side::ServerState,
    pub(in crate::game) _p: std::marker::PhantomData<&'a ()>,
}
""",
            "crates/sunrise-client/src/game/client_side.rs": """//! Client side.
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;

#[path = "misplaced_srv.rs"]
pub(in crate::game) mod misplaced_srv;

#[path = "shared_mod_client.rs"]
pub(in crate::game) mod shared_mod_client;

pub(in crate::game) struct ClientState {
    pub(in crate::game::client_side) c_val: bool,
}

impl ClientState {
    pub(in crate::game) fn new() -> Self {
        Self { c_val: false }
    }
}
""",
            "crates/sunrise-client/src/game/server_side.rs": """//! Server side.
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;

#[path = "srv_only.rs"]
pub(in crate::game) mod srv_only;

pub(in crate::game) struct ServerState {
    pub(in crate::game::server_side) s_val: u32,
}

impl ServerState {
    pub(in crate::game) fn new() -> Self {
        Self { s_val: 0 }
    }
}
""",
            "crates/sunrise-client/src/game/misplaced_srv.rs": """use super::*;

impl Game<'_> {
    pub(in crate::game) fn called_by_server(&mut self) {}

    pub(in crate::game::client_side) fn internal_helper(&self) -> u32 {
        42
    }
}
""",
            "crates/sunrise-client/src/game/shared_mod.rs": """use super::*;

impl Game<'_> {
    pub(in crate::game) fn stays_common(&self) {}

    pub(in crate::game) fn only_server_calls(&self) {}
}
""",
            "crates/sunrise-client/src/game/shared_mod_client.rs": """#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::shared_mod::*;

impl Game<'_> {
    pub(in crate::game) fn wrongly_in_client_mirror(&self) {}
}
""",
            "crates/sunrise-client/src/game/srv_only.rs": """use super::*;

impl Game<'_> {
    pub(in crate::game::server_side) fn pure_server(&mut self) {
        self.server.s_val += 1;
    }

    pub(in crate::game) fn shared_helper_inside_srv(&self) -> u32 {
        7
    }
}
""",
            "tools/game_field_owners.tsv": (
                "field\towner\tvisibility\twrapper\treason\n"
                "c_val\tclient\tside\t-\ttest\n"
                "s_val\tserver\tside\t-\ttest\n"
            ),
            "tools/game_module_sides.tsv": (
                "module\tside\treason\tpaths\n"
                "misplaced_srv\tclient\ttest\tgame/misplaced_srv.rs\n"
                "shared_mod\tgame\ttest\tgame/shared_mod.rs\n"
                "srv_only\tserver\ttest\tgame/srv_only.rs\n"
            ),
            "tools/game_module_includes.tsv": "file\tinclude_arg\tscope\treason\n",
            "tools/game_fn_sides.tsv": (
                "module\timpl\tname\tcfg\tside\treason\n"
                "shared_mod\timpl Game<'_>\twrongly_in_client_mirror\t-\tclient\ttest\n"
            ),
            "tools/game_side_calls.tsv": (
                "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                "client\tmisplaced_srv\timpl Game<'_>\tcalled_by_server\t-\tmethod\tserver\tcalled\n"
                "client\tshared_mod_client\timpl Game<'_>\twrongly_in_client_mirror\t-\tmethod\tserver\tcalled\n"
                "server\tsrv_only\timpl Game<'_>\tshared_helper_inside_srv\t-\tmethod\tclient\tcalled\n"
            ),
        },
    )


class TestGameRehome(unittest.TestCase):
    def test_module_rehome_mirror_dissolve_and_fn_split(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _make_synthetic_repo(root)
            # Verify initial checks pass.
            self.assertEqual(gms.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gfs.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gsc.main(["--check", "--root", str(root)]), 0)

            rehome_tsv = root / "tools/game_rehome.tsv"
            rehome_tsv.write_text(
                "kind\tmodule\timpl\tname\tcfg\tcurrent_side\ttarget_side\treason\n"
                "module\tmisplaced_srv\t-\t-\t-\tclient\tserver\trole-reachability:server\n"
                "fn\tshared_mod\timpl Game<'_>\twrongly_in_client_mirror\t-\tclient\tgame\trole-reachability:common\n"
                "fn\tshared_mod\timpl Game<'_>\tonly_server_calls\t-\tgame\tserver\trole-reachability:server\n",
                encoding="utf-8",
            )
            rows = gr.load_rehome_table(rehome_tsv)
            rc, summ = gr.apply_rehome(root, rows, dry_run=False, strict=True, fmt=False)
            self.assertEqual(rc, 0)
            self.assertEqual(summ["applied_modules"], 1)
            self.assertEqual(summ["applied_fns"], 2)
            self.assertEqual(summ["refused"], 0)
            self.assertEqual(summ["dissolved_mirrors"], ["client:shared_mod_client"])

            # Empty mirror file dissolved and removed from client_side.rs.
            self.assertFalse((root / "crates/sunrise-client/src/game/shared_mod_client.rs").exists())
            self.assertNotIn("shared_mod_client", (root / "crates/sunrise-client/src/game/client_side.rs").read_text())

            # Restored function is back in shared_mod.rs; only_server_calls moved to shared_mod_server.rs.
            shared_txt = (root / "crates/sunrise-client/src/game/shared_mod.rs").read_text()
            self.assertIn("fn wrongly_in_client_mirror", shared_txt)
            self.assertNotIn("fn only_server_calls", shared_txt)
            srv_mirror = root / "crates/sunrise-client/src/game/shared_mod_server.rs"
            self.assertTrue(srv_mirror.is_file())
            self.assertIn("fn only_server_calls", srv_mirror.read_text())

            # Moved module `misplaced_srv` moved to server_side and lost `pub(in crate::game::client_side)`.
            misplaced_txt = (root / "crates/sunrise-client/src/game/misplaced_srv.rs").read_text()
            self.assertNotIn("crate::game::client_side", misplaced_txt)
            self.assertIn("pub(in crate::game::server_side) fn internal_helper", misplaced_txt)
            self.assertIn("mod misplaced_srv;", (root / "crates/sunrise-client/src/game/server_side.rs").read_text())

            # All three side tools pass cleanly after re-homing!
            self.assertEqual(gms.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gfs.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gsc.main(["--check", "--root", str(root)]), 0)

            # `game_side_calls.tsv` dropped `wrongly_in_client_mirror` (moved to common), kept
            # `shared_helper_inside_srv`, and without MIR conservatively kept `called_by_server`
            # wide with `client` (its old own side) and `only_server_calls` wide with `game`.
            sc_rows, _ = gsc.load_call_table(root / "tools/game_side_calls.tsv")
            by_name = {k.name: r for k, r in sc_rows.items()}
            self.assertIn("shared_helper_inside_srv", by_name)
            self.assertIn("called_by_server", by_name)
            self.assertEqual(by_name["called_by_server"].callers, ("client",))
            self.assertNotIn("wrongly_in_client_mirror", by_name)

    def test_honest_refusals(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _make_synthetic_repo(root)
            rehome_tsv = root / "tools/game_rehome.tsv"
            rehome_tsv.write_text(
                "kind\tmodule\timpl\tname\tcfg\tcurrent_side\ttarget_side\treason\n"
                "module\tgame\t-\t-\t-\tgame\tserver\ttest\n"
                "module\tmissing_mod\t-\t-\t-\tclient\tserver\ttest\n"
                "fn\tsrv_only\timpl Game<'_>\tshared_helper_inside_srv\t-\tserver\tgame\ttest\n"
                "fn\tshared_mod\timpl Game<'_>\tmissing_fn\t-\tgame\tclient\ttest\n",
                encoding="utf-8",
            )
            report = root / "report.tsv"
            rows = gr.load_rehome_table(rehome_tsv)
            rc, summ = gr.apply_rehome(root, rows, dry_run=True, strict=False, fmt=False, report_path=report)
            self.assertEqual(rc, 0)
            self.assertEqual(summ["applied_modules"], 0)
            self.assertEqual(summ["applied_fns"], 0)
            self.assertEqual(summ["refused"], 4)
            self.assertEqual(
                summ["refusals_by_code"],
                {
                    "root-module-immutable": 1,
                    "module-not-found": 1,
                    "side-module-cannot-split-to-common": 1,
                    "fn-not-found": 1,
                },
            )
            self.assertTrue(report.is_file())
            self.assertIn("side-module-cannot-split-to-common", report.read_text())

    def test_promote_side_module_to_common_with_retained_mirror(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _make_synthetic_repo(root)
            rehome_tsv = root / "tools/game_rehome.tsv"
            rehome_tsv.write_text(
                "kind\tmodule\timpl\tname\tcfg\tcurrent_side\ttarget_side\treason\n"
                "module\tsrv_only\t-\t-\t-\tserver\tgame\tpromote-for-common-fns:1\n"
                "fn\tsrv_only\timpl Game<'_>\tpure_server\t-\tserver\tserver\tretain-in-server-mirror\n",
                encoding="utf-8",
            )
            rows = gr.load_rehome_table(rehome_tsv)
            rc, summ = gr.apply_rehome(root, rows, dry_run=False, strict=True, fmt=False)
            self.assertEqual(rc, 0)
            self.assertEqual(summ["applied_modules"], 1)
            self.assertEqual(summ["applied_fns"], 1)
            self.assertIn("fn shared_helper_inside_srv", (root / "crates/sunrise-client/src/game/srv_only.rs").read_text())
            self.assertIn("fn pure_server", (root / "crates/sunrise-client/src/game/srv_only_server.rs").read_text())
            self.assertEqual(gms.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gfs.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gsc.main(["--check", "--root", str(root)]), 0)

    def test_promote_module_with_existing_opposite_mirror(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _make_synthetic_repo(root)
            # Add an opposite-side mirror (`srv_only_client.rs`) plus a side-narrow struct in `srv_only.rs`.
            _write_tree(
                root,
                {
                    "crates/sunrise-client/src/game/srv_only.rs": """use super::*;

pub(in crate::game::server_side) struct InternalTag;

impl Game<'_> {
    pub(in crate::game::server_side) fn pure_server(&mut self) {
        self.server.s_val += 1;
    }

    pub(in crate::game) fn shared_helper_inside_srv(&self) -> u32 {
        7
    }
}
""",
                    "crates/sunrise-client/src/game/srv_only_client.rs": """#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::srv_only::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::srv_only::InternalTag;

impl Game<'_> {
    pub(in crate::game) fn client_only_mirror_fn(&mut self) {
        self.client.c_val = true;
    }
}
""",
                },
            )
            cs = root / "crates/sunrise-client/src/game/client_side.rs"
            cs.write_text(
                cs.read_text().replace(
                    'pub(in crate::game) mod shared_mod_client;\n',
                    'pub(in crate::game) mod shared_mod_client;\n\n#[path = "srv_only_client.rs"]\npub(in crate::game) mod srv_only_client;\n',
                ),
                encoding="utf-8",
            )
            fn_tsv = root / "tools/game_fn_sides.tsv"
            fn_tsv.write_text(
                fn_tsv.read_text()
                + "srv_only\timpl Game<'_>\tclient_only_mirror_fn\t-\tclient\ttest\n",
                encoding="utf-8",
            )
            sc_tsv = root / "tools/game_side_calls.tsv"
            sc_lines = sc_tsv.read_text().splitlines()
            sc_lines.append("client\tsrv_only_client\timpl Game<'_>\tclient_only_mirror_fn\t-\tmethod\tgame\tcalled")
            sc_tsv.write_text("\n".join([sc_lines[0], *sorted(sc_lines[1:])]) + "\n", encoding="utf-8")
            self.assertEqual(gms.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gfs.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gsc.main(["--check", "--root", str(root)]), 0)

            rehome_tsv = root / "tools/game_rehome.tsv"
            rehome_tsv.write_text(
                "kind\tmodule\timpl\tname\tcfg\tcurrent_side\ttarget_side\treason\n"
                "module\tsrv_only\t-\t-\t-\tserver\tgame\tpromote-for-common-fns:1\n"
                "fn\tsrv_only\timpl Game<'_>\tpure_server\t-\tserver\tserver\tretain-in-server-mirror\n",
                encoding="utf-8",
            )
            rows = gr.load_rehome_table(rehome_tsv)
            rc, summ = gr.apply_rehome(root, rows, dry_run=False, strict=True, fmt=False)
            self.assertEqual(rc, 0)
            self.assertIn("pub(in crate::game) struct InternalTag;", (root / "crates/sunrise-client/src/game/srv_only.rs").read_text())
            self.assertIn("fn pure_server", (root / "crates/sunrise-client/src/game/srv_only_server.rs").read_text())
            self.assertIn("fn client_only_mirror_fn", (root / "crates/sunrise-client/src/game/srv_only_client.rs").read_text())
            self.assertIn("use crate::game::srv_only::InternalTag;", (root / "crates/sunrise-client/src/game/srv_only_client.rs").read_text())
            self.assertEqual(gms.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gfs.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gsc.main(["--check", "--root", str(root)]), 0)

    def test_sync_tables_reconciles_stale_tables_and_empty_mirrors(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _make_synthetic_repo(root)
            # Simulate hand-edits: add a new ServerState field, a new server module, an empty orphan mirror, and a stale fn_sides row.
            (root / "crates/sunrise-client/src/game/new_srv.rs").write_text(
                "use super::*;\nimpl Game<'_> { pub(in crate::game) fn srv_ping(&self) {} }\n",
                encoding="utf-8",
            )
            (root / "crates/sunrise-client/src/game/stale_orphan_client.rs").write_text("", encoding="utf-8")
            ss = root / "crates/sunrise-client/src/game/server_side.rs"
            ss.write_text(
                ss.read_text()
                .replace(
                    'pub(in crate::game) mod srv_only;\n',
                    'pub(in crate::game) mod srv_only;\n#[path = "new_srv.rs"]\npub(in crate::game) mod new_srv;\n',
                )
                .replace(
                    "    pub(in crate::game::server_side) s_val: u32,\n",
                    "    pub(in crate::game::server_side) s_val: u32,\n    pub(in crate::game::server_side) extra_srv: u64,\n",
                )
                .replace("Self { s_val: 0 }", "Self { s_val: 0, extra_srv: 0 }"),
                encoding="utf-8",
            )
            fn_tsv = root / "tools/game_fn_sides.tsv"
            fn_tsv.write_text(
                fn_tsv.read_text() + "srv_only\timpl Game<'_>\tdeleted_fn\t-\tclient\tstale\n",
                encoding="utf-8",
            )
            self.assertNotEqual(gms.main(["--check", "--root", str(root)]), 0)
            self.assertNotEqual(gfs.main(["--check", "--root", str(root)]), 0)

            rc = gr.sync_tables_to_source(root, fmt=False)
            self.assertEqual(rc, 0)
            self.assertFalse((root / "crates/sunrise-client/src/game/stale_orphan_client.rs").exists())
            self.assertEqual(gms.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gfs.main(["--check", "--root", str(root)]), 0)
            self.assertEqual(gsc.main(["--check", "--root", str(root)]), 0)

    def test_side_ops_and_role_guarded_calls_and_regular_side_module_collision(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _make_synthetic_repo(root)
            # Add a `side_ops/sample.rs` dispatcher and a regular `shared_mod_server` side module.
            _write_tree(
                root,
                {
                    "crates/sunrise-client/src/game/side_ops/sample.rs": """use crate::game::Game;
impl Game<'_> {
    pub(in crate::game) fn side_ping(&mut self) {
        if self.is_logical_server() {
            self.server_ping();
        } else {
            self.client_ping();
        }
    }
    pub(in crate::game) fn side_cli_only(&mut self) {
        if !self.is_logical_server() {
            self.notify_client_only();
        }
    }
}
""",
                    "crates/sunrise-client/src/game/guarded_sample.rs": """use crate::game::Game;
impl Game<'_> {
    pub(in crate::game) fn mixed_guard(&mut self) {
        self.always_called();
        if self.is_logical_server() {
            self.srv_only_call();
            self.always_called();
        } else {
            self.cli_only_call();
        }
        if self.remote.is_some() {
            self.remote_only_call();
        }
    }
    pub(in crate::game) fn early_ret_srv(&mut self) {
        if !self.is_logical_server() {
            return;
        }
        self.after_srv_guard();
    }
}
""",
                    "crates/sunrise-client/src/game/shared_mod_server.rs": """use super::*;
impl Game<'_> {
    pub(in crate::game) fn hand_split_srv(&self) {}
}
""",
                },
            )
            spec = gr.collect_side_ops_spec(root)
            self.assertEqual(spec["dispatcher_fns"], {"side_ping", "side_cli_only"})
            self.assertEqual(spec["server_arms"], {"server_ping"})
            self.assertEqual(spec["client_arms"], {"client_ping", "notify_client_only"})

            srv_g, cli_g = gr.collect_role_guarded_calls(root)
            grel = "crates/sunrise-client/src/game/guarded_sample.rs"
            self.assertEqual(srv_g[(grel, "mixed_guard")], {"srv_only_call"})
            self.assertEqual(cli_g[(grel, "mixed_guard")], {"cli_only_call", "remote_only_call"})
            self.assertNotIn("always_called", srv_g[(grel, "mixed_guard")])
            self.assertEqual(srv_g[(grel, "early_ret_srv")], {"after_srv_guard"})

            # Register `shared_mod_server` as a regular server module: moving a fn from `shared_mod` to `server`
            # must be refused with `mirror-name-is-regular-module`, and `--sync-tables` must keep it!
            ss = root / "crates/sunrise-client/src/game/server_side.rs"
            ss.write_text(
                ss.read_text().replace(
                    'pub(in crate::game) mod srv_only;\n',
                    'pub(in crate::game) mod srv_only;\n#[path = "shared_mod_server.rs"]\npub(in crate::game) mod shared_mod_server;\n',
                ),
                encoding="utf-8",
            )
            grs = root / "crates/sunrise-client/src/game.rs"
            grs.write_text(
                grs.read_text().replace(
                    "pub(in crate::game) use server_side::srv_only;\n",
                    "pub(in crate::game) use server_side::srv_only;\n#[allow(unused_imports)]\npub(in crate::game) use server_side::shared_mod_server;\n",
                ),
                encoding="utf-8",
            )
            ms_tsv = root / "tools/game_module_sides.tsv"
            ms_tsv.write_text(
                ms_tsv.read_text() + "shared_mod_server\tserver\ttest\tgame/shared_mod_server.rs\n",
                encoding="utf-8",
            )
            rehome_tsv = root / "tools/game_rehome.tsv"
            rehome_tsv.write_text(
                "kind\tmodule\timpl\tname\tcfg\tcurrent_side\ttarget_side\treason\n"
                "fn\tshared_mod\timpl Game<'_>\tonly_server_calls\t-\tgame\tserver\ttest\n",
                encoding="utf-8",
            )
            rows = gr.load_rehome_table(rehome_tsv)
            rc, summ = gr.apply_rehome(root, rows, dry_run=True, strict=False, fmt=False)
            self.assertEqual(rc, 0)
            self.assertEqual(summ["refusals_by_code"], {"mirror-name-is-regular-module": 1})

            rc_sync = gr.sync_tables_to_source(root, fmt=False)
            self.assertEqual(rc_sync, 0)
            synced_mods = gms.load_module_sides(ms_tsv)
            self.assertIn("shared_mod_server", synced_mods)
            self.assertEqual(synced_mods["shared_mod_server"][0], "server")

            # Verify `_collect_test_and_external_method_calls` captures both `.method(` and `Type::assoc_fn(`.
            (root / "crates/sunrise-client/src/game/sample_tests.rs").write_text(
                "fn t() { Game::assoc_probe(); x.dot_probe(); }\n",
                encoding="utf-8",
            )
            called = gr._collect_test_and_external_method_calls(root)
            self.assertIn("assoc_probe", called)
            self.assertIn("dot_probe", called)

            # Verify explicit `crate::game::server_side::<moved>::` path rewriting when a module leaves server_side.
            (root / "crates/sunrise-client/src/game/uses_srv_mod.rs").write_text(
                "use crate::game::server_side::srv_only::Item;\n",
                encoding="utf-8",
            )
            rewritten = gr._rewrite_explicit_mod_paths_in_mirrors(root, {"srv_only": ("server", "game")})
            self.assertEqual(len(rewritten), 1)
            self.assertIn(
                "use crate::game::srv_only::Item;",
                (root / "crates/sunrise-client/src/game/uses_srv_mod.rs").read_text(),
            )


if __name__ == "__main__":
    unittest.main()
