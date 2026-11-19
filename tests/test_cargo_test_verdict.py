"""tools/ci/cargo_test_verdict.py: one cargo test command's verdicts, added to the fixture job's daily report."""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

from tools.ci import cargo_test_verdict as verdict

CAPTURED = """running 3 tests
test passes ... ok
test fails_captured ... FAILED
test fails_in_a_thread ... FAILED

failures:

---- fails_captured stdout ----

thread 'fails_captured' panicked at crates/sunrise-mod-wasm/tests/a.rs:12:5:
assertion failed
---- fails_in_a_thread stdout ----
thread '<unnamed>' (17) panicked at crates/sunrise-mod-wasm/src/b.rs:3:9:
boom
"""
# --nocapture: the panic comes before the result line, from the test's own thread.
UNCAPTURED = """running 1 test
thread 'strict' panicked at crates/sunrise-mod-wasm/tests/strict_math.rs:49:13:
assertion `left == right` failed: guest did not complete all reference rows
test strict ... FAILED
"""


class CargoTestVerdictTests(unittest.TestCase):
    def test_each_failure_is_named_with_the_panic_that_failed_it(self):
        problems, passed = verdict.verdicts(io.StringIO(CAPTURED), "sunrise-mod-wasm")
        self.assertEqual([(item["test"], item["panic"]) for item in problems], [
            ("fails_captured", "crates/sunrise-mod-wasm/tests/a.rs:12:5"),
            ("fails_in_a_thread", "crates/sunrise-mod-wasm/src/b.rs:3:9")])
        self.assertEqual(passed, [{"package": "sunrise-mod-wasm", "test": "passes"}])
        self.assertEqual({(item["kind"], item["policy"]) for item in problems}, {("failed", None)})
        problems, _ = verdict.verdicts(io.StringIO(UNCAPTURED), "sunrise-mod-wasm")
        self.assertEqual([(item["test"], item["panic"]) for item in problems],
                         [("strict", "crates/sunrise-mod-wasm/tests/strict_math.rs:49:13")])

    def test_commands_add_to_one_report_and_keep_their_exit_status(self):
        with tempfile.TemporaryDirectory() as folder:
            report = Path(folder, "report.json")
            script = "import sys; print(sys.argv[1]); sys.exit(int(sys.argv[2]))"
            with contextlib.redirect_stdout(io.StringIO()) as printed:
                first = verdict.main([str(report), "--", sys.executable, "-c", script, "test a ... ok", "0", "-p", "one"])
                second = verdict.main([str(report), "--", sys.executable, "-c", script, "test b ... FAILED", "101",
                                       "-p", "two"])
            self.assertEqual((first, second), (0, 101))
            self.assertIn("test b ... FAILED", printed.getvalue())
            written = json.loads(report.read_text())
            self.assertEqual(written["needs"], "SUNRISE_WASM_FIXTURE")
            self.assertEqual(written["passed"], [{"package": "one", "test": "a"}])
            self.assertEqual([(item["package"], item["test"]) for item in written["problems"]], [("two", "b")])

    def test_a_failure_without_a_whole_verdict_is_recorded_as_incomplete(self):
        named = ["     Running tests/a.rs (target/a)\n", "test x ... FAILED\n", "test result: FAILED. 0 passed; 1 failed\n"]
        problems, _ = verdict.verdicts(named, "pkg")
        self.assertIsNone(verdict.unfinished(named, "pkg", 101, problems), "a named failure with every summary")
        self.assertEqual(verdict.unfinished(["error[E0425]: cannot find value\n"], "pkg", 101, []),
                         {"shard": "pkg", "reason": "failed without a failed test (a build failure)", "cargo_exit": 101})
        # The first binary named its failure, the second stopped before its summary (a crash, a stop partway).
        partly = named + ["\x1b[1m\x1b[92m     Running\x1b[0m tests/b.rs (target/b)\n", "test y ... ok\n"]
        problems, _ = verdict.verdicts(partly, "pkg")
        self.assertEqual(verdict.unfinished(partly, "pkg", 101, problems)["reason"],
                         "a test binary stopped before its summary")
        self.assertIsNone(verdict.unfinished([], "pkg", 0, []))

    def test_a_named_failure_and_a_build_failure_both_reach_the_report(self):
        with tempfile.TemporaryDirectory() as folder:
            report = Path(folder, "report.json")
            script = "import sys; print(sys.argv[1]); sys.exit(int(sys.argv[2]))"
            with contextlib.redirect_stdout(io.StringIO()):
                verdict.main([str(report), "--", sys.executable, "-c", script,
                              "test a ... FAILED\ntest result: FAILED. 0 passed; 1 failed", "101", "-p", "one"])
                verdict.main([str(report), "--", sys.executable, "-c", script, "error: could not compile", "101",
                              "-p", "two"])
            written = json.loads(report.read_text())
            self.assertEqual([item["test"] for item in written["problems"]], ["a"])
            self.assertEqual(written["incomplete"], [
                {"shard": "two", "reason": "failed without a failed test (a build failure)", "cargo_exit": 101}])


if __name__ == "__main__":
    unittest.main()
