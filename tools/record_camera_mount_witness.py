#!/usr/bin/env python3
"""Record the real 26.3 Camera.alignWithEntity of a rider on a living or non-living vehicle.

Runs VanillaCameraOracle on a zombie-sized player shell or a marker shell sitting on a real HappyGhast, Ghast, Zombie,
ArmorStand or Boat, third person back and front, in open air, and stores the cases, the real frame line and the real
camera distance in crates/sunrise-client/src/game/network/tests/camera_mount_witness.json for the Rust test to read."""
import json

try:
    from . import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking
except ImportError:
    import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking

OUT = lab.ROOT / 'crates/sunrise-client/src/game/network/tests/camera_mount_witness.json'
NAN = float('nan')
ZOMBIE_EYE = 1.74
BOAT_EYE = 0.5625
RIDER_DISTANCES = [4.0, 10.0]
RIDER_SCALES = [1.0, 2.0]
# (kind, scale base, camera distance base): bases go through the real attribute sanitizing
VEHICLES = [None,
            ('happy_ghast', 1.0, 8.0), ('happy_ghast', 0.5, 8.0), ('happy_ghast', 2.0, 8.0), ('happy_ghast', 0.0625, 8.0),
            ('happy_ghast', NAN, 8.0), ('happy_ghast', 1.0, 16.0), ('happy_ghast', 1.0, 32.0), ('happy_ghast', 1.0, 100.0),
            ('happy_ghast', 1.0, 0.0), ('happy_ghast', 1.0, -0.0), ('happy_ghast', 1.0, 4.0),
            ('ghast', 1.0, 8.0), ('ghast', 2.0, 8.0), ('zombie', 1.0, 4.0), ('zombie', 4.0, 8.0),
            ('armor_stand', 1.0, 4.0), ('boat', 1.0, 4.0)]
CAMERA_TYPES = [0, 1, 2]
PARTIALS = [0.0, 0.5, 1.0]


def mount_input(vehicle):
    return None if vehicle is None else [lab.dbits(vehicle[1]), lab.dbits(vehicle[2]), vehicle[0]]


def entity(rider, scale, dist, mount):
    eye = ZOMBIE_EYE * scale if rider == 'zombie' else BOAT_EYE
    common = dict(pos=[lab.dbits(10.25), lab.dbits(64.0), lab.dbits(-3.75)],
                  prev=[lab.dbits(10.25), lab.dbits(64.0), lab.dbits(-3.75)],
                  xr=lab.fbits(22.5), yr=lab.fbits(45.0), xro=lab.fbits(22.5), yro=lab.fbits(45.0),
                  eye=lab.fbits(eye), mount=mount_input(mount))
    if rider == 'boat':
        return dict(kind='marker', **common)
    return dict(kind='player', flying=False, ws=lab.fbits(0.1), speed=lab.dbits(0.1), use=None,
                scale_attr=lab.dbits(scale), dist=lab.dbits(dist), sleeping=None, dead=None, **common)


def cases():
    for vehicle in VEHICLES:
        for rider in ('zombie', 'boat'):
            riders = [(s, d) for s in RIDER_SCALES for d in RIDER_DISTANCES] if rider == 'zombie' else [(1.0, 4.0)]
            for scale, dist in riders:
                for camera_type in CAMERA_TYPES:
                    for pt in PARTIALS if camera_type else PARTIALS[:1]:
                        ops = [['entity', entity(rider, scale, dist, vehicle)], ['tick'],
                               ['frame', lab.fbits(pt), 0, False, lab.fbits(1600.0), lab.fbits(1000.0), lab.fbits(768.0)]]
                        yield dict(rider=rider, rider_scale=lab.dbits(scale), rider_distance=lab.dbits(dist),
                                   vehicle=None if vehicle is None else [vehicle[0], lab.dbits(vehicle[1]), lab.dbits(vehicle[2])],
                                   eye=lab.fbits(ZOMBIE_EYE * scale if rider == 'zombie' else BOAT_EYE),
                                   camera_type=camera_type, partial=pt,
                                   case=dict(t='camera', fov=70, effect=lab.dbits(1.0), type=camera_type, cells={},
                                             report_distance=True, ops=ops))


def main():
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'camera-mount-witness'
    build.mkdir(exist_ok=True)
    all_cases = list(cases())
    inp = build / 'cases.jsonl'
    inp.write_text(''.join(json.dumps(c['case'], separators=(',', ':')) + '\n' for c in all_cases))
    classes, dependencies, _ = harness.compile_oracle(build, 'VanillaCameraOracle', jars, libraries, lock['minecraft']['java_major'])
    out = build / 'client.txt'
    harness.java_run(build, classes, dependencies, 'VanillaCameraOracle', [inp, out], 'client')
    sections, name = {}, None
    for line in out.read_text().splitlines():
        if line.startswith('== '):
            name = int(line[3:])
            sections[name] = []
        else:
            sections[name].append(line)
    records = []
    for n, c in enumerate(all_cases):
        frame = next(l for l in sections[n] if l.startswith('frame')).split(' ')
        distance = next(l for l in sections[n] if l.startswith('dist ')).split(' ')[1]
        records.append(dict(rider=c['rider'], rider_scale=c['rider_scale'], rider_distance=c['rider_distance'],
                            vehicle=c['vehicle'], eye=c['eye'], camera_type=c['camera_type'], partial=c['partial'],
                            distance=distance, expected=frame[1:20]))
    OUT.write_text(json.dumps(dict(
        minecraft=lock['minecraft']['version'], client_jar_sha256=lab.digest(jars['client'], 'sha256'),
        how='tools/record_camera_mount_witness.py: VanillaCameraOracle (real Camera.alignWithEntity, getMaxZoom over open air) '
            'on the real 26.3 client jar; "expected" is the real frame line fields 1..19 (position bits, xRot, yRot, rotation, '
            'forwards, up, left, detached) and "distance" the float bits of max(scale*distance, mountScale*mountDistance). '
            'A "vehicle" is [class, scale base bits, camera_distance base bits] of a real vehicle class with its own '
            'attribute map; the rider is a zombie-sized player shell or a marker (boat-sized eye). Pose is fixed: '
            'pos (10.25, 64, -3.75), yaw 45, pitch 22.5.',
        records=records), indent=1) + '\n')
    print(f'{len(records)} cases -> {OUT}')


if __name__ == '__main__':
    main()
