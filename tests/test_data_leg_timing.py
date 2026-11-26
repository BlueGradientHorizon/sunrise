"""Safe phase telemetry must not expose captured listing or test payloads."""
import contextlib
import io
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
from threading import Event, Thread
import unittest
from unittest import mock

from tests.os_support import needs_runner_shell
from tools.ci import cgroup_telemetry, ignored_tests, vendored_yaml

ROOT = Path(__file__).resolve().parents[1]
yaml = vendored_yaml.load()


def data_leg_steps():
    return yaml.safe_load((ROOT / ".github/workflows/ignored-tests.yml").read_text())["jobs"]["ignored-tests-shards"]["steps"]


def fake_cargo(calls, raw_only=False):
    """A `cargo test` stand-in: it lists eight tests, and runs the ones not skipped; other commands run for real."""
    names = ["safe::case_" + str(n) for n in range(8)]
    real = subprocess.run

    def run(command, **kwargs):
        if "cargo" not in command:
            return real(command, **kwargs)
        args = command[command.index("cargo") + 1:]
        calls.append(args)
        if "--list" in args:
            return subprocess.CompletedProcess(command, 0, "".join(name + ": test\n" for name in names),
                                               "PRIVATE LISTING PAYLOAD\n")
        if raw_only:
            return subprocess.CompletedProcess(command, 0, "PRIVATE TEST PAYLOAD\n", "")
        skipped = {args[i + 1] for i, arg in enumerate(args) if arg == "--skip"}
        selected = [name for name in names if name not in skipped]
        lines = ["     Running unittests src/lib.rs (target/debug/deps/example)", f"running {len(selected)} tests",
                 "PRIVATE TEST PAYLOAD", *(f"test {name} ... ok" for name in selected),
                 f"test result: ok. {len(selected)} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s"]
        return subprocess.CompletedProcess(command, 0, "\n".join(lines) + "\n", "")
    return run


class DataLegTimingTests(unittest.TestCase):
    def test_workflow_records_start_end_without_bypassing_filtered_runner(self):
        steps = data_leg_steps()
        start = next(i for i, s in enumerate(steps) if s.get("name") == "Record data-leg start and compiler cache")
        end = next(i for i, s in enumerate(steps) if s.get("name") == "Record data-leg end and compiler cache")
        run = next(i for i, s in enumerate(steps) if s.get("name") == "Run the ignored tests that need prepared data")
        self.assertLess(start, run)
        self.assertLess(run, end)
        self.assertEqual(steps[end]["if"], "always()")
        self.assertEqual(steps[run]["env"]["SUNRISE_DATA_LEG_TIMING"], "1")
        # The workflow may invoke only the existing filtered runner, not raw cargo
        # or a tee/cat pipeline forwarding its captured output.
        lines = steps[run]["run"].strip().splitlines()
        self.assertEqual(len(lines), 2)
        self.assertTrue(lines[0].startswith("export SUNRISE_TEST_RESOURCES="))
        self.assertEqual(shlex.split(lines[1]), ["python3", "tools/ci/with_xvfb.py", "python3", "tools/ci/ignored_tests.py", "--report", "$RUNNER_TEMP/daily-report.json", "$SHARD"])

    @needs_runner_shell
    def test_the_start_and_end_steps_print_cache_statistics_and_job_cpu_at_end(self):
        steps = data_leg_steps()
        start = next(i for i, s in enumerate(steps) if s.get("name") == "Record data-leg start and compiler cache")
        end = next(i for i, s in enumerate(steps) if s.get("name") == "Record data-leg end and compiler cache")
        with tempfile.TemporaryDirectory() as directory:
            fake = Path(directory) / "sccache"
            fake.write_text("#!/bin/sh\n[ \"$1\" = --show-stats ] || exit 1\necho 'Compile requests 0'\necho 'Cache hits 0'\n")
            fake.chmod(0o755)
            (Path(directory) / "cpu.stat").write_bytes(
                b"usage_usec 100\nuser_usec 70\nsystem_usec PRIVATE\nnr_throttled 2\nthrottled_usec 25001\nother 999\n")
            for i, label in ((start, "start"), (end, "end")):
                script = steps[i]["run"].replace("/sys/fs/cgroup/", Path(directory).as_posix() + "/")
                script = script.replace("python3 ", shlex.quote(Path(sys.executable).as_posix()) + " ")
                result = subprocess.run(["bash", "-e", "-c", script], cwd=ROOT,
                                        env={**os.environ, "PATH": directory + os.pathsep + os.environ["PATH"]},
                                        text=True, capture_output=True, check=True)
                self.assertRegex(result.stdout, rf"data-leg {label} utc=\d{{4}}-\d{{2}}-\d{{2}}T")
                self.assertIn("Compile requests 0", result.stdout)
                cpu = [line for line in result.stdout.splitlines() if line.startswith("data-leg cgroup-cpu ")]
                self.assertEqual(cpu, [] if i == start else [
                    "data-leg cgroup-cpu scope=whole_job_cgroup_cumulative usage_usec=100 user_usec=70 "
                    "system_usec=unavailable nr_periods=unavailable nr_throttled=2 throttled_usec=25001"])
                self.assertNotIn(directory, result.stdout + result.stderr)
                self.assertNotIn("PRIVATE", result.stdout + result.stderr)

    def test_real_cargo_boundary_markers_keep_payloads_private(self):
        calls = []
        with mock.patch.dict(os.environ, {"SUNRISE_DATA_LEG_TIMING": "1"}), \
                mock.patch.object(subprocess, "run", fake_cargo(calls)), \
                contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()) as err:
            self.assertEqual(ignored_tests.main(["example#1of2"]), 0)
        self.assertNotIn("PRIVATE LISTING PAYLOAD", out.getvalue() + err.getvalue())
        self.assertNotIn("PRIVATE TEST PAYLOAD", out.getvalue() + err.getvalue())
        markers = [line for line in out.getvalue().splitlines() if line.startswith("data-leg phase=")]
        self.assertEqual(len(markers), 3)
        for line, phase in zip(markers, ("compile-list-start", "test-command-start", "test-command-end")):
            self.assertRegex(line, rf"^data-leg phase=example#1of2 {phase} utc=\d{{4}}-\d{{2}}-\d{{2}}T")
        self.assertRegex(markers[1], r"compile_list_seconds=\d+\.\d{3}$")
        self.assertRegex(markers[2], r"execution_seconds=\d+\.\d{3}$")
        self.assertNotIn("data-leg cgroup-phase ", out.getvalue())
        self.assertEqual(len(calls), 2, "telemetry must not add cargo invocations")
        self.assertIn("--list", calls[0])
        self.assertNotIn("--list", calls[1])
        self.assertIn("--include-ignored", calls[1])
        self.assertIn("--exact", calls[1])
        self.assertIn("--test-threads=4", calls[1])
        with mock.patch.dict(os.environ, {"SUNRISE_DATA_LEG_TIMING": "1"}), \
                mock.patch.object(subprocess, "run", fake_cargo([], raw_only=True)), \
                contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()) as err:
            self.assertNotEqual(ignored_tests.main(["example"]), 0)
        self.assertNotIn("PRIVATE TEST PAYLOAD", out.getvalue() + err.getvalue())
        self.assertIn("raw output withheld", out.getvalue())

    def test_client_half_samples_each_existing_batch_command_without_more_invocations(self):
        calls = []
        cargo = fake_cargo(calls)
        with tempfile.TemporaryDirectory() as directory:
            cgroup = Path(directory)
            cpu = cgroup / "cpu.stat"
            cpu.write_text("usage_usec 100\n")

            def execute(command, **kwargs):
                self.assertIn("cargo", command)
                increment = 10 if "--list" in command else 20
                usage = int(cpu.read_text().split()[1]) + increment
                cpu.write_text(f"usage_usec {usage}\n")
                return cargo(command, **kwargs)

            with mock.patch.dict(os.environ, {"SUNRISE_DATA_LEG_TIMING": "1", "CI_CPUS": "16", "CI_JOBS": "10"}), \
                    mock.patch.object(ignored_tests, "CGROUP", cgroup), \
                    mock.patch.object(ignored_tests, "listed_entries", return_value=[]), \
                    mock.patch.object(subprocess, "run", side_effect=execute), \
                    contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()) as err:
                self.assertEqual(ignored_tests.main(["sunrise-client#1of2"]), 0)
        log = out.getvalue() + err.getvalue()
        self.assertNotIn("PRIVATE", log)
        self.assertNotIn(directory, log)
        self.assertEqual(len(calls), 8)
        for listing, testing in zip(calls[::2], calls[1::2]):
            self.assertEqual(listing, ["test", "-p", "sunrise-client", "--lib", "--bins", "--tests",
                                       "--no-fail-fast", "--jobs=10", "--", "--list", "--include-ignored"])
            self.assertEqual(testing[:12], listing[:9] + ["--include-ignored", "--exact", "--test-threads=8"])
        metrics = [dict(field.split("=") for field in line.split()[2:])
                   for line in log.splitlines() if line.startswith("data-leg cgroup-phase ")]
        self.assertEqual([(row["batch"], row["phase"], row["cpu_usage_usec_delta"]) for row in metrics], [
            (f"sunrise-client#{number}of8", phase, delta)
            for number in (1, 3, 5, 7) for phase, delta in (("compile-list", "10"), ("test-command", "20"))])
        for row in metrics:
            self.assertEqual(row["scope"], "job_cgroup_during_command")
            for field in ("current", "anon", "file"):
                self.assertEqual(row[f"memory_{field}_observed_max_bytes"], "unavailable")

    def test_sampler_collects_independent_maxima_and_joins_when_the_command_raises(self):
        stop, interior_sample = Event(), Event()
        wakeup = mock.Mock(wraps=stop)
        workers = []
        with tempfile.TemporaryDirectory() as directory:
            cgroup = Path(directory)
            cpu = cgroup / "cpu.stat"
            current = cgroup / "memory.current"
            stat = cgroup / "memory.stat"
            cpu.write_text("usage_usec 45\nuser_usec 12\nsystem_usec 1\nnr_periods 2\nnr_throttled 0\nthrottled_usec 10\n")
            current.write_text("100\n")
            stat.write_text("anon 60\n")

            def tick(_seconds):
                if wakeup.wait.call_count == 1:
                    current.write_text("200\n")
                    stat.write_text("anon 50\nfile 120\nanon_thp 999\n")
                    return False
                interior_sample.set()
                return stop.wait()

            def new_thread(target):
                workers.append(mock.Mock(wraps=Thread(target=target, daemon=True)))
                return workers[-1]

            wakeup.wait.side_effect = tick
            with mock.patch.object(cgroup_telemetry, "Event", return_value=wakeup), \
                    mock.patch.object(cgroup_telemetry, "Thread", side_effect=new_thread), \
                    contextlib.redirect_stdout(io.StringIO()) as out:
                error = OSError("PRIVATE command error")
                with self.assertRaises(OSError) as raised:
                    with cgroup_telemetry.sample_command(cgroup, "sunrise-client#3of8", "test-command"):
                        self.assertTrue(interior_sample.wait(timeout=2))
                        cpu.write_bytes(b"usage_usec 70\nuser_usec 11\nnr_periods 3\nnr_throttled 0\nthrottled_usec \xff\n")
                        current.write_text("150\n")
                        stat.write_text("anon 90\nfile 30\n")
                        raise error
        self.assertIs(raised.exception, error)
        wakeup.set.assert_called_once_with()
        wakeup.wait.assert_has_calls([mock.call(1), mock.call(1)])
        self.assertEqual(len(workers), 1)
        workers[0].join.assert_called_once_with()
        self.assertFalse(workers[0].is_alive())
        self.assertEqual(out.getvalue().splitlines(), [
            "data-leg cgroup-phase batch=sunrise-client#3of8 phase=test-command scope=job_cgroup_during_command "
            "memory_sample_attempts=3 sample_interval_seconds=1 memory_current_observed_max_bytes=200 "
            "memory_anon_observed_max_bytes=90 memory_file_observed_max_bytes=120 cpu_usage_usec_delta=25 "
            "cpu_user_usec_delta=unavailable cpu_system_usec_delta=unavailable cpu_nr_periods_delta=1 "
            "cpu_nr_throttled_delta=0 cpu_throttled_usec_delta=unavailable"])


if __name__ == "__main__":
    unittest.main()
