#!/usr/bin/env python3
"""Compare ridden NoAI donkey travel and rotation with vanilla 26.3 on the JVM.

Runs VanillaNoAiRiddenTravelOracle on a real headless flat 26.3 ServerLevel, verifies the
committed 26.3 witness matches byte-for-byte, and runs Sunrise's bit-exact witness test.
Use --record to regenerate tools/witness/26.3/noai_ridden_travel.tsv without running Cargo.
"""
import argparse
import os
from pathlib import Path
import shutil
import tempfile

try:
    from . import lab, oracle_harness as harness, test_rust_parity as base
except ImportError:
    import lab
    import oracle_harness as harness
    import test_rust_parity as base

WITNESS = lab.ROOT / "tools/witness/26.3/noai_ridden_travel.tsv"
ORACLE = lab.ROOT / "mods/sunrise/src/test/java/dev/sunriselab/parity/VanillaNoAiRiddenTravelOracle.java"
TEST = "mobs::ai::noai_ridden_tests::a_ridden_noai_donkey_travels_and_turns_with_its_rider"


def run_oracle(build, lock, jars, libraries):
    classes = build / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, [jars["server"], *libraries]))
    harness.run_logged(
        ["javac", "--release", str(lock["minecraft"]["java_major"]), "-proc:none",
         "-cp", cp, "-d", classes, ORACLE],
        build,
        "compile-java",
    )
    world = build / "world"
    shutil.rmtree(world, ignore_errors=True)
    out = build / "vanilla.tsv"
    out.unlink(missing_ok=True)
    harness.run_logged(
        ["java", "-Xmx2g", "-Djava.awt.headless=true", "-cp", str(classes) + os.pathsep + cp,
         "dev.sunriselab.parity.VanillaNoAiRiddenTravelOracle", out, world],
        build,
        "vanilla",
    )
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record", action="store_true", help="record the 26.3 witness file from vanilla and exit")
    parser.add_argument("--java-only", action="store_true", help="run the vanilla oracle and verify the committed witness")
    parser.add_argument("--resources", type=Path,
                        default=Path(os.environ.get("SUNRISE_TEST_RESOURCES", lab.RESOURCES)))
    args = parser.parse_args()
    lock, jars, libraries = base.verify_artifacts()
    build = lab.LOCAL / "noai-ridden-travel-parity"
    build.mkdir(parents=True, exist_ok=True)
    report = build / "report.json"
    report.unlink(missing_ok=True)
    vanilla = run_oracle(build, lock, jars, libraries)
    if args.record:
        WITNESS.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(vanilla, WITNESS)
        print(f"Recorded {WITNESS.relative_to(lab.ROOT)} ({len(WITNESS.read_text(encoding='utf-8').splitlines())} lines)")
        return
    sources = [
        Path(__file__).resolve(),
        ORACLE,
        WITNESS,
        lab.ROOT / "tools/lab.py",
        lab.ROOT / "tools/oracle_harness.py",
        lab.ROOT / "tools/test_rust_parity.py",
        lab.ROOT / "Cargo.toml",
        lab.ROOT / "Cargo.lock",
        lab.ROOT / "crates/sunrise-client/src/mobs/ai.rs",
        lab.ROOT / "crates/sunrise-client/src/mobs/equine.rs",
        lab.ROOT / "crates/sunrise-client/src/mobs/living.rs",
        lab.ROOT / "crates/sunrise-client/src/mobs/living/travel.rs",
    ]
    hashes = base.source_hashes(sources)
    matched = base.compare(vanilla, WITNESS)
    if args.java_only:
        base.ensure_sources_unchanged(sources, hashes)
        print(f"Vanilla 26.3 witness matches ({matched} lines)")
        return
    resources = args.resources.resolve()
    if not (resources / "entity-types.json").is_file():
        raise SystemExit("prepared resources required; pass --resources PATH")
    with tempfile.TemporaryDirectory(prefix="sunrise-noai-ridden-") as scratch:
        env = lab.game_env(
            SUNRISE_HOME=scratch,
            SUNRISE_TEST_RESOURCES=str(resources),
        )
        env.setdefault("CARGO_BUILD_JOBS", "1")
        env.setdefault("RUST_TEST_THREADS", "2")
        harness.run_logged(
            ["cargo", "test", "--locked", "-p", "sunrise-client-lib", "--lib", TEST, "--", "--exact", "--ignored"],
            build,
            "rust",
            env=env,
        )
    base.ensure_sources_unchanged(sources, hashes)
    lab.write_json(
        report,
        dict(
            schema=1,
            minecraft=lock["minecraft"]["version"],
            sources_sha256=hashes,
            server_jar_sha256=lab.digest(jars["server"], "sha256"),
            witness_sha256=lab.digest(WITNESS, "sha256"),
            matched_output_lines=matched,
            scope=__doc__,
        ),
    )
    print(f"Ridden NoAI donkey travel parity passed ({matched} lines): {report}", flush=True)


if __name__ == "__main__":
    main()
