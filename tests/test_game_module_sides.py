"""Unit tests for `tools/game_module_sides.py` on small synthetic trees."""

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
_SPEC = importlib.util.spec_from_file_location(
    "game_module_sides", ROOT / "tools" / "game_module_sides.py"
)
assert _SPEC is not None and _SPEC.loader is not None
gms = importlib.util.module_from_spec(_SPEC)
sys.modules["game_module_sides"] = gms
_SPEC.loader.exec_module(gms)


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
#[path = "game/client_state.rs"]
mod client_state;
#[path = "game/server_state.rs"]
mod server_state;
#[path = "game/ui/hud.rs"]
mod hud;
#[path = "game/ui/pub_ui.rs"]
pub mod pub_ui;
#[cfg(feature = "friends")]
#[path = "game/net/friends.rs"]
mod friends;
#[cfg(not(feature = "friends"))]
#[path = "game/net/friends_off.rs"]
mod friends;
#[cfg(feature = "state-digest")]
#[path = "game/world/state_digest.rs"]
mod state_digest;
#[path = "game/world/srv_mod.rs"]
mod srv_mod;
#[path = "game/modding/native_a.rs"]
mod native_a;
#[path = "game/modding/native_b.rs"]
mod native_b;
#[path = "game/modding/native_c.rs"]
mod native_c;
#[path = "game/world/shared.rs"]
mod shared;
#[cfg(test)]
#[path = "game/ui/hud_tests.rs"]
mod hud_tests;

pub struct Game;

#[cfg(test)]
mod inline_tests {
    #[test]
    fn ok() {}
}
""",
            "crates/sunrise-client/src/game/client_state.rs": """//! Client-owned fields of [`super::Game`].
use super::*;

pub(in crate::game) struct ClientState {
    pub(in crate::game) private_flag: bool,
    pub(in crate::game) wide_flag:
        std::cell::Cell<bool>,
}

pub(in crate::game) struct ClientCpuResources {
    pub(in crate::game) model: u32,
}
""",
            "crates/sunrise-client/src/game/server_state.rs": """//! Server-owned fields of [`super::Game`].
use super::*;

pub(in crate::game) struct ServerState {
    pub(in crate::game) private_tick: u64,
    pub(in crate::game) wide_tick: u64,
}
""",
            "crates/sunrise-client/src/game/ui/hud.rs": """pub(super) fn draw_hud() {}
// pub(super) in comment must stay untouched
const LIT: &str = "pub(super) in string";
mod nested {
    pub(super) fn keep_nested_super() {}
}
""",
            "crates/sunrise-client/src/game/ui/pub_ui.rs": "pub fn entry() {}\n",
            "crates/sunrise-client/src/game/net/friends.rs": "pub(super) fn friends_on() {}\n",
            "crates/sunrise-client/src/game/net/friends_off.rs": "pub(super) fn friends_off() {}\n",
            "crates/sunrise-client/src/game/world/state_digest.rs": "pub(super) fn digest() {}\n",
            "crates/sunrise-client/src/game/world/srv_mod.rs": """pub(super) fn tick_srv() {}
include!("srv_inc.rs");
#[cfg(test)]
mod tests {
    const NAME: &str = "game::srv_mod::tests::runs";
    const SHARED: &str = "game::shared::tests::stays";
}
""",
            "crates/sunrise-client/src/game/world/srv_inc.rs": "pub(super) fn from_include() {}\n",
            "crates/sunrise-client/src/game/modding/native_a.rs": "pub(super) fn a() {}\n",
            "crates/sunrise-client/src/game/modding/native_b.rs": "pub(super) fn b() {}\n",
            "crates/sunrise-client/src/game/modding/native_c.rs": "pub(super) fn c() {}\n",
            "crates/sunrise-client/src/game/world/shared.rs": "pub(super) fn keep_shared() {}\n",
            "crates/sunrise-client/src/game/ui/hud_tests.rs": "#[test]\nfn t() {}\n",
            "tools/game_module_sides.tsv": (
                "module\tside\treason\n"
                "friends\tclient\ttouches only ClientState fields\n"
                "hud\tclient\ttouches only ClientState fields\n"
                "hud_tests\tclient\ttouches only ClientState fields\n"
                "native_a\tgame\ttouches both ClientState and ServerState fields\n"
                "native_b\tclient\ttouches only ClientState fields\n"
                "native_c\tserver\ttouches only ServerState fields\n"
                "pub_ui\tclient\ttouches only ClientState fields\n"
                "shared\tgame\ttouches neither ClientState nor ServerState fields\n"
                "srv_mod\tserver\ttouches only ServerState fields\n"
                "state_digest\tserver\ttouches only ServerState fields\n"
            ),
            "tools/game_field_owners.tsv": (
                "field\ttype\towner\treason\tdeclaration_index\tvisibility\n"
                "private_flag\tbool\tclient\tclient\t0\tside\n"
                "wide_flag\tCell<bool>\tclient\tclient\t1\tgame\n"
                "private_tick\tu64\tserver\tserver\t2\tside\n"
                "wide_tick\tu64\tserver\tserver\t3\tgame\n"
                "world\tWorld\tgame\tshared\t4\t-\n"
            ),
            "tools/ci/ignored-tests/sample.txt": (
                "# comment game::hud::ignored_in_comment\n"
                "sunrise-client game::hud::tests::a lib daily resources\n"
                "sunrise-client game::srv_mod::tests::b lib daily resources\n"
                "sunrise-client game::shared::tests::c lib daily resources\n"
                "sunrise-world game::hud::other_pkg lib daily resources\n"
            ),
            "tools/ci/ignored-test-exceptions.json": json.dumps(
                [
                    {
                        "package": "sunrise-client",
                        "target": "lib",
                        "test": "game::srv_mod::tests::exc",
                        "path": "crates/sunrise-client/src/game/world/srv_mod.rs",
                        "reason": "manual",
                        "rationale": "synthetic",
                    }
                ],
                indent=2,
            )
            + "\n",
            "tools/test_sample_parity.py": (
                "TESTS = [\n"
                "    'game::hud::tests::a',\n"
                "    'game::shared::tests::c',\n"
                "]\n"
            ),
            "tools/test_areas.toml": (
                "[[area]]\n"
                'name = "sample"\n'
                "cargo = [\n"
                '  { package = "sunrise-client", target = "lib", filter = "game::hud::" },\n'
                '  { package = "sunrise-client", target = "lib", filter = "game::native_" },\n'
                '  { package = "sunrise-client", target = "lib", filter = "game::shared::" },\n'
                "]\n"
            ),
            "tools/ci/fn-replace-check/support/sunrise-mod.toml": (
                'target = "sunrise_client::game::server_state::ServerState"\n'
                'target = "sunrise_client::game::srv_mod::Game::tick_srv"\n'
                'target = "sunrise_client::game::shared::Game::keep"\n'
            ),
            "docs/sample.md": (
                "Bridge: `sunrise_client::game::server_state::ServerState`.\n"
                "Test: `game::hud::tests::a` and `game::shared::tests::c`.\n"
            ),
        },
    )


def run_cli(*args: str) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        rc = gms.main(list(args))
    return rc, out.getvalue(), err.getvalue()


class GameModuleSidesTests(unittest.TestCase):
    def test_fix_check_and_idempotency(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)

            rc, _, err = run_cli("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("missing side file", err)

            rc, out, err = run_cli("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 0, err)
            self.assertIn("Applied module sides", out)

            self.assertFalse((root / "crates/sunrise-client/src/game/client_state.rs").exists())
            self.assertFalse((root / "crates/sunrise-client/src/game/server_state.rs").exists())

            client_side = (root / "crates/sunrise-client/src/game/client_side.rs").read_text()
            self.assertIn("pub(in crate::game::client_side) use super::*;", client_side)
            self.assertIn('#[path = "ui/hud.rs"]\npub(in crate::game) mod hud;', client_side)
            self.assertIn('#[path = "ui/pub_ui.rs"]\npub mod pub_ui;', client_side)
            self.assertIn('#[cfg(feature = "friends")]\n#[path = "net/friends.rs"]\npub(in crate::game) mod friends;', client_side)
            self.assertIn('#[cfg(not(feature = "friends"))]\n#[path = "net/friends_off.rs"]\npub(in crate::game) mod friends;', client_side)
            self.assertIn("pub(in crate::game::client_side) private_flag: bool,", client_side)
            self.assertIn("pub(in crate::game) wide_flag:", client_side)
            self.assertIn("pub(in crate::game) model: u32,", client_side)

            server_side = (root / "crates/sunrise-client/src/game/server_side.rs").read_text()
            self.assertIn("pub(in crate::game::server_side) use super::*;", server_side)
            self.assertIn('#[cfg(feature = "state-digest")]\n#[path = "world/state_digest.rs"]\npub(in crate::game) mod state_digest;', server_side)
            self.assertIn("pub(in crate::game::server_side) private_tick: u64,", server_side)
            self.assertIn("pub(in crate::game) wide_tick: u64,", server_side)

            game_rs = (root / "crates/sunrise-client/src/game.rs").read_text()
            self.assertIn("pub(in crate::game) mod client_side;", game_rs)
            self.assertIn("pub(in crate::game) use client_side as client_state;", game_rs)
            self.assertIn("pub use client_side::pub_ui;", game_rs)
            self.assertIn('#[cfg(test)]\n#[allow(unused_imports)]\npub(in crate::game) use client_side::hud_tests;', game_rs)
            self.assertIn('#[cfg(feature = "state-digest")]\n#[allow(unused_imports)]\npub(in crate::game) use server_side::state_digest;', game_rs)
            self.assertEqual(game_rs.count("use client_side::friends;"), 1)
            self.assertIn('#[path = "game/world/shared.rs"]\nmod shared;', game_rs)

            hud = (root / "crates/sunrise-client/src/game/ui/hud.rs").read_text()
            self.assertIn("pub(in crate::game) fn draw_hud()", hud)
            self.assertIn("// pub(super) in comment must stay untouched", hud)
            self.assertIn('"pub(super) in string"', hud)
            self.assertIn("    pub(super) fn keep_nested_super() {}", hud)

            # Both arms of friends and root include!d file are rewritten; shared is untouched
            self.assertIn("pub(in crate::game) fn friends_off()", (root / "crates/sunrise-client/src/game/net/friends_off.rs").read_text())
            self.assertIn("pub(in crate::game) fn from_include()", (root / "crates/sunrise-client/src/game/world/srv_inc.rs").read_text())
            self.assertIn("pub(super) fn keep_shared()", (root / "crates/sunrise-client/src/game/world/shared.rs").read_text())

            # Selectors rewritten
            srv_mod = (root / "crates/sunrise-client/src/game/world/srv_mod.rs").read_text()
            self.assertIn('"game::server_side::srv_mod::tests::runs"', srv_mod)
            self.assertIn('"game::shared::tests::stays"', srv_mod)

            ignored = (root / "tools/ci/ignored-tests/sample.txt").read_text()
            self.assertIn("# comment game::hud::ignored_in_comment", ignored)
            self.assertIn("sunrise-client game::client_side::hud::tests::a ", ignored)
            self.assertIn("sunrise-client game::server_side::srv_mod::tests::b ", ignored)
            self.assertIn("sunrise-client game::shared::tests::c ", ignored)
            self.assertIn("sunrise-world game::hud::other_pkg ", ignored)

            exc = json.loads((root / "tools/ci/ignored-test-exceptions.json").read_text())
            self.assertEqual(exc[0]["test"], "game::server_side::srv_mod::tests::exc")

            parity = (root / "tools/test_sample_parity.py").read_text()
            self.assertIn("'game::client_side::hud::tests::a'", parity)
            self.assertIn("'game::shared::tests::c'", parity)

            areas = (root / "tools/test_areas.toml").read_text()
            self.assertIn('filter = "game::client_side::hud::"', areas)
            self.assertIn('filter = "game::native_"', areas)
            self.assertIn('filter = "game::client_side::native_"', areas)
            self.assertIn('filter = "game::server_side::native_"', areas)
            self.assertIn('filter = "game::shared::"', areas)

            fnr = (root / "tools/ci/fn-replace-check/support/sunrise-mod.toml").read_text()
            self.assertIn('target = "sunrise_client::game::server_side::ServerState"', fnr)
            self.assertIn('target = "sunrise_client::game::server_side::srv_mod::Game::tick_srv"', fnr)
            self.assertIn('target = "sunrise_client::game::shared::Game::keep"', fnr)

            doc = (root / "docs/sample.md").read_text()
            self.assertIn("`sunrise_client::game::server_side::ServerState`", doc)
            self.assertIn("`game::client_side::hud::tests::a`", doc)
            self.assertIn("`game::shared::tests::c`", doc)

            rc, _, err = run_cli("--check", "--root", str(root))
            self.assertEqual(rc, 0, err)

            # Idempotency check
            snap1 = snapshot_tree(root)
            rc, _, err = run_cli("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 0, err)
            self.assertEqual(snap1, snapshot_tree(root))

    def test_move_module_back_to_game(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_cli("--fix", "--no-fmt", "--root", str(root))[0], 0)

            tsv = root / "tools/game_module_sides.tsv"
            tsv.write_text(
                tsv.read_text().replace("hud\tclient\t", "hud\tgame\t"),
                encoding="utf-8",
            )
            rc, _, err = run_cli("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("module `hud` is in `client`, expected `game`", err)

            rc, _, err = run_cli("--fix", "--no-fmt", "--root", str(root))
            self.assertEqual(rc, 0, err)
            rc, _, err = run_cli("--check", "--root", str(root))
            self.assertEqual(rc, 0, err)

            game_rs = (root / "crates/sunrise-client/src/game.rs").read_text()
            self.assertIn('#[path = "game/ui/hud.rs"]\npub(in crate::game) mod hud;', game_rs)
            self.assertNotIn("use client_side::hud;", game_rs)
            self.assertIn("sunrise-client game::hud::tests::a ", (root / "tools/ci/ignored-tests/sample.txt").read_text())

    def test_check_fails_closed(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_cli("--fix", "--no-fmt", "--root", str(root))[0], 0)

            # 1. Missing module in table
            cs = root / "crates/sunrise-client/src/game/client_side.rs"
            orig_cs = cs.read_text()
            cs.write_text(orig_cs.replace("pub(in crate::game) mod hud;", "pub(in crate::game) mod hud;\nmod unlisted;"))
            rc, _, err = run_cli("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("`unlisted`", err)
            cs.write_text(orig_cs)

            # 2. Field silently widened back to pub(in crate::game)
            cs.write_text(orig_cs.replace("pub(in crate::game::client_side) private_flag", "pub(in crate::game) private_flag"))
            rc, _, err = run_cli("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("private_flag", err)
            self.assertIn("wider than table requirement", err)
            cs.write_text(orig_cs)

            # 3. Wide field widened beyond pub(in crate::game)
            cs.write_text(orig_cs.replace("pub(in crate::game) wide_flag:", "pub(crate) wide_flag:"))
            rc, _, err = run_cli("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("wide_flag", err)
            cs.write_text(orig_cs)

            # 4. Forbidden derive on ClientState
            cs.write_text(orig_cs.replace("pub(in crate::game) struct ClientState", "#[derive(Debug)]\npub(in crate::game) struct ClientState"))
            rc, _, err = run_cli("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("must not carry `#[derive(...)]`", err)
            cs.write_text(orig_cs)

            # 5. Root pub(super) left in moved file
            hud = root / "crates/sunrise-client/src/game/ui/hud.rs"
            orig_hud = hud.read_text()
            hud.write_text("pub(super) fn stray() {}\n" + orig_hud)
            rc, _, err = run_cli("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("root-scope `pub(super)`", err)
            hud.write_text(orig_hud)

            # 6. Stale selector
            ig = root / "tools/ci/ignored-tests/sample.txt"
            orig_ig = ig.read_text()
            ig.write_text(orig_ig.replace("game::client_side::hud::tests::a", "game::hud::tests::a"))
            rc, _, err = run_cli("--check", "--root", str(root))
            self.assertEqual(rc, 1)
            self.assertIn("stale module path selector", err)
            ig.write_text(orig_ig)

            # Clean again
            self.assertEqual(run_cli("--check", "--root", str(root))[0], 0)

    def test_list_and_renames(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            make_synthetic_tree(root)
            self.assertEqual(run_cli("--fix", "--no-fmt", "--root", str(root))[0], 0)

            rc, out, err = run_cli("--list", "--root", str(root))
            self.assertEqual(rc, 0)
            self.assertIn("hud\tclient\tclient\tok\t", out)
            self.assertIn("fields private=2, wide=2", err)

            before = root / "before.json"
            after = root / "after.json"
            renames = root / "renames.tsv"
            before.write_text(
                json.dumps(
                    {
                        "functions": {
                            "sunrise_client::game::hud::Game::draw_hud": {},
                            "sunrise_client::game::srv_mod::Game::tick_srv": {},
                            "sunrise_client::game::srv_mod::<A as T>::refused_slot": {},
                            "sunrise_client::game::server_state::ServerState::new": {},
                            "sunrise_client::game::shared::Game::keep": {},
                        }
                    }
                )
            )
            after.write_text(
                json.dumps(
                    {
                        "functions": {
                            "sunrise_client::game::client_side::hud::Game::draw_hud": {},
                            "sunrise_client::game::server_side::srv_mod::Game::tick_srv": {},
                            "sunrise_client::game::server_side::ServerState::new": {},
                            "sunrise_client::game::shared::Game::keep": {},
                        },
                        "refused": {
                            "sunrise_client::game::server_side::srv_mod::<A as T>::refused_slot": {
                                "code": "foreign_trait",
                                "why": "w2",
                            }
                        },
                    }
                )
            )
            rc, _, err = run_cli(
                "--renames", str(before), str(after), "--root", str(root), "--out", str(renames)
            )
            self.assertEqual(rc, 0, err)
            self.assertIn("4 moved (3 in after functions.json, 1 in after refused, 0 missing)", err)
            lines = renames.read_text().splitlines()
            self.assertEqual(lines[0], "old_target\tnew_target\tside")
            self.assertIn(
                "sunrise_client::game::hud::Game::draw_hud\tsunrise_client::game::client_side::hud::Game::draw_hud\tclient",
                lines,
            )
            self.assertIn(
                "sunrise_client::game::server_state::ServerState::new\tsunrise_client::game::server_side::ServerState::new\tserver",
                lines,
            )


if __name__ == "__main__":
    unittest.main()
