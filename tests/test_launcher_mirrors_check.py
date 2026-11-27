"""The launcher mirror hash follows the public surface, and a change needs its announcement."""
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from tools.ci import check_launcher_mirrors as mirrors

ROOT = Path(__file__).resolve().parents[1]
WRITE = "python3 tools/ci/check_launcher_mirrors.py --write"


class MirrorCheckout:
    """A throwaway git checkout holding the check, one mirrored crate and its recorded surface, committed as the base."""

    def __init__(self, test):
        directory = tempfile.TemporaryDirectory()
        test.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        for script in ("tools/ci/check_launcher_mirrors.py", "tools/rust_tokens.py"):
            (self.root / script).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(ROOT / script, self.root / script)
        self.write("crates/mirror/src/lib.rs", "pub fn a() {}\n")
        self.write("tools/launcher_mirrors.toml",
                   'version = 1\n\n[[mirror]]\nlauncher = "mirror"\nsources = ["crates/mirror/src"]\n')
        self.check("--write")
        self.git("init", "-q")
        self.git("add", ".")
        self.git("commit", "-qm", "base")
        self.base = self.git("rev-parse", "HEAD").strip()

    def write(self, path, text):
        (self.root / path).parent.mkdir(parents=True, exist_ok=True)
        (self.root / path).write_text(text, encoding="utf-8", newline="\n")

    def git(self, *arguments):
        return subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                               "-c", "commit.gpgsign=false", "-c", f"core.hooksPath={self.root / 'no-hooks'}", *arguments],
                              cwd=self.root, capture_output=True, text=True, check=True).stdout

    def run(self, *command):
        """A `python3 ...` command line, run in this checkout."""
        return subprocess.run([sys.executable, *command[1:]], cwd=self.root, capture_output=True, text=True,
                              encoding="utf-8")

    def check(self, *arguments):
        return self.run("python3", "tools/ci/check_launcher_mirrors.py", *arguments)

    def change_surface(self, record):
        """A new public function in the mirrored crate; `record` also writes the manifest, as `--write` does."""
        self.write("crates/mirror/src/lib.rs", "pub fn a() {}\npub fn b() {}\n")
        if record:
            self.check("--write")

    def change_privately(self):
        self.write("crates/mirror/src/lib.rs", "pub fn a() { let _ = 1; }\nfn hidden() {}\n")


MANIFEST = {
    "version": 1,
    "mirror": [
        {"launcher": "smod", "sources": ["crates/sunrise-mod-package/src"], "surface": "sha256:a"},
        {"launcher": "proxy", "sources": [], "note": "only in the launcher"},
    ],
}


class LauncherMirrorTests(unittest.TestCase):
    def test_a_private_change_keeps_the_surface(self):
        before = "\n".join(mirrors.public_surface("pub struct S;\npub fn f() -> u8 { 1 }\n"))
        after = "\n".join(mirrors.public_surface("pub struct S;\npub fn f() -> u8 { 2 }\nfn hidden() {}\n"))
        self.assertEqual(before, after)

    def test_a_public_change_moves_the_surface(self):
        before = "\n".join(mirrors.public_surface("pub fn f() -> u8 { 1 }\n"))
        after = "\n".join(mirrors.public_surface("pub fn f() -> u16 { 1 }\n"))
        self.assertNotEqual(before, after)

    def test_a_new_public_item_moves_the_surface(self):
        before = "\n".join(mirrors.public_surface("pub fn f() {}\n"))
        after = "\n".join(mirrors.public_surface("pub fn f() {}\npub fn g() {}\n"))
        self.assertNotEqual(before, after)

    def test_a_reexported_name_moves_the_surface(self):
        before = "\n".join(mirrors.public_surface("pub use crate::{a, b};\n"))
        after = "\n".join(mirrors.public_surface("pub use crate::{a, b, c};\n"))
        self.assertNotEqual(before, after)

    def test_a_mismatched_hash_fails(self):
        problems = mirrors.check(MANIFEST, {"smod": "sha256:b"})
        self.assertIn(f"Run `{WRITE}`", problems[0])

    def test_a_matching_hash_passes(self):
        self.assertEqual(mirrors.check(MANIFEST, {"smod": "sha256:a"}), [])

    def test_a_changed_surface_needs_the_marker_in_the_pull_request_body(self):
        base = dict(MANIFEST, mirror=[dict(MANIFEST["mirror"][0], surface="sha256:old")])
        config = dict(MANIFEST, mirror=[dict(MANIFEST["mirror"][0], surface="sha256:a")])
        self.assertTrue(any("launcher-mirror:" in problem
                            for problem in mirrors.check(config, {"smod": "sha256:a"}, base, "")))
        announced = "launcher-mirror: the launcher must take the new field\n"
        self.assertEqual(mirrors.check(config, {"smod": "sha256:a"}, base, announced), [])

    def test_a_stale_manifest_fails_with_the_write_command_even_when_reminding(self):
        checkout = MirrorCheckout(self)
        checkout.change_surface(record=False)
        for arguments in (("--base", checkout.base, "--remind-body"), ("--base", checkout.base, "--pr-body-file", "x")):
            checkout.write("x", "launcher-mirror: the copy follows\n")
            result = checkout.check(*arguments)
            self.assertEqual(result.returncode, 1, result.stdout)
            self.assertIn(f"Run `{WRITE}`", result.stdout)
            self.assertIn("mirror: public surface is", result.stdout)

    def test_a_changed_surface_passes_with_a_reminder_when_the_run_has_no_body(self):
        checkout = MirrorCheckout(self)
        checkout.change_surface(record=True)
        result = checkout.check("--base", checkout.base, "--remind-body")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("Reminder: launcher mirrors changed (mirror)", result.stdout)
        self.assertIn("launcher-mirror:", result.stdout)

    def test_an_unchanged_surface_passes_without_a_reminder(self):
        checkout = MirrorCheckout(self)
        checkout.change_privately()
        result = checkout.check("--base", checkout.base, "--remind-body")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertNotIn("Reminder", result.stdout)

    def test_a_body_is_still_required_where_one_is_given(self):
        checkout = MirrorCheckout(self)
        checkout.change_surface(record=True)
        checkout.write("empty.md", "")
        checkout.write("announced.md", "Text\nlauncher-mirror: the copy follows\n")
        failed = checkout.check("--base", checkout.base, "--pr-body-file", "empty.md")
        self.assertEqual(failed.returncode, 1, failed.stdout)
        self.assertIn("without a line starting launcher-mirror:", failed.stdout)
        passed = checkout.check("--base", checkout.base, "--pr-body-file", "announced.md")
        self.assertEqual(passed.returncode, 0, passed.stdout)
        self.assertNotIn("Reminder", passed.stdout)

    def test_reminding_needs_a_base_and_takes_no_body(self):
        checkout = MirrorCheckout(self)
        checkout.write("empty.md", "")
        for arguments in (("--remind-body",), ("--base", checkout.base, "--remind-body", "--pr-body-file", "empty.md"),
                          ("--base", checkout.base, "--remind-body", "--event", "empty.md")):
            with self.subTest(arguments=arguments):
                self.assertEqual(checkout.check(*arguments).returncode, 2)

    def test_every_shipped_manifest_source_exists(self):
        manifest = mirrors.load_manifest()
        for mirror in manifest["mirror"]:
            for source in mirror.get("sources", []):
                with self.subTest(launcher=mirror["launcher"], source=source):
                    self.assertTrue(
                        (mirrors.ROOT / source).exists(),
                        f"{mirror['launcher']}: {source} is not in the repository",
                    )

    def test_a_mirror_with_no_source_is_not_hashed(self):
        hashes, notes = mirrors.measured(MANIFEST)
        self.assertNotIn("proxy", hashes)
        self.assertIn("proxy: no source", notes[0])


if __name__ == "__main__":
    unittest.main()
