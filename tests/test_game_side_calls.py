"""Unit tests for `tools/game_side_calls.py` on synthetic trees and real-repo wiring."""

from __future__ import annotations

import importlib.util
import io
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _load_tool(name: str, rel_path: str):
    path = ROOT / rel_path
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


gsc = _load_tool("game_side_calls", "tools/game_side_calls.py")
gfs = _load_tool("game_fn_sides", "tools/game_fn_sides.py")


def _write_tree(root: Path, files: dict[str, str]) -> None:
    for rel, content in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")


def _run_gsc(argv: list[str]) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        try:
            rc = gsc.main(argv)
        except SystemExit as exc:
            rc = exc.code if isinstance(exc.code, int) else 1
            if isinstance(exc.code, str):
                err.write(exc.code + "\n")
    return rc, out.getvalue(), err.getvalue()


def _base_tree(extra: dict[str, str] | None = None) -> dict[str, str]:
    files = {
        "crates/sunrise-client/src/game.rs": (
            '#[path = "game/client_side.rs"]\n'
            "pub(in crate::game) mod client_side;\n"
            '#[path = "game/server_side.rs"]\n'
            "pub(in crate::game) mod server_side;\n"
        ),
        "crates/sunrise-client/src/game/client_side.rs": (
            '#[path = "alpha.rs"]\n'
            "pub(in crate::game) mod alpha;\n"
            "pub(in crate::game) struct ClientState;\n"
            "impl ClientState { pub(in crate::game) fn new() -> Self { Self } }\n"
        ),
        "crates/sunrise-client/src/game/server_side.rs": (
            '#[path = "beta.rs"]\n'
            "pub(in crate::game) mod beta;\n"
            "pub(in crate::game) struct ServerState;\n"
            "impl ServerState { pub(in crate::game) fn new() -> Self { Self } }\n"
        ),
        "crates/sunrise-client/src/game/alpha.rs": (
            "pub(in crate::game) fn kept_wide() {}\n"
            "pub(in crate::game) fn narrow_me() {}\n"
            "fn stay_private() {}\n"
            "pub(super) fn stay_super() {}\n"
        ),
        "crates/sunrise-client/src/game/beta.rs": (
            "pub(crate) fn server_wide_kept() {}\n"
            "pub fn server_narrow_me() {}\n"
        ),
        "tools/game_module_sides.tsv": (
            "module\tside\treason\n"
            "alpha\tclient\ttest\n"
            "beta\tserver\ttest\n"
        ),
        "tools/game_side_calls.tsv": (
            "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
            "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
            "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n"
        ),
    }
    if extra:
        files.update(extra)
    return files


class TestGameSideCalls(unittest.TestCase):
    def test_narrow_unlisted_never_widens_keeps_listed(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(root, _base_tree())
            rc, _out, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("narrow_me", err)
            self.assertIn("server_narrow_me", err)

            rc, out, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0, err)
            self.assertIn("1 client, 1 server", out)

            alpha = (root / "crates/sunrise-client/src/game/alpha.rs").read_text()
            self.assertIn("pub(in crate::game) fn kept_wide()", alpha)
            self.assertIn("pub(in crate::game::client_side) fn narrow_me()", alpha)
            self.assertIn("\nfn stay_private()", alpha)
            self.assertIn("pub(super) fn stay_super()", alpha)

            beta = (root / "crates/sunrise-client/src/game/beta.rs").read_text()
            self.assertIn("pub(crate) fn server_wide_kept()", beta)
            self.assertIn("pub(in crate::game::server_side) fn server_narrow_me()", beta)

            rc, out, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 0, err)

            # Second --fix is a byte-identical no-op
            rc, out2, err2 = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0, err2)
            self.assertIn("0 client, 0 server", out2)
            self.assertEqual((root / "crates/sunrise-client/src/game/alpha.rs").read_text(), alpha)

    def test_pub_in_super_chain_depth(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                _base_tree(
                    {
                        "crates/sunrise-client/src/game/alpha.rs": (
                            "pub(in crate::game) fn kept_wide() {}\n"
                            "pub(in super) fn ok_d0() {}\n"
                            "pub(in super::super) fn wide_d0() {}\n"
                            "mod sub {\n"
                            "    pub(in super::super) fn ok_d1() {}\n"
                            "    pub(in super::super::super) fn wide_d1() {}\n"
                            "}\n"
                        ),
                        "crates/sunrise-client/src/game/beta.rs": "pub(crate) fn server_wide_kept() {}\n",
                    }
                ),
            )
            rc, _out, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0, err)
            alpha = (root / "crates/sunrise-client/src/game/alpha.rs").read_text()
            self.assertIn("pub(in super) fn ok_d0()", alpha)
            self.assertIn("pub(in crate::game::client_side) fn wide_d0()", alpha)
            self.assertIn("pub(in super::super) fn ok_d1()", alpha)
            self.assertIn("pub(in crate::game::client_side) fn wide_d1()", alpha)
            rc, _out, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 0, err)

    def test_trait_impl_recording_and_unrecorded_failure(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                _base_tree(
                    {
                        "crates/sunrise-client/src/game/alpha.rs": (
                            "pub(in crate::game) fn kept_wide() {}\n"
                            "pub(in crate::game) fn narrow_me() {}\n"
                            "impl Default for Foo { fn default() -> Self { Self } }\n"
                        ),
                        "crates/sunrise-client/src/game/beta.rs": "pub(crate) fn server_wide_kept() {}\n",
                    }
                ),
            )
            before_alpha = (root / "crates/sunrise-client/src/game/alpha.rs").read_text()
            # Unrecorded trait impl fails --check and refuses --fix atomically
            rc, _out, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("unrecorded `client` trait_impl", err)

            rc, _out, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("no files modified", err)
            self.assertEqual((root / "crates/sunrise-client/src/game/alpha.rs").read_text(), before_alpha)

            # Recording the trait impl allows --fix and --check to pass
            (root / "tools/game_side_calls.tsv").write_text(
                "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
                "client\talpha\timpl Default for Foo\t-\t-\ttrait_impl\t-\ttrait-impl\n"
                "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n",
                encoding="utf-8",
            )
            rc, _out, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0, err)
            rc, _out, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 0, err)

    def test_against_baseline_ratchet_and_absent_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            files = _base_tree(
                {
                    "crates/sunrise-client/src/game/alpha.rs": (
                        "pub(in crate::game) fn kept_wide() {}\n"
                        "pub(in crate::game) fn second_wide() {}\n"
                    ),
                    "crates/sunrise-client/src/game/beta.rs": "pub(crate) fn server_wide_kept() {}\n",
                }
            )
            del files["tools/game_side_calls.tsv"]
            _write_tree(root, files)
            git_env = {
                "GIT_AUTHOR_NAME": "Test",
                "GIT_AUTHOR_EMAIL": "t@example.com",
                "GIT_COMMITTER_NAME": "Test",
                "GIT_COMMITTER_EMAIL": "t@example.com",
            }
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(["git", "commit", "-q", "-m", "before wall"], cwd=root, env=git_env, check=True)
            rev_no_table = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=True
            ).stdout.strip()

            # Commit with initial table (3 rows)
            (root / "tools/game_side_calls.tsv").write_text(
                "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
                "client\talpha\t-\tsecond_wide\t-\tfn\tgame\tcalled\n"
                "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n",
                encoding="utf-8",
            )
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(["git", "commit", "-q", "-m", "add wall"], cwd=root, env=git_env, check=True)
            rev_base = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=True
            ).stdout.strip()

            # Against rev without table: passes cleanly
            rc, out, err = _run_gsc(["--check", "--root", str(root), "--against", rev_no_table])
            self.assertEqual(rc, 0, err)
            self.assertIn("no baseline table", out)

            # Shrinking the table passes against rev_base
            (root / "tools/game_side_calls.tsv").write_text(
                "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
                "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n",
                encoding="utf-8",
            )
            rc, _out, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0, err)
            rc, _out, err = _run_gsc(["--check", "--root", str(root), "--against", rev_base])
            self.assertEqual(rc, 0, err)

            # Adding a new row absent at rev_base fails the ratchet
            (root / "crates/sunrise-client/src/game/alpha.rs").write_text(
                "pub(in crate::game) fn kept_wide() {}\n"
                "pub(in crate::game::client_side) fn second_wide() {}\n"
                "pub(in crate::game) fn brand_new_wide() {}\n",
                encoding="utf-8",
            )
            (root / "tools/game_side_calls.tsv").write_text(
                "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                "client\talpha\t-\tbrand_new_wide\t-\tfn\tgame\tcalled\n"
                "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
                "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n",
                encoding="utf-8",
            )
            rc, _out, err = _run_gsc(["--check", "--root", str(root), "--against", rev_base])
            self.assertEqual(rc, 1)
            self.assertIn("brand_new_wide", err)
            self.assertIn("may only shrink", err)

    def test_atomic_fix_refusal_writes_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                _base_tree(
                    {
                        "crates/sunrise-client/src/game/beta.rs": (
                            "pub(crate) fn server_wide_kept() {}\n"
                            '#[cfg_attr(feature = "x", cfg(test))]\n'
                            "pub(in crate::game) fn bad_cfg_attr() {}\n"
                        ),
                    }
                ),
            )
            alpha_before = (root / "crates/sunrise-client/src/game/alpha.rs").read_text()
            beta_before = (root / "crates/sunrise-client/src/game/beta.rs").read_text()
            rc, _out, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("cfg_attr", err)
            self.assertIn("no files modified", err)
            self.assertEqual((root / "crates/sunrise-client/src/game/alpha.rs").read_text(), alpha_before)
            self.assertEqual((root / "crates/sunrise-client/src/game/beta.rs").read_text(), beta_before)

    def test_table_validation_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(root, _base_tree())
            tsv = root / "tools/game_side_calls.tsv"

            # Empty table (header only)
            tsv.write_text("side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n", encoding="utf-8")
            rc, _o, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("empty", err)

            # Truncated row
            tsv.write_text(
                "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                "client\talpha\t-\tkept_wide\t-\n",
                encoding="utf-8",
            )
            rc, _o, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("8 tab-separated columns", err)

            # Own-side-only caller rejected
            tsv.write_text(
                "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                "client\talpha\t-\tkept_wide\t-\tfn\tclient\tcalled\n",
                encoding="utf-8",
            )
            rc, _o, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("own-side caller", err)

            # Unsorted table rejected
            tsv.write_text(
                "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient\tcalled\n"
                "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n",
                encoding="utf-8",
            )
            rc, _o, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("not canonically sorted", err)

            # Duplicate row rejected
            tsv.write_text(
                "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
                "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n",
                encoding="utf-8",
            )
            rc, _o, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("duplicate", err)

    def test_multi_arm_cfg_modules_and_raw_string_include(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                _base_tree(
                    {
                        "crates/sunrise-client/src/game/client_side.rs": (
                            '#[cfg(feature = "f")]\n'
                            '#[path = "alpha.rs"]\n'
                            "pub(in crate::game) mod alpha;\n"
                            '#[cfg(not(feature = "f"))]\n'
                            '#[path = "alpha_off.rs"]\n'
                            "pub(in crate::game) mod alpha;\n"
                            "pub(in crate::game) struct ClientState;\n"
                            "impl ClientState { pub(in crate::game) fn new() -> Self { Self } }\n"
                        ),
                        "crates/sunrise-client/src/game/alpha.rs": (
                            'include!(r#"alpha_inc.rs"#);\n'
                            "pub(in crate::game) fn twin() {}\n"
                        ),
                        "crates/sunrise-client/src/game/alpha_inc.rs": "pub(in crate::game) fn from_raw_inc() {}\n",
                        "crates/sunrise-client/src/game/alpha_off.rs": "pub(in crate::game) fn twin() {}\n",
                        "crates/sunrise-client/src/game/beta.rs": "pub(crate) fn server_wide_kept() {}\n",
                        "tools/game_side_calls.tsv": (
                            "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                            'client\talpha\t-\ttwin\t#[cfg(feature = "f")]\tfn\tgame\tcalled\n'
                            'client\talpha\t-\ttwin\t#[cfg(not(feature = "f"))]\tfn\tgame\tcalled\n'
                            "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n"
                        ),
                    }
                ),
            )
            rc, _out, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0, err)
            inc = (root / "crates/sunrise-client/src/game/alpha_inc.rs").read_text()
            self.assertIn("pub(in crate::game::client_side) fn from_raw_inc()", inc)
            rc, _out, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 0, err)

    def test_cfg_attr_lint_allowed_and_cfg_twin_diff_vis_refused(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                _base_tree(
                    {
                        "crates/sunrise-client/src/game/alpha.rs": (
                            "#[cfg_attr(not(test), allow(dead_code))]\n"
                            "pub(in crate::game) fn kept_wide() {}\n"
                            '#[cfg(feature = "a")]\n'
                            "pub(in crate::game) fn split_twin() {}\n"
                            '#[cfg(not(feature = "a"))]\n'
                            "pub(in crate::game::client_side) fn split_twin() {}\n"
                        ),
                        "crates/sunrise-client/src/game/beta.rs": "pub(crate) fn server_wide_kept() {}\n",
                        "tools/game_side_calls.tsv": (
                            "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                            "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
                            'client\talpha\t-\tsplit_twin\t#[cfg(feature = "a")]\tfn\tserver\tcalled\n'
                            "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n"
                        ),
                    }
                ),
            )
            rc, _out, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("cfg-twin differing visibility", err)
            self.assertNotIn("cfg_attr", err)

    def test_macro_visibility_and_macro_rules_refusals(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                _base_tree(
                    {
                        "crates/sunrise-client/src/game/alpha.rs": (
                            "pub(in crate::game) fn kept_wide() {}\n"
                            "vis!() fn sneaky() {}\n"
                        ),
                        "crates/sunrise-client/src/game/beta.rs": (
                            "pub(crate) fn server_wide_kept() {}\n"
                            "macro_rules! mk { ($v:vis $n:ident) => { $v fn $n() {} }; }\n"
                        ),
                    }
                ),
            )
            rc, _out, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("sneaky", err)
            self.assertIn("metavariable visibility", err)

    def test_macro_method_and_macro_invoc_handling(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                _base_tree(
                    {
                        "crates/sunrise-client/src/game/alpha.rs": "pub(in crate::game) fn kept_wide() {}\n",
                        "crates/sunrise-client/src/game/beta.rs": (
                            "pub(crate) fn server_wide_kept() {}\n"
                            "macro_rules! gen_ops { () => {\n"
                            "    pub(in crate::game) fn macro_kept() {}\n"
                            "    pub(in crate::game) fn macro_narrow() {}\n"
                            "}; }\n"
                            "impl Foo { gen_ops!(); }\n"
                        ),
                        "tools/game_side_calls.tsv": (
                            "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                            "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
                            "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n"
                            "server\tbeta\timpl Foo\tgen_ops\t-\tmacro_invoc\t-\tmacro-invoc\n"
                            "server\tbeta\tmacro_rules! gen_ops\tmacro_kept\t-\tmacro_method\tclient\tcalled\n"
                        ),
                    }
                ),
            )
            rc, _out, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0, err)
            beta = (root / "crates/sunrise-client/src/game/beta.rs").read_text()
            self.assertIn("pub(in crate::game) fn macro_kept()", beta)
            self.assertIn("pub(in crate::game::server_side) fn macro_narrow()", beta)
            rc, _out, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 0, err)

    def test_joint_idempotency_with_game_fn_sides(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                {
                    "crates/sunrise-client/src/game.rs": (
                        '#[path = "game/client_side.rs"]\n'
                        "pub(in crate::game) mod client_side;\n"
                        '#[path = "game/server_side.rs"]\n'
                        "pub(in crate::game) mod server_side;\n"
                        '#[path = "game/mixed.rs"]\n'
                        "pub(in crate::game) mod mixed;\n"
                    ),
                    "crates/sunrise-client/src/game/client_side.rs": (
                        "pub(in crate::game) struct ClientState;\n"
                        "impl ClientState { pub(in crate::game) fn new() -> Self { Self } }\n"
                    ),
                    "crates/sunrise-client/src/game/server_side.rs": (
                        "pub(in crate::game) struct ServerState;\n"
                        "impl ServerState { pub(in crate::game) fn new() -> Self { Self } }\n"
                    ),
                    "crates/sunrise-client/src/game/mixed.rs": (
                        "pub(in crate::game) fn c_only_fn() {}\n"
                        "pub(in crate::game) fn c_called_fn() {}\n"
                        "pub(in crate::game) fn g_stay() {}\n"
                        "impl Game<'_> {\n"
                        "    pub(in crate::game) fn c_only_method(&self) {}\n"
                        "    pub(in crate::game) fn s_only_method(&self) {}\n"
                        "}\n"
                    ),
                    "tools/game_module_sides.tsv": "module\tside\treason\nmixed\tgame\tsplit\n",
                    "tools/game_fn_sides.tsv": (
                        "module\timpl\tname\tcfg\tside\treason\n"
                        "mixed\t-\tc_called_fn\t-\tclient\ttest\n"
                        "mixed\t-\tc_only_fn\t-\tclient\ttest\n"
                        "mixed\t-\tg_stay\t-\tgame\ttest\n"
                        "mixed\timpl Game<'_>\tc_only_method\t-\tclient\ttest\n"
                        "mixed\timpl Game<'_>\ts_only_method\t-\tserver\ttest\n"
                    ),
                    "tools/game_side_calls.tsv": (
                        "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                        "client\tmixed_client\t-\tc_called_fn\t-\tfn\tgame\tcalled\n"
                    ),
                },
            )
            # 1. Run game_fn_sides --fix
            rc = gfs.main(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0)
            # 2. Run game_side_calls --fix
            rc, _o, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0, err)
            # 3. Both --check pass
            rc = gfs.main(["--check", "--root", str(root)])
            self.assertEqual(rc, 0)
            rc, _o, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 0, err)
            # 4. Re-running game_fn_sides --fix must NOT widen narrowed mirror methods/fns back
            c_before = (root / "crates/sunrise-client/src/game/mixed_client.rs").read_text()
            self.assertIn("pub(in crate::game::client_side) fn c_only_fn()", c_before)
            self.assertIn("pub(in crate::game::client_side) fn c_only_method(", c_before)
            self.assertIn("pub(in crate::game) fn c_called_fn()", c_before)
            orig_after = (root / "crates/sunrise-client/src/game/mixed.rs").read_text()
            self.assertNotIn("c_only_fn", orig_after)
            self.assertIn("c_called_fn", orig_after)

            rc = gfs.main(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0)
            self.assertEqual((root / "crates/sunrise-client/src/game/mixed_client.rs").read_text(), c_before)
            rc, _o, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 0, err)

    def test_help_states_compiler_invariant_and_ci_wiring(self) -> None:
        proc = subprocess.run(
            [sys.executable, str(ROOT / "tools/game_side_calls.py"), "--help"],
            capture_output=True,
            text=True,
            check=True,
        )
        self.assertIn("compiler", proc.stdout.lower())
        wf = (ROOT / ".github/workflows/dyna-ci.yml").read_text(encoding="utf-8")
        self.assertIn("python3 tools/game_side_calls.py --check", wf)
        pr = (ROOT / "tools/pr_ready.py").read_text(encoding="utf-8")
        self.assertIn("tools/game_side_calls.py --check --against", pr)

    def test_pull_request_merge_baseline_ratchet(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                _base_tree(
                    {
                        "crates/sunrise-client/src/game/alpha.rs": (
                            "pub(in crate::game) fn kept_wide() {}\n"
                            "pub(in crate::game) fn narrow_later() {}\n"
                        ),
                        "crates/sunrise-client/src/game/beta.rs": "pub(crate) fn server_wide_kept() {}\n",
                        "tools/game_side_calls.tsv": (
                            "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                            "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
                            "client\talpha\t-\tnarrow_later\t-\tfn\tgame\tcalled\n"
                            "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n"
                        ),
                    }
                ),
            )
            git_env = {
                "GIT_AUTHOR_NAME": "Test",
                "GIT_AUTHOR_EMAIL": "t@example.com",
                "GIT_COMMITTER_NAME": "Test",
                "GIT_COMMITTER_EMAIL": "t@example.com",
            }
            subprocess.run(["git", "init", "-q", "-b", "main"], cwd=root, check=True)
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(["git", "commit", "-q", "-m", "base"], cwd=root, env=git_env, check=True)
            base_sha = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=True
            ).stdout.strip()

            # Non-merge commit with --pull-request-merge must fail closed
            rc, _o, err = _run_gsc(["--check", "--root", str(root), "--against", base_sha, "--pull-request-merge"])
            self.assertEqual(rc, 1)
            self.assertIn("merge", err)

            # Create PR branch shrinking table, merge --no-ff into main
            subprocess.run(["git", "checkout", "-q", "-b", "pr"], cwd=root, check=True)
            (root / "tools/game_side_calls.tsv").write_text(
                "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
                "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n",
                encoding="utf-8",
            )
            rc, _o, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 0, err)
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(["git", "commit", "-q", "-m", "shrink"], cwd=root, env=git_env, check=True)
            subprocess.run(["git", "checkout", "-q", "main"], cwd=root, check=True)
            subprocess.run(["git", "merge", "-q", "--no-ff", "pr", "-m", "merge pr"], cwd=root, env=git_env, check=True)

            rc, out, err = _run_gsc(["--check", "--root", str(root), "--against", base_sha, "--pull-request-merge"])
            self.assertEqual(rc, 0, err)
            self.assertIn("ratchet vs", out)

    def test_refusals_non_literal_include_orphan_narrow_row_and_dynamic_macro_name(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                _base_tree(
                    {
                        "crates/sunrise-client/src/game/alpha.rs": (
                            "include!(unknown_macro!());\n"
                            "pub(in crate::game::client_side) fn kept_wide() {}\n"
                            "pub(in bogus_mod) fn weird_vis() {}\n"
                        ),
                        "crates/sunrise-client/src/game/beta.rs": (
                            "pub(crate) fn server_wide_kept() {}\n"
                            "macro_rules! dyn_fn { ($n:ident) => { pub(in crate::game) fn $n() {} }; }\n"
                        ),
                    }
                ),
            )
            rc, _o, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("non-literal `include!", err)
            self.assertIn("already `pub(in crate::game::client_side)`", err)
            self.assertIn("unsupported visibility `pub(in bogus_mod)`", err)
            self.assertIn("dynamic name", err)

    def test_fix_refuses_split_cfg_twin_atomically(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(
                root,
                _base_tree(
                    {
                        "crates/sunrise-client/src/game/alpha.rs": (
                            "pub(in crate::game) fn kept_wide() {}\n"
                            "pub(in crate::game) fn would_narrow() {}\n"
                            '#[cfg(feature = "a")]\n'
                            "pub(in crate::game) fn twin() {}\n"
                            '#[cfg(not(feature = "a"))]\n'
                            "pub(in crate::game) fn twin() {}\n"
                        ),
                        "crates/sunrise-client/src/game/beta.rs": "pub(crate) fn server_wide_kept() {}\n",
                        "tools/game_side_calls.tsv": (
                            "side\tmodule\timpl\tname\tcfg\tkind\tcallers\treason\n"
                            "client\talpha\t-\tkept_wide\t-\tfn\tserver\tcalled\n"
                            'client\talpha\t-\ttwin\t#[cfg(feature = "a")]\tfn\tserver\tcalled\n'
                            "server\tbeta\t-\tserver_wide_kept\t-\tfn\tclient,game\tcalled\n"
                        ),
                    }
                ),
            )
            before = (root / "crates/sunrise-client/src/game/alpha.rs").read_text()
            rc, _o, err = _run_gsc(["--fix", "--no-fmt", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("cfg-twin split visibility", err)
            self.assertEqual((root / "crates/sunrise-client/src/game/alpha.rs").read_text(), before)

    def test_missing_rustfmt_and_side_container_refusals(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            _write_tree(root, _base_tree())
            alpha_before = (root / "crates/sunrise-client/src/game/alpha.rs").read_text()
            old_rf = os.environ.get("RUSTFMT")
            try:
                os.environ["RUSTFMT"] = "/nonexistent/rustfmt-binary"
                rc, _o, err = _run_gsc(["--fix", "--root", str(root)])
            finally:
                if old_rf is None:
                    os.environ.pop("RUSTFMT", None)
                else:
                    os.environ["RUSTFMT"] = old_rf
            self.assertEqual(rc, 1)
            self.assertIn("rustfmt not found", err)
            self.assertEqual((root / "crates/sunrise-client/src/game/alpha.rs").read_text(), alpha_before)

            cs = root / "crates/sunrise-client/src/game/client_side.rs"
            cs.write_text(cs.read_text() + "pub(in crate::game) fn stray_in_container() {}\n", encoding="utf-8")
            rc, _o, err = _run_gsc(["--check", "--root", str(root)])
            self.assertEqual(rc, 1)
            self.assertIn("stray_in_container", err)


if __name__ == "__main__":
    unittest.main()
