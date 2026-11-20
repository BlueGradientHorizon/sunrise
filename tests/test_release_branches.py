"""The release branches (docs/release-flow.md "Release branches"): which refs publish, the stable tags and their
order, and the commits the nightly and stable builds take."""
import io
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools import release_file
from tools import release_train as train
from tools.ci import nightly

ROOT = Path(__file__).resolve().parents[1]
MAIN, NIGHTLY, RELEASE = "refs/heads/main", "refs/heads/nightly/26.3", "refs/heads/release/26.3"


def git_in(folder):
    def git(*args):
        return subprocess.check_output(["git", *args], cwd=folder, text=True, stderr=subprocess.DEVNULL).strip()
    return git


def repository(folder):
    """A bare origin and a clone of it with three commits on main, the clone's origin/main at the newest."""
    origin, clone = Path(folder, "origin"), Path(folder, "clone")
    subprocess.check_call(["git", "init", "-q", "--bare", "-b", "main", str(origin)])
    subprocess.check_call(["git", "clone", "-q", str(origin), str(clone)], stderr=subprocess.DEVNULL)
    git = git_in(clone)
    git("config", "user.name", "Test")
    git("config", "user.email", "test@example.invalid")
    commits = []
    for n in range(3):
        git("commit", "-q", "--allow-empty", "-m", str(n))
        commits.append(git("rev-parse", "HEAD"))
    git("push", "-q", "origin", "main")
    git("fetch", "-q", "origin")
    return git, commits


class StableTagTests(unittest.TestCase):
    def test_a_stable_tag_is_its_target_and_a_semver_within_it(self):
        self.assertEqual(train.stable_version("26.3-0.1.0"), ("26.3", (0, 1, 0)))
        self.assertEqual(train.stable_version("26.3.1-2.10.3"), ("26.3.1", (2, 10, 3)))
        for other in ("v26.3-0.1.0", "26.3-0.1", "26.3-01.0.0", "26.3-0.1.0-rc.1", "0.1.0+26.3", "26-0.1.0", "26.3",
                      "nightly", "nightly-2026-11-20", "v2026.10.23", "v0.2.81", "sdk-" + "a" * 40, "launcher@1.0.0"):
            self.assertIsNone(train.stable_version(other), other)

    def test_stable_tags_order_by_the_semver_within_a_target(self):
        tags = ["26.3-1.0.0", "26.3-0.10.0", "26.4-0.1.0", "26.3-0.2.0", "26.3-0.1.1", "26.3-0.9.0", "26.3-0.1.0"]
        self.assertEqual(sorted(tags, key=train.stable_key),
                         ["26.3-0.1.0", "26.3-0.1.1", "26.3-0.2.0", "26.3-0.9.0", "26.3-0.10.0", "26.3-1.0.0",
                          "26.4-0.1.0"])

    def test_the_next_stable_tag(self):
        self.assertEqual(train.next_stable_tag(["nightly", "v2026.10.23", "launcher@1.0.0"]), "26.3-0.1.0")
        tags = ["26.3-0.1.0", "26.3-0.1.1", "26.4-5.0.0", "nightly"]
        self.assertEqual(train.next_stable_tag(tags), "26.3-0.1.2", "the patch after the newest on the target")
        self.assertEqual(train.next_stable_tag(tags, version="0.2.0"), "26.3-0.2.0")
        self.assertEqual(train.next_stable_tag(tags, target="26.4"), "26.4-5.0.1")
        for older in ("0.1.1", "0.1.0", "0.0.9"):
            with self.assertRaisesRegex(SystemExit, "is not newer than 26.3-0.1.1"):
                train.next_stable_tag(tags, version=older)
        for bad in ("1.0", "v0.2.0", "0.2.0-rc.1"):
            with self.assertRaisesRegex(SystemExit, "MAJOR.MINOR.PATCH"):
                train.next_stable_tag(tags, version=bad)

    def test_the_release_file_names_a_stable_build_by_its_tag(self):
        read = {release_file.VERSION_RS: 'pub const VERSION_ID: &str = "26.3";\npub const VANILLA_VERSION: &str = "26.3";\n'
                'pub const PROTOCOL_VERSION: i32 = 777;\npub const JAVA_VERSION: u32 = 25;\n',
                release_file.RUST_VERSION: "1.99.1\n"}.__getitem__
        facts = release_file.facts(read, "stable", "2026-11-20", "a" * 40, "26.3-0.1.0")
        self.assertEqual((facts["channel"], facts["build"]), ("stable", "26.3-0.1.0"))
        for channel, build in (("stable", ""), ("stable", "v26.3.0"), ("nightly", "26.3-0.1.0")):
            with self.assertRaises(SystemExit, msg=(channel, build)):
                release_file.facts(read, channel, "2026-11-20", "a" * 40, build)


class PublishingRefTests(unittest.TestCase):
    def test_only_main_and_the_two_branches_publish_and_each_only_its_kinds(self):
        for ref, kinds in ((MAIN, {"daily", "nightly", "snapshot", "sdk"}), (NIGHTLY, {"nightly"}),
                           (RELEASE, {"daily", "stable", "sdk"})):
            for kind in ("daily", "nightly", "snapshot", "stable", "sdk"):
                self.assertEqual(train.publishes(ref, kind), kind in kinds, (ref, kind))
        for ref in ("refs/heads/topic", "refs/heads/release/26.4", "refs/heads/nightly/build-2026-11-20-1",
                    "refs/tags/26.3-0.1.0", "refs/pull/1/merge", ""):
            for kind in ("daily", "nightly", "snapshot", "stable", "sdk"):
                self.assertFalse(train.publishes(ref, kind), (ref, kind))
        with self.assertRaisesRegex(SystemExit, "a real stable runs only on release/26.3, not refs/heads/main"):
            train.require_ref(MAIN, "stable")
        with self.assertRaisesRegex(SystemExit, "a real nightly runs only on main or nightly/26.3"):
            train.require_ref(RELEASE, "nightly")
        train.require_ref(NIGHTLY, "nightly")

    def test_each_workflow_names_exactly_the_refs_that_publish_its_kinds(self):
        # GitHub evaluates these lists before any script runs (the `release` environment, the keys), so they are
        # the contract PUBLISHES states: a ref listed in a workflow but not here would sign with the real key.
        kinds = {"release.yml": {"nightly", "snapshot"}, "nightly-build.yml": {"nightly", "snapshot", "stable"},
                 "daily.yml": {"daily"}, "sdk-publish.yml": {"sdk"}}
        for name, published in kinds.items():
            text = (ROOT / ".github/workflows" / name).read_text()
            expected = sorted(ref for ref, allowed in train.PUBLISHES.items() if allowed & published)
            lists = re.findall(r"fromJSON\('(\[[^']*refs/heads/[^']*\])'\)", text)
            self.assertTrue(lists, name)
            for found in lists:
                self.assertEqual(sorted(re.findall(r'"([^"]+)"', found)), expected, name)
            self.assertNotIn("github.ref != 'refs/heads/main'", text, name)


class NightlyBranchTests(unittest.TestCase):
    def test_a_nightly_builds_the_nightly_branch_head_once_it_exists(self):
        with tempfile.TemporaryDirectory() as folder:
            git, commits = repository(folder)
            with mock.patch.object(train, "ROOT", Path(folder, "clone")), \
                    mock.patch.object(train, "fork_gate_passed", return_value=True):
                git("tag", "daily-green", commits[2])
                git("push", "-q", "origin", "daily-green")
                self.assertEqual(train.channel_head("nightly", ""), commits[2], "no branch yet: daily-green")
                git("push", "-q", "origin", f"{commits[1]}:refs/heads/nightly/26.3")
                self.assertEqual(train.channel_head("nightly", ""), commits[1], "the branch's head, not daily-green")
                with self.assertRaisesRegex(SystemExit, "is not the head of nightly/26.3"):
                    train.channel_head("nightly", commits[2])
                self.assertEqual(train.channel_head("nightly", commits[0], "owner 2026-11-20: why"), commits[0],
                                 "the owner's grant still names its commit")

    def test_the_train_is_not_held_by_a_stable_build(self):
        runs = [{"databaseId": 1, "status": "in_progress", "displayTitle": "stable build release/26.3"},
                {"databaseId": 2, "status": "queued", "displayTitle": "snapshot build abc"}]
        with mock.patch.object(train, "gh_json", return_value=runs):
            self.assertIsNone(train.nightly_building())
        runs.append({"databaseId": 3, "status": "queued", "displayTitle": "nightly build"})
        with mock.patch.object(train, "gh_json", return_value=runs):
            self.assertEqual(train.nightly_building(), 3)


class StableHeadTests(unittest.TestCase):
    def test_a_stable_release_builds_only_the_release_branch_head_with_a_green_daily(self):
        with tempfile.TemporaryDirectory() as folder:
            git, commits = repository(folder)
            with mock.patch.object(train, "ROOT", Path(folder, "clone")):
                with self.assertRaisesRegex(SystemExit, "release/26.3 does not exist yet"):
                    train.release_head("")
                # Before the cut, a dry run names the commit: it must be daily-green's.
                with self.assertRaisesRegex(SystemExit, "neither the commit `daily-green` nor"):
                    train.release_head(commits[1])
                with mock.patch("sys.stderr", new_callable=io.StringIO) as err:
                    self.assertEqual(train.release_head(commits[1], dry_run=True), commits[1], "a dry run warns")
                self.assertIn("::warning::a real release would stop here", err.getvalue())
                git("tag", "-a", "-m", "green", "daily-green", commits[1])
                git("push", "-q", "origin", "daily-green")
                self.assertEqual(train.release_head(commits[1]), commits[1], "an annotated tag, peeled")
                git("push", "-q", "origin", f"{commits[1]}:refs/heads/release/26.3")
                self.assertEqual(train.release_head(""), commits[1], "the branch cut at daily-green")
                with self.assertRaisesRegex(SystemExit, "is not the head of release/26.3"):
                    train.release_head(commits[2])
                # A later commit on the branch (a fix, or a Nightly merged in) needs its own green daily there.
                git("checkout", "-q", "-b", "release", commits[1])
                git("commit", "-q", "--allow-empty", "-m", "fix")
                fix = git("rev-parse", "HEAD")
                git("push", "-q", "origin", "HEAD:refs/heads/release/26.3")
                with self.assertRaisesRegex(SystemExit, "whose real daily run passed every leg"):
                    train.release_head("")
                git("push", "-q", "origin", f"{fix}:refs/tags/release-green/26.3")
                self.assertEqual(train.release_head(""), fix)
                with self.assertRaisesRegex(SystemExit, "not a commit of this repository"):
                    train.release_head("f" * 40)


class PickStableTests(unittest.TestCase):
    def pick(self, folder, blockers=(), **env):
        out = Path(folder, "out")
        values = {"GITHUB_REPOSITORY": "sunrise-org/sunrise", "GITHUB_OUTPUT": str(out), "GITHUB_REF": RELEASE,
                  "DRY_RUN": "false", "NIGHTLY_BASE": "", "RELEASE_VERSION": "", "RELEASES_PAUSED": "", **env}
        sha = "0c02067" + "a" * 33
        tags = "x\trefs/tags/nightly\nx\trefs/tags/v2026.10.23\nx\trefs/tags/26.4-1.0.0\n"
        back = os.getcwd()
        os.chdir(folder)
        try:
            with mock.patch.dict(os.environ, values), \
                    mock.patch.object(nightly, "git", side_effect=lambda *a: tags if a[0] == "ls-remote" else ""), \
                    mock.patch.object(nightly.train, "release_head", return_value=sha) as head, \
                    mock.patch.object(nightly.train, "open_blockers", return_value=list(blockers)), \
                    mock.patch.object(nightly.train, "known_issues", return_value=[]), \
                    mock.patch.object(nightly, "sdk_ready", return_value=True):
                nightly.pick_stable()
            head.assert_called_once_with(values["NIGHTLY_BASE"], values["DRY_RUN"] == "true")
            return dict(line.split("=", 1) for line in out.read_text().splitlines()), Path("nightly-notes.md").read_text()
        finally:
            os.chdir(back)

    def test_the_first_stable_release_is_its_target_and_0_1_0(self):
        with tempfile.TemporaryDirectory() as folder:
            outputs, notes = self.pick(folder)
        self.assertEqual((outputs["tag"], outputs["version"], outputs["nightly"]), ("26.3-0.1.0", "26.3-0.1.0", ""))
        self.assertEqual(outputs["sdk"], "true")
        self.assertIn("The first stable release for Minecraft 26.3", notes)

    def test_a_pause_or_a_release_blocker_holds_a_real_release_and_a_dry_run_says_so(self):
        for held in ({"RELEASES_PAUSED": "1"}, {"blockers": [3715]}):
            with self.subTest(held=held), tempfile.TemporaryDirectory() as folder:
                with mock.patch.object(nightly, "stop", side_effect=SystemExit("stopped")) as stop:
                    with self.assertRaises(SystemExit):
                        self.pick(folder, **held)
                self.assertIn("held by", stop.call_args.args[2])
                outputs, _ = self.pick(folder, DRY_RUN="true", **held)
                self.assertEqual(outputs["tag"], "26.3-0.1.0")

    def test_a_real_stable_release_runs_only_on_the_release_branch(self):
        with tempfile.TemporaryDirectory() as folder:
            for ref in (MAIN, NIGHTLY, "refs/heads/topic"):
                with self.assertRaisesRegex(SystemExit, "a real stable runs only on release/26.3"):
                    self.pick(folder, GITHUB_REF=ref)
            outputs, _ = self.pick(folder, GITHUB_REF="refs/heads/topic", DRY_RUN="true")
            self.assertEqual(outputs["tag"], "26.3-0.1.0")


if __name__ == "__main__":
    unittest.main()
