//! ITEM_USED_ON_BLOCK after the block phase sees the stack copied before it (#2940).
use super::*;
use serde_json::json;
use sunrise_client::advancements as a;

/// One `item_used_on_block` advancement per stick count the tool must have.
pub(in crate::game) fn install_criteria(
    game: &mut crate::game::ServerGame<'_>,
    counts: &[u32],
    item: &str,
) {
    let mut tree = a::Tree::default();
    for count in counts {
        let trigger = "minecraft:item_used_on_block".to_owned();
        let id = format!("test:sticks{count}");
        let index = tree.list.len();
        tree.by_id.insert(id.clone(), index);
        tree.by_trigger
            .entry(trigger.clone())
            .or_default()
            .push((index, 0));
        let tool = json!({"items": item, "count": count});
        tree.list.push(a::Advancement {
            id,
            parent: None,
            display: None,
            criteria: vec![a::Criterion {
                name: "seen".into(),
                trigger,
                conditions: (if *count == 0 {
                    json!({})
                } else {
                    json!({"location": {"type": "minecraft:match_tool", "predicate": tool}})
                })
                .into(),
            }],
            requirements: vec![vec!["seen".into()]],
            rewards: Default::default(),
        });
    }
    game.adv.tree = Arc::new(tree);
    game.adv.loaded = true;
}
