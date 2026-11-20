#!/usr/bin/env python3
"""Record the real 26.3 fov and hud fov of a dying camera entity into the fixture the client test reads."""
import json
from pathlib import Path

try:
    from . import lab, oracle_harness as harness, test_camera_parity as cam, test_rust_parity as base, test_model_baking_parity as baking
except ImportError:
    import lab, oracle_harness as harness, test_camera_parity as cam, test_rust_parity as base, test_model_baking_parity as baking

DEAD = [None, 0, 5, 19, 20]
PARTIAL = [0.0, 0.5, 1.0]
FIXTURE = lab.ROOT / 'crates/sunrise-client/src/game/network/remote/camera_death_witness.tsv'


def main():
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'camera-death-witness'
    build.mkdir(exist_ok=True)
    cases = []
    for dead in DEAD:
        s = dict(kind='creeper', pos=[lab.dbits(0.5), lab.dbits(64.0), lab.dbits(0.5)], prev=[lab.dbits(0.5), lab.dbits(64.0), lab.dbits(0.5)],
                 xr=lab.fbits(0.0), yr=lab.fbits(0.0), xro=lab.fbits(0.0), yro=lab.fbits(0.0), eye=lab.fbits(1.62), flying=False,
                 ws=lab.fbits(0.0), speed=lab.dbits(cam.f32(0.1)), use=None, scale_attr=lab.dbits(1.0), dist=lab.dbits(4.0),
                 sleeping=None, dead=dead, mount=None)
        ops = [['entity', s]] + [['tick']] * 8 + [['frame', lab.fbits(pt), 0, False, lab.fbits(1600.0), lab.fbits(1000.0), lab.fbits(768.0)] for pt in PARTIAL]
        cases.append(dict(t='camera', fov=70, effect=lab.dbits(1.0), type=0, cells={}, ops=ops))
    inp = build / 'cases.jsonl'
    inp.write_text(''.join(json.dumps(c, separators=(',', ':')) + '\n' for c in cases))
    classes, deps, _ = harness.compile_oracle(build, 'VanillaCameraOracle', jars, libraries, lock['minecraft']['java_major'])
    harness.java_run(build, classes, deps, 'VanillaCameraOracle', [inp, build / 'client.txt'], 'client')
    lines = (build / 'client.txt').read_text().splitlines()
    rows = []
    for dead, case in zip(DEAD, range(len(DEAD))):
        frames = [l.split() for l in lines if l.startswith('frame ')][case * len(PARTIAL):(case + 1) * len(PARTIAL)]
        for pt, f in zip(PARTIAL, frames):
            rows.append(f'{"none" if dead is None else dead}\t{lab.fbits(pt)}\t{f[-2]}\t{f[-1]}')
    FIXTURE.write_text(
        '# Real 26.3 Camera.calculateFov / calculateHudFov of a camera entity with deathTime (none = alive), FOV option 70,\n'
        '# effect scale 1.0, first person, no fluid, a creeper camera entity (26.3). Recorded by tools/record_camera_death_witness.py\n'
        '# (VanillaCameraOracle camera op on the pinned 26.3 client jar). Columns: deathTime, partial bits, fov bits, hudFov bits.\n'
        + '\n'.join(rows) + '\n')
    print(FIXTURE)


if __name__ == '__main__':
    main()
