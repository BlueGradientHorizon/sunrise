"""Unit tests for `tools/game_field_owners.py` bidirectional rewrites, scoping, macros, and ambiguity."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_GFO_SPEC = importlib.util.spec_from_file_location(
    "game_field_owners", ROOT / "tools" / "game_field_owners.py"
)
assert _GFO_SPEC is not None and _GFO_SPEC.loader is not None
gfo = importlib.util.module_from_spec(_GFO_SPEC)
sys.modules["game_field_owners"] = gfo
_GFO_SPEC.loader.exec_module(gfo)

OWNERS: dict[str, str] = {
    "screen": "client",
    "hoppers": "server",
    "flight": "game",
    "fns": "game",
    "world_tables": "game",
    "lent_mobs": "game",
    "pending_entry": "game",
}


def run_scan(
    src: str,
    index: gfo.WorkspaceIndex | None = None,
    rel: str = "crates/sunrise-client/src/game/test.rs",
):
    idx = index if index is not None else gfo.WorkspaceIndex()
    edits, ambig = gfo.scan_file(src, rel, OWNERS, idx)
    return gfo.apply_edits(src, edits), edits, ambig


class GameFieldOwnersShadowingTests(unittest.TestCase):
    def test_if_let_shadows_unknown_and_other_type_bindings(self):
        src = (
            "fn f(game: &Game<'_>, other: Option<Other>) -> i32 {\n"
            "    if let Some(game) = other {\n"
            "        return game.screen;\n"
            "    } else {\n"
            "        let _ = game.screen;\n"
            "    }\n"
            "    game.screen\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 2)
        self.assertIn("return game.screen;", out)
        self.assertIn("let _ = game.client.screen;", out)
        self.assertIn("\n    game.client.screen\n", out)

    def test_if_let_scrutinee_uses_outer_game_and_chain_uses_shadow(self):
        src = (
            "fn f(game: &Game<'_>, other: Option<Other>) -> i32 {\n"
            "    if let Some(game) = game.hoppers.get(&0) && game.screen > 0 {\n"
            "        return game.screen;\n"
            "    }\n"
            "    if let Some(game) = other && game.screen > 0 {\n"
            "        return game.screen;\n"
            "    }\n"
            "    0\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("game.server.hoppers.get(&0) && game.screen > 0", out)
        self.assertNotIn("game.client.screen", out)

    def test_while_let_shadows_tracked_name(self):
        src = (
            "fn f(game: &Game<'_>, mut it: Iter) {\n"
            "    while let Some(game) = it.next() {\n"
            "        let _ = game.screen;\n"
            "    }\n"
            "    let _ = game.screen;\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("while let Some(game) = it.next() {\n        let _ = game.screen;", out)
        self.assertIn("    }\n    let _ = game.client.screen;", out)

    def test_match_arm_binding_guard_block_and_or_pattern(self):
        src = (
            "fn f(game: &Game<'_>, other: Result<Other, Other>) -> i32 {\n"
            "    let a = match other {\n"
            "        Some(game) if game.screen > 0 => game.screen,\n"
            "        | Ok(game) | Err(game) => {\n"
            "            let _ = game.hoppers;\n"
            "            game.screen\n"
            "        }\n"
            "        None => game.screen,\n"
            "    };\n"
            "    a + game.screen\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 2)
        self.assertIn("Some(game) if game.screen > 0 => game.screen,", out)
        self.assertIn("let _ = game.hoppers;", out)
        self.assertIn("None => game.client.screen,", out)
        self.assertIn("a + game.client.screen", out)

    def test_let_rebinding_rhs_and_let_else_and_destructuring(self):
        src = (
            "fn f(game: &Game<'_>, other: (Other, i32), opt: Option<Other>) -> i32 {\n"
            "    let x = {\n"
            "        let game = game.screen;\n"
            "        game.screen\n"
            "    };\n"
            "    let y = {\n"
            "        let Some(game) = opt else {\n"
            "            return game.screen;\n"
            "        };\n"
            "        game.screen\n"
            "    };\n"
            "    let z = {\n"
            "        let (game, _) = other;\n"
            "        game.screen\n"
            "    };\n"
            "    let w = {\n"
            "        let Holder { game, .. } = wrap();\n"
            "        game.screen\n"
            "    };\n"
            "    let u = {\n"
            "        let game @ Some(_) = opt else { return 0; };\n"
            "        game.screen\n"
            "    };\n"
            "    x + y + z + w + u + game.screen\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 3)
        self.assertIn("let game = game.client.screen;\n        game.screen", out)
        self.assertIn("return game.client.screen;\n        };\n        game.screen", out)
        self.assertIn("let (game, _) = other;\n        game.screen", out)
        self.assertIn("let Holder { game, .. } = wrap();\n        game.screen", out)
        self.assertIn("let game @ Some(_) = opt else { return 0; };\n        game.screen", out)
        self.assertIn("+ game.client.screen", out)

    def test_for_loop_shadows_tracked_name(self):
        src = (
            "fn f(game: &Game<'_>) {\n"
            "    for game in game.hoppers.values() {\n"
            "        let _ = game.screen;\n"
            "    }\n"
            "    let _ = game.screen;\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 2)
        self.assertIn("for game in game.server.hoppers.values()", out)
        self.assertIn("{\n        let _ = game.screen;\n    }", out)
        self.assertIn("}\n    let _ = game.client.screen;", out)

    def test_closure_param_of_other_or_unknown_or_destructured_type(self):
        src = (
            "fn f(game: &Game<'_>, items: Vec<(Other, i32)>) {\n"
            "    let _ = |game: &Other| game.screen;\n"
            "    let _ = unknown_helper(|game| game.screen);\n"
            "    items.into_iter().for_each(|(game, _)| {\n"
            "        let _ = game.screen;\n"
            "    });\n"
            "    let _ = |Wrap { game }| game.screen;\n"
            "    let _ = game.screen;\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("|game: &Other| game.screen;", out)
        self.assertIn("unknown_helper(|game| game.screen);", out)
        self.assertIn("|(game, _)| {\n        let _ = game.screen;", out)
        self.assertIn("|Wrap { game }| game.screen;", out)
        self.assertIn("let _ = game.client.screen;", out)

    def test_nested_fn_does_not_capture_outer_game_local(self):
        src = (
            "fn outer(game: &Game<'_>) {\n"
            "    fn inner(game: &Other) {\n"
            "        let _ = game.screen;\n"
            "    }\n"
            "    let _ = game.screen;\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("fn inner(game: &Other) {\n        let _ = game.screen;", out)
        self.assertIn("}\n    let _ = game.client.screen;", out)


class GameFieldOwnersMacroTests(unittest.TestCase):
    def test_token_only_macros_are_left_alone(self):
        src = (
            "fn f(game: &Game<'_>) -> &'static str {\n"
            "    let _ = stringify!(game.screen);\n"
            "    let _ = concat!(\"x\", stringify!(game.hoppers));\n"
            "    let _ = cfg!(feature = \"friends\");\n"
            "    let _ = quote!(game.screen);\n"
            "    let _ = game.screen;\n"
            "    stringify!(game.client.flight)\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("stringify!(game.screen)", out)
        self.assertIn("stringify!(game.hoppers)", out)
        self.assertIn("quote!(game.screen)", out)
        self.assertIn("stringify!(game.client.flight)", out)
        self.assertIn("let _ = game.client.screen;", out)

    def test_expression_macros_and_negated_blocks_are_rewritten(self):
        src = (
            "fn f(game: &Game<'_>) {\n"
            "    assert_eq!(game.screen, 1);\n"
            "    let _ = format!(\"{:?}\", game.hoppers);\n"
            "    if !{ game.screen == 0 } {\n"
            "        let _ = matches!(game.screen, 1);\n"
            "    }\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 4)
        self.assertIn("assert_eq!(game.client.screen, 1);", out)
        self.assertIn('format!("{:?}", game.server.hoppers);', out)
        self.assertIn("if !{ game.client.screen == 0 }", out)
        self.assertIn("matches!(game.client.screen, 1)", out)

    def test_unknown_macro_is_reported_ambiguous_not_rewritten(self):
        src = (
            "fn f(game: &Game<'_>) {\n"
            "    let _ = custom_mod_macro!(game.screen);\n"
            "    let _ = game.hoppers;\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(len(edits), 1)
        self.assertEqual(len(ambig), 1)
        self.assertEqual(ambig[0].field, "screen")
        self.assertIn("custom_mod_macro", ambig[0].detail)
        self.assertIn("custom_mod_macro!(game.screen);", out)
        self.assertIn("let _ = game.server.hoppers;", out)


class GameFieldOwnersBidirectionalTests(unittest.TestCase):
    def test_moves_forward_backward_and_between_tails(self):
        src = (
            "struct Wrapper<'a> { game: &'a mut Game<'a> }\n"
            "impl Game<'_> {\n"
            "    fn demo(&mut self) {\n"
            "        let _ = self.screen;\n"
            "        let _ = self.screen();\n"
            "        let _ = self.hoppers;\n"
            "        let _ = self.client.flight;\n"
            "        let _ = self.server.fns;\n"
            "        let _ = self.client\n"
            "            .pending_entry;\n"
            "        let _ = self.server.screen;\n"
            "        let _ = self.client.hoppers;\n"
            "        let _ = self.client.screen;\n"
            "        let _ = self.server.hoppers;\n"
            "        let _ = self.flight;\n"
            "        let _ = (*self).client.flight;\n"
            "    }\n"
            "}\n"
            "impl Wrapper<'_> {\n"
            "    fn drop_loan(&mut self) {\n"
            "        let _ = self.game.server.lent_mobs;\n"
            "        let _ = self.game.screen;\n"
            "    }\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 10)
        self.assertIn("let _ = self.client.screen;\n        let _ = self.screen();", out)
        self.assertIn("let _ = self.server.hoppers;", out)
        self.assertIn("let _ = self.flight;\n        let _ = self.fns;", out)
        self.assertIn("let _ = self.pending_entry;", out)
        self.assertIn("let _ = (*self).flight;", out)
        self.assertIn("let _ = self.game.lent_mobs;", out)
        self.assertIn("let _ = self.game.client.screen;", out)
        # Re-scanning the rewritten output must be a clean fixed point.
        out2, edits2, ambig2 = run_scan(out)
        self.assertEqual(out2, out)
        self.assertEqual(edits2, [])
        self.assertEqual(ambig2, [])

    def test_macro_var_closure_param_rewrites_correctly(self):
        idx = gfo.WorkspaceIndex()
        idx.pub_fn_closure_params["with_game"] = {0: {0: "Game"}}
        src = (
            "macro_rules! check {\n"
            "    ($game:ident) => {\n"
            "        with_game(|$game| {\n"
            "            let _ = $game.hoppers;\n"
            "            let _ = $game.client.flight;\n"
            "        });\n"
            "    };\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src, index=idx)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 2)
        self.assertIn("let _ = $game.server.hoppers;", out)
        self.assertIn("let _ = $game.flight;", out)


class GameFieldOwnersCliAmbiguousTests(unittest.TestCase):
    def _write_table(self, path: Path) -> None:
        lines = ["field\ttype\towner\treason\tdeclaration_index"]
        for i, (fname, owner) in enumerate(OWNERS.items()):
            lines.append(f"{fname}\tT\t{owner}\ttest\t{i}")
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")

    def test_cli_ambiguous_fails_fix_unless_allowed_and_check_catches_both_directions(self):
        import io
        from contextlib import redirect_stderr, redirect_stdout

        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            table = td_path / "owners.tsv"
            self._write_table(table)
            game_dir = td_path / "crates/sunrise-client/src/game"
            game_dir.mkdir(parents=True)
            target = game_dir / "sample.rs"
            target.write_text(
                "fn f(game: &Game<'_>) {\n"
                "    let _ = mystery_macro!(game.screen);\n"
                "    let _ = game.client.flight;\n"
                "    let _ = game.hoppers;\n"
                "}\n",
                encoding="utf-8",
            )
            old_root = gfo.ROOT
            try:
                gfo.ROOT = td_path
                out_buf, err_buf = io.StringIO(), io.StringIO()
                with redirect_stdout(out_buf), redirect_stderr(err_buf):
                    rc_check = gfo.main(["--check", "--table", str(table), str(target)])
                self.assertEqual(rc_check, 1)
                self.assertIn("stale Game field `client.flight` -> `flight`", out_buf.getvalue())
                self.assertIn("stale Game field `hoppers` -> `server.hoppers`", out_buf.getvalue())
                self.assertIn("ambiguous receiver for Game field `screen`", err_buf.getvalue())

                out_buf, err_buf = io.StringIO(), io.StringIO()
                with redirect_stdout(out_buf), redirect_stderr(err_buf):
                    rc_fix = gfo.main(["--fix", "--no-fmt", "--table", str(table), str(target)])
                self.assertEqual(rc_fix, 1)
                self.assertIn("ambiguous receiver for Game field `screen`", err_buf.getvalue())
                rewritten = target.read_text(encoding="utf-8")
                self.assertIn("mystery_macro!(game.screen);", rewritten)
                self.assertIn("let _ = game.flight;", rewritten)
                self.assertIn("let _ = game.server.hoppers;", rewritten)

                out_buf, err_buf = io.StringIO(), io.StringIO()
                with redirect_stdout(out_buf), redirect_stderr(err_buf):
                    rc_allow = gfo.main(
                        ["--fix", "--no-fmt", "--allow-ambiguous", "--table", str(table), str(target)]
                    )
                self.assertEqual(rc_allow, 0)
            finally:
                gfo.ROOT = old_root

    def test_colliding_free_fn_reported_ambiguous(self):
        idx = gfo.WorkspaceIndex()
        idx.ambiguous_return_fns.add("make_ctx")
        idx.ambiguous_closure_fns.add("with_ctx")
        src = (
            "fn demo() {\n"
            "    let g = make_ctx();\n"
            "    let _ = g.screen;\n"
            "    with_ctx(|h| {\n"
            "        let _ = h.hoppers;\n"
            "    });\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src, index=idx)
        self.assertEqual(out, src)
        self.assertEqual(edits, [])
        self.assertEqual([(a.field, a.detail) for a in ambig], [
            ("screen", "ambiguous variable `g`"),
            ("hoppers", "ambiguous variable `h`"),
        ])


class GameFieldOwnersRound2RegressionTests(unittest.TestCase):
    def test_block_expr_match_if_let_while_let_and_for_shadowing(self):
        src = (
            "fn f(game: &Game<'_>, other: Option<Other>, others: Vec<Other>) -> i32 {\n"
            "    let a = match { other } {\n"
            "        Some(game) => game.screen,\n"
            "        None => 0,\n"
            "    };\n"
            "    if let Some(game) = { other } {\n"
            "        return game.screen;\n"
            "    }\n"
            "    while let Some(game) = { other } {\n"
            "        return game.screen;\n"
            "    }\n"
            "    for game in { others } {\n"
            "        return game.screen;\n"
            "    }\n"
            "    a + game.screen\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("Some(game) => game.screen,", out)
        self.assertIn("if let Some(game) = { other } {\n        return game.screen;", out)
        self.assertIn("while let Some(game) = { other } {\n        return game.screen;", out)
        self.assertIn("for game in { others } {\n        return game.screen;", out)
        self.assertIn("a + game.client.screen", out)

    def test_nested_control_flow_in_match_and_if_let_scrutinee(self):
        src = (
            "fn f(game: &Game<'_>, flag: bool, other: Option<Other>) -> i32 {\n"
            "    let a = match if flag { other } else { None } {\n"
            "        Some(game) => game.screen,\n"
            "        None => game.screen,\n"
            "    };\n"
            "    if let Some(game) = unsafe { other } && { game.screen > 0 } {\n"
            "        return game.screen;\n"
            "    }\n"
            "    a\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("Some(game) => game.screen,", out)
        self.assertIn("None => game.client.screen,", out)
        self.assertIn("&& { game.screen > 0 } {\n        return game.screen;", out)

    def test_local_or_qualified_macro_shadowing_allowlisted_name_fails_closed(self):
        src = (
            "macro_rules! dbg {\n"
            "    ($($tokens:tt)*) => { stringify!($($tokens)*) };\n"
            "}\n"
            "fn f(game: &Game<'_>) -> &'static str {\n"
            "    let _ = other_crate::format!(\"{:?}\", game.hoppers);\n"
            "    dbg!(game.screen)\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(out, src)
        self.assertEqual(edits, [])
        self.assertEqual(len(ambig), 2)
        self.assertEqual({a.field for a in ambig}, {"screen", "hoppers"})

    def test_imported_macro_shadowing_allowlisted_name_fails_closed(self):
        src = (
            "use my_macros::dbg;\n"
            "fn f(game: &Game<'_>) -> &'static str {\n"
            "    dbg!(game.screen)\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(out, src)
        self.assertEqual(edits, [])
        self.assertEqual(len(ambig), 1)
        self.assertEqual(ambig[0].field, "screen")

    def test_raw_identifier_shadows_ordinary_binding_and_matches_receiver(self):
        src = (
            "fn f(game: &Game<'_>, other: Option<Other>) -> i32 {\n"
            "    if let Some(r#game) = other {\n"
            "        return game.screen;\n"
            "    }\n"
            "    let _ = {\n"
            "        let r#game = other;\n"
            "        game.screen\n"
            "    };\n"
            "    r#game.screen\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("if let Some(r#game) = other {\n        return game.screen;", out)
        self.assertIn("let r#game = other;\n        game.screen", out)
        self.assertIn("r#game.client.screen", out)

    def test_uppercase_pattern_binding_colliding_with_tracked_var_fails_closed(self):
        src = (
            "fn f(GameVar: &Game<'_>, other: Option<Other>) -> i32 {\n"
            "    if let Some(GameVar) = other {\n"
            "        return GameVar.screen;\n"
            "    }\n"
            "    GameVar.screen\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(len(edits), 1)
        self.assertEqual(len(ambig), 1)
        self.assertEqual(ambig[0].field, "screen")
        self.assertIn("return GameVar.screen;", out)
        self.assertIn("\n    GameVar.client.screen\n", out)

    def test_non_sunrise_qualified_or_imported_game_type_not_rewritten(self):
        src = (
            "fn f(game: &other::Game) -> i32 {\n"
            "    game.screen\n"
            "}\n"
            "impl other::Game {\n"
            "    fn g(&self) -> i32 {\n"
            "        self.screen\n"
            "    }\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(out, src)
        self.assertEqual(edits, [])
        self.assertEqual(ambig, [])

    def test_imported_foreign_game_suppresses_bare_game_type(self):
        src = (
            "use other_crate::Game;\n"
            "fn f(game: &Game) -> i32 {\n"
            "    game.screen\n"
            "}\n"
            "impl Game {\n"
            "    fn g(&self) -> i32 {\n"
            "        self.screen\n"
            "    }\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src)
        self.assertEqual(out, src)
        self.assertEqual(edits, [])
        self.assertEqual(ambig, [])

    def test_external_file_outside_game_tree_rewrites_nothing_and_reports_manual(self):
        src = (
            "fn mod_hook(game: &mut sunrise_client::game::Game<'_>) {\n"
            "    let _ = game.screen;\n"
            "    let _ = game.client.flight;\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src, rel="my_mod/src/lib.rs")
        self.assertEqual(out, src)
        self.assertEqual(edits, [])
        self.assertEqual([a.field for a in ambig], ["screen", "flight"])

    def test_cli_external_file_fails_closed_and_leaves_file_untouched(self):
        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            table = td_path / "owners.tsv"
            lines = ["field\ttype\towner\treason\tdeclaration_index"]
            for i, (fname, owner) in enumerate(OWNERS.items()):
                lines.append(f"{fname}\tT\t{owner}\ttest\t{i}")
            table.write_text("\n".join(lines) + "\n", encoding="utf-8")
            target = td_path / "mod_hook.rs"
            src = (
                "fn mod_hook(game: &mut sunrise_client::game::Game<'_>) {\n"
                "    let _ = game.screen;\n"
                "}\n"
            )
            target.write_text(src, encoding="utf-8")
            rc = subprocess.run(
                [sys.executable, str(ROOT / "tools/game_field_owners.py"), "--fix", "--no-fmt", "--table", str(table), str(target)],
                capture_output=True,
                text=True,
            )
            self.assertEqual(rc.returncode, 1)
            self.assertEqual(target.read_text(encoding="utf-8"), src)
            self.assertIn("screen", rc.stderr)

    def test_cli_list_prints_sites_with_line_and_justification_without_modifying(self):
        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            table = td_path / "owners.tsv"
            lines = ["field\ttype\towner\treason\tdeclaration_index"]
            for i, (fname, owner) in enumerate(OWNERS.items()):
                lines.append(f"{fname}\tT\t{owner}\ttest\t{i}")
            table.write_text("\n".join(lines) + "\n", encoding="utf-8")
            game_dir = td_path / "crates/sunrise-client/src/game"
            game_dir.mkdir(parents=True)
            target = game_dir / "sample.rs"
            src = (
                "impl Game<'_> {\n"
                "    fn demo(&mut self) {\n"
                "        let _ = self.screen;\n"
                "    }\n"
                "}\n"
                "fn helper(game: &Game<'_>) {\n"
                "    let _ = game.hoppers;\n"
                "}\n"
            )
            target.write_text(src, encoding="utf-8")
            old_root = gfo.ROOT
            try:
                gfo.ROOT = td_path
                import io
                from contextlib import redirect_stdout, redirect_stderr

                out_buf, err_buf = io.StringIO(), io.StringIO()
                with redirect_stdout(out_buf), redirect_stderr(err_buf):
                    rc = gfo.main(["--list", "--table", str(table), str(target)])
            finally:
                gfo.ROOT = old_root
            self.assertEqual(rc, 0)
            self.assertEqual(target.read_text(encoding="utf-8"), src)
            listed = out_buf.getvalue()
            self.assertIn("crates/sunrise-client/src/game/sample.rs:3:", listed)
            self.assertIn("impl-game-self", listed)
            self.assertIn("crates/sunrise-client/src/game/sample.rs:7:", listed)
            self.assertIn("explicit-param", listed)

    def test_aliased_std_macro_and_open_range_for_and_glob_external_import(self):
        src_alias = (
            "use std::stringify as dbg;\n"
            "fn f(game: &Game<'_>) -> &'static str {\n"
            "    dbg!(game.screen)\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src_alias)
        self.assertEqual(out, src_alias)
        self.assertEqual(edits, [])
        self.assertEqual([a.field for a in ambig], ["screen"])

        src_range = (
            "fn f(game: &Game<'_>) {\n"
            "    for game in 0.. {\n"
            "        let _ = game.screen;\n"
            "        break;\n"
            "    }\n"
            "    let _ = game.screen;\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src_range)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("for game in 0.. {\n        let _ = game.screen;", out)
        self.assertIn("}\n    let _ = game.client.screen;", out)

        src_glob = (
            "use sunrise_client::game::*;\n"
            "fn mod_hook(game: &mut Game<'_>) {\n"
            "    let _ = game.screen;\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src_glob, rel="my_mod/src/lib.rs")
        self.assertEqual(out, src_glob)
        self.assertEqual(edits, [])
        self.assertEqual([a.field for a in ambig], ["screen"])


class GameFieldOwnersRound3RegressionTests(unittest.TestCase):
    COMMON = "struct Client { screen: i32 }\nstruct Other { screen: i32, client: Client }\n"

    def _assert_cli_fails_closed(self, snippet: str, rel: str, expected_field: str = "screen") -> None:
        import io
        from contextlib import redirect_stderr, redirect_stdout

        src = self.COMMON + snippet + "\n"
        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            table = td_path / "owners.tsv"
            lines = ["field\ttype\towner\treason\tdeclaration_index"]
            for i, (fname, owner) in enumerate(OWNERS.items()):
                lines.append(f"{fname}\tT\t{owner}\ttest\t{i}")
            table.write_text("\n".join(lines) + "\n", encoding="utf-8")
            target = td_path / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(src, encoding="utf-8")
            old_root = gfo.ROOT
            try:
                gfo.ROOT = td_path
                for mode in ("--list", "--fix", "--check"):
                    out_buf, err_buf = io.StringIO(), io.StringIO()
                    argv = [mode, "--table", str(table), str(target)]
                    if mode == "--fix":
                        argv.insert(1, "--no-fmt")
                    with redirect_stdout(out_buf), redirect_stderr(err_buf):
                        rc = gfo.main(argv)
                    self.assertNotEqual(rc, 0, f"{mode} exited 0 for: {snippet}")
                    self.assertIn(f"Game field `{expected_field}`", err_buf.getvalue())
                    self.assertEqual(target.read_text(encoding="utf-8"), src)
            finally:
                gfo.ROOT = old_root

    def test_repro_finding2_matches_pattern_binding_shadows_guard(self):
        self._assert_cli_fails_closed(
            "fn f(game: &Game<'_>, other: Option<Other>) -> bool { matches!(other, Some(game) if game.screen > 0) }",
            "crates/sunrise-client/src/game/probe.rs",
        )

    def test_finding2_pattern_macros_and_non_shadowed_matches_guard(self):
        src_std_matches = (
            "fn f(game: &Game<'_>, other: Option<Other>) -> bool {\n"
            "    std::matches!(other, Some(game) if game.screen > 0)\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src_std_matches)
        self.assertEqual(out, src_std_matches)
        self.assertEqual(edits, [])
        self.assertEqual([a.field for a in ambig], ["screen"])

        src_assert_matches = (
            "fn f(game: &Game<'_>, other: Option<Other>) {\n"
            "    assert_matches!(other, Some(game) if game.screen > 0);\n"
            "    select! { game = fut => { let _ = game.hoppers; } }\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src_assert_matches)
        self.assertEqual(out, src_assert_matches)
        self.assertEqual(edits, [])
        self.assertEqual({a.field for a in ambig}, {"screen", "hoppers"})

        src_with_entity = (
            "fn f(game: &Game<'_>, h: u32) -> i32 {\n"
            "    with_entity!(game, h, game => game.screen)\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src_with_entity)
        self.assertEqual(out, src_with_entity)
        self.assertEqual(edits, [])
        self.assertEqual([a.field for a in ambig], ["screen"])

        # Non-shadowing `matches!` pattern still rewrites outer `game` in the guard.
        src_ok = (
            "fn f(game: &Game<'_>, other: Option<Other>) -> bool {\n"
            "    matches!(other, Some(x) if game.screen > 0 && x.screen > 0)\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src_ok)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("if game.client.screen > 0 && x.screen > 0", out)

    def test_repro_finding3_qualified_macro_shadowed_by_local_mod_std(self):
        self._assert_cli_fails_closed(
            "mod std { macro_rules! tokens { ($($t:tt)*) => { stringify!($($t)*) }; } pub(super) use tokens as dbg; } "
            "fn f(game: &Game<'_>) -> &'static str { std::dbg!(game.screen) }",
            "crates/sunrise-client/src/game/probe.rs",
        )

    def test_finding3_qualified_macro_roots_aliases_and_enclosing_modules(self):
        for snippet in (
            "use other as std;\nfn f(game: &Game<'_>) { let _ = std::dbg!(game.screen); }\n",
            "extern crate other as core;\nfn f(game: &Game<'_>) { let _ = core::format_args!(\"{:?}\", game.screen); }\n",
            "mod std {}\nmod inner { fn f(game: &super::Game<'_>) { let _ = std::dbg!(game.screen); } }\n",
            "fn f(game: &Game<'_>) { let _ = log::warn!(\"{:?}\", game.screen); }\n",
            "fn f(game: &Game<'_>) { let _ = std::sub::dbg!(game.screen); }\n",
        ):
            out, edits, ambig = run_scan(snippet)
            self.assertEqual(out, snippet)
            self.assertEqual(edits, [])
            self.assertEqual([a.field for a in ambig], ["screen"])

        # Enclosing parent module file declaring `mod std` fails closed too.
        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            mod_dir = td_path / "crates/sunrise-client/src/game/sub"
            mod_dir.mkdir(parents=True)
            (mod_dir / "mod.rs").write_text("mod std {}\n", encoding="utf-8")
            child_rel = "crates/sunrise-client/src/game/sub/child.rs"
            child_src = "fn f(game: &Game<'_>) { let _ = std::dbg!(game.screen); }\n"
            (td_path / child_rel).write_text(child_src, encoding="utf-8")
            old_root = gfo.ROOT
            try:
                gfo.ROOT = td_path
                out, edits, ambig = run_scan(child_src, rel=child_rel)
            finally:
                gfo.ROOT = old_root
            self.assertEqual(out, child_src)
            self.assertEqual(edits, [])
            self.assertEqual([a.field for a in ambig], ["screen"])

        # Unshadowed `std::dbg!` still rewrites cleanly.
        src_ok = "fn f(game: &Game<'_>) { let _ = std::dbg!(game.screen); }\n"
        out, edits, ambig = run_scan(src_ok)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("std::dbg!(game.client.screen)", out)

    def test_repro_finding4_parent_mod_super_game_and_generic_game(self):
        self._assert_cli_fails_closed(
            "mod parent { pub struct Game<'a> { pub screen: i32, pub client: super::Client, pub life: &'a () } "
            "mod child { fn f(game: &super::Game<'_>) -> i32 { game.screen } } }",
            "crates/sunrise-client/src/game/probe.rs",
        )
        self._assert_cli_fails_closed(
            "fn f<Game: std::ops::Deref<Target = Other>>(game: &Game) -> i32 { game.screen }",
            "crates/sunrise-client/src/game/probe.rs",
        )

    def test_finding4_local_item_declarations_self_game_and_sibling_isolation(self):
        for decl in (
            "struct Game { screen: i32 }",
            "enum Game { A }",
            "union Game { x: i32 }",
            "trait Game {}",
            "type Game = Other;",
            "use other::Thing as Game;",
        ):
            src = f"{decl}\nfn f(game: &Game) -> i32 {{ game.screen }}\n"
            out, edits, ambig = run_scan(src)
            self.assertEqual(out, src, decl)
            self.assertEqual(edits, [], decl)
            self.assertEqual([a.field for a in ambig], ["screen"], decl)

        src_self_game = "mod m { struct Game { screen: i32 } fn f(game: &self::Game) -> i32 { game.screen } }\n"
        out, edits, ambig = run_scan(src_self_game)
        self.assertEqual(out, src_self_game)
        self.assertEqual(edits, [])
        self.assertEqual([a.field for a in ambig], ["screen"])

        src_impl_generic = (
            "struct Holder<T>(T);\n"
            "impl<Game> Holder<Game> {\n"
            "    fn f(game: &Game) -> i32 { game.screen }\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src_impl_generic)
        self.assertEqual(out, src_impl_generic)
        self.assertEqual(edits, [])
        self.assertEqual([a.field for a in ambig], ["screen"])

        # A sibling module's `struct Game` does not shadow root's `Game`, unless glob-imported.
        src_sibling = (
            "mod sibling { pub struct Game { pub screen: i32 } }\n"
            "fn f(game: &Game<'_>) -> i32 { game.screen }\n"
        )
        out, edits, ambig = run_scan(src_sibling)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("game.client.screen", out)

        src_glob_foreign = (
            "mod foreign { pub struct Game<'a> { pub screen: i32, pub client: super::Client, pub life: &'a () } }\n"
            "use foreign::*;\n"
            "fn f(game: &Game<'_>) -> i32 { game.screen }\n"
        )
        out, edits, ambig = run_scan(src_glob_foreign)
        self.assertEqual(out, src_glob_foreign)
        self.assertEqual(edits, [])
        self.assertEqual([a.field for a in ambig], ["screen"])

    def test_repro_finding5_qualified_free_function_does_not_infer_game(self):
        self._assert_cli_fails_closed(
            "fn make_game<'a, 'g>(game: &'a Game<'g>) -> &'a Game<'g> { game } "
            "mod other { pub fn make_game() -> super::Other { "
            "super::Other { screen: 11, client: super::Client { screen: 29 } } } } "
            "fn f() -> i32 { let obj = other::make_game(); obj.screen }",
            "crates/sunrise-client/src/game/probe.rs",
        )

    def test_finding5_resolved_vs_unresolved_qualified_calls(self):
        # Unresolved qualified call reports ambiguous when accessing a moved field.
        src_unknown = "fn f() -> i32 { let obj = unknown_mod::make_thing(); obj.screen }\n"
        out, edits, ambig = run_scan(src_unknown)
        self.assertEqual(out, src_unknown)
        self.assertEqual(edits, [])
        self.assertEqual([a.field for a in ambig], ["screen"])

        # Same-module unqualified, `self::`, `Self::`, and indexed `crate::game::...` calls rewrite.
        idx = gfo.WorkspaceIndex()
        idx.indexed_fn_returns[("crate", "game", "helper", "make_remote")] = {None}
        src_resolved = (
            "fn make_local<'a, 'g>(g: &'a Game<'g>) -> &'a Game<'g> { g }\n"
            "impl Game<'_> {\n"
            "    fn clone_ref(&self) -> &Self { self }\n"
            "    fn make_self<'a>(g: &'a Self) -> &'a Self { g }\n"
            "    fn demo(&self) {\n"
            "        let a = make_local(self);\n"
            "        let _ = a.screen;\n"
            "        let b = self::make_local(self);\n"
            "        let _ = b.hoppers;\n"
            "        let c = Self::make_self(self);\n"
            "        let _ = c.screen;\n"
            "        let d = crate::game::helper::make_remote();\n"
            "        let _ = d.hoppers;\n"
            "    }\n"
            "}\n"
        )
        out, edits, ambig = run_scan(src_resolved, index=idx)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 4)
        self.assertIn("a.client.screen", out)
        self.assertIn("b.server.hoppers", out)
        self.assertIn("c.client.screen", out)
        self.assertIn("d.server.hoppers", out)

    def test_repro_followup_game_alias_and_block_let_alias(self):
        self._assert_cli_fails_closed(
            "use sunrise_client::game::Game as GameAlias; fn f(game: &GameAlias<'_>) { let _ = game.screen; }",
            "external/probe.rs",
        )
        self._assert_cli_fails_closed(
            "use super::Game as GameAlias; fn f(game: &GameAlias<'_>) { let _ = game.screen; }",
            "crates/sunrise-client/src/game/probe.rs",
        )
        self._assert_cli_fails_closed(
            "fn f(game: &Game<'_>) { let alias = { game }; let _ = alias.screen; }",
            "crates/sunrise-client/src/game/probe.rs",
        )

    def test_followup_compound_alias_variants_and_non_aliasing_block(self):
        for rhs in ("unsafe { game }", "(game)", "{ let _ = 1; game }"):
            src = f"fn f(game: &Game<'_>) {{ let alias = {rhs}; let _ = alias.screen; }}\n"
            out, edits, ambig = run_scan(src)
            self.assertEqual(out, src, rhs)
            self.assertEqual(edits, [], rhs)
            self.assertEqual([a.field for a in ambig], ["screen"], rhs)

        # A block projecting a field off `game` rewrites `game.screen` and does not flag `x`.
        src_proj = "fn f(game: &Game<'_>) { let x = { game.screen }; let _ = x.hoppers; }\n"
        out, edits, ambig = run_scan(src_proj)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 1)
        self.assertIn("let x = { game.client.screen };", out)


class GameFieldOwnersWrapperTests(unittest.TestCase):
    WRAP_OWNERS = {
        "screen": "client",
        "hoppers": "server",
        "audio": "game",
        "levels": "game",
        "shared": "game",
    }
    WRAPPERS = {
        "audio": "client_only",
        "levels": "server_shared",
    }

    def test_wrapped_field_projection_idempotency_and_fail_closed(self):
        src = (
            "impl Game<'_> {\n"
            "    fn demo(&mut self) {\n"
            "        self.audio.play();\n"
            "        self.audio.0.stop();\n"
            "        let _ = self.audio();\n"
            "        let _ = &mut self.levels;\n"
            "        let _ = self.shared;\n"
            "    }\n"
            "}\n"
            "fn f(game: &mut Game<'_>, other: Option<Other>) {\n"
            "    if let Some(game) = other {\n"
            "        let _ = game.audio;\n"
            "    }\n"
            "    let _ = unknown_macro!(game.levels);\n"
            "}\n"
        )
        idx = gfo.WorkspaceIndex()
        edits, ambig = gfo.scan_file(
            src,
            "crates/sunrise-client/src/game/demo.rs",
            self.WRAP_OWNERS,
            idx,
            wrappers=self.WRAPPERS,
        )
        out = gfo.apply_edits(src, edits)
        self.assertEqual([a.field for a in ambig], ["levels"])
        self.assertEqual(len(edits), 2)
        self.assertIn("self.audio.0.play();", out)
        self.assertIn("self.audio.0.stop();", out)
        self.assertIn("let _ = self.audio();", out)
        self.assertIn("let _ = &mut self.levels.0;", out)
        self.assertIn("if let Some(game) = other {\n        let _ = game.audio;", out)
        self.assertIn("unknown_macro!(game.levels);", out)
        # Re-scanning after rewrite produces zero new edits.
        edits2, ambig2 = gfo.scan_file(
            out,
            "crates/sunrise-client/src/game/demo.rs",
            self.WRAP_OWNERS,
            idx,
            wrappers=self.WRAPPERS,
        )
        self.assertEqual(edits2, [])
        self.assertEqual([a.field for a in ambig2], ["levels"])

    def test_array_param_closure_and_struct_field_closure_rewrite(self):
        src = (
            "struct Fixture {\n"
            "    seed: fn(&mut Game<'_>),\n"
            "}\n"
            "fn demo(g: &mut Game<'_>) {\n"
            "    let f = |game: &mut Game<'_>, pos: [f64; 3]| {\n"
            "        let _ = game.audio;\n"
            "        let _ = pos;\n"
            "    };\n"
            "    let _ = Fixture {\n"
            "        seed: |game| {\n"
            "            let _ = game.levels;\n"
            "        },\n"
            "    };\n"
            "    f(g, [0.0; 3]);\n"
            "}\n"
        )
        idx = gfo.WorkspaceIndex()
        edits, ambig = gfo.scan_file(
            src,
            "crates/sunrise-client/src/game/demo.rs",
            self.WRAP_OWNERS,
            idx,
            wrappers=self.WRAPPERS,
        )
        out = gfo.apply_edits(src, edits)
        self.assertEqual(ambig, [])
        self.assertEqual(len(edits), 2)
        self.assertIn("let _ = game.audio.0;", out)
        self.assertIn("let _ = game.levels.0;", out)

    def test_cli_wrapper_fix_check_and_drift_rejection(self):
        import io
        from contextlib import redirect_stderr, redirect_stdout

        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            table = td_path / "owners.tsv"
            table.write_text(
                "field\ttype\towner\treason\tdeclaration_index\twrapper\n"
                "screen\ti32\tclient\tclient-only\t0\t-\n"
                "hoppers\ti32\tserver\tserver-only\t1\t-\n"
                "audio\tAudio\tgame\tdrop-order (client)\t2\tclient_only\n"
                "levels\tLevels\tgame\tdrop-order (server)\t3\tserver_shared\n"
                "shared\ti32\tgame\tshared\t4\t-\n",
                encoding="utf-8",
            )
            src_dir = td_path / "crates/sunrise-client/src"
            game_dir = src_dir / "game"
            game_dir.mkdir(parents=True)
            game_rs = src_dir / "game.rs"
            game_rs.write_text(
                "mod client_side;\n"
                "mod server_side;\n"
                "pub struct Game<'a> {\n"
                "    audio: Audio,\n"
                "    levels: Levels,\n"
                "    shared: i32,\n"
                "    server: server_side::ServerState,\n"
                "    client: client_side::ClientState,\n"
                "}\n"
                "impl<'a> Game<'a> {\n"
                "    fn finish_construct(audio: Audio, lv: Levels, shared: i32) -> Self {\n"
                "        Game {\n"
                "            audio,\n"
                "            levels: lv,\n"
                "            shared,\n"
                "            server: server_side::ServerState::new(),\n"
                "            client: client_side::ClientState::new(),\n"
                "        }\n"
                "    }\n"
                "    fn use_it(&mut self) {\n"
                "        let _ = self.audio;\n"
                "        let _ = self.levels;\n"
                "    }\n"
                "}\n",
                encoding="utf-8",
            )
            client_rs = game_dir / "client_side.rs"
            client_rs.write_text("pub(in crate::game) struct ClientState {}\n", encoding="utf-8")
            server_rs = game_dir / "server_side.rs"
            server_rs.write_text("pub(in crate::game) struct ServerState {}\n", encoding="utf-8")

            old_root = gfo.ROOT
            try:
                gfo.ROOT = td_path
                out_buf, err_buf = io.StringIO(), io.StringIO()
                with redirect_stdout(out_buf), redirect_stderr(err_buf):
                    rc_fix = gfo.main(["--fix", "--no-fmt", "--table", str(table), str(game_rs), str(game_dir)])
                self.assertEqual(rc_fix, 0, err_buf.getvalue())

                g_txt = game_rs.read_text(encoding="utf-8")
                self.assertIn("audio: client_side::ClientOnly<Audio>,", g_txt)
                self.assertIn("levels: server_side::ServerShared<Levels>,", g_txt)
                self.assertIn("audio: client_side::ClientOnly::new(audio),", g_txt)
                self.assertIn("levels: server_side::ServerShared::new(lv),", g_txt)
                self.assertIn("let _ = self.audio.0;", g_txt)
                self.assertIn("let _ = self.levels.0;", g_txt)
                self.assertIn("struct ClientOnly<T>(pub(in crate::game::client_side) T);", client_rs.read_text(encoding="utf-8"))
                self.assertIn("struct ServerShared<T>(pub(in crate::game) T);", server_rs.read_text(encoding="utf-8"))

                out_buf, err_buf = io.StringIO(), io.StringIO()
                with redirect_stdout(out_buf), redirect_stderr(err_buf):
                    rc_check = gfo.main(["--check", "--table", str(table), str(game_rs), str(game_dir)])
                self.assertEqual(rc_check, 0, err_buf.getvalue())

                # Widening `ClientOnly` -> `ClientShared` in `game.rs` without updating the table fails `--check`.
                drifted = g_txt.replace("client_side::ClientOnly<Audio>", "client_side::ClientShared<Audio>")
                game_rs.write_text(drifted, encoding="utf-8")
                out_buf, err_buf = io.StringIO(), io.StringIO()
                with redirect_stdout(out_buf), redirect_stderr(err_buf):
                    rc_drift = gfo.main(["--check", "--table", str(table), str(game_rs), str(game_dir)])
                self.assertEqual(rc_drift, 1)
                self.assertIn("wrapper is `client_shared`, expected `client_only`", err_buf.getvalue())

                # Restore `game.rs` and verify adding `#[derive(Default)]` or `impl Deref` fails `--check`.
                game_rs.write_text(g_txt, encoding="utf-8")
                c_orig = client_rs.read_text(encoding="utf-8")
                client_rs.write_text(
                    c_orig.replace("#[repr(transparent)]", "#[derive(Default)]\n#[repr(transparent)]", 1),
                    encoding="utf-8",
                )
                out_buf, err_buf = io.StringIO(), io.StringIO()
                with redirect_stdout(out_buf), redirect_stderr(err_buf):
                    rc_derive = gfo.main(["--check", "--table", str(table), str(game_rs), str(game_dir)])
                self.assertEqual(rc_derive, 1)
                self.assertIn("must not carry `#[derive(...)]`", err_buf.getvalue())

                client_rs.write_text(
                    c_orig + "\nimpl<T> std::ops::Deref for ClientOnly<T> { type Target = T; fn deref(&self) -> &T { &self.0 } }\n",
                    encoding="utf-8",
                )
                out_buf, err_buf = io.StringIO(), io.StringIO()
                with redirect_stdout(out_buf), redirect_stderr(err_buf):
                    rc_deref = gfo.main(["--check", "--table", str(table), str(game_rs), str(game_dir)])
                self.assertEqual(rc_deref, 1)
                self.assertIn("must not implement `Deref`", err_buf.getvalue())
            finally:
                gfo.ROOT = old_root


if __name__ == "__main__":
    unittest.main()
