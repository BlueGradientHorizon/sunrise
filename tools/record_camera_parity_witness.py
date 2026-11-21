#!/usr/bin/env python3
"""Record the real vanilla 26.3 camera parity witness (items 1, 6, 12, 14).

Compiles and runs VanillaCameraParityOracle against the pinned 26.3 client jar on JDK 25
and writes tools/witness/26.3/camera_parity/witness.json."""
import json
from pathlib import Path
import subprocess

try:
    from . import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking
except ImportError:
    import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking

OUT = lab.ROOT / 'tools/witness/26.3/camera_parity/witness.json'


def main():
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'camera-parity-witness'
    build.mkdir(parents=True, exist_ok=True)
    classes, dependencies, _ = harness.compile_oracle(
        build, 'VanillaCameraParityOracle', jars, libraries, lock['minecraft']['java_major']
    )
    raw_out = build / 'raw.json'
    harness.java_run(build, classes, dependencies, 'VanillaCameraParityOracle', [raw_out], 'client')
    data = json.loads(raw_out.read_text(encoding='utf-8'))
    java_ver = subprocess.check_output(['java', '--version'], text=True).splitlines()[0]
    witness = dict(
        meta=dict(
            minecraft=lock['minecraft']['version'],
            client_jar_sha256=lab.digest(jars['client'], 'sha256'),
            java_version=java_ver,
            command='python3 tools/record_camera_parity_witness.py',
            oracle='mods/sunrise/src/test/java/dev/sunriselab/parity/VanillaCameraParityOracle.java',
        ),
        **data,
    )
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(witness, indent=2) + '\n', encoding='utf-8')
    print(f'wrote {OUT}')


if __name__ == '__main__':
    main()
