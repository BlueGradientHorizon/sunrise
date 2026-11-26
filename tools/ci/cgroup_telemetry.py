"""Read-only cgroup v2 telemetry for the ignored-test shards; logs contain only fixed fields."""
from contextlib import contextmanager
from pathlib import Path
import sys
from threading import Event, Thread

CPU_FIELDS = ("usage_usec", "user_usec", "system_usec", "nr_periods", "nr_throttled", "throttled_usec")
SAMPLE_INTERVAL_SECONDS = 1


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
    """
    stop = Event()
    maxima = dict.fromkeys(("current", "anon", "file"))
    samples = 0

    def sample_memory():
        nonlocal samples
        samples += 1
        values = _stat(cgroup / "memory.stat", ("anon", "file"))
        values["current"] = _counter(_read(cgroup / "memory.current").strip())
        for field, value in values.items():
            if value is not None and (maxima[field] is None or value > maxima[field]):
                maxima[field] = value

    def sample_periodically():
        while not stop.wait(SAMPLE_INTERVAL_SECONDS):
            sample_memory()

    sample_memory()
    cpu_before = _stat(cgroup / "cpu.stat", CPU_FIELDS)
    sampler = Thread(target=sample_periodically)
    try:
        sampler.start()
    except RuntimeError:
        # If the runner cannot start another thread, still take both boundary samples.
        sampler = None
    try:
        yield
    finally:
        stop.set()
        if sampler is not None:
            sampler.join()
        cpu_after = _stat(cgroup / "cpu.stat", CPU_FIELDS)
        sample_memory()
        metrics = {"memory_sample_attempts": samples, "sample_interval_seconds": SAMPLE_INTERVAL_SECONDS,
                   **{f"memory_{field}_observed_max_bytes": value for field, value in maxima.items()}}
        for field, before in cpu_before.items():
            after = cpu_after[field]
            metrics[f"cpu_{field}_delta"] = after - before if before is not None and after is not None and after >= before else None
        try:
            print(f"data-leg cgroup-phase batch={batch} phase={phase} scope=job_cgroup_during_command "
                  + _metric_fields(metrics), flush=True)
        except OSError:
            # Telemetry must not replace a subprocess error with a logging error.
            pass


if __name__ == "__main__":
    print("data-leg cgroup-cpu scope=whole_job_cgroup_cumulative "
          + _metric_fields(_stat(Path(sys.argv[1]) / "cpu.stat", CPU_FIELDS)))
