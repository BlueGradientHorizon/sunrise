"""The port-map guard reads a row's function out of its file, not out of a neighbouring one."""
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

    def row(self, line):
        (self.root / "tools/port_map/game-1.tsv").write_text(line + "\n", encoding="utf-8")

    def test_only_port_and_inline_rows_name_a_function(self):
        self.row("A.b\tmissing\nC.d\tskip:java\nE.f\tport\tcrates/a.rs::Game::f\nG.h\tinline\tcrates/a.rs::Game::h")
        self.assertEqual([where for _, _, _, where in guard.rows(self.root)],
                         ["crates/a.rs::Game::f", "crates/a.rs::Game::h"])

    def test_a_reexport_names_the_function(self):
        (self.root / "crates/a.rs").write_text("pub use moved::keep;\n", encoding="utf-8")
        self.row("A.b\tport\tcrates/a.rs::Game::keep")
        self.assertEqual(guard.unresolved(guard.rows(self.root), self.root), {})

    def test_a_dead_file_and_a_dead_name_are_told_apart(self):
        (self.root / "crates/a.rs").write_text("fn here() {}\n", encoding="utf-8")
        self.row("A.b\tport\tcrates/gone.rs::Game::f\nC.d\tport\tcrates/a.rs::Game::gone")
        reasons = {name: why for (_, name, _), why in guard.unresolved(guard.rows(self.root), self.root).items()}
        self.assertEqual(reasons, {"A.b": "no file crates/gone.rs",
                                   "C.d": "crates/a.rs does not mention gone"})

    def test_a_repointed_row_is_a_new_row(self):
        (self.root / "crates/a.rs").write_text("fn keep() {}\n", encoding="utf-8")
        before = guard.unresolved([("tools/port_map/game-1.tsv", 1, "A.b", "crates/gone.rs::Game::keep")],
                                  self.root)
        after = guard.unresolved([("tools/port_map/game-1.tsv", 1, "A.b", "crates/a.rs::Game::keep")],
                                 self.root)
        self.assertEqual(len(before), 1)
        self.assertEqual(after, {})
        self.assertNotIn(next(iter(before)), after)

    def test_list_lines_ignore_comments_and_blanks(self):
        self.assertEqual(guard.listed("# a note\n\nm\tA.b\tp::f\n"), {("m", "A.b", "p::f")})


if __name__ == "__main__":
    unittest.main()
