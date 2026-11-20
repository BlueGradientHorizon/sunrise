#!/usr/bin/env python3
"""Run one `cargo test -p PACKAGE ...` command and add its tests' verdicts to a daily report.

Usage: cargo_test_verdict.py REPORT -- cargo test -p PACKAGE ...

The command's output is printed as it runs, and its exit status is this script's. Each test that libtest reports
`ok` or `FAILED` goes into REPORT, with the location of the panic that failed it, in the verdict format
tools/ci/ignored_tests.py --report writes. The daily summary (tools/ci/daily_summary.py) can then file a failure
by name instead of filing the whole step as a leg that did not complete. A command that fails without a whole
verdict (no failed test of its own, or a test binary it started that never printed its `test result:`: a build
failure, a crash, a stop partway) also adds an `incomplete` entry, so its unnamed failure is filed as well.
REPORT gathers several commands: each run adds to the file a previous run left. Names and numbers only, never
test output.
"""
import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parents[2]))
from tools.ci.ignored_tests import safe_identity  # noqa: E402

RESULT = re.compile(r"^test (\S+) \.\.\. (ok|FAILED)$")
SECTION = re.compile(r"^---- (\S+) stdout ----$")
PANIC = re.compile(r"^thread '([^']*)'(?: \(\d+\))? panicked at ([\w./-]+\.rs:\d+:\d+)")
STARTED = re.compile(r"^\s*Running ")
SUMMARY = re.compile(r"^test result: ")
ANSI = re.compile(r"\x1b\[[0-9;]*m")
# The variable the fixture job's report is keyed by (daily_summary.report_for).
NEEDS = "SUNRISE_WASM_FIXTURE"


def verdicts(lines, package):
    """(problems, passed) of one command's output, as ignored_tests.py --report writes them."""
    results, panics, section = {}, {}, None
    for line in lines:
        line = ANSI.sub("", line.rstrip("\n"))
        if found := RESULT.match(line):
            results[found[1]] = found[2] == "ok" and results.get(found[1], True)
        elif found := SECTION.match(line):
            section = found[1]
        elif found := PANIC.match(line):
            # The test's own thread is named for it; a thread it started is told by the failure section it is in.
            panics.setdefault(found[1] if found[1] in results or section is None else section, found[2])
    problems = [{"package": package, "test": safe_identity(test), "kind": "failed", "panic": panics.get(test),
                 "policy": None} for test, ok in sorted(results.items()) if not ok]
    passed = [{"package": package, "test": safe_identity(test)} for test, ok in sorted(results.items()) if ok]
    return problems, passed


def unfinished(lines, package, code, problems):
    """The `incomplete` entry for a command that failed without a whole verdict, or None."""
    lines = [ANSI.sub("", line) for line in lines]
    started = sum(bool(STARTED.match(line)) for line in lines)
    finished = sum(bool(SUMMARY.match(line)) for line in lines)
    if code == 0 or (problems and finished >= started):
        return None
    reason = ("a test binary stopped before its summary" if finished < started else
              "failed without a failed test (a build failure)" if not started else "failed without a failed test")
    return {"shard": package, "reason": reason, "cargo_exit": code}


def main(argv):
    if len(argv) < 4 or argv[1] != "--" or "-p" not in argv:
        sys.exit("usage: cargo_test_verdict.py REPORT -- cargo test -p PACKAGE ...")
    report, command = Path(argv[0]), argv[2:]
    package = command[command.index("-p") + 1]
    lines = []
    with subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                          errors="replace") as process:
        for line in process.stdout:
            print(line, end="", flush=True)
            lines.append(line)
    code = process.returncode
    problems, passed = verdicts(lines, package)
    verdict = json.loads(report.read_text()) if report.exists() else {"needs": NEEDS, "problems": [], "passed": [],
                                                                     "incomplete": []}
    verdict["problems"] += problems
    verdict["passed"] += passed
    if stopped := unfinished(lines, package, code, problems):
        verdict["incomplete"].append(stopped)
    report.write_text(json.dumps(verdict, indent=1, sort_keys=True) + "\n")
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
