#!/usr/bin/env python3
"""Record the 26.3 witness for the client local-game-mode vs PlayerInfo split:
runs the real ClientPacketListener, MultiPlayerGameMode, AbstractClientPlayer,
Player.tick, and LocalPlayer.aiStep from the pinned 26.3 client jar on JDK 25,
writing tools/witness/26.3/gamemode_split/witness.json."""
from pathlib import Path

try:
    from . import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking
except ImportError:
    import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking

OUT = lab.ROOT / 'tools/witness/26.3/gamemode_split/witness.json'


def main():
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'gamemode-split-witness'
    build.mkdir(exist_ok=True)
    classes, dependencies, _ = harness.compile_oracle(
        build, 'VanillaClientGameModeSplitOracle', jars, libraries, lock['minecraft']['java_major']
    )
    OUT.parent.mkdir(parents=True, exist_ok=True)
    harness.java_run(build, classes, dependencies, 'VanillaClientGameModeSplitOracle', [OUT], 'client')
    print(f'wrote {OUT} ({OUT.stat().st_size} bytes)')


if __name__ == '__main__':
    main()
