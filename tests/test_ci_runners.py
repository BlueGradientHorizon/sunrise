"""Which jobs may run on our own runners (docs/ci-runners.md).

- sunrise-light (playsunrise's and other small runners, which refuse a fork's pull request): main's own scripts only.
- sunrise-checks (isolated runners that take any pull request, forks included): no secret and no token that writes.
- Neither takes a job that holds a secret or whose token can push, publish or start workflows: playsunrise is a
  partner's server, and a checks runner keeps what a fork's job left behind.
"""
import json
import unittest
from pathlib import Path

from tests.test_ci_concurrency import yaml

ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github/workflows"
LIGHT = ("sunrise-light", "playsunrise-light", "playsunrise-ci")
CHECKS = ("sunrise-checks",)
FORK_GUARD = "github.event.pull_request.head.repo.full_name == github.repository"
# dyna-ci.yml decides trust once, in its changes job (a fork's pull request, dispatched with `pr` or not).
TRUSTED = "needs.changes.outputs.trusted == 'true'"
TOKEN_WRITE_SCOPES = {"contents", "actions", "packages", "id-token", "deployments", "attestations"}
# actions: write can start workflows too; cancel-on-close needs it to cancel runs and runs no checkout at all.
CANCELS_RUNS = {"cancel-on-close.yml cancel"}


def workflow(path):
    """The parsed workflow, every scalar a string (BaseLoader keeps `on` a key, not True)."""
    return yaml.load(path.read_text(), Loader=yaml.BaseLoader)


def fork_flag_expressions(document):
    """Every `if`, `runs-on` and `env` value of a parsed workflow (the places a job is routed or classified)."""
    def strings(node):
        if isinstance(node, dict):
            for value in node.values():
                yield from strings(value)
        elif isinstance(node, list):
            for value in node:
                yield from strings(value)
        else:
            yield str(node)

    def walk(node):
        if isinstance(node, dict):
            for key, value in node.items():
                if key in ("if", "runs-on", "env"):
                    yield from strings(value)
                else:
                    yield from walk(value)
        elif isinstance(node, list):
            for value in node:
                yield from walk(value)

    return list(walk(document))


def jobs(document):
    """(name, job) for each job of a parsed workflow."""
    return list(document["jobs"].items())


def triggers(document):
    on = document["on"]
    return set(on) if isinstance(on, (dict, list)) else {on}


def permissions(*owners):
    """Every scope a permissions mapping of these workflow or job mappings grants, with its level."""
    return {scope: level for owner in owners for scope, level in owner.get("permissions", {}).items()}


def checkouts(job):
    return [step for step in job.get("steps", []) if step.get("uses", "").startswith("actions/checkout")]


def routed():
    """(where, triggers, workflow, job, runs-on, pools) for every job that can run on our runners."""
    for path in sorted(WORKFLOWS.glob("*.yml")):
        document = workflow(path)
        on = triggers(document)
        for name, job in jobs(document):
            value = job.get("runs-on", "")
            pools = {pool for pool, labels in (("light", LIGHT), ("checks", CHECKS))
                     if any(f'"{label}"' in value for label in labels)}
            if pools:
                yield f"{path.name} {name}", on, document, job, value, pools


class OwnRunnerTests(unittest.TestCase):
    def test_workflows_do_not_classify_pull_requests_by_fork_flag(self):
        for path in sorted(WORKFLOWS.glob("*.y*ml")):
            with self.subTest(workflow=path.name):
                for expression in fork_flag_expressions(yaml.safe_load(path.read_text())):
                    self.assertNotIn("head.repo.fork", expression)

    def test_the_fork_flag_is_found_in_an_if_a_runs_on_and_an_env_value_only(self):
        flagged = "${{ github.event.pull_request.head.repo.fork }}"
        document = yaml.safe_load(
            "jobs:\n  a:\n    if: " + flagged + "\n    runs-on: x\n"
            "  b:\n    runs-on: ${{ " + flagged[3:-3] + " && 'p' || 'q' }}\n"
            "  c:\n    runs-on: x\n    steps:\n      - env:\n          FORK: " + flagged + "\n"
            "        run: echo head.repo.fork\n")
        found = [e for e in fork_flag_expressions(document) if "head.repo.fork" in e]
        self.assertEqual(len(found), 3)

    def test_every_job_on_our_runners_can_go_back_to_github(self):
        count = 0
        for where, _, _, _, value, pools in routed():
            count += 1
            # dyna-ci.yml (ours) routes through the changes job's self_hosted
            # output and dyna-medium, or SELF_HOSTED_CI=all with the fork guard
            # before the playsunrise-ci labels; the carried upstream files use
            # SELF_HOSTED_CI != 'off'. Both fall back to GitHub's runners.
            if "dyna-ci.yml" in where:
                self.assertIn("needs.changes.outputs.self_hosted == 'true'", value, where)
                self.assertIn("vars.SELF_HOSTED_CI == 'all'", value, where)
                self.assertIn(TRUSTED, value, where)
            else:
                self.assertIn("vars.SELF_HOSTED_CI != 'off'", value, where)
            # Both labels select a GitHub-hosted Ubuntu runner when our pools are disabled.
            self.assertRegex(value, r"\|\| 'ubuntu-(?:latest|24\.04)' \}\}$", where)
            if "checks" in pools:
                # The desktop paused: every checks job back to GitHub, the bots stay.
                self.assertIn("vars.SELF_HOSTED_CI != 'light'", value, where)
        self.assertGreaterEqual(count, 10)

    def test_light_runners_take_main_code_only(self):
        for where, on, _, job, value, pools in routed():
            if "light" not in pools:
                continue
            if "pull_request" in on:
                # The guard lives in the job's `if` upstream; dyna-ci.yml (ours)
                # puts it in `runs-on`, before the self-hosted labels.
                condition = job.get("if", "")
                guarded = any(guard in condition or guard in value for guard in (FORK_GUARD, TRUSTED))
                self.assertTrue(guarded, f"{where}: a fork's pull request")
            if "pull_request_target" in on:
                # main's code only: every checkout is main's.
                self.assertEqual(len(checkouts(job)), sum(c.get("with", {}).get("ref") == "main" for c in checkouts(job)), where)

    def test_no_secret_or_writing_token_on_our_runners(self):
        for where, _, document, job, _, pools in routed():
            self.assertNotIn("secrets.", json.dumps(job), where)
            granted = permissions(document, job)
            writes = {scope for scope, level in granted.items() if level == "write" and scope in TOKEN_WRITE_SCOPES}
            if where in CANCELS_RUNS:
                writes -= {"actions"}
                self.assertEqual(checkouts(job), [], where)
            self.assertEqual(writes, set(), where)
            if "checks" in pools:
                # A checks runner keeps what a fork's job left: its jobs' tokens write nothing at all.
                self.assertNotIn("write", granted.values(), where)

    def test_no_deployment_environment_on_our_runners(self):
        for path in sorted(WORKFLOWS.glob("*.y*ml")):
            document = yaml.safe_load(path.read_text())
            for name, job in document["jobs"].items():
                value = job.get("runs-on", "")
                if isinstance(value, dict):
                    value = value.get("labels", [])
                routes = value if isinstance(value, list) else [value]
                if any(route == label or f'"{label}"' in route or f"'{label}'" in route
                       for route in routes for label in LIGHT + CHECKS):
                    self.assertNotIn("environment", job, f"{path.name} {name}")

    def test_deployment_environment_guard_distinguishes_script_variables(self):
        import tempfile
        from unittest.mock import patch

        safe = ("jobs:\n  verify:\n    runs-on: sunrise-checks\n    steps:\n"
                "      - run: |\n          with open('output') as environment:\n              pass\n")
        forbidden = [
            safe.replace("    steps:", f"    {key} deployment\n    steps:")
            for key in ("environment:", "\"environment\":", "'environment' :")
        ] + [
            "jobs:\n    verify:\n        runs-on: sunrise-checks\n        environment: deployment\n",
            "jobs:\n  verify: {runs-on: sunrise-checks, environment: deployment}\n",
            "env: &defaults {environment: deployment}\njobs:\n  verify:\n"
            "    <<: *defaults\n    runs-on: sunrise-checks\n",
        ]
        with tempfile.TemporaryDirectory() as directory, patch(f"{__name__}.WORKFLOWS", Path(directory)):
            workflow = Path(directory) / "fixture.yml"
            workflow.write_text(safe)
            self.test_no_deployment_environment_on_our_runners()
            for text in forbidden:
                with self.subTest(workflow=text):
                    workflow.write_text(text)
                    with self.assertRaises(AssertionError):
                        self.test_no_deployment_environment_on_our_runners()

    def test_checks_verify_runs_on_main_not_on_pull_requests(self):
        document = workflow(WORKFLOWS / "ci.yml")
        # A push gets dyna-checks; a fork's push held GitHub's runners ahead of the release jobs (2026-11-18). A pull
        # request runs verify only when a maintainer labels it, and never with a token that writes.
        self.assertEqual(triggers(document), {"pull_request", "schedule", "workflow_dispatch"})
        self.assertEqual(document["on"]["pull_request"], {"types": ["labeled"]})
        self.assertNotIn("write", permissions(document, *document["jobs"].values()).values())
        ci = dict(jobs(document))
        for part in ("verify-tree", "verify-lint", "verify-test"):
            with self.subTest(part=part):
                self.assertEqual(ci[part]["needs"], "changes")
                self.assertIn("github.event_name == 'schedule' || inputs.verify ||", ci[part]["if"])
                self.assertIn("github.event.label.name == 'ci:verify' &&", ci[part]["if"])
        self.assertEqual(ci["verify"]["needs"], ["verify-tree", "verify-lint", "verify-test"])
        # Never skipped where its parts ran: GitHub counts a skipped required check as passed.
        self.assertTrue(ci["verify"]["if"].startswith("always()"))

    def test_the_light_runners_refuse_forks_themselves(self):
        script = (ROOT / "tools/ci/checks-runners.sh").read_text()
        self.assertIn("ACTIONS_RUNNER_HOOK_JOB_STARTED=$shared/job-started.sh", script)
        self.assertIn('.pull_request.head.repo.full_name // "unknown"', script)
        self.assertIn('if [ "$head" != "${GITHUB_REPOSITORY:-}" ]', script)


if __name__ == "__main__":
    unittest.main()
