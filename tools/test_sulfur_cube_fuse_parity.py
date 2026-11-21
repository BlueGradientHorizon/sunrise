#!/usr/bin/env python3
"""Compare a sulfur cube's server load/save/tick/sync and client MAX_FUSE sync/tick with pinned vanilla 26.3.

Server mode: Java loads each case on a detached ServerLevel through Entity.load (`fuse` absent, at
the signed boundaries, as every numeric tag type and as a string; `Invulnerable` absent, false and
true), saves it, reads the MAX_FUSE a late tracker's snapshot carries, runs tickFuse three times and
reads what packDirty sends to trackers already following it, then what a later load of `fuse:-1`
sends them. Sunrise runs the production load, save, `tick_fuse` and `mob_meta`, with `send_dirty`'s
rule for what goes to existing trackers.

Client mode: Java creates a sulfur cube on a detached ClientLevel, drives `SynchedEntityData.assignValues`
(invoking `SulfurCube.onSyncedDataUpdated` for `MAX_FUSE` and `FROM_BUCKET`) and client `tickFuse`
across signed boundaries (-2147483648, -5, -1, 0, 1, 40, 2147483647), re-syncing `MAX_FUSE` after
ticking, and syncing `FROM_BUCKET`. Sunrise runs the production `client_defaults`,
`remote_mob_data::apply` and `sulfur_cube::client_tick`.

Pass `--record` to run the pinned vanilla 26.3 Java oracle alone and refresh the committed witnesses
under `tools/witness/26.3/`. Every case is a fresh observation; a missing row fails.
"""
import argparse
import os
from pathlib import Path
import tempfile
try:
    from . import (
        lab,
        oracle_harness as harness,
        test_model_baking_parity as baking,
        test_rust_parity as base,
        integration_env,
    )
except ImportError:
    import lab, oracle_harness as harness, test_model_baking_parity as baking, test_rust_parity as base, integration_env

SERVER_LINES = 1 + 14 * 3
CLIENT_LINES = 1 + 24
SERVER_WITNESS = lab.ROOT / 'tools/witness/26.3/sulfur_cube_fuse_server.txt'
CLIENT_WITNESS = lab.ROOT / 'tools/witness/26.3/sulfur_cube_fuse_client.txt'


def run_oracle(build, lock, jars, libraries, server_out, client_out):
    java_root = lab.ROOT / 'mods/sunrise/src/test/java'
    oracle = java_root / 'dev/sunriselab/parity/VanillaSulfurCubeFuseOracle.java'
    classes = build / 'classes'
    classes.mkdir(exist_ok=True)
    dependencies = os.pathsep.join(map(str, [jars['client'], *libraries]))
    harness.run_logged(['javac', '--release', str(lock['minecraft']['java_major']), '-proc:none',
                        '-sourcepath', java_root, '-cp', dependencies, '-d', classes, oracle],
                       build, 'compile-java')
    harness.run_logged(['java', '-Xmx1G', '-Djava.awt.headless=true', '-cp',
                        str(classes) + os.pathsep + dependencies,
                        'dev.sunriselab.parity.VanillaSulfurCubeFuseOracle', server_out, client_out],
                       build, 'vanilla')
    return oracle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--resources', type=Path,
                        default=Path(os.environ.get('SUNRISE_TEST_RESOURCES', lab.RESOURCES)))
    parser.add_argument('--record', action='store_true',
                        help='Run only the pinned vanilla Java oracle and write tools/witness/26.3/ witnesses')
    args = parser.parse_args()
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'rust-sulfur-cube-fuse-parity'
    build.mkdir(parents=True, exist_ok=True)
    if args.record:
        run_oracle(build, lock, jars, libraries, SERVER_WITNESS, CLIENT_WITNESS)
        server_lines = len(SERVER_WITNESS.read_text(encoding='utf-8').splitlines())
        client_lines = len(CLIENT_WITNESS.read_text(encoding='utf-8').splitlines())
        if server_lines != SERVER_LINES or client_lines != CLIENT_LINES:
            raise AssertionError(
                f'Unexpected witness line counts: server={server_lines} (expected {SERVER_LINES}), '
                f'client={client_lines} (expected {CLIENT_LINES})'
            )
        print(f'Recorded {server_lines} server and {client_lines} client witness lines for {lock["minecraft"]["version"]}', flush=True)
        return

    resources = args.resources.resolve()
    if not (resources / 'entity-types.json').is_file():
        raise SystemExit('prepared resources required; pass --resources PATH')
    report = build / 'report.json'
    report.unlink(missing_ok=True)
    for name in ['server.txt', 'client.txt', 'rust_server.txt', 'rust_client.txt']:
        (build / name).unlink(missing_ok=True)
    oracle = lab.ROOT / 'mods/sunrise/src/test/java/dev/sunriselab/parity/VanillaSulfurCubeFuseOracle.java'
    sources = [Path(__file__), oracle, lab.ROOT / 'tools/lab.py',
               lab.ROOT / 'tools/oracle_harness.py', lab.ROOT / 'tools/test_rust_parity.py',
               lab.ROOT / 'tools/test_model_baking_parity.py',
               lab.ROOT / 'tools/integration_env.py', lab.ROOT / 'Cargo.toml', lab.ROOT / 'Cargo.lock',
               SERVER_WITNESS, CLIENT_WITNESS,
               lab.ROOT / 'crates/sunrise-client/src/mobs.rs',
               lab.ROOT / 'crates/sunrise-client/src/entity_nbt.rs',
               lab.ROOT / 'crates/sunrise-client/src/mobs/sulfur_cube.rs',
               lab.ROOT / 'crates/sunrise-client/src/game/network/lan/mod.rs',
               lab.ROOT / 'crates/sunrise-client/src/game/network/lan/sulfur_cube_meta_tests.rs',
               lab.ROOT / 'crates/sunrise-client/src/game/network/remote_mob_data.rs',
               lab.ROOT / 'crates/sunrise-world/src/nbt_path.rs']
    hashes = base.source_hashes(sources)
    run_oracle(build, lock, jars, libraries, build / 'server.txt', build / 'client.txt')
    with tempfile.TemporaryDirectory(prefix='sunrise-sulfur-cube-fuse-') as scratch:
        env = integration_env.game_environment(os.environ, scratch)
        env.update(SUNRISE_TEST_RESOURCES=str(resources),
                   SUNRISE_SULFUR_CUBE_FUSE_OUTPUT=str(build / 'rust_server.txt'),
                   SUNRISE_SULFUR_CUBE_CLIENT_FUSE_OUTPUT=str(build / 'rust_client.txt'))
        env.setdefault('CARGO_BUILD_JOBS', '1')
        env.setdefault('RUST_TEST_THREADS', '2')
        harness.run_logged(['cargo', 'test', '--locked', '-p', 'sunrise-client', '--lib', '--',
                            'a_loaded_sulfur_cube_fuse_matches_the_pinned_oracle',
                            'sulfur_cube_max_fuse_sets_the_client_fuse'], build, 'rust', env=env)
    matched_server = harness.compare(build / 'server.txt', build / 'rust_server.txt', SERVER_LINES)
    matched_client = harness.compare(build / 'client.txt', build / 'rust_client.txt', CLIENT_LINES)
    base.ensure_sources_unchanged(sources, hashes)
    lab.write_json(report, dict(schema=1, minecraft=lock['minecraft']['version'],
        sources_sha256=hashes, client_jar_sha256=lab.digest(jars['client'], 'sha256'),
        matched_server_lines=matched_server, matched_client_lines=matched_client,
        matched_output_lines=matched_server + matched_client,
        server_output_sha256=lab.digest(build / 'rust_server.txt', 'sha256'),
        client_output_sha256=lab.digest(build / 'rust_client.txt', 'sha256'),
        scope=__doc__))
    print(f'{matched_server} server and {matched_client} client sulfur cube fuse lines match pinned {lock["minecraft"]["version"]}: {report}', flush=True)


if __name__ == '__main__':
    main()
