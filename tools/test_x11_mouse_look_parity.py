#!/usr/bin/env python3
"""Vanilla 26.3 X11 mouse-look bit-exact parity harness (#3022).

Replays the committed 4-route XTEST recording (look_x, look_y, look_counts, regrab) on Xvfb with
unscaled input through tools/client_packet_parity.py and compares every serverbound rotation's raw
IEEE-754 float bits and look-dependent action fields against the pinned vanilla 26.3 client.

  python3 tools/test_x11_mouse_look_parity.py --record --runs 2
  python3 tools/test_x11_mouse_look_parity.py --bin target/fast --runs 2
  python3 tools/test_x11_mouse_look_parity.py --live --bin target/fast --runs 2
  python3 tools/test_x11_mouse_look_parity.py --compare RUNS --a vanilla --b sunrise
  python3 tools/test_x11_mouse_look_parity.py --compare-witness RUNS --client sunrise --witness FILE
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys

try:
    from . import client_packet_diff as diff
    from . import client_packet_routes as routes
    from . import input_recording
    from . import lab
    from . import packet_parity_runs
except ImportError:
    import client_packet_diff as diff
    import client_packet_routes as routes
    import input_recording
    import lab
    import packet_parity_runs

RECORDING_REL = "tools/fixtures/26.3/x11_mouse_look_recording.jsonl"
DEFAULT_RECORDING = lab.ROOT / RECORDING_REL
DEFAULT_LABEL = "x11-mouse-look-26.3"
ROUTES = list(routes.MOUSE_LOOK)


def default_witness_path():
    """Location of the recorded vanilla 26.3 X11 mouse-look witness."""
    return lab.ROOT / "tools" / "witness" / "26.3" / "x11_mouse_look.json"


def _load_packet_parity():
    # Imported lazily so importing this module in unit tests never mutates sys.path.
    try:
        from . import client_packet_parity
    except ImportError:
        import client_packet_parity
    return client_packet_parity


def _relative_or_str(path):
    try:
        return Path(path).resolve().relative_to(lab.ROOT.resolve()).as_posix()
    except ValueError:
        return str(path)


def _java_version():
    done = subprocess.run(["java", "-version"], capture_output=True, text=True, check=True)
    text = (done.stderr or done.stdout).strip()
    return text.splitlines()[0] if text else None


def record_witness(recording_path, witness_path, runs=2, label=DEFAULT_LABEL, rebuild_world=False):
    """Runs vanilla 26.3 on Xvfb via client_packet_parity and writes the verified 26.3 witness JSON."""
    recording = input_recording.read(recording_path)
    route_names = [name for name, _ in input_recording.routes(recording)]
    if route_names != ROUTES:
        raise ValueError(f"{recording_path}: expected routes {ROUTES}, got {route_names}")
    java_ver = _java_version()
    harness = _load_packet_parity()
    if rebuild_world or not harness.course_current():
        harness.build_world()
    indices = set(range(1, runs + 1))
    for i in sorted(indices):
        print(f"vanilla 26.3 run {i}", flush=True)
        harness.run_once("vanilla", i, recording, bins=None, label=label, mouse_scale=1.0, replace=True)
    prefix = packet_parity_runs.prefix("vanilla", label)
    captured = diff.runs_of(harness.WORK / "runs", prefix, indices)
    witness = diff.build_look_witness(
        captured, ROUTES,
        recording_path=_relative_or_str(recording_path),
        recording_file_sha256=lab.digest(recording_path, "sha256"),
        java_version=java_ver,
    )
    witness_path = Path(witness_path)
    witness_path.parent.mkdir(parents=True, exist_ok=True)
    witness_path.write_text(json.dumps(witness, indent=2) + "\n", encoding="utf-8")
    print(f"recorded 26.3 witness ({runs} runs verified): {witness_path}", flush=True)
    return witness


def verify_live(recording_path, bins, runs=2, label=DEFAULT_LABEL, rebuild_world=False, out_json=None):
    """Runs vanilla 26.3 and Sunrise on Xvfb with unscaled input and compares raw float bits."""
    recording = input_recording.read(recording_path)
    route_names = [name for name, _ in input_recording.routes(recording)]
    if route_names != ROUTES:
        raise ValueError(f"{recording_path}: expected routes {ROUTES}, got {route_names}")
    harness = _load_packet_parity()
    if rebuild_world or not harness.course_current():
        harness.build_world()
    indices = set(range(1, runs + 1))
    for client in ("vanilla", "sunrise"):
        for i in sorted(indices):
            print(f"{client} run {i}", flush=True)
            harness.run_once(client, i, recording, bins=bins, label=label, mouse_scale=1.0, replace=True)
    names = tuple(packet_parity_runs.prefix(c, label) for c in ("vanilla", "sunrise"))
    report = diff.compare_look_runs(
        diff.runs_of(harness.WORK / "runs", names[0], indices),
        diff.runs_of(harness.WORK / "runs", names[1], indices),
        ROUTES, names,
    )
    if out_json:
        Path(out_json).write_text(json.dumps(report, indent=1), encoding="utf-8")
    print(diff.describe_look(report), flush=True)
    return report


def verify_against_witness(recording_path, witness_path, bins, runs=2, label=DEFAULT_LABEL,
                           rebuild_world=False, out_json=None):
    """Runs Sunrise on Xvfb with unscaled input and compares raw float bits against the 26.3 witness."""
    recording = input_recording.read(recording_path)
    route_names = [name for name, _ in input_recording.routes(recording)]
    if route_names != ROUTES:
        raise ValueError(f"{recording_path}: expected routes {ROUTES}, got {route_names}")
    witness = json.loads(Path(witness_path).read_text(encoding="utf-8"))
    harness = _load_packet_parity()
    if rebuild_world or not harness.course_current():
        harness.build_world()
    indices = set(range(1, runs + 1))
    for i in sorted(indices):
        print(f"sunrise run {i}", flush=True)
        harness.run_once("sunrise", i, recording, bins=bins, label=label, mouse_scale=1.0, replace=True)
    prefix = packet_parity_runs.prefix("sunrise", label)
    report = diff.compare_look_against_witness(
        diff.runs_of(harness.WORK / "runs", prefix, indices), witness, prefix)
    if out_json:
        Path(out_json).write_text(json.dumps(report, indent=1), encoding="utf-8")
    print(diff.describe_look(report), flush=True)
    return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--record", action="store_true",
                        help="run vanilla 26.3 on Xvfb and write the recorded 26.3 witness JSON")
    parser.add_argument("--live", action="store_true",
                        help="run both vanilla 26.3 and Sunrise live on Xvfb instead of using a recorded witness")
    parser.add_argument("--compare", type=Path,
                        help="offline: compare two captured run prefixes in this runs folder (no game launch)")
    parser.add_argument("--compare-witness", type=Path,
                        help="offline: compare captured runs in this runs folder against --witness (no game launch)")
    parser.add_argument("--a", default="vanilla", help="first run prefix for --compare")
    parser.add_argument("--b", default="sunrise", help="second run prefix for --compare")
    parser.add_argument("--client", default="sunrise", help="run prefix for --compare-witness")
    parser.add_argument("--recording", type=Path, default=DEFAULT_RECORDING,
                        help="26.3 input recording JSONL to replay")
    parser.add_argument("--witness", type=Path, default=default_witness_path(),
                        help="recorded vanilla 26.3 witness JSON")
    parser.add_argument("--bin", type=Path, default=None, help="folder with the built Sunrise binary")
    parser.add_argument("--runs", type=int, default=2, help="runs per client (default: 2)")
    parser.add_argument("--label", default=DEFAULT_LABEL, help="run set label under .local/packet-parity/runs")
    parser.add_argument("--rebuild-world", action="store_true", help="rebuild the world snapshot before running")
    parser.add_argument("--json", type=Path, help="optional JSON report output path")
    args = parser.parse_args(argv)

    if args.runs < 1:
        parser.error("--runs must be at least 1")
    if args.compare:
        rep = diff.compare_look_runs(
            diff.runs_of(args.compare, args.a),
            diff.runs_of(args.compare, args.b),
            ROUTES, (args.a, args.b),
        )
        if args.json:
            args.json.write_text(json.dumps(rep, indent=1), encoding="utf-8")
        print(diff.describe_look(rep))
        return 0 if rep["bit_exact"] else 1
    if args.compare_witness:
        witness = json.loads(args.witness.read_text(encoding="utf-8"))
        rep = diff.compare_look_against_witness(diff.runs_of(args.compare_witness, args.client), witness, args.client)
        if args.json:
            args.json.write_text(json.dumps(rep, indent=1), encoding="utf-8")
        print(diff.describe_look(rep))
        return 0 if rep["bit_exact"] else 1
    if args.record:
        record_witness(args.recording, args.witness, args.runs, args.label, args.rebuild_world)
        return 0
    bins = args.bin if args.bin is not None else lab.cargo_target() / "fast"
    if not args.live and args.witness.exists():
        rep = verify_against_witness(args.recording, args.witness, bins, args.runs, args.label,
                                     args.rebuild_world, args.json)
    else:
        rep = verify_live(args.recording, bins, args.runs, args.label, args.rebuild_world, args.json)
    return 0 if rep["bit_exact"] else 1


if __name__ == "__main__":
    sys.exit(main())
