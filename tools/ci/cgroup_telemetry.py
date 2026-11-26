"""Read-only cgroup v2 telemetry for the ignored-test shards; logs contain only fixed fields."""
from contextlib import contextmanager
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryFile
from threading import Event, Thread
from time import monotonic_ns

CPU_FIELDS = ("usage_usec", "user_usec", "system_usec", "nr_periods", "nr_throttled", "throttled_usec")
MEMORY_FIELDS = ("current", "anon", "file")
SAMPLE_INTERVAL_SECONDS = 1
END_CPU_READ_SECONDS = 1


def _read(path: Path) -> bytes:
    try:
        return path.read_bytes()
    except OSError:
        return b""


def _counter(value: bytes) -> int | None:
    # Kernel counters are unsigned decimal, at most 20 digits for a u64.
    return int(value) if len(value) <= 20 and value.isdigit() else None


def _stat(path: Path, fields: tuple[str, ...]) -> dict[str, int | None]:
    keys = {field.encode("ascii"): field for field in fields}
    counters = dict.fromkeys(fields)
    for line in _read(path).splitlines():
        parts = line.split()
        if len(parts) == 2 and (field := keys.get(parts[0])) is not None:
            counters[field] = _counter(parts[1])
    return counters


def _metric_fields(fields: dict[str, int | None]) -> str:
    return " ".join(f"{key}={value if value is not None else 'unavailable'}" for key, value in fields.items())


@contextmanager
def sample_command(cgroup: Path, batch: str, phase: str):
    """Sample the job cgroup around one command (batch and phase are supplied by the shard runner).

    The maxima are independent observations, not additive or isolated to the command's processes.
    File cache and other memory still charged from earlier phases count too. memory.peak is not used.
    CPU deltas cover the span between completed snapshots, not the command's exact boundaries.
    The attempt count includes only completed snapshots, even if a counter in one is unavailable.
    """
    stop = Event()
    no_cpu = (None,) * len(CPU_FIELDS)
    reading = 0, (None,) * len(MEMORY_FIELDS), None, no_cpu, no_cpu

    def sample_periodically():
        nonlocal reading
        samples = 0
        maxima = dict.fromkeys(MEMORY_FIELDS)
        first_at = None
        first_cpu = no_cpu
        while not stop.is_set():
            values = _stat(cgroup / "memory.stat", ("anon", "file"))
            if stop.is_set():
                return
            values["current"] = _counter(_read(cgroup / "memory.current").strip())
            if stop.is_set():
                return
            cpu = tuple(_stat(cgroup / "cpu.stat", CPU_FIELDS).values())
            at = monotonic_ns()
            if stop.is_set():
                return
            for field, value in values.items():
                if value is not None and (maxima[field] is None or value > maxima[field]):
                    maxima[field] = value
            if first_at is None:
                first_at, first_cpu = at, cpu
            span_ms = (at - first_at) // 1_000_000 if samples else None
            samples += 1
            # Publish one immutable reading; the command thread never waits for the next one.
            reading = samples, tuple(maxima.values()), span_ms, first_cpu, cpu
            if stop.wait(SAMPLE_INTERVAL_SECONDS):
                return

    try:
        try:
            Thread(target=sample_periodically, daemon=True).start()
        except RuntimeError:
            # No reader available; the command still runs with unavailable telemetry.
            pass
        yield
    finally:
        stop.set()
        samples, maxima, span_ms, first_cpu, last_cpu = reading
        metrics = {"memory_sample_attempts": samples, "sample_interval_seconds": SAMPLE_INTERVAL_SECONDS,
                   "cpu_sample_span_ms": span_ms,
                   **{f"memory_{field}_observed_max_bytes": value for field, value in zip(MEMORY_FIELDS, maxima)}}
        for field, before, after in zip(CPU_FIELDS, first_cpu, last_cpu):
            metrics[f"cpu_{field}_delta"] = (after - before if span_ms is not None and before is not None
                                              and after is not None and after >= before else None)
        try:
            print(f"data-leg cgroup-phase batch={batch} phase={phase} scope=job_cgroup_during_command "
                  + _metric_fields(metrics), flush=True)
        except OSError:
            # Telemetry must not replace a subprocess error with a logging error.
            pass


def _observe_job_cpu(cgroup: str) -> str:
    # The reader gets no workflow pipes. A kernel read that outlives the deadline
    # cannot delay step exit via either a pipe or a wait (including Popen.__exit__).
    try:
        with TemporaryFile(mode="w+t", encoding="ascii") as observation:
            reader = subprocess.Popen([sys.executable, __file__, "--read-cpu", cgroup],
                                      stdin=subprocess.DEVNULL, stdout=observation, stderr=subprocess.DEVNULL)
            try:
                if reader.wait(timeout=END_CPU_READ_SECONDS) == 0:
                    observation.seek(0)
                    return observation.read()
            finally:
                if reader.returncode is None:
                    reader.kill()
    except (OSError, subprocess.TimeoutExpired):
        pass
    return _metric_fields(dict.fromkeys(CPU_FIELDS))


if __name__ == "__main__":
    if sys.argv[1] == "--read-cpu":
        sys.stdout.write(_metric_fields(_stat(Path(sys.argv[2]) / "cpu.stat", CPU_FIELDS)))
    else:
        print("data-leg cgroup-cpu scope=whole_job_cgroup_cumulative " + _observe_job_cpu(sys.argv[1]))
