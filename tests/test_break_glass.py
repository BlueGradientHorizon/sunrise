"""Break-glass stable patch releases (docs/release-flow.md "Break-glass patch releases"): the legs a patch's changes
select, the rules never waived, the two approvals, and how the workflows hold to them."""
import io
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tests import test_daily_summary as daily
from tests.test_release_branches import REPO, daily_run, daily_runs, green_jobs, git_in, workflow
from tools import release_train as train
from tools.ci import nightly

ALWAYS = sorted(train.BREAK_GLASS_ALWAYS)


class LegsTests(unittest.TestCase):
    def test_each_path_selects_its_legs_and_an_unmapped_path_needs_the_full_daily(self):
        for paths, legs in (
                (["docs/modding/src/quickstart.md"], ALWAYS + ["book-quickstart"]),
                (["tools/book_quickstart.py"], ALWAYS + ["book-quickstart"]),
                (["docs/hosting.md"], ALWAYS),
                (["crates/sunrise-world/src/lib.rs", "Cargo.lock"], ALWAYS + ["cargo-sunrise-mod"]),
                ([], ALWAYS)):
            with self.subTest(paths=paths):
                self.assertEqual(train.break_glass_legs(paths), (sorted(legs), []))
        for path in (".github/workflows/daily.yml", "tools/ci/nightly.py", "packaging/server/install.sh", "docs",
                     "cratesx/a.rs", "tools/book_quickstart.py.bak", "README.md"):
            with self.subTest(path=path):
                self.assertEqual(train.break_glass_legs(["docs/hosting.md", path])[1], [path])

    def test_every_selected_leg_is_a_daily_leg(self):
        legs = set(train.BREAK_GLASS_ALWAYS).union(*(selected for _, selected in train.BREAK_GLASS_LEGS))
        self.assertLessEqual(legs, set(workflow("daily.yml")["jobs"]["summary"]["needs"]))


class ProblemsTests(unittest.TestCase):
    """break_glass_problems on a real history: 26.3-0.1.0 at `parent`, the cherry-picks above it."""

    def history(self, folder, *paths):
        repo = Path(folder)
        git = git_in(repo)
        git("init", "-q", "-b", "release")
        git("config", "user.name", "Test")
        git("config", "user.email", "test@example.invalid")
        git("commit", "-q", "--allow-empty", "-m", "0.1.0")
        parent = git("rev-parse", "HEAD")
        for path in paths:
            (repo / path).parent.mkdir(parents=True, exist_ok=True)
            (repo / path).write_text("fix\n")
            git("add", path)
        git("commit", "-q", "--allow-empty", "-m", "cherry-pick")
        return parent, git("rev-parse", "HEAD")

    def problems(self, folder, sha, tags, tag="26.3-0.1.1", legs="url", parent_daily="url", gate=True):
        with mock.patch.object(train, "ROOT", Path(folder)), \
                mock.patch.object(train, "legs_passed", return_value=legs) as passed, \
                mock.patch.object(train, "newest_daily_green", return_value=parent_daily), \
                mock.patch.object(train, "fork_gate_passed", return_value=gate) as gated:
            found = train.break_glass_problems(sha, tag, tags)
        if gated.called:
            gated.assert_called_once_with(sha, train.RELEASE_BRANCH)
        return found, passed

    def test_a_patch_whose_legs_gate_and_parent_daily_passed_may_ship(self):
        with tempfile.TemporaryDirectory() as folder:
            parent, sha = self.history(folder, "docs/modding/src/a.md")
            tags = {"26.3-0.1.0": parent, "nightly": "1" * 40}
            (problems, found, legs), passed = self.problems(folder, sha, tags)
        self.assertEqual((problems, found, legs), ([], ("26.3-0.1.0", parent), sorted(ALWAYS + ["book-quickstart"])))
        passed.assert_called_once_with(sha, legs)

    def test_each_rule_refuses_on_its_own(self):
        with tempfile.TemporaryDirectory() as folder:
            parent, sha = self.history(folder, "crates/sunrise-world/src/lib.rs")
            tags = {"26.3-0.1.0": parent}
            for change, reason in (({"tag": "26.3-0.2.0"}, "is not a patch release of 26.3-0.1.0"),
                                   ({"tag": "26.3-1.0.0"}, "is not a patch release of 26.3-0.1.0"),
                                   ({"legs": ""}, "has no real daily run that passed the legs"),
                                   ({"parent_daily": ""}, "the newest full daily run of 26.3-0.1.0"),
                                   ({"gate": False}, "gate jobs (lint, verify, verify-static) did not all pass")):
                with self.subTest(change=change):
                    (problems, _, _), _ = self.problems(folder, sha, tags, **change)
                    self.assertEqual(len(problems), 1, problems)
                    self.assertIn(reason, problems[0])

    def test_an_unmapped_path_a_foreign_parent_or_no_parent_refuses(self):
        with tempfile.TemporaryDirectory() as folder:
            parent, sha = self.history(folder, "docs/a.md", ".github/workflows/release.yml")
            (problems, _, _), passed = self.problems(folder, sha, {"26.3-0.1.0": parent})
            self.assertIn("`.github/workflows/release.yml`", problems[0])
            passed.assert_not_called()
            other = git_in(Path(folder))("commit-tree", "-m", "elsewhere", "4b825dc642cb6eb9a060e54bf8d69288fbee4904")
            (problems, _, _), _ = self.problems(folder, sha, {"26.3-0.1.0": other})
            self.assertIn("does not descend from 26.3-0.1.0", problems[0])
            (problems, found, legs), _ = self.problems(folder, sha, {"26.4-0.1.0": parent}, tag="26.3-0.1.0")
            self.assertEqual((found, legs), (None, []))
            self.assertIn("no earlier stable release on 26.3", problems[0])

    def test_the_parent_is_the_newest_stable_release_on_the_target(self):
        tags = {"26.3-0.1.0": "a", "26.3-0.1.2": "b", "26.3-0.1.10": "c", "26.4-0.5.0": "d", "nightly": "e"}
        self.assertEqual(train.stable_parent(tags, own="26.3-0.1.11"), ("26.3-0.1.10", "c"))
        self.assertEqual(train.stable_parent(tags, own="26.3-0.1.10"), ("26.3-0.1.2", "b"))
        self.assertIsNone(train.stable_parent({"nightly": "e"}))


def legs_job(entry, sha, legs, conclusion="success"):
    return [{"name": f"legs {sha} {' '.join(legs)}", "status": "completed", "conclusion": conclusion,
             "run_id": entry["id"], "run_attempt": entry["run_attempt"]}]


class EvidenceTests(unittest.TestCase):
    SHA = "c" * 40

    def answer(self, function, *runs, **kwargs):
        with mock.patch.object(train, "gh_json", side_effect=daily_runs(*runs)), mock.patch.object(train, "REPO", REPO):
            return function(self.SHA, **kwargs)

    def test_a_break_glass_run_counts_for_the_legs_it_passed_and_no_fewer(self):
        legs = ["fn-replace-check", "machine-sample", "sdk"]
        run = daily_run(1, self.SHA, branch=train.RELEASE_BRANCH, title=f"daily {self.SHA}{train.PARTIAL_DAILY}")
        url = f"https://github.com/{REPO}/actions/runs/1"
        self.assertEqual(self.answer(train.legs_passed, (run, legs_job(run, self.SHA, legs)), legs=legs), url)
        self.assertEqual(self.answer(train.legs_passed, (run, legs_job(run, self.SHA, legs + ["shards"])), legs=legs), url)
        for jobs in (legs_job(run, self.SHA, legs[:2]), legs_job(run, self.SHA, legs, "failure"),
                     legs_job(run, "d" * 40, legs)):
            with self.subTest(jobs=jobs):
                self.assertEqual(self.answer(train.legs_passed, (run, jobs), legs=legs), "")
        dry = daily_run(2, self.SHA, title=f"daily {self.SHA}{train.PARTIAL_DAILY} (dry run)")
        self.assertEqual(self.answer(train.legs_passed, (dry, legs_job(dry, self.SHA, legs)), legs=legs), "")
        full = daily_run(3, self.SHA)
        self.assertEqual(self.answer(train.legs_passed, (full, green_jobs(full)), legs=legs),
                         f"https://github.com/{REPO}/actions/runs/3", "a full passed daily covers every leg")

    def test_the_parents_newest_full_daily_decides_and_a_break_glass_or_cancelled_run_is_none(self):
        green, red = daily_run(1, self.SHA), daily_run(2, self.SHA, conclusion="failure")
        partial = daily_run(3, self.SHA, branch=train.RELEASE_BRANCH, title=f"daily {self.SHA}{train.PARTIAL_DAILY}")
        cancelled = daily_run(4, self.SHA, conclusion="cancelled")
        self.assertTrue(self.answer(train.newest_daily_green, (cancelled, []), (partial, []), (green, green_jobs(green))))
        self.assertEqual(self.answer(train.newest_daily_green, (red, green_jobs(red)), (green, green_jobs(green))), "",
                         "a red after the green: the follow-up daily of a break-glass release failed")
        self.assertEqual(self.answer(train.newest_daily_green), "", "no daily run at all")


class ApprovalTests(unittest.TestCase):
    def approvals(self, *entries):
        with mock.patch.object(train, "gh_json", return_value=list(entries)) as call:
            lines = train.break_glass_approvals("55")
        call.assert_called_once_with("api", f"repos/{train.REPO}/actions/runs/55/approvals")
        return lines

    @staticmethod
    def approval(login, *environments, state="approved", comment="the save fix"):
        return {"state": state, "comment": comment, "user": {"login": login},
                "environments": [{"name": name} for name in environments]}

    def test_both_environments_approved_by_two_people(self):
        first, second = train.BREAK_GLASS_ENVIRONMENTS
        self.assertEqual(self.approvals(self.approval("UMCEKO", first), self.approval("arismoko", second, comment=None)),
                         [f"@UMCEKO ({first}): the save fix", f"@arismoko ({second}): no comment"])

    def test_a_missing_rejected_or_one_person_approval_refuses(self):
        first, second = train.BREAK_GLASS_ENVIRONMENTS
        for entries, reason in (([self.approval("UMCEKO", first)], f"no approval recorded for {second}"),
                                ([], "no approval recorded"),
                                ([self.approval("UMCEKO", first), self.approval("arismoko", second, state="rejected")],
                                 f"no approval recorded for {second}"),
                                ([self.approval("UMCEKO", first, second)], "different people"),
                                ([self.approval("UMCEKO", first), self.approval("UMCEKO", second)], "different people")):
            with self.subTest(entries=entries), self.assertRaisesRegex(SystemExit, reason):
                self.approvals(*entries)


class PartialSummaryTests(unittest.TestCase):
    run_summary = daily.DailySummaryTests.run_summary
    CHOSEN = ALWAYS + ["book-quickstart"]

    def results(self, **overrides):
        results = {name: {"result": "success" if name in self.CHOSEN or name == "sdk-windows-macos" else "skipped"}
                   for name in daily.LEGS}
        return {**results, **overrides, "resolve": {"outputs": {"legs": json.dumps(self.CHOSEN)}}}

    def jobs(self):
        return [entry for entry in daily.GREEN_JOBS if entry["name"].partition(" / ")[0] in self.CHOSEN] + [
            daily.job("shards", "skipped"), daily.job("parity", "skipped")]

    def test_a_break_glass_run_is_judged_on_its_own_legs_and_writes_nothing(self):
        api = daily.Api(self.jobs())
        _, out = self.run_summary(api, results=self.results(), ref="refs/heads/release/26.3", parity=None)
        self.assertIn("green=true", out.splitlines())
        self.assertEqual(api.writes, [])
        jobs = [daily.job("book-quickstart / quickstart", "failure", ["Run the quickstart"]) if entry["name"] ==
                "book-quickstart / quickstart" else entry for entry in self.jobs()]
        _, out = self.run_summary(daily.Api(jobs), results=self.results(**{"book-quickstart": {"result": "failure"}}),
                                  ref="refs/heads/release/26.3", parity=None)
        self.assertIn("green=false", out.splitlines())


class WorkflowTests(unittest.TestCase):
    def test_the_build_waits_for_both_environments_and_their_recorded_approvals(self):
        jobs = workflow("nightly-build.yml")["jobs"]
        bound = {name: job for name, job in jobs.items() if "break-glass-" in job.get("environment", "")}
        self.assertEqual({job["environment"] for job in bound.values()},
                         {f"${{{{ needs.pick.outputs.publish == 'true' && '{name}' || '' }}}}"
                          for name in train.BREAK_GLASS_ENVIRONMENTS})
        self.assertEqual(set(jobs["break-glass"]["needs"]), {"pick", *bound})
        self.assertIn("break-glass-approvals", jobs["break-glass"]["steps"][-1]["run"])
        self.assertIn("needs.break-glass.result == 'success'", jobs["exporters"]["if"])
        self.assertIn("break-glass", jobs["exporters"]["needs"])
        for name in ("build", "musl-build"):
            self.assertIn("exporters", jobs[name]["needs"])
        publish = next(step for step in jobs["publish"]["steps"] if step.get("name", "").startswith("Publish as the stable"))
        self.assertEqual(publish["env"]["APPROVALS"], "${{ needs.break-glass.outputs.note }}")
        self.assertIn(nightly.BREAK_GLASS_APPROVALS, publish["run"])

    def test_the_full_daily_follows_a_published_break_glass_release(self):
        full = workflow("nightly-build.yml")["jobs"]["full-daily"]
        self.assertIn("needs.publish.result == 'success'", full["if"])
        self.assertIn("needs.pick.outputs.break_glass == 'true'", full["if"])
        run = full["steps"][0]["run"]
        self.assertIn('gh workflow run daily.yml', run)
        self.assertIn('-f dry_run=false -f commit="$SHA"', run)
        self.assertNotIn("break_glass", run)

    def test_a_break_glass_daily_is_never_a_full_daily(self):
        jobs = workflow("daily.yml")["jobs"]
        self.assertIn("needs.resolve.outputs.legs == ''", jobs["green"]["if"])
        self.assertTrue(jobs["legs"]["name"].startswith("legs ${{ needs.resolve.outputs.commit }} "))
        self.assertIn("needs.resolve.outputs.branch == 'release/26.3'", jobs["legs"]["if"])
        self.assertIn(train.PARTIAL_DAILY, workflow("daily.yml")["run-name"])
        for leg in jobs["summary"]["needs"]:
            if leg != "resolve":
                with self.subTest(leg=leg):
                    self.assertIn("needs.resolve.outputs.legs == ''", jobs[leg]["if"])

    def test_the_release_branch_pushes_run_the_gate(self):
        on = workflow("dyna-ci.yml")["on"]
        self.assertEqual(on["push"]["branches"], ["main", train.RELEASE_BRANCH])


if __name__ == "__main__":
    unittest.main()
