#!/usr/bin/env python3
"""Records tools/fixtures/26.3/set_camera_witness.tsv: real 26.3 ClientPacketListener.handleSetCamera over a level
holding a player stand-in (id 7) and one entity (id 40) of each listed class; the camera entity id after the
packets 40, 7, 40, 99 (unknown), 40, 99."""
try:
    from . import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking
except ImportError:
    import lab, oracle_harness as harness, test_rust_parity as base, test_model_baking_parity as baking

KINDS = [
    ('minecraft:zombie', 'net.minecraft.world.entity.monster.zombie.Zombie'),
    ('minecraft:armor_stand', 'net.minecraft.world.entity.decoration.ArmorStand'),
    ('minecraft:oak_boat', 'net.minecraft.world.entity.vehicle.boat.Boat'),
    ('minecraft:item_frame', 'net.minecraft.world.entity.decoration.ItemFrame'),
    ('minecraft:tnt', 'net.minecraft.world.entity.item.PrimedTnt'),
    ('minecraft:minecart', 'net.minecraft.world.entity.vehicle.minecart.Minecart'),
    ('minecraft:arrow', 'net.minecraft.world.entity.projectile.arrow.Arrow'),
    ('minecraft:end_crystal', 'net.minecraft.world.entity.boss.enderdragon.EndCrystal'),
    ('minecraft:item', 'net.minecraft.world.entity.item.ItemEntity'),
    ('minecraft:experience_orb', 'net.minecraft.world.entity.ExperienceOrb'),
    ('minecraft:falling_block', 'net.minecraft.world.entity.item.FallingBlockEntity'),
    ('minecraft:painting', 'net.minecraft.world.entity.decoration.painting.Painting'),
    ('minecraft:interaction', 'net.minecraft.world.entity.Interaction'),
    ('minecraft:block_display', 'net.minecraft.world.entity.Display$BlockDisplay'),
]


def main():
    lock, jars, _ = base.verify_artifacts()
    libraries = baking.client_libraries()
    build = lab.LOCAL / 'set-camera-witness'
    build.mkdir(exist_ok=True)
    source = build / 'kinds.tsv'
    source.write_text(''.join(f'{label}\t{cls}\n' for label, cls in KINDS))
    classes, dependencies, _ = harness.compile_oracle(build, 'VanillaSetCameraOracle', jars, libraries,
                                                      lock['minecraft']['java_major'])
    out = build / 'client.txt'
    harness.java_run(build, classes, dependencies, 'VanillaSetCameraOracle', [out, source], 'client')
    target = lab.ROOT / 'tools/fixtures/26.3/set_camera_witness.tsv'
    header = '# minecraft 26.3 witness: real ClientPacketListener.handleSetCamera, recorded by tools/record_set_camera_witness.py\n# columns: entity type, camera entity id after SetCamera 40,7,40,99,40,99 (7 = local player)\n'
    target.parent.mkdir(exist_ok=True)
    target.write_text(header + out.read_text())
    print(target)


if __name__ == '__main__':
    main()
