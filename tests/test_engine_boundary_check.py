"""The engine-boundary ratchet refuses new gameplay -> engine references and only shrinks."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from tests import git_history
from tools.ci import check_engine_boundary as boundary

ROOT = Path(__file__).resolve().parents[1]


CONFIG = {
    "modules": ["gfx", "audio", "window"],
    "scan": ["crates/sunrise-client/src/game/world"],
    "allowed": {},
}


class EngineBoundaryTests(unittest.TestCase):
    def test_qualified_and_grouped_imports_count(self):
        source = """
            use sunrise_client::{audio::Audio, chunk::Chunk};
            fn f() -> sunrise_client::gfx::Frame { todo!() }
            use crate::window::PointerSample;
        """
        self.assertEqual(boundary.referenced_modules(source, CONFIG["modules"]), {"audio", "gfx", "window"})

    def test_the_split_client_crates_count_as_engine_heads(self):
        source = """
            use sunrise_client_lib::audio::Audio;
            use sunrise_client_render::gfx::Frame;
            use sunrise_client_input::window::PointerSample;
            use sunrise_client_other::gfx::Frame;
        """
        self.assertEqual(
            boundary.referenced_modules(source, CONFIG["modules"]), {"audio", "gfx", "window"}
        )

    def test_a_submodule_named_like_an_engine_module_is_not_a_hit(self):
        source = "use crate::game::{gfx, audio}; use other::gfx::Frame;"
        self.assertEqual(boundary.referenced_modules(source, CONFIG["modules"]), set())

    def test_a_new_offender_fails(self):
        problems, notes = boundary.check(CONFIG, {"a.rs": ["gfx"]})
        self.assertEqual(len(problems), 1)
        self.assertIn("a.rs: references gfx", problems[0])
        self.assertEqual(notes, [])

    def test_a_recorded_offender_passes(self):
        config = dict(CONFIG, allowed={"a.rs": ["gfx"]})
        self.assertEqual(boundary.check(config, {"a.rs": ["gfx"]}), ([], []))

    def test_a_new_module_on_a_recorded_file_fails(self):
        config = dict(CONFIG, allowed={"a.rs": ["gfx"]})
        problems, _ = boundary.check(config, {"a.rs": ["gfx", "audio"]})
        self.assertIn("now also references audio", problems[0])

    def test_an_entry_that_shrank_is_a_note(self):
        config = dict(CONFIG, allowed={"a.rs": ["gfx", "audio"]})
        problems, notes = boundary.check(config, {"a.rs": ["gfx"]})
        self.assertEqual(problems, [])
        self.assertIn("no longer references audio", notes[0])

    def test_an_entry_that_went_away_is_a_note(self):
        config = dict(CONFIG, allowed={"a.rs": ["gfx"]})
        problems, notes = boundary.check(config, {})
        self.assertEqual(problems, [])
        self.assertIn("remove it", notes[0])

    def test_a_branch_may_not_add_or_widen_an_entry(self):
        base = dict(CONFIG, allowed={"a.rs": ["gfx"]})
        config = dict(CONFIG, allowed={"a.rs": ["gfx", "audio"], "b.rs": ["gfx"]})
        problems, _ = boundary.check(config, {"a.rs": ["gfx", "audio"], "b.rs": ["gfx"]}, base)
        self.assertTrue(any("b.rs: added" in problem for problem in problems))
        self.assertTrue(any("a.rs: widened" in problem for problem in problems))

    def test_the_scan_roots_and_modules_are_fixed_between_refs(self):
        base = dict(CONFIG, allowed={})
        config = dict(CONFIG, scan=["elsewhere"], allowed={})
        problems, _ = boundary.check(config, {}, base)
        self.assertIn("scan roots and modules are fixed", problems[0])


class EngineBoundaryBaseTests(unittest.TestCase):
    """`--base` and `--pull-request-merge` in a throwaway checkout, run as the script CI runs."""
    SCAN = "crates/sunrise-client/src/game/world"

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        for script in ("tools/ci/check_engine_boundary.py", "tools/rust_tokens.py", "tools/merge_base.py",
                       "tools/check_layers.py", "tools/layers.json"):
            (self.root / script).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(ROOT / script, self.root / script)
        self.write("crates/sunrise-client/src/game/world/a.rs", "use crate::gfx::Frame;\n")
        self.write("crates/sunrise-client/src/game/world/b.rs", "fn quiet() {}\n")
        self.allow({"crates/sunrise-client/src/game/world/a.rs": ["gfx"]})
        git_history.git(self.root, "init", "-q")
        git_history.git(self.root, "add", ".")
        git_history.git(self.root, "commit", "-qm", "base")
        self.base = git_history.git(self.root, "rev-parse", "HEAD").stdout.strip()

    def write(self, path, text):
        git_history.write(self.root, path, text)

    def allow(self, allowed):
        self.write("tools/ci/engine_boundary.json", json.dumps(
            {"scan": [self.SCAN], "modules": ["gfx", "audio", "window"], "allowed": allowed}, indent=2) + "\n")

    def check(self, *arguments):
        return subprocess.run([sys.executable, "tools/ci/check_engine_boundary.py", *arguments], cwd=self.root,
                              env=git_history.clean_env(), capture_output=True, text=True, encoding="utf-8")

    def merge(self, main_change, pr_change):
        return git_history.merge_pull_request(self.root, self.base, ("crates", "tools"), main_change, pr_change)

    def widen_with_b(self):
        """A new offender b.rs, recorded in the baseline, as a change to the ratchet."""
        self.write("crates/sunrise-client/src/game/world/b.rs", "use crate::audio::Audio;\n")
        self.allow({"crates/sunrise-client/src/game/world/a.rs": ["gfx"],
                    "crates/sunrise-client/src/game/world/b.rs": ["audio"]})

    def test_a_stale_base_does_not_blame_the_pull_request_for_mains_new_entry(self):
        # Main (after the event's base) recorded another offender; the pull request changes nothing of the sort.
        pr, main = self.merge(self.widen_with_b, lambda: self.write("crates/unrelated.txt", "x\n"))
        merged = self.check("--base", self.base, "--pull-request-merge")
        self.assertEqual(merged.returncode, 0, merged.stdout + merged.stderr)
        stale = self.check("--base", self.base)
        self.assertEqual(stale.returncode, 1, stale.stdout)
        self.assertIn("b.rs: added", stale.stdout)

    def test_a_pull_request_that_adds_or_widens_an_entry_still_fails(self):
        self.merge(lambda: self.write("crates/unrelated.txt", "x\n"), self.widen_with_b)
        result = self.check("--base", self.base, "--pull-request-merge")
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("b.rs: added", result.stdout)

    def test_the_pull_request_merge_needs_a_merge_of_a_descendant_of_the_base(self):
        single = self.check("--base", self.base, "--pull-request-merge")
        self.assertEqual(single.returncode, 1, single.stdout)
        self.assertIn("descendant of --base", single.stdout)
        pr, _ = self.merge(lambda: None, lambda: self.write("crates/unrelated.txt", "x\n"))
        wrong = self.check("--base", pr, "--pull-request-merge")
        self.assertEqual(wrong.returncode, 1, wrong.stdout)
        self.assertIn("descendant of --base", wrong.stdout)

    def test_the_pull_request_merge_takes_a_base(self):
        result = self.check("--pull-request-merge")
        self.assertEqual(result.returncode, 2)
        self.assertIn("--pull-request-merge takes --base", result.stderr)

    def test_a_baseline_that_cannot_be_read_fails_closed(self):
        missing = self.check("--base", "0" * 40)
        self.assertEqual(missing.returncode, 1, missing.stdout)
        self.assertNotIn("Engine boundary check passed", missing.stdout)
        git_history.git(self.root, "update-index", "--add", "--cacheinfo",
                        "160000,1111111111111111111111111111111111111111,tools/ci/engine_boundary.json")
        git_history.git(self.root, "commit", "-qm", "gitlink")
        corrupt = git_history.git(self.root, "rev-parse", "HEAD").stdout.strip()
        unreadable = self.check("--base", corrupt)
        self.assertEqual(unreadable.returncode, 1, unreadable.stdout)
        self.assertIn("tools/ci/engine_boundary.json", unreadable.stderr)
        self.assertNotIn("Engine boundary check passed", unreadable.stdout)

    def test_importing_the_check_leaves_sys_path_alone(self):
        script = ("import sys; before = list(sys.path); import tools.ci.check_engine_boundary; "
                  "assert sys.path == before, sys.path")
        result = subprocess.run([sys.executable, "-c", script], cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
