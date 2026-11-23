//! Dropped-item hurt uses `ItemStack.canBeHurtBy` of the source that hit it.
//! A fire-tagged source must not also require `minecraft:in_fire` resistance.
use sunrise_client::{item_damage::stack_can_be_hurt_by, item_data::parse_stack};
use sunrise_world::damage_source::Catalog;

fn catalog() -> Catalog {
    let types = [
        "minecraft:in_fire",
        "minecraft:hot_floor",
        "minecraft:cactus",
    ]
    .map(|id| {
        serde_json::json!({
            "id": id, "message_id": "test", "exhaustion": "00000000",
            "scaling": "never", "effects": "hurt", "death_message_type": "default"
        })
    });
    Catalog::from_json(
        &serde_json::json!({"data_version": sunrise_world::DATA_VERSION, "types": types}),
        &serde_json::json!({
            "minecraft:is_fire": r#"{"values":["in_fire","hot_floor"]}"#,
        }),
    )
    .unwrap()
}

fn stack(snbt: &str) -> sunrise_client::items::ItemStack {
    parse_stack(snbt).unwrap().unwrap()
}

fn takes_source(
    catalog: &Catalog,
    stack: &sunrise_client::items::ItemStack,
    damage_type: &str,
) -> bool {
    stack_can_be_hurt_by(None, catalog, stack, damage_type) != Some(false)
}

#[test]
fn an_in_fire_resistant_stack_still_takes_hot_floor() {
    let catalog = catalog();
    assert!(catalog.in_tag(
        catalog.id("minecraft:hot_floor").unwrap(),
        "minecraft:is_fire"
    ));
    let resistant = stack(
        r#"{id:"minecraft:stone",count:1,components:{"minecraft:damage_resistant":{types:"minecraft:in_fire"}}}"#,
    );
    assert!(!takes_source(&catalog, &resistant, "minecraft:in_fire"));
    assert!(takes_source(&catalog, &resistant, "minecraft:hot_floor"));
    assert!(takes_source(&catalog, &resistant, "minecraft:cactus"));
    let plain = stack(r#"{id:"minecraft:stone",count:1}"#);
    assert!(takes_source(&catalog, &plain, "minecraft:in_fire"));
    assert!(takes_source(&catalog, &plain, "minecraft:hot_floor"));
}
