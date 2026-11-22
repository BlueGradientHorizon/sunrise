#!/usr/bin/env python3
"""Compare terrain section vertical render-distance clamping against real Minecraft 26.3.

Compiles and runs VanillaSectionVisibilityOracle against the pinned 26.3 client jar on JDK 25,
writing the witness and meta.json into .local/witness/<version>/visibility/, then runs the
6 Rust tests in sunrise-client-render against it via SUNRISE_SECTION_VISIBILITY_WITNESS."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

try:
    from . import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking
except ImportError:
    import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking

ORACLE = 'VanillaSectionVisibilityOracle'
GENPVP_NON_EMPTY_SY = [0, 1, 2, 3, 8, 9, 10, 11]

TESTS = [
    'gfx::culling_control::tests::occlusion_radius_matches_vanilla_26_3_view_distance',
    'gfx::gpu_culling::visibility_tests::gpu_culling_tables_and_keeps_enforce_vanilla_26_3_vertical_clamp',
    'gfx::occlusion::visibility_tests::occlusion_reaches_exact_vanilla_26_3_sections_across_heights_and_radii',
    'gfx::occlusion::visibility_tests::occlusion_visible_for_camera_is_unknown_outside_vanilla_vertical_band',
    'gfx::resident::visibility_tests::resident_walk_enforces_vanilla_26_3_vertical_clamp_without_cave_culling',
    'gfx::view_sections::tests::a_view_respects_vanilla_26_3_vertical_render_distance',
]


def build_cases():
    cases = [
        # 1-5: Genpvp menu geometry (min_y=-64, 24 sections sy=-4..19, non-empty at sy={0..3,8..11}, empty at 12..19).
        {
            'name': 'genpvp_menu_level',
            'min_y': -64, 'sections': 24, 'render_distance': 2,
            'camera': [0.5, 301.620015, 0.5], 'smart_cull': True,
            'loaded_chunk_range': [-3, 3, -3, 3],
            'non_empty_world_sy': GENPVP_NON_EMPTY_SY,
            'frustum': {'yaw': 0.0, 'pitch': 0.0, 'fov': 70.0, 'aspect': 1.6},
        },
        {
            'name': 'genpvp_menu_looking_down',
            'min_y': -64, 'sections': 24, 'render_distance': 2,
            'camera': [0.5, 301.620015, 0.5], 'smart_cull': True,
            'loaded_chunk_range': [-3, 3, -3, 3],
            'non_empty_world_sy': GENPVP_NON_EMPTY_SY,
            'frustum': {'yaw': 0.0, 'pitch': 90.0, 'fov': 70.0, 'aspect': 1.6},
        },
        {
            'name': 'genpvp_menu_looking_down_no_smart_cull',
            'min_y': -64, 'sections': 24, 'render_distance': 2,
            'camera': [0.5, 301.620015, 0.5], 'smart_cull': False,
            'loaded_chunk_range': [-3, 3, -3, 3],
            'non_empty_world_sy': GENPVP_NON_EMPTY_SY,
            'frustum': {'yaw': 0.0, 'pitch': 90.0, 'fov': 70.0, 'aspect': 1.6},
        },
        {
            'name': 'genpvp_menu_rd6_misses_lobby',
            'min_y': -64, 'sections': 24, 'render_distance': 6,
            'camera': [0.5, 301.620015, 0.5], 'smart_cull': True,
            'non_empty_world_sy': GENPVP_NON_EMPTY_SY,
            'frustum': {'yaw': 0.0, 'pitch': 90.0, 'fov': 70.0, 'aspect': 1.6},
        },
        {
            'name': 'genpvp_menu_rd7_reaches_lobby',
            'min_y': -64, 'sections': 24, 'render_distance': 7,
            'camera': [0.5, 301.620015, 0.5], 'smart_cull': True,
            'non_empty_world_sy': GENPVP_NON_EMPTY_SY,
            'frustum': {'yaw': 0.0, 'pitch': 90.0, 'fov': 70.0, 'aspect': 1.6},
        },
        # 6-11: Exact vertical clamp across heights, render distances, smart_cull on/off, and Nether height.
        {
            'name': 'vertical_clamp_overworld_rd2_mid',
            'min_y': -64, 'sections': 24, 'render_distance': 2,
            'camera': [8.5, 104.5, 8.5], 'smart_cull': True,
        },
        {
            'name': 'vertical_clamp_overworld_rd2_mid_no_smart_cull',
            'min_y': -64, 'sections': 24, 'render_distance': 2,
            'camera': [8.5, 104.5, 8.5], 'smart_cull': False,
        },
        {
            'name': 'vertical_clamp_overworld_rd3_mid',
            'min_y': -64, 'sections': 24, 'render_distance': 3,
            'camera': [8.5, 72.5, 8.5], 'smart_cull': True,
        },
        {
            'name': 'vertical_clamp_overworld_rd4_high',
            'min_y': -64, 'sections': 24, 'render_distance': 4,
            'camera': [8.5, 296.5, 8.5], 'smart_cull': True,
        },
        {
            'name': 'vertical_clamp_overworld_rd4_low',
            'min_y': -64, 'sections': 24, 'render_distance': 4,
            'camera': [8.5, -56.5, 8.5], 'smart_cull': True,
        },
        {
            'name': 'vertical_clamp_overworld_rd8_mid',
            'min_y': -64, 'sections': 24, 'render_distance': 8,
            'camera': [8.5, 72.5, 8.5], 'smart_cull': True,
        },
        {
            'name': 'vertical_clamp_nether_rd3_top',
            'min_y': 0, 'sections': 16, 'render_distance': 3,
            'camera': [8.5, 248.5, 8.5], 'smart_cull': True,
        },
        # 12-17: Camera outside world build height (above maxSectionY and below minSectionY).
        {
            'name': 'camera_above_world_rd2_near',
            'min_y': -64, 'sections': 24, 'render_distance': 2,
            'camera': [8.5, 328.5, 8.5], 'smart_cull': True,
            'frustum': {'yaw': 0.0, 'pitch': 90.0, 'fov': 70.0, 'aspect': 1.6},
        },
        {
            'name': 'camera_above_world_rd2_far',
            'min_y': -64, 'sections': 24, 'render_distance': 2,
            'camera': [8.5, 408.5, 8.5], 'smart_cull': True,
            'frustum': {'yaw': 0.0, 'pitch': 90.0, 'fov': 70.0, 'aspect': 1.6},
        },
        {
            'name': 'camera_above_world_rd2_far_no_smart_cull',
            'min_y': -64, 'sections': 24, 'render_distance': 2,
            'camera': [8.5, 408.5, 8.5], 'smart_cull': False,
            'frustum': {'yaw': 0.0, 'pitch': 90.0, 'fov': 70.0, 'aspect': 1.6},
        },
        {
            'name': 'camera_above_world_rd4_mid',
            'min_y': -64, 'sections': 24, 'render_distance': 4,
            'camera': [8.5, 360.5, 8.5], 'smart_cull': True,
            'frustum': {'yaw': 0.0, 'pitch': 90.0, 'fov': 70.0, 'aspect': 1.6},
        },
        {
            'name': 'camera_below_world_rd2_near',
            'min_y': -64, 'sections': 24, 'render_distance': 2,
            'camera': [8.5, -72.5, 8.5], 'smart_cull': True,
            'frustum': {'yaw': 0.0, 'pitch': -90.0, 'fov': 70.0, 'aspect': 1.6},
        },
        {
            'name': 'camera_below_world_rd2_far',
            'min_y': -64, 'sections': 24, 'render_distance': 2,
            'camera': [8.5, -152.5, 8.5], 'smart_cull': True,
            'frustum': {'yaw': 0.0, 'pitch': -90.0, 'fov': 70.0, 'aspect': 1.6},
        },
        # 18-19: World boundary sealing (no stepping into sy < minSectionY or sy > maxSectionY).
        {
            'name': 'cave_wall_at_bottom_world_boundary',
            'min_y': -64, 'sections': 24, 'render_distance': 3,
            'camera': [8.5, -56.5, 8.5], 'smart_cull': True,
            'vis_overrides': [
                {'section': [1, -4, 0], 'mask': 0},
                {'section': [-1, -4, 0], 'mask': 0},
                {'section': [0, -4, 1], 'mask': 0},
                {'section': [0, -4, -1], 'mask': 0},
                {'section': [0, -3, 0], 'mask': 0},
            ],
        },
        {
            'name': 'cave_wall_at_top_world_boundary',
            'min_y': -64, 'sections': 24, 'render_distance': 3,
            'camera': [8.5, 312.5, 8.5], 'smart_cull': True,
            'vis_overrides': [
                {'section': [1, 19, 0], 'mask': 0},
                {'section': [-1, 19, 0], 'mask': 0},
                {'section': [0, 19, 1], 'mask': 0},
                {'section': [0, 19, -1], 'mask': 0},
                {'section': [0, 18, 0], 'mask': 0},
            ],
        },
        # 20: Solid floor at sy=8 with single open shaft at (1,8,0), camera at sy=10, rd=4 (reaches down to sy=6, not 5).
        {
            'name': 'cave_floor_with_shaft_and_vertical_clamp',
            'min_y': -64, 'sections': 24, 'render_distance': 4,
            'camera': [8.5, 168.5, 8.5], 'smart_cull': True,
            'vis_overrides': [
                {'section': [x, 8, z], 'mask': 0}
                for x in range(-4, 5) for z in range(-4, 5)
                if not (x == 1 and z == 0)
            ],
        },
        # 21: Distant ray culling (>60 blocks) combined with vertical clamp at rd=8.
        {
            'name': 'distant_ray_and_vertical_clamp_rd8',
            'min_y': -64, 'sections': 24, 'render_distance': 8,
            'camera': [3.3, 134.3, -5.6], 'smart_cull': True,
            'vis_overrides': [
                {'section': [2, y, z], 'mask': 0}
                for z in range(-2, 3) for y in range(0, 17)
            ],
        },
        # 22: Solid roof at sy=4 with single hole at (2,4,0): vanilla never turns back -X above the roof.
        {
            'name': 'cave_roof_hole_no_sky_band_overdraw',
            'min_y': -64, 'sections': 24, 'render_distance': 4,
            'camera': [8.5, 40.5, 8.5], 'smart_cull': True,
            'vis_overrides': [
                {'section': [x, 4, z], 'mask': 0}
                for x in range(-4, 5) for z in range(-4, 5)
                if not (x == 2 and z == 0)
            ],
        },
    ]
    return {'cases': cases}


def record(lock, jars, libraries, build):
    version = lock['minecraft']['version']
    witness_dir = lab.LOCAL / 'witness' / version / 'visibility'
    witness_dir.mkdir(parents=True, exist_ok=True)
    cases_path = build / 'cases.json'
    cases_path.write_text(json.dumps(build_cases(), indent=2) + '\n', encoding='utf-8')
    classes, dependencies, oracle_path = harness.compile_oracle(
        build, ORACLE, jars, libraries, lock['minecraft']['java_major']
    )
    raw_out = build / 'witness.json'
    harness.java_run(build, classes, dependencies, ORACLE, [cases_path, raw_out], 'client')
    data = json.loads(raw_out.read_text(encoding='utf-8'))
    lines = [
        '{',
        f'  "meta": {json.dumps(data["meta"], separators=(",", ":"))},',
        '  "cases": [',
    ]
    for i, case in enumerate(data['cases']):
        comma = ',' if i + 1 < len(data['cases']) else ''
        lines.append(f'    {json.dumps(case, separators=(",", ":"))}{comma}')
    lines.extend(['  ]', '}'])
    witness = witness_dir / 'witness.json'
    witness.write_text('\n'.join(lines) + '\n', encoding='utf-8')
    java_ver = subprocess.check_output(['java', '--version'], text=True).splitlines()[0].strip()
    lab.write_json(witness_dir / 'meta.json', dict(
        minecraft=version,
        client_jar_sha256=lab.digest(jars['client'], 'sha256'),
        java_version=java_ver,
        command='python3 tools/test_visibility_parity.py',
        oracle=f'mods/sunrise/src/test/java/dev/sunriselab/parity/{ORACLE}.java',
        inputs_sha256={
            'oracle': lab.digest(oracle_path, 'sha256'),
            'cases': lab.digest(cases_path, 'sha256'),
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
    parser.add_argument('--record-only', action='store_true')
    args = parser.parse_args()
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'visibility-parity'
    build.mkdir(parents=True, exist_ok=True)
    witness = record(lock, jars, libraries, build)
    if args.record_only:
        print(f'wrote {witness} ({witness.stat().st_size} bytes)')
        return
    env = dict(os.environ, SUNRISE_SECTION_VISIBILITY_WITNESS=str(witness.resolve()))
    binary = build_test_binary(build, 'sunrise-client-render', 'sunrise_client_render', env)
    log = run_logged([binary, '--ignored', '--exact', *TESTS], build, 'render-tests', env=env)
    expected = f'{len(TESTS)} passed; 0 failed'
    assert expected in log.read_text(encoding='utf-8'), f'expected "{expected}" in {log}'
    print(f'Section visibility parity passed ({len(TESTS)} tests against {witness})')


if __name__ == '__main__':
    main()
