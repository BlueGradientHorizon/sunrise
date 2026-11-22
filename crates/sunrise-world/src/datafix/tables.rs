//! The lookup tables of the 1.13-era data fixers (DataVersion 99 to 1631) and the
//! `BlockStateData` table. They are vanilla's data, so they are not in this repository:
//! `sunrise-prepare` exports them from the player's own server jar
//! (`VanillaDataFixTablesExporter`) into `datafix-tables.json` of the prepared resources,
//! and [`tables`] reads that file once, the first time data from before 1.13 is upgraded.
//!
//! Maps and sets come sorted by key: every use here is a lookup by key or a membership
//! test, so the order is not part of their meaning. `block_state_data` keeps the order of
//! vanilla's `register` calls (see `block_state_data.rs`).

use mod_api::NbtUpgradeError;
use serde::Deserialize;
use std::cell::Cell;
use std::fmt::Display;
use std::panic::{UnwindSafe, catch_unwind, resume_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The file in the prepared resources.
pub const FILE: &str = "datafix-tables.json";

type Pairs = &'static [(&'static str, &'static str)];
type Strs = &'static [&'static str];
type IntNames = &'static [(i32, &'static str)];

pub struct Tables {
    /// `ItemStackTheFlatteningFix.MAP`: `id.damage` to the flattened item.
    pub item_flattening: Pairs,
    /// `ItemStackTheFlatteningFix.DAMAGE_IDS`: items that keep their damage in `tag.Damage`.
    pub item_damage_ids: Strs,
    /// `EntityBlockStateFix.MAP`: 1.12 block name to numeric id.
    pub block_ids: &'static [(&'static str, i32)],
    /// `ItemIdFix.ITEM_NAMES`: numeric item id to name.
    pub item_ids: IntNames,
    /// `StatsCounterFix`'s tables.
    pub special_objective_criteria: Strs,
    pub stats_skip: Strs,
    pub stats_custom: Pairs,
    pub stats_item_keys: Pairs,
    pub stats_entity_keys: Pairs,
    pub stats_entities: Pairs,
    /// `RecipesFix.RECIPES`, `RecipesRenameningFix.RECIPES`.
    pub recipes: Pairs,
    pub recipes_renamening: Pairs,
    /// `AdvancementsFix.RENAMES`.
    pub advancements: Pairs,
    /// `LevelDataGeneratorOptionsFix.MAP`.
    pub generator_biomes: Pairs,
    /// `ItemStackSpawnEggFix.MAP`.
    pub spawn_eggs: Pairs,
    /// `ItemStackEnchantmentNamesFix.MAP`.
    pub enchantments: IntNames,
    /// `RenamedCoralFix.RENAMED_IDS`, `RenamedCoralFansFix.RENAMED_IDS`.
    pub coral: Pairs,
    pub coral_fans: Pairs,
    /// `EntityTheRenameningFix.RENAMED_IDS`, `RENAMED_BLOCKS`.
    pub the_renamening_entities: Pairs,
    pub the_renamening_blocks: Pairs,
    /// `EntityCodSalmonFix.RENAMED_IDS`, `RENAMED_EGG_IDS`.
    pub cod_salmon: Pairs,
    pub cod_salmon_eggs: Pairs,
    /// The entity ids V1460 registers (V705 has the same ones).
    pub entities_v1460: Strs,
    /// The entity and block entity ids V99 registers.
    pub entities_v99: Strs,
    pub block_entities_v99: Strs,
    /// `ItemSpawnEggFix.ID_TO_ENTITY`: spawn egg damage to entity id.
    pub spawn_egg_entities: IntNames,
    /// `EntityIdFix.ID_MAP`, `BlockEntityIdFix.ID_MAP`.
    pub entity_ids: Pairs,
    pub block_entity_ids: Pairs,
    /// `V99.ITEM_TO_BLOCKENTITY`.
    pub item_to_block_entity_v99: Pairs,
    /// `ItemPotionFix.POTIONS`: damage & 127 to potion (absent: null).
    pub item_potions: IntNames,
    /// `ChunkStructuresTemplateRenameFix.RENAMES`: structure id, child id, template renames.
    pub structure_template_renames: &'static [(&'static str, &'static str, Pairs)],
    /// `BlockStateData`: one line per `register` call, in call order (block_state_data.rs).
    pub block_state_data: Strs,
}

#[derive(Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
struct Raw {
    item_flattening: Vec<(String, String)>,
    item_damage_ids: Vec<String>,
    block_ids: Vec<(String, i32)>,
    item_ids: Vec<(i32, String)>,
    special_objective_criteria: Vec<String>,
    stats_skip: Vec<String>,
    stats_custom: Vec<(String, String)>,
    stats_item_keys: Vec<(String, String)>,
    stats_entity_keys: Vec<(String, String)>,
    stats_entities: Vec<(String, String)>,
    recipes: Vec<(String, String)>,
    recipes_renamening: Vec<(String, String)>,
    advancements: Vec<(String, String)>,
    generator_biomes: Vec<(String, String)>,
    spawn_eggs: Vec<(String, String)>,
    enchantments: Vec<(i32, String)>,
    coral: Vec<(String, String)>,
    coral_fans: Vec<(String, String)>,
    the_renamening_entities: Vec<(String, String)>,
    the_renamening_blocks: Vec<(String, String)>,
    cod_salmon: Vec<(String, String)>,
    cod_salmon_eggs: Vec<(String, String)>,
    entities_v1460: Vec<String>,
    entities_v99: Vec<String>,
    block_entities_v99: Vec<String>,
    spawn_egg_entities: Vec<(i32, String)>,
    entity_ids: Vec<(String, String)>,
    block_entity_ids: Vec<(String, String)>,
    item_to_block_entity_v99: Vec<(String, String)>,
    item_potions: Vec<(i32, String)>,
    structure_template_renames: Vec<(String, String, Vec<(String, String)>)>,
    block_state_data: Vec<String>,
}

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Read the tables from the prepared resources at `root` (the game's `--resources`). Without
/// it, `SUNRISE_RESOURCES`, then the checkout's `.local/sunrise-client/<version>` (tests and
/// probes). Only the first call counts, and only before the tables are first read.
pub fn set_resources(root: &Path) {
    let _ = ROOT.set(root.to_path_buf());
}

fn path() -> PathBuf {
    ROOT.get()
        .cloned()
        .or_else(|| std::env::var_os("SUNRISE_RESOURCES").map(PathBuf::from))
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../.local/sunrise-client")
                .join(sunrise_core::version::VANILLA_VERSION)
        })
        .join(FILE)
}

/// Whether the prepared tables are there (tests skip without them).
pub fn available() -> bool {
    path().is_file()
}

fn text(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn pairs(v: Vec<(String, String)>) -> Pairs {
    v.into_iter()
        .map(|(a, b)| (text(a), text(b)))
        .collect::<Vec<_>>()
        .leak()
}

fn strs(v: Vec<String>) -> Strs {
    v.into_iter().map(text).collect::<Vec<_>>().leak()
}

fn int_names(v: Vec<(i32, String)>) -> IntNames {
    v.into_iter()
        .map(|(i, s)| (i, text(s)))
        .collect::<Vec<_>>()
        .leak()
}

fn load(path: &Path) -> std::io::Result<Tables> {
    let r: Raw = crate::json_file::read(path)?;
    Ok(Tables {
        item_flattening: pairs(r.item_flattening),
        item_damage_ids: strs(r.item_damage_ids),
        block_ids: r
            .block_ids
            .into_iter()
            .map(|(s, i)| (text(s), i))
            .collect::<Vec<_>>()
            .leak(),
        item_ids: int_names(r.item_ids),
        special_objective_criteria: strs(r.special_objective_criteria),
        stats_skip: strs(r.stats_skip),
        stats_custom: pairs(r.stats_custom),
        stats_item_keys: pairs(r.stats_item_keys),
        stats_entity_keys: pairs(r.stats_entity_keys),
        stats_entities: pairs(r.stats_entities),
        recipes: pairs(r.recipes),
        recipes_renamening: pairs(r.recipes_renamening),
        advancements: pairs(r.advancements),
        generator_biomes: pairs(r.generator_biomes),
        spawn_eggs: pairs(r.spawn_eggs),
        enchantments: int_names(r.enchantments),
        coral: pairs(r.coral),
        coral_fans: pairs(r.coral_fans),
        the_renamening_entities: pairs(r.the_renamening_entities),
        the_renamening_blocks: pairs(r.the_renamening_blocks),
        cod_salmon: pairs(r.cod_salmon),
        cod_salmon_eggs: pairs(r.cod_salmon_eggs),
        entities_v1460: strs(r.entities_v1460),
        entities_v99: strs(r.entities_v99),
        block_entities_v99: strs(r.block_entities_v99),
        spawn_egg_entities: int_names(r.spawn_egg_entities),
        entity_ids: pairs(r.entity_ids),
        block_entity_ids: pairs(r.block_entity_ids),
        item_to_block_entity_v99: pairs(r.item_to_block_entity_v99),
        item_potions: int_names(r.item_potions),
        structure_template_renames: r
            .structure_template_renames
            .into_iter()
            .map(|(s, c, t)| (text(s), text(c), pairs(t)))
            .collect::<Vec<_>>()
            .leak(),
        block_state_data: strs(r.block_state_data),
    })
}

/// Read the tables without panicking. Cache only success, and serialise the cold path because
/// `load` allocates the tables for the lifetime of the process.
fn try_tables() -> std::io::Result<&'static Tables> {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    static LOADING: Mutex<()> = Mutex::new(());
    if let Some(tables) = TABLES.get() {
        return Ok(tables);
    }
    let _loading = LOADING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(tables) = TABLES.get() {
        return Ok(tables);
    }
    let tables = load(&path())?;
    Ok(TABLES.get_or_init(|| tables))
}

thread_local! {
    static STANDALONE: Cell<bool> = const { Cell::new(false) };
}

struct TableUnavailable;

/// The standalone upgrade already contains unwinds from the shared chain. Carry a
/// distinct failure through its infallible walkers without running a panic hook.
/// Ordinary save upgrades keep their diagnostic if a required table is missing.
pub(super) fn required<T>(result: Result<T, impl Display>) -> T {
    match result {
        Ok(value) => value,
        Err(_) if STANDALONE.get() => resume_unwind(Box::new(TableUnavailable)),
        Err(error) => panic!(
            "{error} ({}): this upgrade needs the data fixers' tables of the prepared resources; \
             prepare them again (sunrise-prepare --prepare)",
            path().display()
        ),
    }
}

pub(super) fn standalone<T>(run: impl FnOnce() -> T + UnwindSafe) -> Result<T, NbtUpgradeError> {
    let previous = STANDALONE.replace(true);
    let result = catch_unwind(run);
    STANDALONE.set(previous);
    result.map_err(|failure| {
        if failure.is::<TableUnavailable>() {
            NbtUpgradeError::LegacyTablesUnavailable
        } else {
            NbtUpgradeError::MalformedCompound
        }
    })
}

/// Load the legacy tables on the first lookup, not just because the source is old.
pub fn tables() -> &'static Tables {
    required(try_tables())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "daily"]
    fn standalone_upgrade_only_refuses_at_a_failed_table_lookup() {
        use crate::datafix::{dynamic, upgrade_compound};
        use mod_api::NbtDataType;
        use sunrise_core::nbt::{Compound, Tag};

        const CHILD: &str = "SUNRISE_DATAFIX_TABLE_REFUSAL_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let scratch = std::env::temp_dir().join(format!(
                "sunrise-datafix-table-refusal-{}",
                std::process::id()
            ));
            std::fs::create_dir(&scratch).unwrap();
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "datafix::tables::tests::standalone_upgrade_only_refuses_at_a_failed_table_lookup",
                ])
                .env("SUNRISE_RESOURCES", &scratch)
                .env(CHILD, "1")
                .output();
            std::fs::remove_dir_all(scratch).unwrap();
            let result = result.unwrap();
            let output = String::from_utf8_lossy(&result.stdout);
            assert!(
                result.status.success() && output.contains("1 passed"),
                "{output}"
            );
            return;
        }

        let state = |name| Compound::from_iter([("Name".into(), dynamic::string(name))]);
        let sign = state("minecraft:sign");
        // 1.13.2 predates the sign rename (1802), but no fix on this path needs the tables.
        assert_eq!(
            upgrade_compound(NbtDataType::BlockState, 1631, sign),
            Ok(Compound::from_iter([(
                "id".into(),
                dynamic::string("minecraft:oak_sign")
            )])),
        );
        // The coral-fan rename (1515) is in range but cannot see a block state here.
        let sign = Compound::from_iter([("id".into(), dynamic::string("minecraft:sign"))]);
        assert!(upgrade_compound(NbtDataType::BlockEntity, 1510, sign).is_ok());
        assert_eq!(
            upgrade_compound(NbtDataType::BlockState, 1400, state("minecraft:stone")),
            Err(NbtUpgradeError::LegacyTablesUnavailable),
        );

        let prepared = PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let mut raw: serde_json::Value = crate::json_file::read(&prepared.join(FILE)).unwrap();
        raw["BLOCK_STATE_DATA"] = serde_json::json!(["not a registration"]);
        raw["ITEM_FLATTENING"] = serde_json::json!([["minecraft:wool", "minecraft:red_wool"]]);
        std::fs::write(path(), serde_json::to_vec(&raw).unwrap()).unwrap();
        assert_eq!(
            upgrade_compound(NbtDataType::BlockState, 1400, state("minecraft:stone")),
            Err(NbtUpgradeError::LegacyTablesUnavailable),
        );
        assert_eq!(
            upgrade_compound(
                NbtDataType::ItemStack,
                1343,
                Compound::from_iter([("id".into(), dynamic::string("minecraft:wool"))]),
            ),
            Err(NbtUpgradeError::LegacyTablesUnavailable),
        );
        assert_eq!(
            upgrade_compound(
                NbtDataType::ItemStack,
                1343,
                Compound::from_iter([("id".into(), Tag::Int(35))]),
            ),
            Err(NbtUpgradeError::MalformedCompound),
        );
    }

    /// The prepared file holds every table, each non-empty, and `BlockStateData` registers
    /// air at id 0 first, as its `bootstrap0` does.
    #[test]
    fn prepared_tables_load() {
        if !available() {
            eprintln!("skipping: no {FILE} at {}", path().display());
            return;
        }
        let t = tables();
        assert!(t.block_state_data[0].starts_with("0\tminecraft:air\t"));
        let id = |name: &str| {
            t.block_ids
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, i)| *i)
        };
        assert_eq!(id("minecraft:stone"), Some(1));
        assert_eq!(
            t.item_ids.iter().find(|(i, _)| *i == 276).map(|(_, n)| *n),
            Some("minecraft:diamond_sword")
        );
    }
}
