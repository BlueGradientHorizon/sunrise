//! Vanilla 26.3 `SynchedEntityData` serializer layouts by `minecraft:entity_type` id.

/// Base `Entity` data entries (`0..8`) shared by every entity type.
pub(super) const ENTITY_BASE: &[i32] = &[0, 1, 6, 8, 8, 8, 20, 1];

/// Expected `EntityDataSerializers` id at each accessor index for vanilla 26.3 entity types.
pub(super) fn serializers(kind: &str) -> Option<&'static [i32]> {
    Some(match kind {
        "minecraft:acacia_boat"
        | "minecraft:acacia_chest_boat"
        | "minecraft:bamboo_chest_raft"
        | "minecraft:bamboo_raft"
        | "minecraft:birch_boat"
        | "minecraft:birch_chest_boat"
        | "minecraft:cherry_boat"
        | "minecraft:cherry_chest_boat"
        | "minecraft:dark_oak_boat"
        | "minecraft:dark_oak_chest_boat"
        | "minecraft:jungle_boat"
        | "minecraft:jungle_chest_boat"
        | "minecraft:mangrove_boat"
        | "minecraft:mangrove_chest_boat"
        | "minecraft:oak_boat"
        | "minecraft:oak_chest_boat"
        | "minecraft:pale_oak_boat"
        | "minecraft:pale_oak_chest_boat"
        | "minecraft:poplar_boat"
        | "minecraft:poplar_chest_boat"
        | "minecraft:spruce_boat"
        | "minecraft:spruce_chest_boat" => &[0, 1, 6, 8, 8, 8, 20, 1, 1, 1, 3, 8, 8, 1],
        "minecraft:allay" | "minecraft:pillager" | "minecraft:squid" | "minecraft:tadpole"
        | "minecraft:witch" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8],
        "minecraft:area_effect_cloud" => &[0, 1, 6, 8, 8, 8, 20, 1, 3, 8, 16],
        "minecraft:armadillo" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 36],
        "minecraft:armor_stand" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 9, 9, 9, 9, 9, 9,
        ],
        "minecraft:arrow" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 0, 8, 1],
        "minecraft:axolotl" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 1, 8, 8,
        ],
        "minecraft:bat"
        | "minecraft:blaze"
        | "minecraft:cave_spider"
        | "minecraft:iron_golem"
        | "minecraft:snow_golem"
        | "minecraft:spider"
        | "minecraft:vex" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 0],
        "minecraft:bee" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0, 2,
        ],
        "minecraft:block_display" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 1, 1, 1, 39, 39, 40, 40, 0, 1, 3, 3, 3, 3, 3, 1, 14,
        ],
        "minecraft:bogged"
        | "minecraft:cod"
        | "minecraft:ghast"
        | "minecraft:piglin_brute"
        | "minecraft:ravager"
        | "minecraft:skeleton"
        | "minecraft:vindicator"
        | "minecraft:zoglin" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8],
        "minecraft:breeze"
        | "minecraft:endermite"
        | "minecraft:giant"
        | "minecraft:parched"
        | "minecraft:silverfish"
        | "minecraft:stray"
        | "minecraft:wither_skeleton" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0],
        "minecraft:breeze_wind_charge"
        | "minecraft:dragon_fireball"
        | "minecraft:evoker_fangs"
        | "minecraft:leash_knot"
        | "minecraft:lightning_bolt"
        | "minecraft:llama_spit"
        | "minecraft:marker"
        | "minecraft:shulker_bullet"
        | "minecraft:wind_charge" => &[0, 1, 6, 8, 8, 8, 20, 1],
        "minecraft:camel" | "minecraft:camel_husk" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0, 8, 2,
        ],
        "minecraft:cat" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0, 13, 21, 8, 8, 1, 22,
        ],
        "minecraft:chest_minecart"
        | "minecraft:hopper_minecart"
        | "minecraft:minecart"
        | "minecraft:spawner_minecart"
        | "minecraft:tnt_minecart" => &[0, 1, 6, 8, 8, 8, 20, 1, 1, 1, 3, 15, 1],
        "minecraft:chicken" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 30, 31,
        ],
        "minecraft:command_block_minecart" => &[0, 1, 6, 8, 8, 8, 20, 1, 1, 1, 3, 15, 1, 4, 5],
        "minecraft:copper_golem" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 38, 37],
        "minecraft:cow" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 23, 24,
        ],
        "minecraft:creaking" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 8, 11,
        ],
        "minecraft:creeper" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 1, 8, 8],
        "minecraft:cushion" => &[0, 1, 6, 8, 8, 8, 20, 1, 43],
        "minecraft:dolphin" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 8, 1,
        ],
        "minecraft:donkey" | "minecraft:mule" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0, 8,
        ],
        "minecraft:drowned"
        | "minecraft:husk"
        | "minecraft:zombie"
        | "minecraft:zombified_piglin" => {
            &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 1, 8]
        }
        "minecraft:egg"
        | "minecraft:ender_pearl"
        | "minecraft:experience_bottle"
        | "minecraft:eye_of_ender"
        | "minecraft:fireball"
        | "minecraft:item"
        | "minecraft:lingering_potion"
        | "minecraft:ominous_item_spawner"
        | "minecraft:small_fireball"
        | "minecraft:snowball"
        | "minecraft:splash_potion" => &[0, 1, 6, 8, 8, 8, 20, 1, 7],
        "minecraft:elder_guardian"
        | "minecraft:guardian"
        | "minecraft:pufferfish"
        | "minecraft:salmon"
        | "minecraft:tropical_fish" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 1],
        "minecraft:end_crystal" => &[0, 1, 6, 8, 8, 8, 20, 1, 11, 8],
        "minecraft:ender_dragon" | "minecraft:phantom" | "minecraft:warden" => {
            &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 1]
        }
        "minecraft:enderman" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 15, 8, 8],
        "minecraft:evoker" | "minecraft:illusioner" => {
            &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 0]
        }
        "minecraft:experience_orb" => &[0, 1, 6, 8, 8, 8, 20, 1, 1],
        "minecraft:falling_block" => &[0, 1, 6, 8, 8, 8, 20, 1, 10],
        "minecraft:firework_rocket" => &[0, 1, 6, 8, 8, 8, 20, 1, 7, 19, 8],
        "minecraft:fishing_bobber" => &[0, 1, 6, 8, 8, 8, 20, 1, 1, 8],
        "minecraft:fox" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 1, 0, 13, 13,
        ],
        "minecraft:frog" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 27, 19,
        ],
        "minecraft:furnace_minecart" => &[0, 1, 6, 8, 8, 8, 20, 1, 1, 1, 3, 15, 1, 8],
        "minecraft:glow_item_frame" | "minecraft:item_frame" => {
            &[0, 1, 6, 8, 8, 8, 20, 1, 12, 7, 1]
        }
        "minecraft:glow_squid"
        | "minecraft:magma_cube"
        | "minecraft:mooshroom"
        | "minecraft:rabbit"
        | "minecraft:slime"
        | "minecraft:wandering_trader" => {
            &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 1]
        }
        "minecraft:goat" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 8, 8, 8,
        ],
        "minecraft:happy_ghast" | "minecraft:piglin" | "minecraft:turtle" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 8, 8,
        ],
        "minecraft:hoglin" | "minecraft:ocelot" | "minecraft:polar_bear" => {
            &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 8]
        }
        "minecraft:horse" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0, 1,
        ],
        "minecraft:interaction" => &[0, 1, 6, 8, 8, 8, 20, 1, 3, 3, 8],
        "minecraft:item_display" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 1, 1, 1, 39, 39, 40, 40, 0, 1, 3, 3, 3, 3, 3, 1, 7, 0,
        ],
        "minecraft:llama" | "minecraft:trader_llama" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0, 8, 1, 1,
        ],
        "minecraft:mannequin" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 42, 0, 41, 8, 6,
        ],
        "minecraft:nautilus" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0, 13, 8,
        ],
        "minecraft:painting" => &[0, 1, 6, 8, 8, 8, 20, 1, 12, 34],
        "minecraft:panda" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 1, 1, 1, 0, 0, 0,
        ],
        "minecraft:parrot" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0, 13, 1,
        ],
        "minecraft:pig" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 1, 28, 29,
        ],
        "minecraft:player" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 42, 0, 3, 1, 19, 19,
        ],
        "minecraft:sheep" | "minecraft:skeleton_horse" | "minecraft:zombie_horse" => {
            &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0]
        }
        "minecraft:shulker" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 12, 0, 0],
        "minecraft:sniffer" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 35, 1,
        ],
        "minecraft:spectral_arrow" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 0, 8],
        "minecraft:strider" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 1, 8,
        ],
        "minecraft:sulfur_cube" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 1, 1, 8,
        ],
        "minecraft:text_display" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 1, 1, 1, 39, 39, 40, 40, 0, 1, 3, 3, 3, 3, 3, 1, 5, 1, 1, 0, 0,
        ],
        "minecraft:tnt" => &[0, 1, 6, 8, 8, 8, 20, 1, 1, 14],
        "minecraft:trident" => &[0, 1, 6, 8, 8, 8, 20, 1, 0, 0, 8, 0, 8],
        "minecraft:villager" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 1, 18, 8,
        ],
        "minecraft:wither" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 1, 1, 1, 1,
        ],
        "minecraft:wither_skull" => &[0, 1, 6, 8, 8, 8, 20, 1, 8],
        "minecraft:wolf" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0, 13, 8, 1, 2, 25, 26,
        ],
        "minecraft:zombie_nautilus" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 8, 0, 13, 8, 32,
        ],
        "minecraft:zombie_villager" => &[
            0, 1, 6, 8, 8, 8, 20, 1, 0, 3, 17, 8, 1, 1, 11, 0, 8, 1, 8, 8, 18, 8,
        ],
        _ => return None,
    })
}
