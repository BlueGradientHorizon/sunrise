"""The port-map guard resolves a row's function to a declaration or a re-export, never to a call."""
import contextlib
import io
import subprocess
import tempfile
import unittest
from pathlib import Path

from tools.ci import check_port_map_paths as guard


class PortMapPathTests(unittest.TestCase):
    def setUp(self):
        self.tree = tempfile.TemporaryDirectory()
        self.root = Path(self.tree.name)
        (self.root / "tools/port_map").mkdir(parents=True)
        (self.root / "crates").mkdir()

    def tearDown(self):
        self.tree.cleanup()

    def row(self, line, name="game-1.tsv"):
        (self.root / "tools/port_map" / name).write_text(line + "\n", encoding="utf-8")

    def source(self, text, name="a.rs"):
        (self.root / "crates" / name).write_text(text, encoding="utf-8")

    def stale(self):
        return guard.unresolved(guard.rows(self.root), self.root)

    def run_main(self, argv):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = guard.main(argv, self.root)
        return code, out.getvalue() + err.getvalue()

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.root), "-c", "user.name=t", "-c", "user.email=t@t",
                               *args], check=True, capture_output=True, text=True).stdout

    def commit(self, message):
        self.git("add", "-A")
        self.git("commit", "-q", "-m", message)
        return self.git("rev-parse", "HEAD").strip()

    def test_only_port_and_inline_rows_name_a_function(self):
        self.row("A.b\tmissing\nC.d\tskip:java\nE.f\tport\tcrates/a.rs::Game::f\nG.h\tinline\tcrates/a.rs::Game::h")
        self.assertEqual([where for _, _, _, where in guard.rows(self.root)],
                         ["crates/a.rs::Game::f", "crates/a.rs::Game::h"])

    def test_a_declaration_names_the_function(self):
        self.source("impl Game {\n    pub(super) fn keep(&self) {}\n}\n")
        self.row("A.b\tport\tcrates/a.rs::Game::keep")
        self.assertEqual(self.stale(), {})

    def test_a_call_or_a_comment_is_not_a_declaration(self):
        self.source("// fn gone() {}\nconst NOTE: &str = \"fn gone\";\nfn caller(&self) { self.gone(); }\n")
        self.row("A.b\tport\tcrates/a.rs::Game::gone")
        stale = self.stale()
        self.assertEqual(set(stale), {("tools/port_map/game-1.tsv", "A.b", "crates/a.rs::Game::gone")})
        self.assertIn("gone", next(iter(stale.values())))

    def test_a_reexport_names_the_function(self):
        self.source("pub use moved::keep;\n")
        self.row("A.b\tport\tcrates/a.rs::Game::keep")
        self.assertEqual(self.stale(), {})

    def test_an_included_file_declares_into_its_includer(self):
        self.source('include!("a/moved.rs");\n', name="a.rs")
        (self.root / "crates/a").mkdir()
        (self.root / "crates/a/moved.rs").write_text("fn keep() {}\n", encoding="utf-8")
        self.row("A.b\tport\tcrates/a.rs::keep")
        self.assertEqual(self.stale(), {})

    def test_a_dead_file_and_a_dead_name_are_told_apart(self):
        self.source("fn here() {}\n")
        self.row("A.b\tport\tcrates/gone.rs::Game::f\nC.d\tport\tcrates/a.rs::Game::gone")
        reasons = {name: why for (_, name, _), why in self.stale().items()}
        self.assertEqual(set(reasons), {"A.b", "C.d"})
        self.assertIn("gone.rs", reasons["A.b"])
        self.assertNotIn("a.rs", reasons["A.b"])
        self.assertNotIn("gone.rs", reasons["C.d"])
        self.assertIn("gone", reasons["C.d"])

    def test_a_repointed_row_is_a_new_row(self):
        self.source("fn keep() {}\n")
        self.row("A.b\tport\tcrates/gone-1.rs::Game::keep\nA.b\tport\tcrates/gone-2.rs::Game::keep")
        before = self.stale()
        self.assertEqual(len(before), 2)
        (self.root / "tools/port_map/game-1.tsv").write_text(
            "A.b\tport\tcrates/a.rs::Game::keep\nA.b\tport\tcrates/gone-2.rs::Game::keep\n", encoding="utf-8")
        after = self.stale()
        self.assertNotIn(("tools/port_map/game-1.tsv", "A.b", "crates/gone-1.rs::Game::keep"), after)
        self.assertIn(("tools/port_map/game-1.tsv", "A.b", "crates/gone-2.rs::Game::keep"), after)

    def test_a_malformed_target_is_reported_at_its_line(self):
        self.row("A.b\tport\nC.d\tport\taway\nE.f\tinline\tcrates/a.rs::Game::\nG.h\tport\t")
        rows, malformed = guard.parse(self.root)
        self.assertEqual(rows, [])
        self.assertEqual([(line, name) for _, line, name, _ in malformed],
                         [(1, "A.b"), (2, "C.d"), (3, "E.f"), (4, "G.h")])
        code, output = self.run_main([])
        self.assertEqual(code, 1)
        for where in ("game-1.tsv:1", "game-1.tsv:2", "game-1.tsv:3", "game-1.tsv:4"):
            self.assertIn(where, output)

    def test_refresh_deletes_only_the_lines_that_resolve(self):
        self.source("fn keep() {}\n")
        self.row("A.b\tport\tcrates/a.rs::Game::keep\nC.d\tport\tcrates/a.rs::Game::gone")
        (self.root / guard.LIST).parent.mkdir(parents=True)
        (self.root / guard.LIST).write_text(
            "# stale rows\n"
            "tools/port_map/game-1.tsv\tA.b\tcrates/a.rs::Game::keep\n"
            "tools/port_map/game-1.tsv\tC.d\tcrates/a.rs::Game::gone\n", encoding="utf-8")
        code, output = self.run_main(["--refresh"])
        self.assertEqual(code, 0, output)
        self.assertEqual((self.root / guard.LIST).read_text(encoding="utf-8"),
                         "# stale rows\n"
                         "tools/port_map/game-1.tsv\tC.d\tcrates/a.rs::Game::gone\n")

    def test_the_list_may_not_grow_past_its_base(self):
        self.source("fn keep() {}\n")
        self.row("A.b\tport\tcrates/a.rs::Game::keep\nC.d\tport\tcrates/a.rs::Game::gone")
        (self.root / guard.LIST).parent.mkdir(parents=True)
        (self.root / guard.LIST).write_text("tools/port_map/game-1.tsv\tC.d\tcrates/a.rs::Game::gone\n",
                                            encoding="utf-8")
        self.git("init", "-q")
        base = self.commit("base")
        self.assertEqual(self.run_main(["--base", base])[0], 0)
        with (self.root / guard.LIST).open("a", encoding="utf-8") as list_file:
            list_file.write("tools/port_map/game-1.tsv\tA.b\tcrates/a.rs::Game::keep\n")
        code, output = self.run_main(["--base", base])
        self.assertEqual(code, 1)
        self.assertIn("A.b", output)
        self.assertNotIn("C.d", output)

    def test_a_base_without_the_list_is_a_new_list(self):
        self.source("fn here() {}\n")
        self.row("C.d\tport\tcrates/a.rs::Game::gone")
        self.git("init", "-q")
        base = self.commit("base")
        (self.root / guard.LIST).parent.mkdir(parents=True)
        (self.root / guard.LIST).write_text("tools/port_map/game-1.tsv\tC.d\tcrates/a.rs::Game::gone\n",
                                            encoding="utf-8")
        self.assertEqual(self.run_main(["--base", base])[0], 0)

    def test_a_base_whose_list_cannot_be_read_fails_closed(self):
        self.source("fn here() {}\n")
        self.row("C.d\tport\tcrates/a.rs::Game::gone")
        (self.root / guard.LIST).parent.mkdir(parents=True)
        (self.root / guard.LIST).write_text("", encoding="utf-8")
        self.git("init", "-q")
        self.commit("base")
        # A gitlink at the list's path pointing at no object: ls-tree names it, git show cannot read it.
        self.git("update-index", "--add", "--cacheinfo",
                 "160000,1111111111111111111111111111111111111111,tools/ci/port_map_known_stale.txt")
        self.git("commit", "-q", "-m", "gitlink")
        corrupt = self.git("rev-parse", "HEAD").strip()
        with self.assertRaises(SystemExit) as exit:
            guard.base_listed(corrupt, self.root)
        self.assertIn(guard.LIST, str(exit.exception))

    def test_use_names_follows_nested_braces_and_renames(self):
        self.assertEqual(guard.imported("use self::layout::OptionsLayout;\n"
                                        "use super::{combat::do_hurt, other};\n"
                                        "use crate::x::{y as z, *};\n"
                                        "use elsewhere::*;\n"),
                         {"OptionsLayout", "do_hurt", "other", "z"})

    def test_list_lines_ignore_comments_and_blanks(self):
        self.assertEqual(guard.listed("# a note\n\nm\tA.b\tp::f\n"), {("m", "A.b", "p::f")})


if __name__ == "__main__":
    unittest.main()
