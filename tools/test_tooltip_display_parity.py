#!/usr/bin/env python3
"""Compare item tooltip display visibility, styled names, and tooltip_style sprites with vanilla 26.3."""
import argparse
import json
from pathlib import Path
try:
    from . import lab, oracle_harness as harness, test_model_baking_parity as baking, test_rust_parity as base
except ImportError:
    import lab, oracle_harness as harness, test_model_baking_parity as baking, test_rust_parity as base

STYLE_CASES = [
    ('custom_namespace', 'minecraft:stone', 'lobby:gold', (40, 30, 60, 8)),
    ('minecraft_namespace', 'minecraft:stick', 'minecraft:fancy', (40, 30, 60, 8)),
    ('nested_path', 'minecraft:stone', 'lobby:themes/royal', (20, 20, 50, 8)),
    ('bare_default_namespace', 'minecraft:stick', 'fancy', (12, 16, 48, 18)),
]


def cases(build):
    values = [
        (False, False, False),
        (False, False, True),
        (False, True, False),
        (False, True, True),
        (True, False, False),
        (True, False, True),
        (True, True, False),
        (True, True, True),
    ]
    path = build / 'cases.txt'
    # Lowercase: Rust's bool parser takes only true/false (Java's parseBoolean ignores case).
    names = [
        'Plain name',
        {'text': '', 'extra': [
            {'text': 'Red', 'color': '#ff0000', 'italic': False},
            {'text': 'Blue', 'color': '#0000ff'}]},
        {'text': 'Gold', 'color': 'gold', 'bold': True, 'italic': False},
        [{'text': 'List ', 'bold': True}, {'text': 'tail', 'bold': False}],
    ]
    styles = [
        f'style\t{x}\t{y}\t{w}\t{h}\t{{id:"{item}",count:1,components:{{"minecraft:tooltip_style":"{style}"}}}}\n'
        for _, item, style, (x, y, w, h) in STYLE_CASES
    ]
    path.write_text('\n'.join('\t'.join(str(v).lower() for v in row) for row in values) + '\n'
                    + ''.join('name\t' + json.dumps(name, ensure_ascii=False) + '\n' for name in names)
                    + ''.join(styles), encoding='utf-8')
    return path, len(values) + len(names) + len(STYLE_CASES)


def record_style_witness(output_path):
    """Run VanillaTooltipDisplayOracle on pinned 26.3 and record tooltip_style sprite draws."""
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'tooltip-display-parity'
    build.mkdir(parents=True, exist_ok=True)
    input_path, _ = cases(build)
    classes, dependencies, _ = harness.compile_oracle(
        build, 'VanillaTooltipDisplayOracle', jars, libraries, lock['minecraft']['java_major'])
    client_out = build / 'client.txt'
    harness.java_run(build, classes, dependencies, 'VanillaTooltipDisplayOracle',
                     [input_path, client_out], 'client')
    style_lines = [line.removeprefix('style\t')
                   for line in client_out.read_text(encoding='utf-8').splitlines()
                   if line.startswith('style\t')]
    if len(style_lines) != len(STYLE_CASES):
        raise AssertionError(f'Expected {len(STYLE_CASES)} style lines, got {len(style_lines)}')
    recorded = []
    for (label, item, style_input, box), raw in zip(STYLE_CASES, style_lines):
        draws = []
        for entry in raw.split('|'):
            sprite, x, y, w, h, color = entry.split(',')
            draws.append({
                'sprite': sprite,
                'x': int(x),
                'y': int(y),
                'w': int(w),
                'h': int(h),
                'color': int(color),
            })
        recorded.append({
            'label': label,
            'item': item,
            'tooltip_style': style_input,
            'box': list(box),
            'draws': draws,
        })
    dest = Path(output_path)
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps({
        'minecraft_version': lock['minecraft']['version'],
        'oracle': 'VanillaTooltipDisplayOracle',
        'cases': recorded,
    }, indent=2) + '\n', encoding='utf-8')
    print(f'Recorded {len(recorded)} tooltip_style sprite cases for {lock["minecraft"]["version"]}: {dest}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', type=Path, help='Record tooltip_style sprite witness JSON using Java only')
    args = parser.parse_args()
    if args.record:
        record_style_witness(args.record)
        return
    harness.differential(
        'tooltip-display-parity', 'VanillaTooltipDisplayOracle', 'tooltip_display_fixture', cases,
        'Tooltip visibility, styled custom-name inheritance, and 26.3 tooltip_style sprite selection',
        sources=[
            lab.ROOT / 'crates/sunrise-client/src/game/blocks/stations.rs',
            lab.ROOT / 'crates/sunrise-client/src/item_tooltip.rs',
            lab.ROOT / 'crates/sunrise-client-render/src/ui.rs',
            lab.ROOT / 'crates/sunrise-items/src/item_data.rs',
        ],
        expected_lines=12 + len(STYLE_CASES))


if __name__ == '__main__':
    main()
