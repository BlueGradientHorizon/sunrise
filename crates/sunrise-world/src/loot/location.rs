//! What `LocationCheck` and `MatchBlock` read of a roll: its origin and level, and its
//! block entity. Both loot engines evaluate these conditions here, so a block drop, a
//! container fill and `LevelRef::roll_loot` follow the same rules.
use serde_json::Value;
use sunrise_core::nbt::{Compound, Tag};

/// `LootContextParams.ORIGIN` and the level a `LocationCheck` reads at it. A roll
/// without an origin has none, and every location check in it fails.
#[derive(Clone, Copy)]
pub struct LootLocation<'a> {
    /// `ORIGIN`: a broken block's centre, a chest's centre, `/loot`'s position.
    pub origin: [f64; 3],
    /// `LocationPredicate.matches(level, x, y, z)` against the roll's level: position and
    /// dimension, then biome, structure piece, smoke, light, block and fluid with
    /// `Level.isLoaded`'s rule, and `Level.canSeeSky` on its own.
    pub level: &'a dyn Fn(&Value, [f64; 3]) -> bool,
}

impl LootLocation<'_> {
    /// `LocationCheck.test`: true with no predicate, else the predicate at the origin
    /// moved by the check's integer offsets. It draws nothing from the roll's random.
    pub fn check(&self, condition: &Value) -> bool {
        let predicate = &condition["predicate"];
        if predicate.is_null() {
            return true;
        }
        let Ok(offset) = location_offset(condition) else {
            return false;
        };
        let at = std::array::from_fn(|i| self.origin[i] + f64::from(offset[i]));
        (self.level)(predicate, at)
    }
}

/// `LocationCheck`'s `offsetX`, `offsetY` and `offsetZ`: `Codec.INT` fields that default
/// to zero, so a JSON number reads as Java's `Number.intValue` (a fraction truncated, a
/// wide integer wrapped to 32 bits) and anything else fails to load.
pub fn location_offset(condition: &Value) -> Result<[i32; 3], String> {
    let mut offset = [0; 3];
    for (axis, key) in ["offsetX", "offsetY", "offsetZ"].into_iter().enumerate() {
        offset[axis] = match condition.get(key) {
            None => 0,
            Some(Value::Number(number)) => super::number::int_value(number) as i32,
            Some(other) => return Err(format!("{key}: not a number: {other}")),
        };
    }
    Ok(offset)
}

/// The NBT counterpart of [`location_offset`], for a condition admitted from a command:
/// `NbtOps.getNumberValue` takes any numeric tag's `intValue`.
pub fn location_offset_tag(condition: &Tag) -> Result<[i32; 3], String> {
    let mut offset = [0; 3];
    for (axis, key) in ["offsetX", "offsetY", "offsetZ"].into_iter().enumerate() {
        offset[axis] = match condition.get(key) {
            None => 0,
            Some(Tag::Byte(v)) => i32::from(*v),
            Some(Tag::Short(v)) => i32::from(*v),
            Some(Tag::Int(v)) => *v,
            Some(Tag::Long(v)) => *v as i32,
            Some(Tag::Float(bits)) => f32::from_bits(*bits) as i32,
            Some(Tag::Double(bits)) => f64::from_bits(*bits) as i32,
            Some(_) => return Err(format!("{key}: not a number")),
        };
    }
    Ok(offset)
}

/// `BlockPredicate.willMatchBlockEntity`: the predicate has an NBT test or component
/// matchers, so it needs a block entity. Empty matchers do not.
pub fn reads_block_entity(predicate: &Value) -> bool {
    !predicate["nbt"].is_null()
        || matchers_present(&predicate["components"])
        || matchers_present(&predicate["predicates"])
}

fn matchers_present(value: &Value) -> bool {
    value.as_object().is_some_and(|map| !map.is_empty())
}

/// `BlockPredicate.matchesBlockEntity`: the NBT test against the block entity's
/// `saveWithFullMetadata` (`NbtPredicate.matches`, `NbtUtils.compareNbt` with partial
/// lists), then the component matchers against `BlockEntity.collectComponents`. A missing
/// block entity fails any test it would answer; a predicate with neither test holds.
pub fn block_entity_matches(
    predicate: &Value,
    nbt: Option<&Tag>,
    components: impl FnOnce() -> Option<Compound>,
) -> bool {
    if !predicate["nbt"].is_null() {
        let Some(pattern @ Tag::Compound(_)) = super::nbt_predicate_tag(&predicate["nbt"]) else {
            return false;
        };
        if nbt.is_none() || !sunrise_core::nbt::compare_nbt(&pattern, nbt) {
            return false;
        }
    }
    if !matchers_present(&predicate["components"]) && !matchers_present(&predicate["predicates"]) {
        return true;
    }
    components().is_some_and(|components| components_match(predicate, &components))
}

/// `DataComponentMatchers.test` against a collected component map: every exact
/// component (`DataComponentExactPredicate`) equals the map's value, read with vanilla's
/// tag types and no other number type tried, and every partial predicate holds. Of the
/// partial predicates a block entity's components answer, `minecraft:custom_data`
/// (`CustomDataPredicate`) is read; any other fails.
pub fn components_match(matchers: &Value, components: &Compound) -> bool {
    let exact = matchers["components"].as_object().into_iter().flatten();
    for (id, want) in exact {
        let id = full_id(id);
        let want = if id == "minecraft:custom_data" {
            super::nbt_predicate_tag(want)
        } else {
            super::json_tag(want)
        };
        if want.is_none() || components.get(&id.as_str().into()) != want.as_ref() {
            return false;
        }
    }
    let partial = matchers["predicates"].as_object().into_iter().flatten();
    for (id, pattern) in partial {
        let holds = match full_id(id).as_str() {
            "minecraft:custom_data" => {
                let data = match components.get(&"minecraft:custom_data".into()) {
                    Some(Tag::Compound(data)) => Some(data),
                    _ => None,
                };
                super::custom_data_matches(pattern, data)
            }
            _ => false,
        };
        if !holds {
            return false;
        }
    }
    true
}

fn full_id(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `LocationCheck`'s offset codec is `Codec.INT`: a fraction truncates, a 33-bit value
    /// wraps, a negative one stays negative, and a string refuses to load.
    #[test]
    fn offsets_read_as_java_ints() {
        let read = |v: Value| location_offset(&v);
        assert_eq!(read(json!({})), Ok([0, 0, 0]));
        assert_eq!(
            read(json!({"offsetX": -3, "offsetY": 1.75, "offsetZ": 4_294_967_297_i64})),
            Ok([-3, 1, 1])
        );
        assert!(read(json!({"offsetX": "1"})).is_err());
    }

    /// The check moves the origin by whole blocks before asking the level, holds with no
    /// predicate, and asks nothing for it.
    #[test]
    fn check_moves_the_origin_by_the_offsets() {
        let asked = std::cell::Cell::new(None);
        let level = |_: &Value, at: [f64; 3]| {
            asked.set(Some(at));
            true
        };
        let location = LootLocation {
            origin: [0.5, 64.5, -0.5],
            level: &level,
        };
        assert!(location.check(&json!({"type": "minecraft:location_check"})));
        assert_eq!(asked.get(), None);
        assert!(location.check(&json!({"predicate": {}, "offsetX": -1, "offsetZ": 2})));
        assert_eq!(asked.get(), Some([-0.5, 64.5, 1.5]));
    }

    /// Exact components compare tags with their own types: `{k:1b}` is not `{k:1}`, and a
    /// JSON object's whole numbers take the smallest type, as `JsonOps` gives them.
    #[test]
    fn exact_custom_data_keeps_its_tag_types() {
        let components: Compound = [(
            "minecraft:custom_data".into(),
            Tag::compound([("k", Tag::Byte(1))]),
        )]
        .into();
        let matches = |v: Value| components_match(&v, &components);
        assert!(matches(json!({"components": {"custom_data": "{k:1b}"}})));
        assert!(matches(
            json!({"components": {"minecraft:custom_data": {"k": 1}}})
        ));
        assert!(!matches(json!({"components": {"custom_data": "{k:1}"}})));
        assert!(matches(json!({"predicates": {"custom_data": "{k:1b}"}})));
        assert!(!matches(json!({"predicates": {"custom_data": "{k:2b}"}})));
    }

    /// No block entity fails a predicate that reads one and holds one that does not.
    #[test]
    fn a_missing_block_entity_fails_only_tests_that_read_it() {
        let none = || None;
        assert!(block_entity_matches(
            &json!({"blocks": "minecraft:chest"}),
            None,
            none
        ));
        assert!(!reads_block_entity(&json!({"components": {}})));
        assert!(!block_entity_matches(&json!({"nbt": "{}"}), None, none));
        assert!(!block_entity_matches(
            &json!({"predicates": {"custom_data": "{}"}}),
            None,
            none
        ));
        let chest = Tag::compound([("id", Tag::String("minecraft:chest".into()))]);
        assert!(block_entity_matches(
            &json!({"nbt": "{id:\"minecraft:chest\"}"}),
            Some(&chest),
            none
        ));
    }

    /// Invalid offsets and block-entity NBT fail on the loot load path, not in a roll.
    #[test]
    fn invalid_predicate_data_fails_to_load() {
        let check = crate::loot::validate_loot_condition;
        assert!(check(&json!({"type": "minecraft:location_check", "offsetX": "1"})).is_err());
        assert!(check(&json!({"type": "minecraft:location_check", "offsetY": -2})).is_ok());
        assert!(check(&json!({"type": "minecraft:match_block", "nbt": "{a:"})).is_err());
        assert!(check(&json!({"type": "minecraft:match_block", "nbt": "{a:1b}"})).is_ok());
        assert!(
            check(&json!({"type": "minecraft:location_check",
                "predicate": {"block": {"nbt": "[1]"}}}))
            .is_err()
        );
    }
}
