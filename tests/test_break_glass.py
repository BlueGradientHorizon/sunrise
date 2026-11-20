"""Break-glass stable patch releases (docs/release-flow.md "Break-glass patch releases"): the legs a patch's changes
select, the rules never waived, the two approvals, and how the workflows hold to them."""
import contextlib
import io
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tests import test_daily_summary as daily
from tests.test_release_branches import REPO, git_in, workflow
from tools import release_train as train
from tools.ci import daily_summary as summary
from tools.ci import nightly

ALWAYS = sorted(train.BREAK_GLASS_ALWAYS)
CRATES = ["cargo-sunrise-mod", "shards", "windows-runtime-leases"]
PARENT_TAG = "26.3-0.1.0"


def history(folder, *changes):
    """26.3-0.1.0 at `parent`, a cherry-pick above it writing each (path, kind): a file, or a symlink."""
    repo = Path(folder)
    git = git_in(repo)
    git("init", "-q", "-b", "release")
    git("config", "user.name", "Test")
    git("config", "user.email", "test@example.invalid")
    (repo / "docs").mkdir()
    (repo / "docs" / "modding").mkdir()
    (repo / "docs" / "modding" / "a.md").write_text("book\n")
    git("add", "-A")
    git("commit", "-q", "-m", "0.1.0")
    parent = git("rev-parse", "HEAD")
    for path, kind in changes:
        target = repo / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if kind == "link":
            target.symlink_to("a.md")
        else:
            target.write_text("fix\n")
        subprocess.run(["git", "add", "--", path], cwd=repo, check=True)
    git("commit", "-q", "--allow-empty", "-m", "cherry-pick")
    return parent, git("rev-parse", "HEAD")


class LegsTests(unittest.TestCase):
    def test_each_path_selects_its_legs_and_an_unmapped_path_needs_the_full_daily(self):
        for paths, legs in (
                (["docs/modding/src/quickstart.md"], ALWAYS + ["book-quickstart"]),
                (["docs/modding"], ALWAYS + ["book-quickstart"]),  # the folder replaced by a file
                (["tools/book_quickstart.py"], ALWAYS + ["book-quickstart"]),
                (["docs/hosting.md"], ALWAYS),
                (["docs"], ALWAYS),
                (["crates/sunrise-world/src/lib.rs", "Cargo.lock"], ALWAYS + CRATES),
                ([], ALWAYS)):
            with self.subTest(paths=paths):
                self.assertEqual(train.break_glass_legs(paths), (sorted(legs), []))
        for path in (".github/workflows/daily.yml", "tools/ci/nightly.py", "packaging/server/install.sh",
                     "cratesx/a.rs", "tools/book_quickstart.py.bak", "README.md"):
            with self.subTest(path=path):
                self.assertEqual(train.break_glass_legs(["docs/hosting.md", path])[1], [path])

    def test_the_crates_that_judge_a_release_need_the_full_daily(self):
        for crate in ("sunrise-mod-api-gen", "sunrise-mod-surface", "sunrise-mod-package", "sunrise-release-sign"):
            for path in (f"crates/{crate}/src/lib.rs", f"crates/{crate}"):
                with self.subTest(path=path):
                    self.assertEqual(train.break_glass_legs([path, "crates/sunrise-world/src/lib.rs"])[1], [path])

    def test_every_selected_leg_is_a_daily_leg(self):
        legs = set(train.BREAK_GLASS_ALWAYS).union(*(selected or () for _, selected in train.BREAK_GLASS_LEGS))
        self.assertLessEqual(legs, set(workflow("daily.yml")["jobs"]["summary"]["needs"]))

    def test_each_legs_job_name_fits_githubs_limit(self):
        # A longer name is cut by the Jobs API, and legs_passed would never find the leg it names.
        job = workflow("daily.yml")["jobs"]["legs"]
        self.assertEqual(job["name"], "legs ${{ needs.resolve.outputs.commit }} ${{ matrix.leg }}")
        self.assertIn("needs.resolve.outputs.legs", job["strategy"]["matrix"]["leg"])
        every = set(ALWAYS).union(*(selected or () for _, selected in train.BREAK_GLASS_LEGS))
        self.assertLessEqual(max(len(f"legs {'f' * 40} {leg}") for leg in every), 100)

    def test_changed_paths_are_read_raw_and_links_or_line_breaks_are_never_mapped(self):
        with tempfile.TemporaryDirectory() as folder:
            parent, sha = history(folder, ("docs/ü.md", "file"), ("docs/modding/link.md", "link"),
                                  ("docs/Cargo.toml\nCargo.lock", "file"))
            with mock.patch.object(train, "ROOT", Path(folder)):
                plain, special = train.changed_since(parent, sha)
        self.assertEqual(plain, ["docs/ü.md"], "unquoted, so it maps")
        self.assertEqual(sorted(special), ["docs/Cargo.toml\nCargo.lock", "docs/modding/link.md"])


def run_record(id, created="2026-11-20T12:00:00Z", branch=train.RELEASE_BRANCH, head="f" * 40):
    return {"id": id, "head_branch": branch, "head_sha": head, "created_at": created,
            "html_url": f"https://github.com/{REPO}/actions/runs/{id}"}


def legs_jobs(sha, legs):
    return {f"legs {sha} {leg}": "success" for leg in legs}


def full(sha, green=True):
    return {f"summary {sha}": "success" if green else "failure", f"green {sha}": "success" if green else "skipped"}


class EvidenceTests(unittest.TestCase):
    SHA = "c" * 40

    def test_a_break_glass_run_counts_for_the_legs_it_passed_and_no_fewer(self):
        legs = ALWAYS + ["book-quickstart"]
        run = run_record(1)
        self.assertEqual(train.legs_passed(self.SHA, legs, [(run, legs_jobs(self.SHA, legs))]), run["html_url"])
        self.assertEqual(train.legs_passed(self.SHA, legs, [(run, legs_jobs(self.SHA, legs + ["shards"]))]),
                         run["html_url"], "more legs than needed")
        for jobs, branch in ((legs_jobs(self.SHA, ALWAYS), train.RELEASE_BRANCH),
                             ({**legs_jobs(self.SHA, legs), f"legs {self.SHA} sdk": "failure"}, train.RELEASE_BRANCH),
                             (legs_jobs("d" * 40, legs), train.RELEASE_BRANCH),
                             (legs_jobs(self.SHA, legs), "main")):
            with self.subTest(jobs=jobs, branch=branch):
                self.assertEqual(train.legs_passed(self.SHA, legs, [(run_record(2, branch=branch), jobs)]), "")
        # Two runs that each passed some of the legs are not one run that passed them all.
        split = [(run_record(3), legs_jobs(self.SHA, ALWAYS)), (run_record(4), legs_jobs(self.SHA, ["book-quickstart"]))]
        self.assertEqual(train.legs_passed(self.SHA, legs, split), "")

    def test_the_newest_full_daily_of_exactly_the_commit_decides(self):
        green, red = run_record(1, "2026-11-19T09:00:00Z"), run_record(2, "2026-11-20T09:00:00Z")
        skipped = run_record(3, "2026-11-21T09:00:00Z")  # a scheduled run that found the commit already done
        partial = run_record(4, "2026-11-21T10:00:00Z")
        runs = [(partial, {f"summary {self.SHA} (break-glass legs)": "success", **legs_jobs(self.SHA, ALWAYS)}),
                (skipped, {f"summary {self.SHA}": "skipped"}), (red, full(self.SHA, green=False)),
                (green, full(self.SHA))]
        self.assertEqual(train.newest_full_daily(self.SHA, runs), (red["html_url"], False))
        self.assertEqual(train.newest_full_daily(self.SHA, runs[1:2] + runs[3:]), (green["html_url"], True),
                         "a skipped scheduled run is no verdict")
        # The branch moved on before the follow-up ran: its head_sha is the newer head, its jobs name the commit.
        moved = run_record(5, head="e" * 40)
        self.assertEqual(train.newest_full_daily(self.SHA, [(moved, full(self.SHA))]), (moved["html_url"], True))
        self.assertIsNone(train.newest_full_daily(self.SHA, [(moved, full("e" * 40))]))
        self.assertIsNone(train.newest_full_daily(self.SHA, [(green, full(self.SHA))], since="2026-11-20T00:00:00Z"),
                          "a break-glass parent's follow-up runs after its publication")


class ProblemsTests(unittest.TestCase):
    """break_glass_problems on a real history."""

    def problems(self, folder, sha, parent, tag="26.3-0.1.1", runs=None, gate=True):
        with mock.patch.object(train, "ROOT", Path(folder)), \
                mock.patch.object(train, "break_glass_runs", return_value=runs if runs is not None else []), \
                mock.patch.object(train, "fork_gate_passed", return_value=gate) as gated:
            found = train.break_glass_problems(sha, tag, parent)
        if gated.called:
            gated.assert_called_once_with(sha, train.RELEASE_BRANCH)
        return found

    @staticmethod
    def evidence(sha, parent_sha, legs=("book-quickstart",)):
        return [(run_record(2), legs_jobs(sha, ALWAYS + list(legs))),
                (run_record(1, "2026-11-19T09:00:00Z"), full(parent_sha))]

    def test_a_patch_whose_legs_gate_and_parent_daily_passed_may_ship(self):
        with tempfile.TemporaryDirectory() as folder:
            parent, sha = history(folder, ("docs/modding/src/a.md", "file"))
            release = train.Release(PARENT_TAG, parent, "2026-11-18T00:00:00Z", False)
            found = self.problems(folder, sha, release, runs=self.evidence(sha, parent))
        self.assertEqual(found, ([], sorted(ALWAYS + ["book-quickstart"])))

    def test_each_rule_refuses_on_its_own(self):
        with tempfile.TemporaryDirectory() as folder:
            parent, sha = history(folder, ("crates/sunrise-world/src/lib.rs", "file"))
            release = train.Release(PARENT_TAG, parent, "2026-11-18T00:00:00Z", False)
            good = self.evidence(sha, parent, CRATES)
            own_red = [(run_record(3, "2026-11-20T13:00:00Z"), full(sha, green=False))]
            for change, reason in (
                    ({"tag": "26.3-0.2.0"}, "is not a patch release of 26.3-0.1.0"),
                    ({"tag": "26.3-1.0.0"}, "is not a patch release of 26.3-0.1.0"),
                    ({"runs": good[1:]}, "has no real daily run that passed the legs"),
                    ({"runs": self.evidence(sha, parent, CRATES[:2])}, "passed the legs"),
                    ({"runs": good[:1]}, "the newest full daily run of 26.3-0.1.0"),
                    ({"runs": good[:1] + [(run_record(1), full(parent, green=False))]}, "failed"),
                    ({"runs": own_red + good}, f"the newest full daily run of {sha} failed"),
                    ({"gate": False}, "gate jobs (lint, verify, verify-static) did not all pass")):
                with self.subTest(change=change):
                    problems, _ = self.problems(folder, sha, release, **{"runs": good, **change})
                    self.assertEqual(len(problems), 1, problems)
                    self.assertIn(reason, problems[0])

    def test_a_break_glass_parent_needs_its_follow_up_daily_after_publication(self):
        with tempfile.TemporaryDirectory() as folder:
            parent, sha = history(folder, ("docs/a.md", "file"))
            release = train.Release(PARENT_TAG, parent, "2026-11-20T00:00:00Z", True)
            before = [(run_record(2), legs_jobs(sha, ALWAYS)),
                      (run_record(1, "2026-11-19T09:00:00Z"), full(parent))]
            problems, _ = self.problems(folder, sha, release, runs=before)
            self.assertIn("since its break-glass publication (2026-11-20T00:00:00Z) is missing", problems[0])
            self.assertIn(f"-f commit={parent}", problems[0])
            after = before + [(run_record(3, "2026-11-20T05:00:00Z"), full(parent))]
            self.assertEqual(self.problems(folder, sha, release, runs=after)[0], [])

    def test_an_unmapped_or_special_path_a_foreign_parent_or_no_parent_refuses(self):
        with tempfile.TemporaryDirectory() as folder:
            parent, sha = history(folder, ("docs/a.md", "file"), (".github/workflows/release.yml", "file"))
            release = train.Release(PARENT_TAG, parent, "", False)
            problems, legs = self.problems(folder, sha, release, runs=self.evidence(sha, parent, ()))
            self.assertIn("'.github/workflows/release.yml'", problems[0])
            other = git_in(Path(folder))("commit-tree", "-m", "elsewhere", "4b825dc642cb6eb9a060e54bf8d69288fbee4904")
            problems, _ = self.problems(folder, sha, release._replace(sha=other), runs=self.evidence(sha, other, ()))
            self.assertIn("is not a new commit descending from 26.3-0.1.0", problems[0])
            problems, _ = self.problems(folder, parent, release, runs=self.evidence(parent, parent, ()))
            self.assertIn("is not a new commit descending from 26.3-0.1.0", problems[0])
            self.assertEqual(self.problems(folder, sha, None, tag=PARENT_TAG)[1], [])
            self.assertIn("no published stable release on 26.3", self.problems(folder, sha, None, tag=PARENT_TAG)[0][0])
        with tempfile.TemporaryDirectory() as folder:
            parent, sha = history(folder, ("docs/modding/link.md", "link"))
            problems, _ = self.problems(folder, sha, train.Release(PARENT_TAG, parent, "", False),
                                        runs=self.evidence(sha, parent))
            self.assertIn("'docs/modding/link.md'", problems[0])


class ApprovalTests(unittest.TestCase):
    UMCEKO, ARIA = (f"break-glass-{name}" for name in ("umceko", "aria"))

    def approvals(self, *records, attempt="1", pages=None):
        with mock.patch.object(train, "gh_json", return_value=pages or [list(records)]) as call:
            lines = train.break_glass_approvals("55", attempt)
        call.assert_called_once_with("api", "--paginate", f"repos/{train.REPO}/actions/runs/55/approvals?per_page=100")
        return lines

    @staticmethod
    def record(login, user_id, *environments, state="approved", comment="the save fix", kind="User"):
        return {"state": state, "comment": comment, "user": {"login": login, "id": user_id, "type": kind},
                "environments": [{"name": name} for name in environments]}

    def umceko(self, **kwargs):
        return self.record("UMCEKO", 11256132, self.UMCEKO, **kwargs)

    def aria(self, **kwargs):
        return self.record("arismoko", 19542195, self.ARIA, **kwargs)

    def test_each_environment_approved_once_by_its_own_approver(self):
        self.assertEqual(self.approvals(self.umceko(), self.aria(comment=" two\nlines ")),
                         [f"@UMCEKO ({self.UMCEKO}): the save fix", f"@arismoko ({self.ARIA}): two lines"])
        # The approvals come over several pages.
        self.assertEqual(len(self.approvals(pages=[[self.umceko()], [self.aria(comment=None)]])), 2)

    def test_a_missing_rejected_repeated_or_foreign_approval_refuses(self):
        for records, reason in (
                ([self.umceko()], f"{self.ARIA} needs the approval of user 19542195, not nobody"),
                ([], f"{self.UMCEKO} needs the approval of user 11256132, not nobody"),
                ([self.umceko(), self.aria(state="rejected")], f"{self.ARIA} was rejected by @arismoko"),
                ([self.aria(state="rejected"), self.umceko(), self.aria()], f"{self.ARIA} was rejected"),
                ([self.umceko(), self.aria(), self.aria()], f"{self.ARIA} was approved by @arismoko after another"),
                ([self.record("UMCEKO", 11256132, self.UMCEKO, self.ARIA)], "needs the approval of user 19542195"),
                ([self.umceko(), self.record("someone", 42, self.ARIA)], "not @someone"),
                ([self.umceko(), self.record("arismoko", 19542195, self.ARIA, kind="Bot")], "Bot 19542195")):
            with self.subTest(records=records), self.assertRaisesRegex(SystemExit, reason):
                self.approvals(*records)

    def test_only_the_first_attempt_counts(self):
        with mock.patch.object(train, "gh_json") as call, self.assertRaisesRegex(SystemExit, "not attempt 2's"):
            train.break_glass_approvals("55", "2")
        call.assert_not_called()

    def test_a_dry_run_reports_and_verifies_nothing(self):
        with tempfile.TemporaryDirectory() as folder, mock.patch.dict(os.environ, {
                "PUBLISH": "false", "BREAK_GLASS_REASON": "a\nsave  fix", "GITHUB_STEP_SUMMARY": f"{folder}/s",
                "GITHUB_OUTPUT": f"{folder}/o"}), mock.patch.object(train, "break_glass_approvals") as verify, \
                mock.patch("sys.argv", ["release_train.py", "break-glass-approvals"]), \
                contextlib.redirect_stdout(io.StringIO()):
            Path(folder, "o").touch()
            self.assertEqual(train.main(), 0)
            verify.assert_not_called()
            self.assertIn("Why: a save fix\n", Path(folder, "s").read_text())
            self.assertEqual(Path(folder, "o").read_text(), "", "no note: publishing needs a real run's")


class NotesTests(unittest.TestCase):
    ENV = {"TAG": "26.3-0.1.1", "SHA": "c" * 40, "PARENT_TAG": PARENT_TAG, "LEGS": "sdk book-quickstart",
           "BREAK_GLASS_REASON": "the save\nfix & \\1", "BREAK_GLASS_APPROVALS": "@UMCEKO (break-glass-umceko): ok &"}

    def test_the_section_goes_above_the_closing_disclaimer_as_written(self):
        with tempfile.TemporaryDirectory() as folder:
            notes = Path(folder, "notes.md")
            # A commit subject may hold anything, even the section's title or the disclaimer itself.
            body = f"- {nightly.BREAK_GLASS_SECTION} <!-- break-glass approvals --> {train.DISCLAIMER}\n\n"
            notes.write_text(body + train.DISCLAIMER + "\n")
            with mock.patch.dict(os.environ, self.ENV):
                nightly.break_glass_notes(notes)
            text = notes.read_text()
        self.assertTrue(text.startswith(body + nightly.BREAK_GLASS_SECTION + "\n26.3-0.1.1 (commit " + "c" * 40))
        self.assertTrue(text.endswith("Why: the save fix & \\1\n\nApproved by: @UMCEKO (break-glass-umceko): ok &\n\n"
                                      + train.DISCLAIMER + "\n"))

    def test_no_verified_approvals_no_release(self):
        with tempfile.TemporaryDirectory() as folder:
            notes = Path(folder, "notes.md")
            notes.write_text(train.DISCLAIMER)
            with mock.patch.dict(os.environ, {**self.ENV, "BREAK_GLASS_APPROVALS": ""}), \
                    self.assertRaisesRegex(SystemExit, "without its verified approvals"):
                nightly.break_glass_notes(notes)


class ReserveTests(unittest.TestCase):
    SHA = "c" * 40
    PARENT = train.Release(PARENT_TAG, "a" * 40, "", False)

    def reserve(self, parent, problems=()):
        env = {"GITHUB_REF": f"refs/heads/{train.RELEASE_BRANCH}", "GITHUB_REPOSITORY": REPO, "BREAK_GLASS": "true",
               "PARENT_SHA": self.PARENT.sha, "GITHUB_RUN_ID": "55", "GITHUB_RUN_ATTEMPT": "1"}
        with mock.patch.dict(os.environ, env), \
                mock.patch.object(nightly.train, "release_head", return_value=self.SHA) as head, \
                mock.patch.object(nightly.train, "origin_tags", return_value={PARENT_TAG: self.PARENT.sha,
                                                                              "26.3-0.1.1": self.SHA}), \
                mock.patch.object(nightly, "stable_parent", return_value=parent), \
                mock.patch.object(nightly.train, "break_glass_problems", return_value=(list(problems), [])) as rules, \
                mock.patch.object(nightly.train, "break_glass_approvals") as approvals, \
                contextlib.redirect_stdout(io.StringIO()):
            nightly.reserve_stable_tag(self.SHA, "26.3-0.1.1")
        head.assert_called_once_with(self.SHA, daily=False)
        rules.assert_called_once_with(self.SHA, "26.3-0.1.1", parent)
        approvals.assert_called_once_with("55", "1")

    def test_the_tag_waits_for_the_same_parent_its_rules_and_both_approvals_again(self):
        self.reserve(self.PARENT)
        with self.assertRaisesRegex(SystemExit, "the parent release changed since the pick"):
            self.reserve(self.PARENT._replace(sha="b" * 40))
        with self.assertRaisesRegex(SystemExit, "the gate failed"):
            self.reserve(self.PARENT, ["the gate failed"])


class PartialSummaryTests(unittest.TestCase):
    run_summary = daily.DailySummaryTests.run_summary
    CHOSEN = ALWAYS + ["book-quickstart"]
    PREFIX = summary.WRITING_REFS[f"refs/heads/{train.RELEASE_BRANCH}"]

    def results(self, **overrides):
        results = {name: {"result": "success" if name in self.CHOSEN or name == "sdk-windows-macos" else "skipped"}
                   for name in daily.LEGS}
        return {**results, **overrides, "resolve": {"outputs": {"legs": json.dumps(self.CHOSEN)}}}

    def jobs(self, failing=""):
        chosen = [daily.job(entry["name"], "failure", ["Run the quickstart"]) if entry["name"] == failing else entry
                  for entry in daily.GREEN_JOBS if entry["name"].partition(" / ")[0] in self.CHOSEN]
        return chosen + [daily.job("shards", "skipped"), daily.job("parity", "skipped")]

    def test_a_break_glass_run_is_judged_on_its_own_legs_and_files_or_closes_nothing(self):
        issue = {"number": 9, "title": summary.leg_title("book-quickstart", self.PREFIX), "body": ""}
        api = daily.Api(self.jobs(), issues=[issue])
        _, out = self.run_summary(api, results=self.results(), ref=f"refs/heads/{train.RELEASE_BRANCH}", parity=None)
        self.assertIn("green=true", out.splitlines())
        self.assertEqual(api.writes, [], "its green closes no issue")
        api = daily.Api(self.jobs("book-quickstart / quickstart"))
        _, out = self.run_summary(api, results=self.results(**{"book-quickstart": {"result": "failure"}}),
                                  ref=f"refs/heads/{train.RELEASE_BRANCH}", parity=None)
        self.assertIn("green=false", out.splitlines())
        self.assertEqual(api.writes, [], "its red files no issue")

    def test_its_sdk_publish_report_writes_nothing(self):
        issue = {"number": 9, "title": summary.leg_title("sdk-publish", self.PREFIX), "body": ""}
        for partial, writes in (("true", 0), ("false", 1)):
            api = daily.Api([], issues=[issue])
            env = {"GITHUB_REPOSITORY": REPO, "GITHUB_RUN_ID": "77", "GITHUB_REF": f"refs/heads/{train.RELEASE_BRANCH}",
                   "GITHUB_SERVER_URL": "https://github.com", "COMMIT": daily.COMMIT, "DRY_RUN": "false",
                   "PARTIAL": partial}
            with self.subTest(partial=partial), mock.patch.dict(os.environ, env, clear=True), \
                    contextlib.redirect_stdout(io.StringIO()):
                summary.after("sdk-publish", "success", api, daily.git((), True), daily.closers({}))
                self.assertEqual(len([write for write in api.writes if write[0] == "PATCH"]), writes)


class ReleaseLineIssueTests(unittest.TestCase):
    run_summary = daily.DailySummaryTests.run_summary

    def test_the_release_lines_reds_file_their_own_issues_and_never_move_the_tag(self):
        red = [daily.job("shards / ignored-tests-shards (sunrise-world)", "failure", ["Run"])] + daily.GREEN_JOBS[2:]
        results = dict(daily.LEGS, shards={"result": "failure"})
        main_issue = {"number": 5, "title": "Daily: sunrise-world a::b", "body": ""}
        api = daily.Api(red, issues=[main_issue])
        self.run_summary(api, [daily.shard_report([daily.failure("a::b")])], ref="refs/heads/release/26.3",
                         results=results)
        created = [body["title"] for method, path, body in api.writes if method == "POST" and path == "issues"]
        self.assertEqual(created, ["Daily (release/26.3): sunrise-world a::b"], "main's open issue is not the line's")
        api = daily.Api(daily.GREEN_JOBS, issues=[main_issue])
        self.run_summary(api, ref="refs/heads/release/26.3")
        self.assertEqual(api.writes, [], "a green release-line run neither moves daily-green nor closes main's issue")


class WorkflowTests(unittest.TestCase):
    def test_nothing_is_built_signed_or_published_before_both_approvals(self):
        jobs = workflow("nightly-build.yml")["jobs"]
        bound = {name: job for name, job in jobs.items() if "break-glass-" in job.get("environment", "")}
        self.assertEqual({job["environment"] for job in bound.values()},
                         {f"${{{{ needs.pick.outputs.publish == 'true' && '{name}' || '' }}}}"
                          for name in train.BREAK_GLASS_APPROVERS})
        self.assertEqual(set(jobs["break-glass"]["needs"]), {"pick", *bound})
        self.assertIn("break-glass-approvals", jobs["break-glass"]["steps"][-1]["run"])
        self.assertIn("needs.pick.result == 'success'", jobs["exporters"]["if"])
        for name in ("exporters", "sign", "publish"):
            with self.subTest(job=name):
                self.assertIn("break-glass", jobs[name]["needs"])
                self.assertIn("needs.break-glass.result == 'success'", jobs[name]["if"])
        for name in ("build", "musl-build"):
            self.assertIn("exporters", jobs[name]["needs"])

    def test_the_surface_is_checked_at_the_pick_and_again_before_the_tag(self):
        jobs = workflow("nightly-build.yml")["jobs"]
        pick = next(step for step in jobs["pick"]["steps"] if "stable-mod-api" in step.get("run", ""))
        self.assertEqual(pick["env"]["BREAK_GLASS"], "true")
        self.assertIn("steps.pick.outputs.break_glass == 'true'", pick["if"])
        publish = jobs["publish"]
        self.assertEqual(publish["steps"][0]["with"]["fetch-depth"], "0")
        check = next(step for step in publish["steps"] if "stable-mod-api" in step.get("run", ""))
        self.assertEqual(check["env"]["BREAK_GLASS"], "${{ needs.pick.outputs.break_glass }}")
        stable = next(step for step in publish["steps"] if step.get("name", "").startswith("Publish as the stable"))
        self.assertEqual(stable["env"]["BREAK_GLASS_APPROVALS"], "${{ needs.break-glass.outputs.note }}")
        self.assertEqual(stable["env"]["PARENT_SHA"], "${{ needs.pick.outputs.parent_sha }}")
        self.assertLess(stable["run"].index("break-glass-notes"), stable["run"].index("reserve-stable-tag"))
        self.assertLess(publish["steps"].index(check), publish["steps"].index(stable))

    def test_the_full_daily_follows_a_published_break_glass_release(self):
        full_daily = workflow("nightly-build.yml")["jobs"]["full-daily"]
        self.assertIn("needs.publish.result == 'success'", full_daily["if"])
        self.assertIn("needs.pick.outputs.break_glass == 'true'", full_daily["if"])
        run = full_daily["steps"][0]["run"]
        self.assertIn('gh workflow run daily.yml', run)
        self.assertIn('-f dry_run=false -f commit="$SHA"', run)
        self.assertNotIn("break_glass", run)

    def test_a_break_glass_daily_is_never_a_full_daily(self):
        daily_yml = workflow("daily.yml")
        jobs = daily_yml["jobs"]
        self.assertIn("needs.resolve.outputs.legs == ''", jobs["green"]["if"])
        self.assertTrue(jobs["legs"]["name"].startswith("legs ${{ needs.resolve.outputs.commit }}"))
        self.assertIn("needs.resolve.outputs.branch == 'release/26.3'", jobs["legs"]["if"])
        self.assertIn("(break-glass legs)", jobs["summary"]["name"])
        for leg in jobs["summary"]["needs"]:
            if leg != "resolve":
                with self.subTest(leg=leg):
                    self.assertIn("needs.resolve.outputs.legs == ''", jobs[leg]["if"])
        report = jobs["sdk-publish-report"]["steps"][-1]["env"]
        self.assertEqual(report["PARTIAL"], "${{ needs.resolve.outputs.legs != '' }}")

    def test_each_release_line_commit_has_its_own_daily_queue(self):
        # A release's follow-up full daily waits for nobody else's and is never replaced by another commit's.
        group = workflow("daily.yml")["concurrency"]["group"]
        self.assertIn("format('-release-{0}', inputs.commit || github.sha)", group)

    def test_the_release_branch_pushes_run_the_gate(self):
        on = workflow("dyna-ci.yml")["on"]
        self.assertEqual(on["push"]["branches"], ["main", train.RELEASE_BRANCH])


if __name__ == "__main__":
    unittest.main()
