"""Unit tests for `tools/game_field_owners.py` bidirectional rewrites, scoping, macros, and ambiguity."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import game_field_owners as gfo  # noqa: E402

OWNERS: dict[str, str] = {
    "screen": "client",
    "hoppers": "server",
    "flight": "game",
    "fns": "game",
    "world_tables": "game",
    "lent_mobs": "game",
    "pending_entry": "game",
}


def run_scan(src: str, index: gfo.WorkspaceIndex | None = None, rel: str = "test.rs"):
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
        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            table = td_path / "owners.tsv"
            self._write_table(table)
            target = td_path / "sample.rs"
            target.write_text(
                "fn f(game: &Game<'_>) {\n"
                "    let _ = mystery_macro!(game.screen);\n"
                "    let _ = game.client.flight;\n"
                "    let _ = game.hoppers;\n"
                "}\n",
                encoding="utf-8",
            )
            # `--check` fails on stale accesses and reports the ambiguous site.
            rc_check = subprocess.run(
                [sys.executable, str(ROOT / "tools/game_field_owners.py"), "--check", "--table", str(table), str(target)],
                capture_output=True,
                text=True,
            )
            self.assertEqual(rc_check.returncode, 1)
            self.assertIn("stale Game field `client.flight` -> `flight`", rc_check.stdout)
            self.assertIn("stale Game field `hoppers` -> `server.hoppers`", rc_check.stdout)
            self.assertIn("ambiguous receiver for Game field `screen`", rc_check.stderr)

            # `--fix` rewrites unambiguous sites, leaves ambiguous untouched, and exits 1.
            rc_fix = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "tools/game_field_owners.py"),
                    "--fix",
                    "--no-fmt",
                    "--table",
                    str(table),
                    str(target),
                ],
                capture_output=True,
                text=True,
            )
            self.assertEqual(rc_fix.returncode, 1)
            self.assertIn("ambiguous receiver for Game field `screen`", rc_fix.stderr)
            rewritten = target.read_text(encoding="utf-8")
            self.assertIn("mystery_macro!(game.screen);", rewritten)
            self.assertIn("let _ = game.flight;", rewritten)
            self.assertIn("let _ = game.server.hoppers;", rewritten)

            # `--fix --allow-ambiguous` exits 0 once unambiguous sites are clean.
            rc_allow = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "tools/game_field_owners.py"),
                    "--fix",
                    "--no-fmt",
                    "--allow-ambiguous",
                    "--table",
                    str(table),
                    str(target),
                ],
                capture_output=True,
                text=True,
            )
            self.assertEqual(rc_allow.returncode, 0)

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


if __name__ == "__main__":
    unittest.main()
