#!/usr/bin/env python3
"""Record the real 26.3 Hud's draw calls for a view following a mob or another player (hud_camera_witness/26.3)."""
import os
import subprocess
import sys

try:
    from . import lab, test_rust_parity as base, test_model_baking_parity as baking
except ImportError:
    import lab, test_rust_parity as base, test_model_baking_parity as baking

PARITY = lab.ROOT / 'mods/sunrise/src/test/java/dev/sunriselab/parity'
DIR = lab.ROOT / 'tools/hud_camera_witness/26.3'


def main():
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'hud-camera-witness'; build.mkdir(parents=True, exist_ok=True)
    classes = build / 'classes'; classes.mkdir(exist_ok=True)
    dependencies = os.pathsep.join(map(str, [jars['client'], *libraries]))
    subprocess.run(['javac', '--release', str(lock['minecraft']['java_major']), '-proc:none', '-nowarn', '-cp', dependencies, '-d', str(classes),
                    *(str(PARITY / f'{n}.java') for n in ('ParityData', 'ParityFont', 'VanillaHudOracle'))], cwd=lab.ROOT, check=True)
    subprocess.run(['java', '-Xmx2G', '-cp', str(classes) + os.pathsep + dependencies, 'dev.sunriselab.parity.VanillaHudOracle',
                    str(DIR / 'cases.jsonl'), str(DIR / 'vanilla.jsonl')], cwd=lab.ROOT, check=True)


if __name__ == '__main__':
    main()
