//! Advancement file codecs; reject only the owning malformed file.
use super::*;
pub(super) fn parse(id: &str, v: &Value) -> Option<Advancement> {
    if let Err(error) = sunrise_world::item_defaults::validate_stack(&v["display"]["icon"])
        .and_then(|()| sunrise_world::item_defaults::validate_text(&v["display"]["title"]))
        .and_then(|()| sunrise_world::item_defaults::validate_text(&v["display"]["description"]))
    {
        log::warn!("advancement {id} rejected: {error}");
        return None;
    }
    let mut criteria = Vec::new();
    for (name, criterion) in v["criteria"].as_object()? {
        let Some(trigger) = criterion["trigger"]
            .as_str()
            .and_then(|key| sunrise_core::identifier::Identifier::parse(key).ok())
            .filter(|key| !key.path().is_empty())
        else {
            log::warn!("advancement {id} rejected: criterion {name} has an invalid trigger id");
            return None;
        };
        let builtin = is_builtin_trigger(trigger.as_str());
        let conditions = &criterion["conditions"];
        if let Err(error) = validate_conditions(conditions, builtin) {
            log::warn!("advancement {id} rejected: criterion {name}: {error}");
            return None;
        }
        criteria.push(Criterion {
            name: name.clone(),
            trigger: trigger.as_str().to_owned(),
            conditions: if builtin {
                LazyJson::new(conditions)
            } else {
                conditions.clone().into()
            },
        });
    }
    let requirements = match v["requirements"].as_array() {
        Some(r) => r
            .iter()
            .map(|group| {
                group
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|s| s.as_str().map(str::to_owned))
                    .collect()
            })
            .collect(),
        // AdvancementRequirements.allOf: every criterion.
        None => criteria.iter().map(|c| vec![c.name.clone()]).collect(),
    };
    let d = &v["display"];
    let display = d.is_object().then(|| Display {
        icon: crate::crafting::recipe_result(&d["icon"])
            .unwrap_or_else(|| crate::items::ItemStack::new("minecraft:air", 1)),
        title: d["title"].clone(),
        description: d["description"].clone(),
        frame: frame(&d["frame"]),
        background: d["background"].as_str().map(str::to_owned),
        show_toast: d["show_toast"].as_bool().unwrap_or(true),
        announce: d["announce_to_chat"].as_bool().unwrap_or(true),
        hidden: d["hidden"].as_bool().unwrap_or(false),
    });
    let r = &v["rewards"];
    let strings = |v: &Value| -> Vec<String> {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_owned))
            .collect()
    };
    Some(Advancement {
        id: id.to_owned(),
        parent: v["parent"].as_str().map(|p| full(p).into_owned()),
        display,
        criteria,
        requirements,
        rewards: Rewards {
            recipes: strings(&r["recipes"]),
            experience: r["experience"].as_i64().unwrap_or(0) as i32,
            loot: strings(&r["loot"]),
            function: r["function"].as_str().map(|id| full(id).into_owned()),
        },
    })
}

fn validate_conditions(c: &Value, builtin: bool) -> Result<(), String> {
    use sunrise_world::loot::{
        validate_entity_predicate, validate_item_predicate, validate_loot_condition,
    };
    // SimpleInstance owns `player`; the custom trigger owns its other fields.
    validate_loot_condition(&c["player"])?;
    if !builtin {
        return Ok(());
    }
    for key in ["item", "rod", "fired_from_weapon"] {
        validate_item_predicate(&c[key])?;
    }
    for key in ["items", "ingredients"] {
        for item in c[key].as_array().into_iter().flatten() {
            validate_item_predicate(item)?;
        }
    }
    for key in [
        "bystander",
        "cause",
        "child",
        "entity",
        "lightning",
        "location",
        "parent",
        "partner",
        "projectile",
        "source",
        "villager",
        "zombie",
        "victims",
    ] {
        validate_loot_condition(&c[key])?;
    }
    for damage in [&c["killing_blow"], &c["damage"]["type"]] {
        validate_entity_predicate(&damage["direct_entity"])?;
        validate_entity_predicate(&damage["source_entity"])?;
    }
    validate_loot_condition(&c["damage"]["source_entity"])?;
    for location in [&c["start_position"], &c["location"]] {
        validate_item_predicate(&location["block"])?;
    }
    Ok(())
}
