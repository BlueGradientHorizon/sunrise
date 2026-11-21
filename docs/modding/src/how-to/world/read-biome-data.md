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

Grass, foliage and dry-foliage colormaps are client-only resources. On the client, read the blended tint at a block position with `LevelRef::block_tint(host, pos, resolver)` (matching `ClientLevel.getBlockTint` and `BiomeColors.getAverageGrassColor` / `getAverageFoliageColor` / `getAverageDryFoliageColor` / `getAverageWaterColor`):

```rust,ignore
use mod_api::{ClientHost, ColorResolver, LevelRef};

fn sample_tints(host: &dyn ClientHost, level: LevelRef, pos: [i32; 3]) {
    if let Ok(grass_argb) = level.block_tint(host, pos, ColorResolver::Grass) {
        let _grass_rgb = grass_argb & 0x00FF_FFFF;
    }
    let _foliage_argb = level.block_tint(host, pos, ColorResolver::Foliage);
    let _dry_foliage_argb = level.block_tint(host, pos, ColorResolver::DryFoliage);
    let _water_argb = level.block_tint(host, pos, ColorResolver::Water);
}
```

`block_tint` averages over the client's biome blend radius using the loaded chunks (plains where absent) and active biome colours (including resource pack colormaps and `biome_colors`), without loading chunks. It returns `Err(LevelReadError::Unavailable)` before login or outside the world phase, and `Err(LevelReadError::Stale)` for a stale handle or another dimension.
