#!/usr/bin/env python3
"""Compare the two live-level spawn gates independently with the pinned server."""
import argparse
import os
from pathlib import Path
import subprocess

try:
    from . import lab, test_rust_parity as base
except ImportError:
    import lab
    import test_rust_parity as base

PARITY_MANUAL = "python3 tools/test_spawn_obstruction_parity.py --test-binary GAMEPLAY_TEST_BINARY"
FIXTURE = "game::natural_spawning::spawn_obstruction_fixture::spawn_entity_obstruction"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=Path)
    parser.add_argument("--reference-only", action="store_true")
    parser.add_argument("--resources", type=Path, default=lab.RESOURCES)
    parser.add_argument("--recording", type=Path, default=lab.LOCAL / "spawn-obstruction-parity")
    args = parser.parse_args()
    if not args.test_binary and not args.reference_only:
        parser.error("pass the focused sunrise-client libtest binary or --reference-only")

    work = args.recording.resolve()
    classes = work / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    for name in ("vanilla.txt", "rust.txt", "report.json"):
        (work / name).unlink(missing_ok=True)
    source = lab.ROOT / "tests/fixtures/spawn_obstruction.tsv"
    cases = [line.split() for line in source.read_text(encoding="utf-8").splitlines() if not line.startswith("#")]
    inputs = work / "inputs.txt"
    inputs.write_text("".join(
        f"{name} mode=obstruction installation={installation} type={kind} dx={dx} entity_data={nbt} z={8.5 + 16 * i}\n"
        for i, (name, installation, kind, dx, nbt, _, _) in enumerate(cases)
    ), encoding="utf-8")
    names = [case[0] for case in cases]
    lock, jars, libraries = base.verify_artifacts()
    sourcepath = lab.ROOT / "mods/sunrise/src/test/java"
    oracle = sourcepath / "dev/sunriselab/parity/VanillaNaturalSpawnOracle.java"
    deps = os.pathsep.join(map(str, [jars["server"], *libraries]))

    def run(log_name, command, env=None):
        with (work / f"{log_name}.log").open("w", encoding="utf-8") as log:
            return subprocess.run(list(map(str, command)), cwd=work, env=env, stdout=log,
                                  stderr=subprocess.STDOUT, timeout=600).returncode

    if run("javac", ["javac", "--release", lock["minecraft"]["java_major"], "-proc:none",
                     "-sourcepath", sourcepath, "-cp", deps, "-d", classes, oracle]):
        raise SystemExit("oracle compilation failed (see the private javac log)")
    if run("java", ["java", "-Xmx1G", "-cp", str(classes) + os.pathsep + deps,
                    "dev.sunriselab.parity.VanillaNaturalSpawnOracle", inputs, work / "vanilla.txt"]):
        raise SystemExit("oracle failed (see the private Java log)")

    def read_recording(file):
        if not file.is_file():
            raise SystemExit(f"missing recording {file.name}; see the private logs")
        rows = [line.split() for line in file.read_text(encoding="utf-8").splitlines()]
        if [row[0] for row in rows] != names or any(
            len(row) != 4 or row[1] != "obstruction" or any(value not in ("true", "false") for value in row[2:])
            for row in rows
        ):
            raise SystemExit(f"incomplete or invalid recording {file.name}; see the private logs")
        return [row[2:] for row in rows]

    vanilla = read_recording(work / "vanilla.txt")
    expected = [case[-2:] for case in cases]
    if vanilla != expected:
        labels = [name for name, actual, spec in zip(names, vanilla, expected) if actual != spec]
        raise SystemExit(f"fixture assumptions disagree with the oracle: {', '.join(labels)}")
    version = lock["minecraft"]["version"]
    if args.reference_only:
        print(f"{version}: reference fixture passed")
        return

    scratch = {key: work / key for key in ("SUNRISE_HOME", "SUNRISE_LAUNCHER_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME")}
    for path in scratch.values():
        path.mkdir(exist_ok=True)
    env = lab.game_env(**scratch, SUNRISE_TEST_RESOURCES=args.resources.resolve(),
                       SUNRISE_SPAWN_OBSTRUCTION_RECORDING=work / "rust.txt")
    status = run("rust", [args.test_binary.resolve(), FIXTURE, "--exact", "--ignored"], env)
    rust = read_recording(work / "rust.txt")
    collision = [name for name, ref, got in zip(names, vanilla, rust) if ref[0] != got[0]]
    obstruction = [name for name, ref, got in zip(names, vanilla, rust) if ref[1] != got[1]]
    lab.write_json(work / "report.json", dict(minecraft=version, cases=len(names), collision=collision,
                                               obstruction=obstruction, test_exit=status))
    if collision or obstruction or status:
        raise SystemExit(f"{version}: collision failures: {', '.join(collision) or 'none'}; "
                         f"obstruction failures: {', '.join(obstruction) or 'none'}; test exit {status}")
    print(f"{version}: collision and obstruction passed ({len(names)} cases)")


if __name__ == "__main__":
    main()
