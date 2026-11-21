# Read biome data

Read the level's biome table without loading chunks. A stale level or unknown biome returns `None`.

```rust,ignore
use mod_api::{Host, LevelRef};

fn inspect(host: &dyn Host, level: LevelRef) {
    if let Some(effects) = host.level_biome_effects(level, "minecraft:plains") {
        let water_rgb = effects.water;
        // Sky and fog preserve omitted declarations as None. Water fog instead
        // uses 0x050533 when omitted. These are not dimension-resolved colours.
        let water_fog_rgb = effects.water_fog;
    }
    if let Some(categories) = host.level_biome_spawners(level, "minecraft:plains") {
        for category in categories {
            // Categories are sorted by name; entries retain biome-file order.
            let entries = category.entries;
        }
    }
}
```

The table includes data packs and biome modifiers. Spawners describe the biome's own natural spawn lists, not a position-specific potential-spawn query.

## Read blended biome tints on the client

Grass, foliage and dry-foliage colormaps are client-only resources. On the client, read blended tints through the existing client tint APIs:

- **Inside `BlockTintSource` callbacks** (`color_in_world`, `color_as_terrain_particle`), call `BlockAndTintGetter::block_tint(pos, resolver)` with `ColorResolver::Grass`, `ColorResolver::Foliage`, `ColorResolver::DryFoliage`, `ColorResolver::Water`, or a mod resolver registered in `register_color_resolvers` (matching `BiomeColors.getAverageGrassColor` / `getAverageFoliageColor` / `getAverageDryFoliageColor` / `getAverageWaterColor`, opaque ARGB `0xFF00_0000 | rgb` blended over the biome blend radius):

```rust,ignore
use mod_api::{BlockAndTintGetter, ColorResolver};

fn sample_in_tint_source(level: &dyn BlockAndTintGetter, pos: [i32; 3]) {
    let _grass_argb = level.block_tint(pos, ColorResolver::Grass);
    let _foliage_argb = level.block_tint(pos, ColorResolver::Foliage);
    let _dry_foliage_argb = level.block_tint(pos, ColorResolver::DryFoliage);
    let _water_argb = level.block_tint(pos, ColorResolver::Water);
}
```

- **In other client hooks** (World phase), call `ClientHost::block_tint_color(state, tint_index, pos)` for a drawn block or fluid face's `0xRRGGBB` tint (for example, a face's `FaceSprite::tint_index`, or tint index `0` on `minecraft:grass_block`, `minecraft:oak_leaves`, `minecraft:leaf_litter`, or `minecraft:water`). This applies biome blending together with active resource pack `biome_colors` and Polytone block and fluid colormaps, matching what the chunk and fluid meshers draw:

```rust,ignore
use mod_api::{BlockState, ClientHost};

fn sample_drawn_tint(host: &dyn ClientHost, grass_block: BlockState, pos: [i32; 3]) -> Option<u32> {
    host.block_tint_color(grass_block, 0, pos)
}
```
