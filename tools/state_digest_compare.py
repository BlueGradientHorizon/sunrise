#!/usr/bin/env python3
"""Compares two state digests step by step and names the first differing step per group: A/digest.txt B/digest.txt; a --coverage run's coverage.txt beside them is compared too."""
import argparse
import sys
from pathlib import Path

# `tick` and `dim` are compared too: a changed game time or level routing is a behaviour change.
GROUPS = (
    'tick', 'dim', 'walk_error', 'entities', 'randoms', 'hangings', 'cushions', 'other_entities', 'server_thrown_eyes',
    'fangs', 'lightning', 'end_crystals', 'dragon', 'breath_clouds', 'ominous_spawners', 'leash_knots',
    'fishing_hooks', 'server_thrown_potions', 'seats', 'entity_identity', 'entity_shared', 'players',
    'block_entities', 'level_random', 'counters', 'scheduled',
)


IGNORABLE_GROUPS = tuple(group for group in GROUPS if group not in ('tick', 'dim', 'walk_error'))


def parse(lines):
    """The running ticks' fields, and how many frozen ticks moved the level random."""
    running, frozen_moves, last_random = [], 0, None
    for line in lines:
        if not line.startswith('tick='):
            continue
        payload, separator, walk_error = line.rstrip().partition(' walk_error=')
        fields = dict(part.split('=', 1) for part in payload.split())
        if separator:
            fields['walk_error'] = walk_error
        if fields['run'] == '1':
            running.append(fields)
        elif last_random is not None and fields['level_random'] != last_random:
            frozen_moves += 1
        last_random = fields['level_random']
    return running, frozen_moves


def compare(a_lines, b_lines, ignore_groups=()):
    """Report first divergences by group and return whether all required groups match."""
    (a, fa), (b, fb) = parse(a_lines), parse(b_lines)
    if not a or not b:
        return [f'running ticks: {len(a)} vs {len(b)}', 'DIFFERENT'], False
    steps = range(min(len(a), len(b)))
    report = [f'running ticks: {len(a)} vs {len(b)}, game time {a[0]["tick"]}..{a[-1]["tick"]} vs '
              f'{b[0]["tick"]}..{b[-1]["tick"]}',
              f'frozen ticks that moved the level random: {fa} vs {fb}']
    same = len(a) == len(b)
    first_failure = first_ignored = None
    invalid_walks = {t for t in steps if 'walk_error' in a[t] or 'walk_error' in b[t]}
    walk_groups = {'entities', 'randoms', 'entity_identity', 'entity_shared'}
    ignored_differences = False
    for group in GROUPS:
        missing = False
        if group == 'walk_error':
            diff = sorted(invalid_walks)
        else:
            diff = []
            for t in steps:
                if t in invalid_walks and group in walk_groups:
                    continue
                left, right = a[t].get(group), b[t].get(group)
                absent = left is None or right is None
                if absent or left != right:
                    diff.append(t)
                    missing |= absent
        if diff:
            t = diff[0]
            ignored = not missing and group in IGNORABLE_GROUPS and group in ignore_groups
            ignored_differences |= ignored
            if ignored:
                if first_ignored is None or t < first_ignored[0]:
                    first_ignored = (t, group, True)
            else:
                same = False
                if first_failure is None or t < first_failure[0]:
                    first_failure = (t, group, False)
            label = 'DIFFERS (ignored)' if ignored else 'DIFFERS'
            report.append(f'{group:21} {label} on {len(diff)} steps, first step {t + 1} '
                          f'(tick {a[t]["tick"]} vs {b[t]["tick"]}): '
                          f'{a[t].get(group, "<missing>")} vs {b[t].get(group, "<missing>")}')
        else:
            label = 'identical'
            if invalid_walks and group in walk_groups:
                label = 'skipped on steps with a walk error'
            report.append(f'{group:21} {label}')
    first = first_failure or (first_ignored if same else None)
    if first is not None:
        t, group, ignored = first
        label = f'{group} (ignored)' if ignored else group
        report.insert(2, f'first differing group: {label} at step {t + 1} '
                         f'(tick {a[t]["tick"]} vs {b[t]["tick"]})')
    verdict = 'MATCH (ignored differences)' if ignored_differences else 'IDENTICAL'
    report.append(verdict if same else 'DIFFERENT')
    return report, same


def compare_coverage(a_dir, b_dir):
    """The coverage.txt lines of two --coverage runs (spots reached, saved spawner data), or None when neither has one."""
    a, b = (Path(d) / 'coverage.txt' for d in (a_dir, b_dir))
    if not a.exists() and not b.exists():
        return None
    if a.exists() != b.exists():
        return ['coverage       only in one run'], False
    same = a.read_text() == b.read_text()
    return [f'coverage       {"identical" if same else "DIFFERS"}: ' + ' | '.join(a.read_text().splitlines())], same


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('left', type=Path)
    parser.add_argument('right', type=Path)
    parser.add_argument('--ignore-group', action='append', choices=IGNORABLE_GROUPS, default=[])
    args = parser.parse_args()
    with args.left.open() as a, args.right.open() as b:
        report, same = compare(a, b, args.ignore_group)
    coverage = compare_coverage(args.left.parent, args.right.parent)
    if coverage:
        report = report[:-1] + coverage[0] + [report[-1] if same and coverage[1] else 'DIFFERENT']
        same = same and coverage[1]
    print('\n'.join(report))
    return 0 if same else 1


if __name__ == '__main__':
    sys.exit(main())
