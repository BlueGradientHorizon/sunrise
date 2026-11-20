#!/usr/bin/env python3
"""Compare real 26.3's dialog codec with the answers Sunrise's tests pin.

`tools/fixtures/dialog_codec_cases.json` lists dialogs (decoded by `Dialog.DIRECT_CODEC`) and small data
packs (loaded by `RegistryDataLoader`, as a world open loads them), each with whether vanilla accepted it and
the codec's own message when it did not. The oracle runs the real codecs over the same cases and this
harness fails on any answer that differs. Sunrise's side is the unit test
`dialog_admission_answers_as_the_real_codecs_do` (`sunrise-client`, `lan/data_pack_registries.rs`), which
reads the same file."""
import json
import os
import subprocess

try:
    from . import lab, test_rust_parity as base
except ImportError:
    import lab
    import test_rust_parity as base

FIXTURE = lab.ROOT / 'tools/fixtures/dialog_codec_cases.json'


def main():
    build = lab.LOCAL / 'dialog-codec-parity'
    build.mkdir(parents=True, exist_ok=True)
    lock, jars, libraries = base.verify_artifacts()
    oracle = lab.ROOT / 'mods/sunrise/src/test/java/dev/sunriselab/parity/VanillaDialogCodecOracle.java'
    classes = build / 'classes'
    classes.mkdir(exist_ok=True)
    dependencies = os.pathsep.join(map(str, libraries))

    def run(command, name):
        with (build / (name + '.log')).open('w', encoding='utf-8') as log:
            base.run(list(map(str, command)), stdout=log, stderr=subprocess.STDOUT)

    run(['javac', '--release', lock['minecraft']['java_major'], '-proc:none', '-sourcepath',
         lab.ROOT / 'mods/sunrise/src/test/java', '-cp', str(jars['server']) + os.pathsep + dependencies,
         '-d', classes, oracle], 'compile-java')
    answers = build / 'answers.jsonl'
    run(['java', '-Xmx1g', '-Djava.awt.headless=true', '-cp',
         os.pathsep.join([str(classes), str(jars['server']), dependencies]),
         'dev.sunriselab.parity.VanillaDialogCodecOracle', FIXTURE, answers], 'oracle')
    recorded = json.loads(FIXTURE.read_text(encoding='utf-8'))
    recorded = {case['name']: case for case in recorded['cases'] + recorded['packs']}
    seen = set()
    for line in answers.read_text(encoding='utf-8').splitlines():
        answer = json.loads(line)
        case = recorded[answer['name']]
        seen.add(answer['name'])
        if (answer['accept'], answer.get('error')) != (case['accept'], case.get('vanilla_error')):
            raise AssertionError(f"{answer['name']}: fixture says {case['accept']} {case.get('vanilla_error')!r}, "
                                 f"real codec says {answer['accept']} {answer.get('error')!r}")
    if seen != set(recorded):
        raise AssertionError('the oracle skipped cases: ' + ', '.join(sorted(set(recorded) - seen)))
    print(f'{len(seen)} dialog codec answers match the fixture')


if __name__ == '__main__':
    main()
