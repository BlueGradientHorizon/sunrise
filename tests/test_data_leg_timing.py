"""Safe phase telemetry must not expose captured listing or test payloads."""
import contextlib
import io
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import sys
import tempfile
from threading import Event, Semaphore, Thread
import unittest
from unittest import mock

from tests.os_support import needs_runner_shell
from tools.ci import cgroup_telemetry, ignored_tests, vendored_yaml

ROOT = Path(__file__).resolve().parents[1]
yaml = vendored_yaml.load()
NO_JOB_CPU = ("data-leg cgroup-cpu scope=whole_job_cgroup_cumulative usage_usec=unavailable user_usec=unavailable "
              "system_usec=unavailable nr_periods=unavailable nr_throttled=unavailable throttled_usec=unavailable")


def data_leg_steps():
    return yaml.safe_load((ROOT / ".github/workflows/ignored-tests.yml").read_text())["jobs"]["ignored-tests-shards"]["steps"]


def cache_step(label, cgroup):
    step = next(s for s in data_leg_steps() if s.get("name") == f"Record data-leg {label} and compiler cache")
    script = step["run"].replace("/sys/fs/cgroup/", cgroup.as_posix() + "/")
    script = script.replace("python3 ", shlex.quote(Path(sys.executable).as_posix()) + " ")
    fake = cgroup / "sccache"
    fake.write_text("#!/bin/sh\n[ \"$1\" = --show-stats ] || exit 1\necho 'Compile requests 0'\necho 'Cache hits 0'\n")
    fake.chmod(0o755)
    return ["bash", "-e", "-c", script], {**os.environ, "PATH": str(cgroup) + os.pathsep + os.environ["PATH"]}


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
        with tempfile.TemporaryDirectory() as directory:
            cgroup = Path(directory)
            cpu = cgroup / "cpu.stat"
            cpu.write_bytes(b"usage_usec 100\nuser_usec 70\nsystem_usec PRIVATE\nnr_throttled 2\nthrottled_usec 25001\nother 999\n")
            readable = ("data-leg cgroup-cpu scope=whole_job_cgroup_cumulative usage_usec=100 user_usec=70 "
                        "system_usec=unavailable nr_periods=unavailable nr_throttled=2 throttled_usec=25001")
            for label, expected in (("start", []), ("end", [readable]), ("end", [NO_JOB_CPU])):
                with self.subTest(label=label, counter_present=cpu.exists()):
                    command, env = cache_step(label, cgroup)
                    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True,
                                            check=True, timeout=5)
                    self.assertRegex(result.stdout, rf"data-leg {label} utc=\d{{4}}-\d{{2}}-\d{{2}}T")
                    self.assertIn("Compile requests 0", result.stdout)
                    self.assertEqual([line for line in result.stdout.splitlines()
                                      if line.startswith("data-leg cgroup-cpu ")], expected)
                    self.assertNotIn(directory, result.stdout + result.stderr)
                    self.assertNotIn("PRIVATE", result.stdout + result.stderr)
                if expected == [readable]:
                    cpu.unlink()

    @needs_runner_shell
    @unittest.skipUnless(hasattr(os, "mkfifo") and hasattr(os, "killpg"), "needs a POSIX FIFO and process group")
    def test_the_end_step_reports_cache_and_exits_with_a_stalled_cpu_read(self):
        with tempfile.TemporaryDirectory() as directory:
            cgroup = Path(directory)
            cpu = cgroup / "cpu.stat"
            os.mkfifo(cpu)
            # Keep a writer open without sending data: the real reader blocks in read, not on EOF.
            writer = os.open(cpu, os.O_RDWR | os.O_NONBLOCK)
            try:
                command, env = cache_step("end", cgroup)
                with subprocess.Popen(command, cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE,
                                      stderr=subprocess.PIPE, start_new_session=True) as step:
                    try:
                        stdout, stderr = step.communicate(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(step.pid, signal.SIGKILL)
                        raise
                self.assertEqual(step.returncode, 0, stderr)
            finally:
                os.close(writer)
        self.assertEqual(stdout.splitlines()[-3:], ["Compile requests 0", "Cache hits 0", NO_JOB_CPU])
        self.assertEqual(stderr, "")

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
        with tempfile.TemporaryDirectory() as directory:
            cgroup = Path(directory)
            report_path = cgroup / "report.json"
            with mock.patch.dict(os.environ, {"SUNRISE_DATA_LEG_TIMING": "1", "CI_CPUS": "16", "CI_JOBS": "10"}), \
                    mock.patch.object(ignored_tests, "CGROUP", cgroup), \
                    mock.patch.object(ignored_tests, "listed_entries", return_value=[]), \
                    mock.patch.object(subprocess, "run", side_effect=fake_cargo(calls)), \
                    contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()) as err:
                self.assertEqual(ignored_tests.main(["sunrise-client#1of2"], report_path), 0)
            report = json.loads(report_path.read_text())
        self.assertEqual(set(report), {"shards", "problems", "passed", "incomplete"})
        self.assertEqual(report["problems"], [])
        self.assertEqual(report["incomplete"], [])
        self.assertEqual(len(report["passed"]), 4)
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
            (f"sunrise-client#{number}of8", phase, "unavailable")
            for number in (1, 3, 5, 7) for phase in ("compile-list", "test-command")])
        for row in metrics:
            self.assertEqual(row["scope"], "job_cgroup_during_command")
            for field in ("current", "anon", "file"):
                self.assertEqual(row[f"memory_{field}_observed_max_bytes"], "unavailable")

    def test_complete_snapshots_keep_independent_maxima_cpu_deltas_and_coverage(self):
        stop, sampled = Event(), Event()
        wakeup = mock.Mock(wraps=stop)
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
                    cpu.write_text("usage_usec 53\nuser_usec 15\nsystem_usec 2\n")
                    current.write_text("200\n")
                    stat.write_text("anon 50\nfile 120\nanon_thp 999\n")
                elif wakeup.wait.call_count == 2:
                    cpu.write_bytes(b"usage_usec 70\nuser_usec 11\nnr_periods 3\nnr_throttled 0\nthrottled_usec \xff\n")
                    current.write_text("150\n")
                    stat.write_text("anon 90\nfile 30\n")
                else:
                    sampled.set()
                    stop.wait(timeout=5)
                    return True
                return False

            wakeup.wait.side_effect = tick
            with mock.patch.object(cgroup_telemetry, "Event", return_value=wakeup), \
                    mock.patch.object(cgroup_telemetry, "monotonic_ns", side_effect=[0, 1_200_000_000, 2_500_000_000]), \
                    contextlib.redirect_stdout(io.StringIO()) as out:
                with cgroup_telemetry.sample_command(cgroup, "sunrise-client#3of8", "test-command"):
                    self.assertTrue(sampled.wait(timeout=5))
        self.assertEqual(out.getvalue().splitlines(), [
            "data-leg cgroup-phase batch=sunrise-client#3of8 phase=test-command scope=job_cgroup_during_command "
            "memory_sample_attempts=3 sample_interval_seconds=1 cpu_sample_span_ms=2500 memory_current_observed_max_bytes=200 "
            "memory_anon_observed_max_bytes=90 memory_file_observed_max_bytes=120 cpu_usage_usec_delta=25 "
            "cpu_user_usec_delta=unavailable cpu_system_usec_delta=unavailable cpu_nr_periods_delta=1 "
            "cpu_nr_throttled_delta=0 cpu_throttled_usec_delta=unavailable"])

    def test_stalled_phase_reads_do_not_delay_cargo_result_or_exception(self):
        for raises in (False, True):
            with self.subTest(raises=raises), tempfile.TemporaryDirectory() as directory:
                entered, release = Semaphore(0), Event()
                workers, stops, outcome, calls = [], [], [], []
                cargo = fake_cargo(calls)
                error = OSError("PRIVATE command error")

                def stalled_read(_path):
                    entered.release()
                    release.wait()
                    return b"anon 123456789\nPRIVATE counter payload\n"

                def execute(command, **kwargs):
                    self.assertTrue(entered.acquire(timeout=5), "each command should see an in-flight read")
                    if raises and "--list" not in command:
                        raise error
                    return cargo(command, **kwargs)

                def run():
                    try:
                        outcome.append(ignored_tests.run_leg("sunrise-client", [], 1, 2))
                    except BaseException as exc:
                        outcome.append(exc)

                def new_stop():
                    stops.append(Event())
                    return stops[-1]

                def track_worker(**kwargs):
                    workers.append(Thread(**kwargs))
                    return workers[-1]

                driver = Thread(target=run, daemon=True)
                with mock.patch.dict(os.environ, {"SUNRISE_DATA_LEG_TIMING": "1"}), \
                        mock.patch.object(ignored_tests, "CGROUP", Path(directory)), \
                        mock.patch.object(subprocess, "run", side_effect=execute), \
                        mock.patch.object(cgroup_telemetry, "_read", side_effect=stalled_read), \
                        mock.patch.object(cgroup_telemetry, "Event", side_effect=new_stop), \
                        mock.patch.object(cgroup_telemetry, "Thread", side_effect=track_worker), \
                        contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()) as err:
                    driver.start()
                    try:
                        driver.join(timeout=5)
                        self.assertFalse(driver.is_alive(), "driver waited on a blocked stat read")
                        self.assertEqual(len(workers), 2)
                        self.assertTrue(all(worker.daemon and worker.is_alive() for worker in workers))
                        self.assertTrue(all(stop.is_set() for stop in stops))
                        if raises:
                            self.assertIs(outcome[0], error)
                        else:
                            self.assertEqual(list(outcome[0].values()), [True] * 4)
                            self.assertIn("sunrise-client: cargo exit 0", out.getvalue())
                            self.assertIn("test-command-end utc=", out.getvalue())
                    finally:
                        for stop in stops:
                            stop.set()
                        release.set()
                        driver.join(timeout=5)
                        for worker in workers:
                            worker.join(timeout=5)
                self.assertFalse(any(worker.is_alive() for worker in workers))
                log = out.getvalue() + err.getvalue()
                self.assertEqual(log.count("memory_sample_attempts=0"), 2)
                self.assertEqual(log.count("cpu_usage_usec_delta=unavailable"), 2)
                self.assertNotIn("123456789", log)
                self.assertNotIn("PRIVATE", log)
                self.assertNotIn(directory, log)

    def test_interrupt_during_thread_start_still_stops_sampler(self):
        stop, workers = Event(), []

        class InterruptedStart(Thread):
            def start(self):
                workers.append(self)
                super().start()
                raise KeyboardInterrupt

        with mock.patch.object(cgroup_telemetry, "Event", return_value=stop), \
                mock.patch.object(cgroup_telemetry, "Thread", InterruptedStart), \
                mock.patch.object(cgroup_telemetry, "_read", return_value=b""), contextlib.redirect_stdout(io.StringIO()):
            try:
                with self.assertRaises(KeyboardInterrupt):
                    with cgroup_telemetry.sample_command(Path("."), "sunrise-client#3of8", "test-command"):
                        self.fail("the start interruption did not propagate")
                self.assertTrue(stop.is_set())
            finally:
                stop.set()
                for worker in workers:
                    worker.join(timeout=5)


if __name__ == "__main__":
    unittest.main()
