#!/usr/bin/env python3
"""Record or verify the vanilla 26.3 fireball hit-tick rotation witness."""
import argparse
import os
from pathlib import Path
import subprocess
import sys

try:
    from . import lab, test_rust_parity as base
except ImportError:
    import lab
    import test_rust_parity as base

ORACLE = 'VanillaFireballHitTurnOracle'
SOURCE = lab.ROOT / f'mods/sunrise/src/test/java/dev/sunriselab/parity/{ORACLE}.java'
WITNESS = lab.ROOT / 'tools/witness/26.3/fireball_hit_turn.json'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--record', action='store_true',
                        help='record tools/witness/26.3/fireball_hit_turn.json from real vanilla 26.3')
    parser.add_argument('--out', type=Path, default=WITNESS)
    args = parser.parse_args()

    if args.prepare:
        lab.prepare()
    lock, jars, libraries = base.verify_artifacts()
    java_major = str(lock['minecraft']['java_major'])

    build = lab.LOCAL / 'fireball-hit-turn-parity'
    build.mkdir(parents=True, exist_ok=True)
    classes = build / 'classes'
    classes.mkdir(exist_ok=True)
    deps = os.pathsep.join(map(str, libraries))
    cp = str(jars['server']) + os.pathsep + deps

    compile_cmd = ['javac', '--release', java_major, '-proc:none', '-nowarn',
                   '-cp', cp, '-d', str(classes), str(SOURCE)]
    res = subprocess.run(compile_cmd, cwd=lab.ROOT)
    if res.returncode != 0:
        sys.exit(f'javac failed with exit code {res.returncode}')

    out_path = args.out.resolve() if args.record else (build / 'fireball_hit_turn.json').resolve()
    run_cmd = ['java', '-Xmx2G', '-cp', str(classes) + os.pathsep + cp,
               f'dev.sunriselab.parity.{ORACLE}', '-', str(out_path)]
    res = subprocess.run(run_cmd, cwd=lab.ROOT)
    if res.returncode != 0:
        sys.exit(f'{ORACLE} failed with exit code {res.returncode}')

    if not args.record:
        expected = WITNESS.read_text(encoding='utf-8')
        actual = out_path.read_text(encoding='utf-8')
        if actual != expected:
            sys.exit(f'Recorded witness differs from {WITNESS}; run with --record to refresh')
        print(f'Verified {WITNESS} matches fresh vanilla 26.3 run')
    return 0


if __name__ == '__main__':
    sys.exit(main())
