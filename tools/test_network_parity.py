#!/usr/bin/env python3
"""Compare play packet bundles, post_effects lifecycle, and strict entity_data decoding with vanilla 26.3.

Compiles and runs VanillaNetworkParityOracle against the pinned 26.3 client jar on JDK 25,
writing bundle.json, post_effects.json, entity_data.json, and meta.json into
.local/witness/<version>/network/, then runs the 12 Rust tests across sunrise-protocol,
sunrise-client-lib, and sunrise-client against them via SUNRISE_NETWORK_WITNESS_DIR."""
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

ORACLE = 'VanillaNetworkParityOracle'

PROTOCOL_TESTS = [
    'entity_data::tests::unknown_entity_data_serializers_fail_decode_like_26_3',
    'entity_data::tests::valid_26_3_optional_global_pos_and_painting_variant_decode_completely',
    'packets::configuration::tests::configuration_decodes_post_effects_packet_26_3',
]

LIB_TESTS = [
    'net::tests::bundle_holds_packets_until_closing_delimiter_and_emits_one_event_26_3',
    'net::tests::bundled_keepalive_waits_for_game_thread_in_bundle_order_26_3',
    'net::tests::bundle_rejects_terminal_packet_and_oversize_over_4096_26_3',
    'net::tests::configuration_post_effects_disconnects_with_packet_error_26_3',
    'net::tests::unknown_entity_data_serializer_disconnects_play_session_like_26_3',
]

CLIENT_TESTS = [
    'game::remote::raw_ingress::bundle_parity_tests::genpvp_ping_rotation_and_keepalive_bundle_apply_atomically_in_order_26_3',
    'game::remote::raw_ingress::bundle_parity_tests::dependent_entity_bundle_never_splits_across_poll_or_tick_26_3',
    'game::remote::raw_ingress::post_effects_parity_tests::post_effects_cleared_on_login_respawn_and_reconfigure_26_3',
    'game::remote::raw_ingress::post_effects_parity_tests::post_effects_before_login_disconnects_with_packet_error_26_3',
]


def record(lock, jars, libraries, build):
    version = lock['minecraft']['version']
    witness_dir = lab.LOCAL / 'witness' / version / 'network'
    witness_dir.mkdir(parents=True, exist_ok=True)
    classes, dependencies, oracle_path = harness.compile_oracle(
        build, ORACLE, jars, libraries, lock['minecraft']['java_major']
    )
    harness.java_run(build, classes, dependencies, ORACLE, [str(witness_dir.resolve())], 'client')
    log = (build / 'client.log').read_text(encoding='utf-8', errors='replace')
    assert 'NETWORK_PARITY_WITNESS_OK' in log, log
    java_ver = subprocess.check_output(['java', '--version'], text=True).splitlines()[0].strip()
    lab.write_json(witness_dir / 'meta.json', dict(
        minecraft=version,
        client_jar_sha256=lab.digest(jars['client'], 'sha256'),
        java_version=java_ver,
        command='python3 tools/test_network_parity.py',
        oracle=f'mods/sunrise/src/test/java/dev/sunriselab/parity/{ORACLE}.java',
        inputs_sha256={
            'oracle': lab.digest(oracle_path, 'sha256'),
        },
    ))
    return witness_dir


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
    build = lab.LOCAL / 'network-parity'
    build.mkdir(parents=True, exist_ok=True)
    witness_dir = record(lock, jars, libraries, build)
    if args.record_only:
        print(f'wrote network witnesses under {witness_dir}')
        return
    with tempfile.TemporaryDirectory(prefix='sunrise-network-parity-') as tmp:
        scratch = Path(tmp)
        homes = {k: scratch / k for k in ('SUNRISE_HOME', 'SUNRISE_LAUNCHER_HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME')}
        env = lab.game_env(os.environ, **homes, RUST_MIN_STACK='16777216')
        env['SUNRISE_TEST_RESOURCES'] = str(args.resources.resolve())
        env['SUNRISE_NETWORK_WITNESS_DIR'] = str(witness_dir.resolve())

        for package, target_name, name, tests in (
            ('sunrise-protocol', 'sunrise_protocol', 'protocol-tests', PROTOCOL_TESTS),
            ('sunrise-client-lib', 'sunrise_client_lib', 'client-lib-tests', LIB_TESTS),
            ('sunrise-client', 'sunrise_client', 'client-tests', CLIENT_TESTS),
        ):
            binary = build_test_binary(build, package, target_name, env)
            log = run_logged([binary, '--ignored', '--exact', *tests], build, name, cwd=scratch, env=env)
            expected = f'{len(tests)} passed; 0 failed'
            assert expected in log.read_text(encoding='utf-8'), f'expected "{expected}" in {log}'

    total = len(PROTOCOL_TESTS) + len(LIB_TESTS) + len(CLIENT_TESTS)
    print(f'Network parity passed ({total} tests against {witness_dir})')


if __name__ == '__main__':
    main()
