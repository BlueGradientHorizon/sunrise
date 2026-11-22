#!/usr/bin/env python3
"""Compare crosshair targeting and hit results (#32 TNT/falling blocks, #34 root-vehicle exclusion) with vanilla 26.3.

Compiles and runs VanillaCrosshairPickOracle against the pinned 26.3 client jar on JDK 25,
writing witness.json and meta.json into .local/witness/<version>/crosshair_pick/, then runs
the sunrise-client crosshair parity tests against it via SUNRISE_CROSSHAIR_PICK_WITNESS."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

try:
    from . import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking
except ImportError:
    import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking

TESTS = [
    'game::crosshair_pick_tests::primed_tnt_and_falling_blocks_match_26_3_witness',
    'game::crosshair_pick_tests::nested_ride_root_vehicle_exclusion_matches_26_3_witness',
    'game::crosshair_pick_tests::singleplayer_and_unstored_pick_share_unified_query_and_root_exclusion_26_3',
]


def record(lock, jars, libraries, build):
    version = lock['minecraft']['version']
    witness_dir = lab.LOCAL / 'witness' / version / 'crosshair_pick'
    witness_dir.mkdir(parents=True, exist_ok=True)
    witness = witness_dir / 'witness.json'
    classes, dependencies, oracle_path = harness.compile_oracle(
        build, 'VanillaCrosshairPickOracle', jars, libraries, lock['minecraft']['java_major']
    )
    harness.java_run(build, classes, dependencies, 'VanillaCrosshairPickOracle', [witness], 'client')
    java_ver = subprocess.check_output(['java', '--version'], text=True).splitlines()[0].strip()
    lab.write_json(witness_dir / 'meta.json', dict(
        minecraft=version,
        client_jar_sha256=lab.digest(jars['client'], 'sha256'),
        java_version=java_ver,
        command='python3 tools/test_crosshair_pick_parity.py',
        oracle='mods/sunrise/src/test/java/dev/sunriselab/parity/VanillaCrosshairPickOracle.java',
        inputs_sha256={
            'oracle': lab.digest(oracle_path, 'sha256'),
        },
    ))
    return witness


def run_logged(command, build, name, cwd=lab.ROOT, env=None):
    path = build / f'{name}.log'
    with path.open('w', encoding='utf-8') as log:
        result = subprocess.run([str(c) for c in command], cwd=cwd, env=env, timeout=3600,
                                stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        tail = path.read_text(encoding='utf-8', errors='replace').splitlines()[-40:]
        print('\n'.join(tail), file=sys.stderr, flush=True)
        sys.exit(f'{name} failed: see {path}')
    return path


def build_test_binary(build, package, target_name, env):
    compiled = run_logged(
        ['cargo', 'test', '--locked', '-p', package, '--lib', '--no-run', '--message-format=json'],
        build, f'compile-{package}', env=env,
    )
    binaries = []
    for line in compiled.read_text(encoding='utf-8').splitlines():
        if not line.startswith('{'):
            continue
        row = json.loads(line)
        if (row.get('reason') == 'compiler-artifact' and row.get('executable')
                and row.get('target', {}).get('name') == target_name and row.get('profile', {}).get('test')):
            binaries.append(Path(row['executable']))
    assert len(binaries) == 1, f'expected one {target_name} test binary, found {binaries}'
    return binaries[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--resources', type=Path, default=Path(os.environ.get('SUNRISE_TEST_RESOURCES', lab.RESOURCES)))
    parser.add_argument('--record-only', action='store_true')
    args = parser.parse_args()
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'crosshair-pick-parity'
    build.mkdir(parents=True, exist_ok=True)
    witness = record(lock, jars, libraries, build)
    if args.record_only:
        print(f'wrote {witness} ({witness.stat().st_size} bytes)')
        return
    with tempfile.TemporaryDirectory(prefix='sunrise-crosshair-pick-') as tmp:
        scratch = Path(tmp)
        homes = {k: scratch / k for k in ('SUNRISE_HOME', 'SUNRISE_LAUNCHER_HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME')}
        env = lab.game_env(os.environ, **homes, RUST_MIN_STACK='16777216')
        binary = build_test_binary(build, 'sunrise-client', 'sunrise_client', env)
        env['SUNRISE_TEST_RESOURCES'] = str(args.resources.resolve())
        env['SUNRISE_CROSSHAIR_PICK_WITNESS'] = str(witness.resolve())
        log = run_logged([binary, '--ignored', '--exact', *TESTS], build, 'game', cwd=scratch, env=env)
        expected = f'{len(TESTS)} passed; 0 failed'
        assert expected in log.read_text(encoding='utf-8'), f'expected "{expected}" in {log}'
    print(f'Crosshair pick parity passed ({len(TESTS)} tests against {witness})')


if __name__ == '__main__':
    main()
