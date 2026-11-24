"""Unit tests for `tools/game_fn_sides.py` on small synthetic trees."""

from __future__ import annotations

import importlib.util
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _load_tool(name: str, rel: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / rel)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


gfs = _load_tool("game_fn_sides", "tools/game_fn_sides.py")
gms = _load_tool("game_module_sides", "tools/game_module_sides.py")


def write_tree(root: Path, files: dict[str, str]) -> None:
    for rel, content in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def snapshot_tree(root: Path) -> dict[str, str]:
    return {
        p.relative_to(root).as_posix(): p.read_text(encoding="utf-8")
        for p in sorted(root.rglob("*"))
        if p.is_file()
    }


def make_synthetic_tree(root: Path) -> None:
    write_tree(
        root,
        {
            "crates/sunrise-client/src/game.rs": """//! Game root.
use super::RootType;
use native_server::ServerHelper;
pub use super::PublicType;

#[path = "game/client_side.rs"]
pub(in crate::game) mod client_side;
#[allow(unused_imports)]
pub(in crate::game) use client_side as client_state;
#[path = "game/server_side.rs"]
pub(in crate::game) mod server_side;
#[allow(unused_imports)]
pub(in crate::game) use server_side as server_state;
#[path = "game/mixed/mod.rs"]
mod mixed;
#[cfg(feature = "extra")]
#[path = "game/reparented.rs"]
mod reparented;
#[path = "game/native_server.rs"]
mod native_server;

struct PrivateGameHelper(u32);

macro_rules! game_shared_macro {
    ($x:expr) => {
        $x
    };
}

pub struct Game<'a> {
    pub(in crate::game) client: client_side::ClientState,
    pub(in crate::game) server: server_side::ServerState,
    pub(in crate::game) _p: std::marker::PhantomData<&'a ()>,
}

impl Game<'_> {
    pub(super) fn game_client_method(&mut self) {
        let _ = (RootType, PrivateGameHelper(game_shared_macro!(1)));
        self.client.c_only = true;
    }

    fn game_server_method(&mut self) {
        let _ = ServerHelper;
        self.server.s_only += 1;
    }

    fn game_both_method(&mut self) {
        self.client.c_wide = true;
        self.server.s_wide += 1;
    }
}

#[cfg(test)]
mod game_inline_tests {
    #[test]
    fn game_srv_test() {}
}
""",
            "crates/sunrise-client/src/game/client_side.rs": """//! Client-owned fields.
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;

pub(in crate::game) struct ClientState {
    pub(in crate::game::client_side) c_only: bool,
    pub(in crate::game) c_wide: bool,
}
""",
            "crates/sunrise-client/src/game/server_side.rs": """//! Server-owned fields.
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;

pub(in crate::game) struct ServerState {
    pub(in crate::game::server_side) s_only: u64,
    pub(in crate::game) s_wide: u64,
}
""",
            "crates/sunrise-client/src/game/mixed/mod.rs": """//! Mixed module with child mods, root include!, trait impl, and macros.
use super::*;
use sub::SubHelper;

mod sub;
include!("shared.rs");

mod inc_child {
    include!("inc_child.rs");
}

#[cfg(feature = "hw")]
mod multi;
#[cfg(not(feature = "hw"))]
mod multi {
    use super::super::Game;
    impl Game<'_> {
        pub(super) fn multi_fn(&mut self) {
            self.client.c_only = false;
        }
    }
}

#[cfg_attr(test, derive(Default))]
struct LocalBox {
    val: u32,
}

thread_local! {
    static LOCAL_TLS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

macro_rules! srv_only_macro {
    ($g:expr) => {
        $g.server.s_only += 1;
    };
}

macro_rules! shared_macro {
    ($g:expr) => {
        $g.server.s_wide += 1;
    };
}

trait SideTrait {
    fn touch(&mut self);
}

struct Holder<'a> {
    g: &'a mut Game<'a>,
}

impl SideTrait for Holder<'_> {
    fn touch(&mut self) {
        self.g.server.s_only += 1;
    }
}

fn client_free(g: &mut Game<'_>) -> u32 {
    g.client.c_only = true;
    LOCAL_TLS.get() + LocalBox { val: SubHelper::VAL }.val
}

impl Game<'_> {
    fn both_fn(&mut self) {
        self.client.c_wide = true;
        shared_macro!(self);
    }

    fn server_macro_fn(&mut self) {
        srv_only_macro!(self);
    }

    fn stays_for_macro(&mut self) {
        shared_macro!(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SELF_NAME: &str = "game::mixed::tests::client_test";

    #[test]
    fn client_test() {
        let _ = SELF_NAME;
    }
}
""",
            "crates/sunrise-client/src/game/mixed/shared.rs": """impl Game<'_> {
    fn from_root_include(&mut self) {
        self.client.c_only = true;
    }
}

#[cfg(test)]
mod inc_tests {
    #[test]
    fn from_inc_inline_mod() {}
}
""",
            "crates/sunrise-client/src/game/mixed/sub.rs": """use super::*;

pub(super) struct SubHelper;
impl SubHelper {
    pub(super) const VAL: u32 = 7;
}

impl Game<'_> {
    pub(super) fn sub_server_fn(&mut self) {
        self.server.s_only += 1;
    }
}
""",
            "crates/sunrise-client/src/game/mixed/inc_child.rs": """use super::*;

impl Game<'_> {
    pub(super) fn inc_client_fn(&mut self) {
        self.client.c_only = false;
    }
}
""",
            "crates/sunrise-client/src/game/mixed/multi.rs": """use super::super::Game;

impl Game<'_> {
    pub(super) fn multi_fn(&mut self) {
        self.client.c_only = true;
    }
}
""",
            "crates/sunrise-client/src/game/reparented.rs": """use super::*;

impl Game<'_> {
    pub(super) fn maj_server_a(&mut self) {
        self.server.s_only += 1;
    }
    pub(super) fn maj_server_b(&mut self) {
        self.server.s_only += 2;
    }
    pub(super) fn min_client(&mut self) {
        self.client.c_only = true;
    }
}
""",
            "crates/sunrise-client/src/game/native_server.rs": """use super::*;

impl Game<'_> {
    pub(super) fn srv_only(&mut self) {
        self.server.s_only += 1;
    }
    pub(super) fn cli_only(&mut self) {
        self.client.c_only = true;
    }
}
""",
            "tools/game_module_sides.tsv": (
                "module\tside\treason\n"
                "mixed\tgame\ttouches both ClientState and ServerState fields\n"
                "native_server\tserver\ttouches only ServerState fields (opposite-side functions moved to native_server_client)\n"
                "reparented\tserver\ttouches only ServerState fields (opposite-side functions moved to reparented_client)\n"
            ),
            "tools/game_field_owners.tsv": (
                "field\ttype\towner\treason\tdeclaration_index\tvisibility\n"
                "c_only\tbool\tclient\tclient\t0\tside\n"
                "c_wide\tbool\tclient\tclient\t1\tgame\n"
                "s_only\tu64\tserver\tserver\t2\tside\n"
                "s_wide\tu64\tserver\tserver\t3\tgame\n"
            ),
            "tools/game_fn_sides.tsv": (
                "module\timpl\tname\tcfg\tside\treason\n"
                "game\timpl Game<'_>\tgame_both_method\t-\tgame\ttouches both ClientState and ServerState fields\n"
                "game\timpl Game<'_>\tgame_client_method\t-\tclient\ttouches only ClientState fields\n"
                "game\timpl Game<'_>\tgame_server_method\t-\tserver\ttouches only ServerState fields\n"
                "game::game_inline_tests\t-\tgame_srv_test\t#[cfg(test)]\tserver\ttouches only ServerState fields\n"
                "mixed\t-\tclient_free\t-\tclient\ttouches only ClientState fields\n"
                "mixed\timpl Game<'_>\tboth_fn\t-\tgame\ttouches both ClientState and ServerState fields\n"
                "mixed\timpl Game<'_>\tfrom_root_include\t-\tclient\ttouches only ClientState fields\n"
                "mixed\timpl Game<'_>\tserver_macro_fn\t-\tserver\ttouches only ServerState fields\n"
                "mixed\timpl Game<'_>\tstays_for_macro\t-\tgame\tinvokes module-local macro_rules! shared_macro used by staying items\n"
                "mixed\timpl SideTrait for Holder<'_>\ttouch\t-\tserver\twhole trait impl touches only ServerState fields\n"
                "mixed::inc_child\timpl Game<'_>\tinc_client_fn\t-\tclient\ttouches only ClientState fields\n"
                "mixed::inc_tests\t-\tfrom_inc_inline_mod\t#[cfg(test)]\tserver\ttouches only ServerState fields\n"
                "mixed::multi\timpl Game<'_>\tmulti_fn\t#[cfg(feature = \"hw\")]\tclient\ttouches only ClientState fields\n"
                "mixed::multi\timpl Game<'_>\tmulti_fn\t#[cfg(not(feature = \"hw\"))]\tclient\ttouches only ClientState fields\n"
                "mixed::sub\timpl Game<'_>\tsub_server_fn\t-\tserver\ttouches only ServerState fields\n"
                "mixed::tests\t-\tclient_test\t#[cfg(test)]\tclient\ttouches only ClientState fields\n"
                "native_server\timpl Game<'_>\tcli_only\t-\tclient\ttouches only ClientState fields\n"
                "reparented\timpl Game<'_>\tmin_client\t#[cfg(feature = \"extra\")]\tclient\ttouches only ClientState fields\n"
            ),
            "tools/ci/ignored-tests/sample.txt": (
                "# comment game::mixed::tests::client_test\n"
                "sunrise-client game::mixed::tests::client_test lib daily resources\n"
                "sunrise-client game::game_inline_tests::game_srv_test lib daily resources\n"
            ),
            "tools/port_map/sample.tsv": (
                "A.m1\tport\tcrates/sunrise-client/src/game.rs::Game::game_client_method\tnote\n"
                "A.m2\tport\tcrates/sunrise-client/src/game.rs::Game::game_both_method\tnote\n"
                "B.m3\tport\tcrates/sunrise-client/src/game/mixed/mod.rs::Game::server_macro_fn\tnote\n"
            ),
            "tools/ci/ignored-test-exceptions.json": json.dumps(
                [
                    {
                        "package": "sunrise-client",
                        "target": "lib",
                        "test": "game::mixed::tests::client_test",
                        "path": "crates/sunrise-client/src/game/mixed/mod.rs",
                        "reason": "manual",
                        "rationale": "synthetic",
                    }
                ],
                indent=2,
            )
            + "\n",
            "tools/test_sample_parity.py": "TESTS = ['game::mixed::tests::client_test']\n",
            "tools/test_areas.toml": (
                "[[area]]\n"
                'name = "sample"\n'
                "cargo = [\n"
                '  { package = "sunrise-client", target = "lib", filter = "game::mixed::" },\n'
                "]\n"
            ),
            "tools/ci/fn-replace-check/alpha/sunrise-mod.toml": (
                'target = "sunrise_client::game::mixed::Game<\'_>::server_macro_fn"\n'
                'target = "sunrise_client::game::mixed::Game<\'_>::both_fn"\n'
            ),
        },
    )


def run_fn(*args: str) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        rc = gfs.main(list(args))
    return rc, out.getvalue(), err.getvalue()


def run_mod(*args: str) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        rc = gms.main(list(args))
    return rc, out.getvalue(), err.getvalue()


class GameFnSidesTests(unittest.TestCase):
    def test_fix_check_and_joint_idempotency(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)

            rc, _, err = run_fn("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("missing mirror module `mixed_client`", err)

            # Run game_module_sides first, then game_fn_sides
            rc, _, err = run_mod("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 0, err)
            rc, out, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 0, err)
            self.assertIn("Applied fn sides:", out)

            # Both checks pass
            self.assertEqual(run_mod("--check", "--root", str(root))[0], 0)
            rc, _, err = run_fn("--check", "--root", str(root))
            self.assertEqual(rc, 0, err)

            # Mirror files and side declarations exist with cfg preserved
            cs = (root / "crates/sunrise-client/src/game/client_side.rs").read_text()
            self.assertIn('#[path = "game_client.rs"]\npub(in crate::game) mod game_client;', cs)
            self.assertIn('#[path = "mixed/mod_client.rs"]\npub(in crate::game) mod mixed_client;', cs)
            self.assertIn('#[cfg(feature = "extra")]\n#[path = "reparented_client.rs"]\npub(in crate::game) mod reparented_client;', cs)

            ss = (root / "crates/sunrise-client/src/game/server_side.rs").read_text()
            self.assertIn('#[path = "game_server.rs"]\npub(in crate::game) mod game_server;', ss)
            self.assertIn('#[path = "mixed/mod_server.rs"]\npub(in crate::game) mod mixed_server;', ss)
            self.assertIn('#[cfg(feature = "extra")]\n#[path = "reparented.rs"]\npub(in crate::game) mod reparented;', ss)

            # game.rs mirror rules: super:: -> crate::, child mod -> crate::game::, pub(super) -> pub(crate),
            # staying private items and macros stay untouched in game.rs
            game_orig = (root / "crates/sunrise-client/src/game.rs").read_text()
            game_cli = (root / "crates/sunrise-client/src/game/game_client.rs").read_text()
            game_srv = (root / "crates/sunrise-client/src/game/game_server.rs").read_text()
            self.assertIn("struct PrivateGameHelper(u32);", game_orig)
            self.assertNotIn("pub(in crate::game) struct PrivateGameHelper", game_orig)
            self.assertIn("macro_rules! game_shared_macro", game_orig)
            self.assertNotIn("use game_shared_macro;", game_orig)
            self.assertIn("fn game_both_method", game_orig)
            self.assertNotIn("fn game_client_method", game_orig)
            self.assertNotIn("fn game_server_method", game_orig)
            self.assertIn("use crate::RootType;", game_cli)
            self.assertIn("use crate::game::native_server::ServerHelper;", game_cli)
            self.assertNotIn("PublicType", game_cli)
            self.assertIn("pub(crate) fn game_client_method", game_cli)
            self.assertIn("pub(in crate::game) fn game_server_method", game_srv)
            self.assertIn("#[cfg(test)]\npub(in crate::game) mod game_inline_tests", game_srv)
            self.assertIn("fn game_srv_test", game_srv)

            # Reparented mirror imports from canonical server_side::reparented
            rep_c = (root / "crates/sunrise-client/src/game/reparented_client.rs").read_text()
            self.assertIn("use crate::game::server_side::reparented::*;", rep_c)
            self.assertIn("fn min_client", rep_c)

            # Side-exclusive macro moved with server; staying macro stayed in orig
            mixed_orig = (root / "crates/sunrise-client/src/game/mixed/mod.rs").read_text()
            mixed_srv = (root / "crates/sunrise-client/src/game/mixed/mod_server.rs").read_text()
            mixed_cli = (root / "crates/sunrise-client/src/game/mixed/mod_client.rs").read_text()
            self.assertNotIn("macro_rules! srv_only_macro", mixed_orig)
            self.assertIn("macro_rules! srv_only_macro", mixed_srv)
            self.assertIn("macro_rules! shared_macro", mixed_orig)
            self.assertIn("fn both_fn", mixed_orig)
            self.assertIn("fn stays_for_macro", mixed_orig)
            self.assertIn("impl SideTrait for Holder<'_>", mixed_srv)
            self.assertNotIn("impl SideTrait for Holder<'_>", mixed_orig)
            self.assertIn("use crate::game::client_side::mixed_client::client_free;", mixed_orig)
            self.assertIn("pub(in crate::game) val: u32,", mixed_orig)
            self.assertIn("pub(in crate::game) static LOCAL_TLS:", mixed_orig)
            self.assertIn("use crate::game::mixed::LOCAL_TLS;", mixed_cli)
            self.assertNotIn("cfg_attr", mixed_cli)
            self.assertNotIn("cfg_attr", mixed_srv)
            self.assertIn("fn from_inc_inline_mod", mixed_srv)
            self.assertIn('#[path = "native_server_client.rs"]\npub(in crate::game) mod native_server_client;', cs)
            self.assertIn('#[path = "native_server.rs"]\npub(in crate::game) mod native_server;', ss)
            self.assertIn("fn from_root_include", mixed_cli)
            self.assertNotIn("fn from_root_include", (root / "crates/sunrise-client/src/game/mixed/shared.rs").read_text())

            # Multi-arm child mod handled without collision and #[cfg(test)] mod emitted after items
            self.assertIn('#[cfg(feature = "hw")]\n#[path = "multi_client.rs"]', mixed_cli)
            self.assertIn('#[cfg(not(feature = "hw"))]\npub(in crate::game) mod multi {', mixed_cli)
            self.assertLess(mixed_cli.index("fn from_root_include"), mixed_cli.index("#[cfg(test)]\npub(in crate::game) mod tests"))

            # Selectors and port_map rewritten
            ignored_txt = (root / "tools/ci/ignored-tests/sample.txt").read_text()
            self.assertIn("game::client_side::mixed_client::tests::client_test", ignored_txt)
            self.assertIn("game::server_side::game_server::game_inline_tests::game_srv_test", ignored_txt)
            self.assertIn("game::client_side::mixed_client::tests::client_test", (root / "tools/test_sample_parity.py").read_text())
            self.assertIn("game::client_side::mixed_client::tests::client_test", mixed_orig)
            self.assertIn('filter = "game::client_side::mixed_client::"', (root / "tools/test_areas.toml").read_text())
            self.assertIn("sunrise_client::game::server_side::mixed_server::Game<'_>::server_macro_fn", (root / "tools/ci/fn-replace-check/alpha/sunrise-mod.toml").read_text())
            pm = (root / "tools/port_map/sample.tsv").read_text()
            self.assertIn("crates/sunrise-client/src/game/game_client.rs::Game::game_client_method", pm)
            self.assertIn("crates/sunrise-client/src/game.rs::Game::game_both_method", pm)
            self.assertIn("crates/sunrise-client/src/game/mixed/mod_server.rs::Game::server_macro_fn", pm)

            # Joint idempotency: running both --fix tools again produces zero diff
            snap = snapshot_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            self.assertEqual(run_fn("--fix", "--no-fmt", "--root", str(root))[0], 0)
            self.assertEqual(snap, snapshot_tree(root))

    def test_move_fn_back_to_game_and_fail_closed(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            self.assertEqual(run_fn("--fix", "--no-fmt", "--root", str(root))[0], 0)

            # Fail closed on stray function added to mirror
            mc = root / "crates/sunrise-client/src/game/reparented_client.rs"
            orig_mc = mc.read_text()
            mc.write_text(orig_mc + "\nfn stray() {}\n", encoding="utf-8")
            rc, _, err = run_fn("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("unlisted function `reparented::stray`", err)
            mc.write_text(orig_mc, encoding="utf-8")

            # Move min_client back to game -> mirror file and decl removed cleanly
            tsv = root / "tools/game_fn_sides.tsv"
            tsv.write_text(
                tsv.read_text().replace("min_client\t#[cfg(feature = \"extra\")]\tclient\t", "min_client\t#[cfg(feature = \"extra\")]\tgame\t"),
                encoding="utf-8",
            )
            self.assertEqual(run_fn("--check", "--root", str(root))[0], 1)
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 0, err)
            self.assertFalse(mc.exists())
            self.assertNotIn("reparented_client", (root / "crates/sunrise-client/src/game/client_side.rs").read_text())
            self.assertIn("fn min_client", (root / "crates/sunrise-client/src/game/reparented.rs").read_text())
            # Also move game_client_method back to game -> game_client.rs and decl removed, port_map restored
            gc = root / "crates/sunrise-client/src/game/game_client.rs"
            self.assertTrue(gc.exists())
            tsv.write_text(
                tsv.read_text().replace("game_client_method\t-\tclient\t", "game_client_method\t-\tgame\t"),
                encoding="utf-8",
            )
            self.assertEqual(run_fn("--check", "--root", str(root))[0], 1)
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 0, err)
            self.assertFalse(gc.exists())
            self.assertNotIn("game_client", (root / "crates/sunrise-client/src/game/client_side.rs").read_text())
            self.assertIn("fn game_client_method", (root / "crates/sunrise-client/src/game.rs").read_text())
            self.assertIn("crates/sunrise-client/src/game.rs::Game::game_client_method", (root / "tools/port_map/sample.tsv").read_text())
            self.assertEqual(run_fn("--check", "--root", str(root))[0], 0)
            self.assertEqual(run_mod("--check", "--root", str(root))[0], 0)

    def test_list_and_renames(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            self.assertEqual(run_fn("--fix", "--no-fmt", "--root", str(root))[0], 0)

            rc, out, _ = run_fn("--list", "--root", str(root))
            self.assertEqual(rc, 0)
            self.assertIn("mixed\t-\tclient_free\t-\tclient\tclient\tok\t", out)
            self.assertIn("game\timpl Game<'_>\tgame_client_method\t-\tclient\tclient\tok\t", out)

            before = root / "before.json"
            after = root / "after.json"
            renames = root / "renames.tsv"
            before.write_text(
                json.dumps(
                    {
                        "functions": {
                            "sunrise_client::game::Game<'_>::game_client_method": {},
                            "sunrise_client::game::Game<'_>::game_both_method": {},
                            "sunrise_client::game::mixed::client_free": {},
                            "sunrise_client::game::mixed::Game<'_>::both_fn": {},
                            "sunrise_client::game::reparented::Game<'_>::maj_server_a": {},
                            "sunrise_client::game::reparented::Game<'_>::min_client": {},
                        }
                    }
                )
            )
            after.write_text(
                json.dumps(
                    {
                        "functions": {
                            "sunrise_client::game::client_side::game_client::Game<'_>::game_client_method": {},
                            "sunrise_client::game::Game<'_>::game_both_method": {},
                            "sunrise_client::game::client_side::mixed_client::client_free": {},
                            "sunrise_client::game::mixed::Game<'_>::both_fn": {},
                            "sunrise_client::game::server_side::reparented::Game<'_>::maj_server_a": {},
                            "sunrise_client::game::client_side::reparented_client::Game<'_>::min_client": {},
                        },
                        "refused": {},
                    }
                )
            )
            rc, _, err = run_fn("--renames", str(before), str(after), "--root", str(root), "--out", str(renames))
            self.assertEqual(rc, 0, err)
            lines = renames.read_text().splitlines()
            self.assertEqual(lines[0], "old_target\tnew_target\tside")
            self.assertIn(
                "sunrise_client::game::Game<'_>::game_client_method\tsunrise_client::game::client_side::game_client::Game<'_>::game_client_method\tclient",
                lines,
            )
            self.assertEqual(len(lines), 5)

    def test_row9_r19_atomic_fix_refusal_leaves_tree_byte_identical(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            tsv = root / "tools/game_fn_sides.tsv"
            tsv.write_text(
                tsv.read_text(encoding="utf-8") + "reparented\timpl Game<'_>\tmissing_client\t-\tclient\ttest\n",
                encoding="utf-8",
            )
            before = snapshot_tree(root)
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("missing_client", err)
            self.assertEqual(snapshot_tree(root), before)

    def test_row10_r22_handwritten_mirror_content_fails_check_and_refuses_fix(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            self.assertEqual(run_fn("--fix", "--no-fmt", "--root", str(root))[0], 0)

            mc = root / "crates/sunrise-client/src/game/mixed/mod_client.rs"
            orig_mc = mc.read_text(encoding="utf-8")
            mutated = orig_mc.replace(
                "self.client.c_only = true;",
                "let _ = MANUAL;\n        self.client.c_only = true;",
            ) + "\nconst MANUAL: u32 = 42;\n"
            mc.write_text(mutated, encoding="utf-8")
            snap = snapshot_tree(root)

            rc_c, _, err_c = run_fn("--check", "--root", str(root))
            self.assertEqual(rc_c, 1)
            self.assertIn("MANUAL", err_c)

            rc_f, _, err_f = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc_f, 1)
            self.assertIn("MANUAL", err_f)
            self.assertEqual(snapshot_tree(root), snap)

            # Extra hand-written `use` and `#![allow(...)]` are also caught.
            mc.write_text("#![allow(dead_code)]\nuse std::fs;\n" + orig_mc, encoding="utf-8")
            rc_c2, _, err_c2 = run_fn("--check", "--root", str(root))
            self.assertEqual(rc_c2, 1)
            self.assertIn("#![...]", err_c2)
            self.assertIn("use", err_c2)
            rc_f2, _, _ = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc_f2, 1)

    def test_row12_r4_r5_r17a_r20_attrs_and_cfg_handling(self):
        # R4: non-cfg attrs on an impl block are preserved instead of merged/dropped.
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            m = root / "crates/sunrise-client/src/game/mixed/mod.rs"
            m.write_text(
                m.read_text(encoding="utf-8") + "\n#[allow(dead_code)]\nimpl Game<'_> {\n    fn probe(&self) {}\n}\n",
                encoding="utf-8",
            )
            tsv = root / "tools/game_fn_sides.tsv"
            tsv.write_text(
                tsv.read_text(encoding="utf-8") + "mixed\timpl Game<'_>\tprobe\t-\tclient\tr4\n",
                encoding="utf-8",
            )
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 0, err)
            mc_txt = (root / "crates/sunrise-client/src/game/mixed/mod_client.rs").read_text(encoding="utf-8")
            self.assertIn("#[allow(dead_code)]\nimpl Game<'_>", mc_txt)
            self.assertEqual(run_fn("--check", "--root", str(root))[0], 0)

        # R5: cfg_attr-disjoint impls are refused atomically.
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            m = root / "crates/sunrise-client/src/game/mixed/mod.rs"
            m.write_text(
                m.read_text(encoding="utf-8")
                + "\n#[cfg_attr(not(test), cfg(any()))]\nimpl Game<'_> {\n    fn probe(&self) { let _ = 10; }\n}\n"
                + "#[cfg_attr(test, cfg(any()))]\nimpl Game<'_> {\n    fn probe(&self) { let _ = 20; }\n}\n",
                encoding="utf-8",
            )
            tsv = root / "tools/game_fn_sides.tsv"
            tsv.write_text(
                tsv.read_text(encoding="utf-8") + "mixed\timpl Game<'_>\tprobe\t-\tclient\tr5\n",
                encoding="utf-8",
            )
            before = snapshot_tree(root)
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("cfg_attr", err)
            self.assertEqual(snapshot_tree(root), before)

        # R17a: #[cfg(...)] on root include! is carried into key and mirror output.
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            (root / "crates/sunrise-client/src/game/mixed/guarded.rs").write_text(
                "impl Game<'_> {\n    fn probe(&self) {}\n}\n", encoding="utf-8"
            )
            m = root / "crates/sunrise-client/src/game/mixed/mod.rs"
            m.write_text(
                m.read_text(encoding="utf-8").replace(
                    'include!("shared.rs");',
                    'include!("shared.rs");\n#[cfg(any())]\ninclude!("guarded.rs");',
                ),
                encoding="utf-8",
            )
            tsv = root / "tools/game_fn_sides.tsv"
            base_tsv = tsv.read_text(encoding="utf-8")
            # Unconditional row must be refused atomically.
            tsv.write_text(base_tsv + "mixed\timpl Game<'_>\tprobe\t-\tclient\tr17a\n", encoding="utf-8")
            before = snapshot_tree(root)
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("probe", err)
            self.assertEqual(snapshot_tree(root), before)

            # Matching cfg row carries #[cfg(any())] into the mirror and passes --check idempotently.
            tsv.write_text(base_tsv + "mixed\timpl Game<'_>\tprobe\t#[cfg(any())]\tclient\tr17a\n", encoding="utf-8")
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 0, err)
            mc_txt = (root / "crates/sunrise-client/src/game/mixed/mod_client.rs").read_text(encoding="utf-8")
            self.assertIn("#[cfg(any())]\nimpl Game<'_>", mc_txt)
            self.assertEqual(run_fn("--check", "--root", str(root))[0], 0)
            snap = snapshot_tree(root)
            self.assertEqual(run_fn("--fix", "--no-fmt", "--root", str(root))[0], 0)
            self.assertEqual(snapshot_tree(root), snap)

        # R20: cfg_attr on a path-backed child mod is refused atomically.
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            (root / "crates/sunrise-client/src/game/mixed/p.rs").write_text(
                "impl super::super::Game<'_> {\n    fn probe(&self) {}\n}\n", encoding="utf-8"
            )
            m = root / "crates/sunrise-client/src/game/mixed/mod.rs"
            m.write_text(
                m.read_text(encoding="utf-8").replace(
                    "mod sub;",
                    'mod sub;\n#[cfg_attr(not(test), cfg(any()))]\n#[path = "p.rs"]\nmod p;',
                ),
                encoding="utf-8",
            )
            tsv = root / "tools/game_fn_sides.tsv"
            tsv.write_text(
                tsv.read_text(encoding="utf-8") + "mixed::p\timpl super::super::Game<'_>\tprobe\t-\tclient\tr20\n",
                encoding="utf-8",
            )
            before = snapshot_tree(root)
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("cfg_attr", err)
            self.assertEqual(snapshot_tree(root), before)

    def test_row13_r17b_r17c_relative_include_dir_change_refused(self):
        # R17b: include!("local.rs") in a fragment from mixed/frags/ moving to mixed/mod_client.rs.
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            write_tree(
                root,
                {
                    "crates/sunrise-client/src/game/mixed/local.rs": "1u32\n",
                    "crates/sunrise-client/src/game/mixed/frags/local.rs": "2u32\n",
                    "crates/sunrise-client/src/game/mixed/frags/probe.rs": (
                        'impl Game<\'_> {\n    fn probe(&self) -> u32 { include!("local.rs") }\n}\n'
                    ),
                },
            )
            m = root / "crates/sunrise-client/src/game/mixed/mod.rs"
            m.write_text(
                m.read_text(encoding="utf-8").replace(
                    'include!("shared.rs");',
                    'include!("shared.rs");\ninclude!("frags/probe.rs");',
                ),
                encoding="utf-8",
            )
            tsv = root / "tools/game_fn_sides.tsv"
            tsv.write_text(
                tsv.read_text(encoding="utf-8") + "mixed\timpl Game<'_>\tprobe\t-\tclient\tr17b\n",
                encoding="utf-8",
            )
            before = snapshot_tree(root)
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("changes directory", err)
            self.assertEqual(snapshot_tree(root), before)

        # R17c: include!("plugin.rs") in a moved game.rs method moving to game/game_client.rs.
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            write_tree(
                root,
                {
                    "crates/sunrise-client/src/plugin.rs": "1u32\n",
                    "crates/sunrise-client/src/game/plugin.rs": "2u32\n",
                },
            )
            g = root / "crates/sunrise-client/src/game.rs"
            g.write_text(
                g.read_text(encoding="utf-8").replace(
                    "self.client.c_only = true;",
                    'let _ = include!("plugin.rs");\n        self.client.c_only = true;',
                ),
                encoding="utf-8",
            )
            before = snapshot_tree(root)
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("include!", err)
            self.assertIn("changes directory", err)
            self.assertEqual(snapshot_tree(root), before)

    def test_row14_r12_location_macros_direct_and_local_refused(self):
        for label, extra_items, body in [
            ("direct_module_path", "", "let _ = (module_path!(), file!(), line!(), column!());"),
            (
                "local_macro",
                "macro_rules! location { () => { module_path!() }; }\nmacro_rules! outer_loc { () => { location!() }; }\n",
                "let _ = outer_loc!();",
            ),
        ]:
            with tempfile.TemporaryDirectory() as td:
                root = Path(td)
                make_synthetic_tree(root)
                self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
                m = root / "crates/sunrise-client/src/game/mixed/mod.rs"
                m.write_text(
                    m.read_text(encoding="utf-8")
                    + f"\n{extra_items}impl Game<'_> {{\n    fn probe(&self) {{ {body} }}\n}}\n",
                    encoding="utf-8",
                )
                tsv = root / "tools/game_fn_sides.tsv"
                tsv.write_text(
                    tsv.read_text(encoding="utf-8") + "mixed\timpl Game<'_>\tprobe\t-\tclient\tr12\n",
                    encoding="utf-8",
                )
                before = snapshot_tree(root)
                rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
                self.assertEqual(rc, 1, label)
                self.assertIn("location-sensitive macro", err, label)
                self.assertEqual(snapshot_tree(root), before, label)

    def test_row15_r8_r9_r2_lexer_parser_and_block_doc(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            m = root / "crates/sunrise-client/src/game/mixed/mod.rs"
            m.write_text(
                m.read_text(encoding="utf-8")
                + "\nstruct Wrapper<const N: usize>;\n"
                + "impl Game<'_> {\n"
                + "    fn r8_probe(&self) {\n"
                + "        let _ = ('\\'','\"');\n"
                + "        let _ = b'\\'';\n"
                + "    }\n"
                + "    fn r8_stays(&self) {}\n"
                + "    fn r9_probe(&self) -> Wrapper<{ 1 }> {\n"
                + "        Wrapper\n"
                + "    }\n"
                + "    fn r9_stays(&self) {}\n"
                + "    /** probe doc /* nested */ */\n"
                + "    fn r2_probe(&self) {}\n"
                + "    fn r2_stays(&self) {}\n"
                + "}\n",
                encoding="utf-8",
            )
            tsv = root / "tools/game_fn_sides.tsv"
            tsv.write_text(
                tsv.read_text(encoding="utf-8")
                + "mixed\timpl Game<'_>\tr8_probe\t-\tclient\tr8\n"
                + "mixed\timpl Game<'_>\tr9_probe\t-\tclient\tr9\n"
                + "mixed\timpl Game<'_>\tr2_probe\t-\tclient\tr2\n",
                encoding="utf-8",
            )
            rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 0, err)
            rc_c, _, err_c = run_fn("--check", "--root", str(root))
            self.assertEqual(rc_c, 0, err_c)

            orig_txt = m.read_text(encoding="utf-8")
            mir_txt = (root / "crates/sunrise-client/src/game/mixed/mod_client.rs").read_text(encoding="utf-8")
            self.assertIn("fn r8_stays", orig_txt)
            self.assertNotIn("fn r8_stays", mir_txt)
            self.assertIn("fn r8_probe", mir_txt)
            self.assertIn("fn r9_stays", orig_txt)
            self.assertNotIn("fn r9_stays", mir_txt)
            self.assertIn("fn r9_probe(&self) -> Wrapper<{ 1 }>", mir_txt)
            self.assertIn("/** probe doc /* nested */ */", mir_txt)
            self.assertNotIn("/** probe doc /* nested */ */", orig_txt)
            self.assertIn("fn r2_stays", orig_txt)

    def test_row16_c16_c17_c6_c8_c9_c10_c18_c19_d4_fail_closed(self):
        # C16 & C17: header-only table on pre-move tree and truncated TSV row refused cleanly.
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            tsv = root / "tools/game_fn_sides.tsv"
            orig_tsv = tsv.read_text(encoding="utf-8")

            tsv.write_text("module\timpl\tname\tcfg\tside\treason\n", encoding="utf-8")
            with self.assertRaises(SystemExit) as ctx_empty:
                run_fn("--fix", "--no-fmt", "--root", str(root))
            self.assertIn("empty", str(ctx_empty.exception).lower())
            with self.assertRaises(SystemExit):
                run_fn("--check", "--root", str(root))

            tsv.write_text(orig_tsv + "mixed\t-\tx\n", encoding="utf-8")
            with self.assertRaises(SystemExit) as ctx_trunc:
                run_fn("--check", "--root", str(root))
            self.assertIn("columns", str(ctx_trunc.exception).lower())

        # C6, C8, C9, C10, C18, C19 on a split tree.
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
            self.assertEqual(run_fn("--fix", "--no-fmt", "--root", str(root))[0], 0)

            mc = root / "crates/sunrise-client/src/game/mixed/mod_client.rs"
            m = root / "crates/sunrise-client/src/game/mixed/mod.rs"
            cs = root / "crates/sunrise-client/src/game/client_side.rs"
            orig_mc, orig_m, orig_cs = mc.read_text(), m.read_text(), cs.read_text()

            # C6: identical method key twice in client mirror.
            mc.write_text(orig_mc + "\nimpl Game<'_> {\n    fn from_root_include(&self) {}\n}\n", encoding="utf-8")
            rc, _, err = run_fn("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("duplicate function key", err)
            mc.write_text(orig_mc, encoding="utf-8")

            # C8: overlapping #[cfg(test)] variant in original impl.
            m.write_text(orig_m + "\nimpl Game<'_> {\n    #[cfg(test)]\n    fn from_root_include(&self) {}\n}\n", encoding="utf-8")
            rc, _, err = run_fn("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("overlapping function definitions", err)
            m.write_text(orig_m, encoding="utf-8")

            # C9: overlapping impl Game<'static> specialization in original module.
            m.write_text(orig_m + "\nimpl Game<'static> {\n    fn from_root_include(&self) {}\n}\n", encoding="utf-8")
            rc, _, err = run_fn("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("overlapping function definitions", err)
            m.write_text(orig_m, encoding="utf-8")

            # C10: undeclared orphan mirror file under game/.
            orphan = root / "crates/sunrise-client/src/game/orphan_client.rs"
            orphan.write_text("impl Game<'_> { fn stray(&self) {} }\n", encoding="utf-8")
            rc, _, err = run_fn("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("orphan mirror file", err)
            orphan.unlink()

            # C18: undeclared member behind raw-string include in mirror.
            untracked = root / "crates/sunrise-client/src/game/mixed/untracked.rs"
            untracked.write_text("fn hidden() {}\n", encoding="utf-8")
            mc.write_text(orig_mc + '\ninclude!(r#"untracked.rs"#);\n', encoding="utf-8")
            rc, _, err = run_fn("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("hidden", err)
            mc.write_text(orig_mc, encoding="utf-8")
            untracked.unlink()

            # C19: second top-level mirror arm before existing mixed_client.
            cs.write_text(
                orig_cs.replace(
                    '#[path = "mixed/mod_client.rs"]\npub(in crate::game) mod mixed_client;',
                    '#[cfg(any())]\n#[path = "mixed/extra_client.rs"]\npub(in crate::game) mod mixed_client;\n'
                    '#[path = "mixed/mod_client.rs"]\npub(in crate::game) mod mixed_client;',
                ),
                encoding="utf-8",
            )
            rc, _, err = run_fn("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("duplicate top-level mirror module", err)
            cs.write_text(orig_cs, encoding="utf-8")

        # D4: split-side complementary cfg variants refuse deterministically regardless of TSV row order.
        errs_by_order: list[str] = []
        for row_pair in [
            (
                'mixed\timpl Game<\'_>\tcfg_split\t#[cfg(feature = "a")]\tclient\td4\n',
                'mixed\timpl Game<\'_>\tcfg_split\t#[cfg(not(feature = "a"))]\tserver\td4\n',
            ),
            (
                'mixed\timpl Game<\'_>\tcfg_split\t#[cfg(not(feature = "a"))]\tserver\td4\n',
                'mixed\timpl Game<\'_>\tcfg_split\t#[cfg(feature = "a")]\tclient\td4\n',
            ),
        ]:
            with tempfile.TemporaryDirectory() as td:
                root = Path(td)
                make_synthetic_tree(root)
                self.assertEqual(run_mod("--fix", "--no-fmt", "--root", str(root))[0], 0)
                m = root / "crates/sunrise-client/src/game/mixed/mod.rs"
                m.write_text(
                    m.read_text(encoding="utf-8")
                    + '\nimpl Game<\'_> {\n'
                    + '    #[cfg(feature = "a")]\n    fn cfg_split(&self) {}\n'
                    + '    #[cfg(not(feature = "a"))]\n    fn cfg_split(&self) {}\n'
                    + "}\n",
                    encoding="utf-8",
                )
                fnr = root / "tools/ci/fn-replace-check/alpha/sunrise-mod.toml"
                fnr.write_text(
                    fnr.read_text(encoding="utf-8") + 'target = "sunrise_client::game::mixed::Game<\'_>::cfg_split"\n',
                    encoding="utf-8",
                )
                tsv = root / "tools/game_fn_sides.tsv"
                tsv.write_text(tsv.read_text(encoding="utf-8") + row_pair[0] + row_pair[1], encoding="utf-8")
                before = snapshot_tree(root)
                rc, _, err = run_fn("--fix", "--no-fmt", "--root", str(root))
                self.assertEqual(rc, 1)
                self.assertIn("conflicting", err)
                self.assertEqual(snapshot_tree(root), before)
                errs_by_order.append(err)
        self.assertEqual(errs_by_order[0], errs_by_order[1])


if __name__ == "__main__":
    unittest.main()
