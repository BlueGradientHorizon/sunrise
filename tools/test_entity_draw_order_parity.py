#!/usr/bin/env python3
"""Compare the body submission order after Login and same-level Respawn.

The vanilla side runs its lookup and submit loop with already-visible bodies;
the Sunrise probe sends packets through the client and observes nonempty body
submissions. This compares order, not culling, fire, pixels or GPU draw order.
"""
import json
import os

try:
    from . import lab, oracle_harness
except ImportError:
    import lab
    import oracle_harness


def snapshots(path):
    prefix = "entity-draw-order: "
    # Libtest may print the running test name on the same line as its first snapshot.
    return [json.loads(line.split(prefix, 1)[1])
            for line in path.read_text().splitlines() if prefix in line]


def main():
    lock, directory = lab.config()
    manifest, client = lab.import_client(lock, directory)
    libraries = [lab.import_library(artifact, directory / "libraries/client")
                 for artifact in lab.client_library_artifacts(manifest)
                 if "/natives-" not in artifact["path"]]
    build = lab.LOCAL / "entity-draw-order"
    build.mkdir(parents=True, exist_ok=True)
    oracle = "VanillaEntityDrawOrderOracle"
    classes, dependencies, _ = oracle_harness.compile_oracle(
        build, oracle, {"client": client}, libraries, lock["minecraft"]["java_major"])
    scene = lab.ROOT / "tools/fixtures/entity-draw-order.txt"
    oracle_harness.java_run(build, classes, dependencies, oracle, [scene], "vanilla")
    env = lab.game_env(
        SUNRISE_TEST_RESOURCES=os.environ.get("SUNRISE_TEST_RESOURCES", lab.RESOURCES),
        CARGO_TARGET_DIR=lab.LOCAL / "parity-target",
    )
    oracle_harness.run_logged(
        ["cargo", "test", "--locked", "-p", "sunrise-client", "--lib",
         "entity_draw_order_after_login_and_same_level_respawn", "--", "--ignored", "--nocapture"],
        build, "rust", env=env, timeout=2400)
    expected, actual = snapshots(build / "vanilla.log"), snapshots(build / "rust.log")
    if len(expected) != 2 or actual != expected:
        raise AssertionError("Body submission order differs; see the local entity-draw-order logs")
    print("Both mixed-body submission snapshots match the pinned vanilla client")


if __name__ == "__main__":
    main()
