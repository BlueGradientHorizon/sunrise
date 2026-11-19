#!/usr/bin/env python3
"""Record the real 26.3 Camera.alignWithEntity view of a sleeping living entity (a real Villager and a real
RemotePlayer seen through a server's set-camera) for the four bed facings: VanillaCameraOracle is the real Camera
driven with those entities, asleep (real sleeping-pos synched data) in a real red bed block. Writes cases.jsonl and vanilla.txt beside the Rust test that replays them."""
import json
import random
import sys
from pathlib import Path

try:
    from . import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking, test_camera_parity as cam
except ImportError:
    import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking, test_camera_parity as cam

OUT = lab.ROOT / 'crates/sunrise-client/src/game/network/tests/fixtures/remote_camera_sleeping'
# (yaw, pitch, previous yaw, previous pitch, partial tick)
VIEWS = [(0.0, 0.0, 0.0, 0.0, 1.0), (33.5, 12.25, 30.0, 10.0, 0.5), (-120.0, -45.0, -100.0, -40.0, 0.25),
         (200.0, 80.0, 190.0, 75.0, 0.75), (90.0, 0.0, 90.0, 0.0, 0.0)]


def cases():
    rng = random.Random(0x534c4550)
    for kind, facing in [(k, f) for k in ('villager', 'remote_player') for f in range(4)]:
        for yaw, pitch, yaw_o, pitch_o, pt in VIEWS:
            s = cam.entity_state(rng, (0, 64, 0))
            pos = [10.5, 64.0625, -3.5]
            s.update(kind=kind, sleeping_pos=[10, 64, -4], pos=[lab.dbits(v) for v in pos], prev=[lab.dbits(v) for v in [10.25, 64.0625, -3.75]],
                     xr=lab.fbits(pitch), yr=lab.fbits(yaw), xro=lab.fbits(pitch_o), yro=lab.fbits(yaw_o), eye=lab.fbits(0.2),
                     scale_attr=lab.dbits(1.0), dist=lab.dbits(4.0), sleeping=[facing], dead=None, mount=None)
            yield dict(t='camera', bed=[10, 64, -4, facing], fov=70, effect=lab.dbits(1.0), type=0, cells={},
                       ops=[['entity', s], ['tick'], ['frame', lab.fbits(pt), 0, False] + cam.window(rng)])


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / 'cases.jsonl').write_text(''.join(json.dumps(c, separators=(',', ':')) + '\n' for c in cases()))
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'remote-camera-sleeping'
    build.mkdir(exist_ok=True)
    classes, dependencies, _ = harness.compile_oracle(build, 'VanillaCameraOracle', jars, libraries, lock['minecraft']['java_major'])
    harness.java_run(build, classes, dependencies, 'VanillaCameraOracle', [OUT / 'cases.jsonl', OUT / 'vanilla.txt'], 'client')
    (OUT / 'README.md').write_text(
        'Witness for a spectated sleeping living entity (Minecraft 26.3).\n\n'
        f'Produced with `tools/record_remote_camera_sleeping.py` on the real Minecraft {lock["minecraft"]["version"]} client jar: '
        'VanillaCameraOracle runs the real `Camera.tick` and `Camera.alignWithEntity` (first person) with a real `Villager` and a real '
        '`RemotePlayer` (allocated without a constructor, own synched data and attributes) asleep through the real SLEEPING_POS_ID '
        'data in a real red bed block at each of the four facings (`bed: [x, y, z, Direction.get2DDataValue]`: 0 south, 1 west, 2 north, 3 east).\n'
        'cases.jsonl is the input (floats and doubles as hex bits), vanilla.txt the output.\n')
    print('recorded', OUT)


if __name__ == '__main__':
    main()
