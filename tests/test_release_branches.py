"""The release branches (docs/release-flow.md "Release branches"): which runs publish, the evidence of a passed daily
run, the stable tags and their order, and the commits the nightly and stable builds take."""
import io
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import yaml

from tools import release_file
from tools import release_train as train
from tools.ci import nightly

ROOT = Path(__file__).resolve().parents[1]
MAIN, NIGHTLY, RELEASE = "refs/heads/main", "refs/heads/nightly/26.3", "refs/heads/release/26.3"
REPO, DISPATCH = "sunrise-org/sunrise", "workflow_dispatch"
WORKFLOWS = ROOT / ".github/workflows"


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


def workflow(name):
    return yaml.load((WORKFLOWS / name).read_text(), Loader=yaml.BaseLoader)


def run_gate(script, **env):
    """A workflow's authorize step, run as Actions runs it: the outputs it writes."""
    with tempfile.TemporaryDirectory() as folder:
        out = Path(folder, "out")
        values = {**os.environ, "GITHUB_OUTPUT": str(out), "GITHUB_REPOSITORY": REPO, "GITHUB_EVENT_NAME": DISPATCH,
                  **env}
        subprocess.run(["bash", "-e", "-c", script], cwd=ROOT, env=values, check=True, capture_output=True)
        return dict(line.split("=", 1) for line in out.read_text().splitlines())


class StableTagTests(unittest.TestCase):
    def test_a_stable_tag_is_its_target_and_a_semver_within_it(self):
        self.assertEqual(train.stable_version("26.3-0.1.0"), ("26.3", (0, 1, 0)))
        self.assertEqual(train.stable_version("26.3.1-2.10.3"), ("26.3.1", (2, 10, 3)))
        for other in ("v26.3-0.1.0", "26.3-0.1", "26.3-01.0.0", "26.3-0.1.0-rc.1", "0.1.0+26.3", "26-0.1.0", "26.3",
                      "26.3-٠.1.0", "٢6.3-0.1.0", "nightly", "nightly-2026-11-20", "v2026.10.23", "v0.2.81",
                      "sdk-" + "a" * 40, "launcher@1.0.0"):
            self.assertIsNone(train.stable_version(other), other)

    def test_stable_tags_order_by_the_semver_within_a_target(self):
        tags = ["26.3-1.0.0", "26.3-0.10.0", "26.4-0.1.0", "26.3-0.2.0", "26.3-0.1.1", "26.3-0.9.0", "26.3-0.1.0"]
        self.assertEqual(sorted(tags, key=train.stable_key),
                         ["26.3-0.1.0", "26.3-0.1.1", "26.3-0.2.0", "26.3-0.9.0", "26.3-0.10.0", "26.3-1.0.0",
                          "26.4-0.1.0"])

    def test_the_next_stable_tag(self):
        self.assertEqual(train.next_stable_tag(["nightly", "v2026.10.23", "launcher@1.0.0"]), "26.3-0.1.0")
        self.assertEqual(train.next_stable_tag(["nightly"], version="0.1.0"), "26.3-0.1.0")
        for first in ("0.1.1", "1.0.0", "0.0.1"):
            with self.assertRaisesRegex(SystemExit, "the first stable release on 26.3 is 26.3-0.1.0"):
                train.next_stable_tag(["nightly"], version=first)
        tags = ["26.3-0.1.0", "26.3-0.1.1", "26.4-5.0.0", "nightly"]
        self.assertEqual(train.next_stable_tag(tags), "26.3-0.1.2", "the patch after the newest on the target")
        self.assertEqual(train.next_stable_tag(tags, version="0.2.0"), "26.3-0.2.0")
        self.assertEqual(train.next_stable_tag(tags, target="26.4"), "26.4-5.0.1")
        for older in ("0.1.1", "0.1.0", "0.0.9"):
            with self.assertRaisesRegex(SystemExit, "is not newer than 26.3-0.1.1"):
                train.next_stable_tag(tags, version=older)
        self.assertEqual(train.next_stable_tag(tags, version="0.1.1", own="26.3-0.1.1"), "26.3-0.1.1",
                         "a retry of the release its tag already names")
        for bad in ("1.0", "v0.2.0", "0.2.0-rc.1", "0.٢.0"):
            with self.assertRaisesRegex(SystemExit, "MAJOR.MINOR.PATCH"):
                train.next_stable_tag(tags, version=bad)

    def test_the_release_file_names_a_stable_build_by_its_tag(self):
        read = {release_file.VERSION_RS: 'pub const VERSION_ID: &str = "26.3";\npub const VANILLA_VERSION: &str = "26.3";\n'
                'pub const PROTOCOL_VERSION: i32 = 777;\npub const JAVA_VERSION: u32 = 25;\n',
                release_file.RUST_VERSION: "1.99.1\n"}.__getitem__
        facts = release_file.facts(read, "stable", "2026-11-20", "a" * 40, "26.3-0.1.0")
        self.assertEqual((facts["channel"], facts["build"]), ("stable", "26.3-0.1.0"))
        for channel, build in (("stable", ""), ("stable", "v26.3.0"), ("nightly", "26.3-0.1.0"), ("stable", "26.3-٠.1.0")):
            with self.assertRaises(SystemExit, msg=(channel, build)):
                release_file.facts(read, channel, "2026-11-20", "a" * 40, build)


class AuthorizeTests(unittest.TestCase):
    KINDS = ("daily", "nightly", "snapshot", "stable", "sdk")

    def test_only_exact_refs_of_this_repository_dispatched_for_real_publish(self):
        for ref, kinds in ((MAIN, {"daily", "nightly", "snapshot", "sdk"}), (NIGHTLY, {"nightly"}),
                           (RELEASE, {"daily", "stable", "sdk"})):
            for kind in self.KINDS:
                self.assertEqual(train.authorized(kind, "false", ref, REPO, DISPATCH), kind in kinds, (ref, kind))
        # Actions' == and contains() ignore case; a branch named Main is another branch, and never publishes.
        for ref in ("refs/heads/Main", "refs/heads/MAIN", "refs/heads/Release/26.3", "refs/heads/release/26.3 ",
                    "refs/heads/nightly/26.3/x", "refs/heads/x/refs/heads/main", "refs/tags/main",
                    "refs/tags/26.3-0.1.0", "refs/pull/1/merge", "refs/heads/topic", "refs/heads/release/26.4", "",
                    "main"):
            for kind in self.KINDS:
                self.assertFalse(train.authorized(kind, "false", ref, REPO, DISPATCH), (ref, kind))
        for dry in ("true", "", "False", "FALSE", "0", "no"):
            self.assertFalse(train.authorized("stable", dry, RELEASE, REPO, DISPATCH), dry)
        for repository in ("Sunrise-Org/sunrise", "someone/sunrise", ""):
            self.assertFalse(train.authorized("stable", "false", RELEASE, repository, DISPATCH), repository)
        for event in ("push", "pull_request", "pull_request_target", "schedule", "issue_comment", "WORKFLOW_DISPATCH"):
            self.assertFalse(train.authorized("stable", "false", RELEASE, REPO, event), event)
        with self.assertRaisesRegex(SystemExit, "a real stable runs only on release/26.3, not refs/heads/main"):
            train.require_ref(MAIN, "stable")
        with self.assertRaisesRegex(SystemExit, "a real nightly runs only on main or nightly/26.3"):
            train.require_ref(RELEASE, "nightly")
        with self.assertRaises(SystemExit):
            train.require_ref("refs/heads/Release/26.3", "stable")

    def test_each_workflow_runs_the_gate_and_its_variants_stay_dry(self):
        # (job, step, kind, the one ref that publishes it): each gate as its workflow runs it, with its variants.
        gates = [(workflow("daily.yml")["jobs"]["resolve"], "auth", "daily", RELEASE),
                 (workflow("nightly-build.yml")["jobs"]["pick"], "mode", "stable", RELEASE),
                 (workflow("nightly-build.yml")["jobs"]["pick"], "mode", "nightly", NIGHTLY),
                 (workflow("release.yml")["jobs"]["decide"], "mode", "snapshot", MAIN),
                 (workflow("sdk-publish.yml")["jobs"]["authorize"], "authorize", "sdk", MAIN)]
        for job, step_id, kind, ref in gates:
            step = next(step for step in job["steps"] if step.get("id") == step_id)
            upper = ref[:11] + ref[11].upper() + ref[12:]
            for case, dry, event, publish in ((ref, "false", DISPATCH, True), (upper, "false", DISPATCH, False),
                                              (ref, "true", DISPATCH, False), (ref, "false", "push", False),
                                              ("refs/tags/" + ref[11:], "false", DISPATCH, False),
                                              ("refs/pull/3768/merge", "false", DISPATCH, False)):
                with self.subTest(step=step_id, kind=kind, ref=case, dry=dry, event=event):
                    out = run_gate(step["run"], KIND=kind, DRY_INPUT=dry, GITHUB_REF=case, GITHUB_EVENT_NAME=event)
                    self.assertEqual(out, {"publish": str(publish).lower(), "dry_run": str(not publish).lower()})

    def test_every_key_environment_and_promotion_reads_the_gate_never_the_ref(self):
        def leaves(node, key=None):
            if isinstance(node, dict):
                for name, value in node.items():
                    yield from leaves(value, name)
            elif isinstance(node, list):
                for value in node:
                    yield from leaves(value, key)
            else:
                yield key, str(node)
        for name in ("daily.yml", "nightly-build.yml", "release.yml", "sdk-publish.yml"):
            for job_name, job in workflow(name)["jobs"].items():
                for key, text in leaves(job):
                    if key in ("if", "environment") or "secrets." in text:
                        self.assertNotIn("github.ref", text, f"{name} {job_name} {key}")
        sdk = workflow("sdk-publish.yml")["jobs"]
        self.assertEqual(sdk["sign"]["environment"], "${{ needs.authorize.outputs.publish == 'true' && 'release' || '' }}")
        self.assertEqual(sdk["publish"]["env"]["PUBLISH"], "${{ needs.authorize.outputs.publish }}")
        build = workflow("nightly-build.yml")["jobs"]
        self.assertEqual(build["pick"]["outputs"]["dry_run"], "${{ steps.mode.outputs.dry_run }}")
        for job in ("sign", "musl-attach"):
            self.assertEqual(build[job]["environment"], "${{ needs.pick.outputs.dry_run != 'true' && 'release' || '' }}")
        daily = workflow("daily.yml")["jobs"]
        self.assertEqual(daily["resolve"]["steps"][-1]["env"]["DRY_RUN"], "${{ steps.auth.outputs.dry_run }}")
        for job in ("nightly-branch", "nightly"):
            self.assertIn("needs.green.result == 'success'", daily[job]["if"])
            self.assertIn("needs.resolve.outputs.branch == 'main'", daily[job]["if"])
        green = daily["green"]
        self.assertEqual(green["name"], "green ${{ needs.resolve.outputs.commit }}")
        for condition in ("!cancelled()", "needs.resolve.result == 'success'", "needs.resolve.outputs.dry_run == 'false'",
                          "needs.summary.result == 'success'", "needs.summary.outputs.green == 'true'",
                          "needs.parity.result == 'success'", "needs.sdk.outputs.needed == 'false'"):
            self.assertIn(condition, green["if"])


def daily_runs(*runs):
    """A gh_json for daily_passed: the runs page, then each run's jobs."""
    pages = {"runs": [{"workflow_runs": [run for run, _ in runs]}]}
    jobs = {run["id"]: [{"jobs": run_jobs}] for run, run_jobs in runs}

    def answer(*arguments):
        path = arguments[2]
        if "/workflows/daily.yml/runs" in path:
            return pages["runs"]
        return jobs[int(path.split("/runs/")[1].split("/")[0])]
    return answer


def daily_run(id, sha, branch="main", title=None, conclusion="success", event=DISPATCH, path=".github/workflows/daily.yml",
              repository=REPO, head=None):
    return {"id": id, "head_sha": head or "f" * 40, "display_title": title or f"daily {sha}", "head_branch": branch,
            "conclusion": conclusion, "event": event, "path": path, "repository": {"full_name": repository},
            "html_url": f"https://github.com/{REPO}/actions/runs/{id}"}


def green_job(sha, conclusion="success"):
    return {"name": f"green {sha}", "status": "completed", "conclusion": conclusion}


class DailyEvidenceTests(unittest.TestCase):
    SHA = "c" * 40

    def passed(self, *runs):
        with mock.patch.object(train, "gh_json", side_effect=daily_runs(*runs)), \
                mock.patch.object(train, "REPO", REPO):
            return train.daily_passed(self.SHA)

    def test_only_a_real_runs_green_job_on_exactly_the_sha_counts(self):
        sha = self.SHA
        self.assertEqual(self.passed((daily_run(1, sha), [green_job(sha)])), f"https://github.com/{REPO}/actions/runs/1")
        self.assertTrue(self.passed((daily_run(2, sha, branch=train.RELEASE_BRANCH), [green_job(sha)])))
        self.assertTrue(self.passed((daily_run(3, sha, title="daily main", head=sha), [green_job(sha)])),
                        "dispatched without a commit: the run's own head")
        for run, jobs in (
                (daily_run(4, sha, title=f"daily {sha} (dry run)"), [green_job(sha)]),
                (daily_run(5, sha), [green_job(sha, "skipped")]),
                (daily_run(6, sha), [green_job(sha, "failure")]),
                (daily_run(7, sha), [green_job("d" * 40)]),
                (daily_run(8, sha), [{"name": f"summary {sha}", "status": "completed", "conclusion": "success"}]),
                (daily_run(9, sha, conclusion="cancelled"), [green_job(sha)]),
                (daily_run(10, sha, branch="Main"), [green_job(sha)]),
                (daily_run(11, sha, branch="topic"), [green_job(sha)]),
                (daily_run(12, sha, branch="nightly/26.3"), [green_job(sha)]),
                (daily_run(13, sha, event="push"), [green_job(sha)]),
                (daily_run(14, sha, path=".github/workflows/other.yml"), [green_job(sha)]),
                (daily_run(15, sha, repository="someone/sunrise"), [green_job(sha)]),
                (daily_run(16, sha, title="daily " + sha[:9]), [green_job(sha)])):
            with self.subTest(run=run["id"]):
                self.assertEqual(self.passed((run, jobs)), "")

    def test_the_lookup_asks_for_completed_dispatches_and_the_latest_attempt(self):
        calls = []
        answer = daily_runs((daily_run(1, self.SHA), [green_job(self.SHA)]))
        with mock.patch.object(train, "gh_json", side_effect=lambda *a: calls.append(a) or answer(*a)), \
                mock.patch.object(train, "REPO", REPO):
            train.daily_passed(self.SHA)
        self.assertIn("event=workflow_dispatch&status=completed", calls[0][2])
        self.assertIn("filter=latest", calls[1][2])


class NightlyBranchTests(unittest.TestCase):
    def test_a_nightly_builds_the_nightly_branch_head_once_it_exists_and_it_passed_its_daily(self):
        with tempfile.TemporaryDirectory() as folder:
            git, commits = repository(folder)
            with mock.patch.object(train, "ROOT", Path(folder, "clone")), \
                    mock.patch.object(train, "fork_gate_passed", return_value=True):
                git("tag", "daily-green", commits[2])
                git("push", "-q", "origin", "daily-green")
                with mock.patch.object(train, "daily_passed", side_effect=AssertionError("not consulted")):
                    self.assertEqual(train.channel_head("nightly", ""), commits[2], "no branch yet: daily-green")
                git("push", "-q", "origin", f"{commits[1]}:refs/heads/nightly/26.3")
                with mock.patch.object(train, "daily_passed", return_value="url") as passed:
                    self.assertEqual(train.channel_head("nightly", ""), commits[1], "the branch's head, not daily-green")
                passed.assert_called_once_with(commits[1])
                with mock.patch.object(train, "daily_passed", return_value=""):
                    with self.assertRaisesRegex(SystemExit, "has no completed real daily run"):
                        train.channel_head("nightly", "")
                with mock.patch.object(train, "daily_passed", return_value="url"):
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
    def test_a_real_release_takes_only_the_approved_head_of_an_existing_branch_with_its_daily(self):
        with tempfile.TemporaryDirectory() as folder:
            git, commits = repository(folder)
            with mock.patch.object(train, "ROOT", Path(folder, "clone")), \
                    mock.patch.object(train, "daily_passed", side_effect=lambda sha: "url" if sha == commits[1] else ""):
                for named in ("", "main", commits[1][:9], commits[1].upper()):
                    with self.assertRaisesRegex(SystemExit, "names the full 40-character commit approved"):
                        train.release_head(named)
                with self.assertRaisesRegex(SystemExit, "release/26.3 does not exist"):
                    train.release_head(commits[1])
                with mock.patch("sys.stderr", new_callable=io.StringIO):
                    self.assertEqual(train.release_head(commits[1], dry_run=True), commits[1], "a dry run, before the cut")
                    with self.assertRaisesRegex(SystemExit, "release/26.3 does not exist"):
                        train.release_head("", dry_run=True)
                git("push", "-q", "origin", f"{commits[1]}:refs/heads/release/26.3")
                with mock.patch("sys.stderr", new_callable=io.StringIO):
                    self.assertEqual(train.release_head(commits[1]), commits[1])
                    self.assertEqual(train.release_head("", dry_run=True), commits[1], "a dry run takes the head")
                with self.assertRaisesRegex(SystemExit, "is not the head of release/26.3"):
                    train.release_head(commits[2])
                # A later commit on the branch (a fix, or a Nightly merged in) needs its own passed daily run.
                git("checkout", "-q", "-b", "release", commits[1])
                git("commit", "-q", "--allow-empty", "-m", "fix")
                fix = git("rev-parse", "HEAD")
                git("push", "-q", "origin", "HEAD:refs/heads/release/26.3")
                with self.assertRaisesRegex(SystemExit, "has no completed real daily run that passed every leg"):
                    train.release_head(fix)
                with self.assertRaisesRegex(SystemExit, "is not the head of release/26.3"):
                    train.release_head(commits[1])
                with mock.patch("sys.stderr", new_callable=io.StringIO) as err:
                    self.assertEqual(train.release_head(fix, dry_run=True), fix)
                self.assertIn("::warning::a real release would stop here", err.getvalue())
                with self.assertRaisesRegex(SystemExit, "not a commit of this repository"):
                    train.release_head("f" * 40, dry_run=True)


class PickStableTests(unittest.TestCase):
    SHA = "0c02067" + "a" * 33

    def pick(self, folder, blockers=(), tags=None, **env):
        out = Path(folder, "out")
        values = {"GITHUB_REPOSITORY": REPO, "GITHUB_OUTPUT": str(out), "GITHUB_REF": RELEASE, "DRY_RUN": "false",
                  "NIGHTLY_BASE": self.SHA, "RELEASE_VERSION": "0.1.0", "RELEASES_PAUSED": "", **env}
        tags = tags or {"nightly": "1" * 40, "v2026.10.23": "2" * 40, "26.4-1.0.0": "3" * 40}
        back = os.getcwd()
        os.chdir(folder)
        try:
            with mock.patch.dict(os.environ, values), mock.patch.object(nightly, "git", return_value=""), \
                    mock.patch.object(nightly, "remote_tags", return_value=tags), \
                    mock.patch.object(nightly.train, "release_head", return_value=self.SHA) as head, \
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

    def test_a_real_release_names_its_version_and_the_first_is_0_1_0(self):
        with tempfile.TemporaryDirectory() as folder:
            with mock.patch.object(nightly, "stop", side_effect=SystemExit("stopped")) as stop:
                with self.assertRaises(SystemExit):
                    self.pick(folder, RELEASE_VERSION="")
            self.assertIn("names its version", stop.call_args.args[2])
            with self.assertRaisesRegex(SystemExit, "the first stable release on 26.3 is 26.3-0.1.0"):
                self.pick(folder, RELEASE_VERSION="0.2.0")
            outputs, _ = self.pick(folder, RELEASE_VERSION="", DRY_RUN="true")
            self.assertEqual(outputs["tag"], "26.3-0.1.0", "a dry run defaults to the next")
            # A retry after the tag was made at the approved commit; a tag elsewhere is not newer.
            outputs, _ = self.pick(folder, tags={"26.3-0.1.0": self.SHA})
            self.assertEqual(outputs["tag"], "26.3-0.1.0")
            with self.assertRaisesRegex(SystemExit, "is not newer than 26.3-0.1.0"):
                self.pick(folder, tags={"26.3-0.1.0": "9" * 40})

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
            for ref in (MAIN, NIGHTLY, "refs/heads/topic", "refs/heads/Release/26.3"):
                with self.assertRaisesRegex(SystemExit, "a real stable runs only on release/26.3"):
                    self.pick(folder, GITHUB_REF=ref)
            outputs, _ = self.pick(folder, GITHUB_REF="refs/heads/topic", DRY_RUN="true")
            self.assertEqual(outputs["tag"], "26.3-0.1.0")


class ReserveStableTagTests(unittest.TestCase):
    SHA = "0c02067" + "a" * 33

    def reserve(self, tags, created=0, after="", head=None, ref=RELEASE, tag="26.3-0.1.0"):
        calls = []

        def run(command, **kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, created, "", "Reference already exists" if created else "")
        with mock.patch.dict(os.environ, {"GITHUB_REF": ref, "GITHUB_REPOSITORY": REPO}), \
                mock.patch.object(nightly, "remote_tags", return_value=tags), \
                mock.patch.object(nightly.train, "release_head", return_value=head or self.SHA) as release_head, \
                mock.patch.object(nightly.train, "origin_commit", return_value=after), \
                mock.patch.object(nightly.subprocess, "run", side_effect=run), \
                mock.patch("sys.stdout", new_callable=io.StringIO):
            nightly.reserve_stable_tag(self.SHA, tag)
        release_head.assert_called_once_with(self.SHA)
        return calls

    def test_the_tag_is_created_at_the_approved_commit_in_one_write(self):
        calls = self.reserve({"nightly": "1" * 40})
        self.assertEqual(calls, [["gh", "api", f"repos/{REPO}/git/refs", "-f", "ref=refs/tags/26.3-0.1.0", "-f",
                                  f"sha={self.SHA}"]])

    def test_an_earlier_attempts_tag_is_kept_and_any_other_refused(self):
        self.assertEqual(self.reserve({"26.3-0.1.0": self.SHA}), [], "a publish-only retry")
        with self.assertRaisesRegex(SystemExit, "already names " + "9" * 40):
            self.reserve({"26.3-0.1.0": "9" * 40})
        # Made by someone else between the lookup and the write: only the same commit is acceptable.
        self.reserve({}, created=1, after=self.SHA)
        with self.assertRaisesRegex(SystemExit, "could not create 26.3-0.1.0"):
            self.reserve({}, created=1, after="9" * 40)

    def test_the_head_ref_and_version_are_checked_again(self):
        with self.assertRaisesRegex(SystemExit, "no longer the approved release head"):
            self.reserve({}, head="9" * 40)
        with self.assertRaisesRegex(SystemExit, "a real stable runs only on release/26.3"):
            self.reserve({}, ref="refs/heads/Release/26.3")
        with self.assertRaisesRegex(SystemExit, "is not newer than 26.3-0.2.0"):
            self.reserve({"26.3-0.2.0": "9" * 40}, tag="26.3-0.1.1")
        for bad in ("26.4-0.1.0", "v26.3-0.1.0", "26.3-0.1"):
            with self.assertRaisesRegex(SystemExit, "is not a stable tag on 26.3"):
                self.reserve({}, tag=bad)

    def test_the_publish_step_reserves_the_tag_then_publishes_on_it(self):
        steps = workflow("nightly-build.yml")["jobs"]["publish"]["steps"]
        run = next(step["run"] for step in steps if step.get("name") == "Publish as the stable release <target>-<semver>")
        self.assertLess(run.index("require-sdk"), run.index('reserve-stable-tag "$SHA" "$TAG"'))
        self.assertLess(run.index('reserve-stable-tag "$SHA" "$TAG"'), run.index('gh release create "$TAG" --verify-tag'))
        self.assertNotIn("--target", run)
        self.assertNotIn("--prerelease", run)


if __name__ == "__main__":
    unittest.main()
