use super::*;
use crate::game::commands::Target;
use crate::game::native_entity_refs::tests::with_game;
use sunrise_core::nbt::parse_snbt;

const WITNESS_26_3: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/witness/26.3/moved_thrown_item_save.json"
));

struct WitnessCase {
    kind: String,
    uuid: String,
    summon_pos: [f64; 3],
    nbt: String,
    moved_pos: Vec3,
    moved_pos_tag: Tag,
    moved_item: Tag,
    reloaded_item: Tag,
}

fn parse_exact_snbt(text: &str) -> Tag {
    let (tag, used) = parse_snbt(text).unwrap_or_else(|e| panic!("invalid snbt {text:?}: {e}"));
    assert_eq!(used, text.len(), "trailing snbt in {text:?}");
    tag
}

fn entity_data_snbt(text: &str) -> Tag {
    let (_, body) = text
        .split_once(" has the following entity data: ")
        .unwrap_or_else(|| panic!("unexpected /data get output: {text:?}"));
    parse_exact_snbt(body)
}

fn vec3_from_pos_tag(tag: &Tag) -> Vec3 {
    let Tag::List(items) = tag else {
        panic!("expected Pos list, got {tag:?}");
    };
    let [Tag::Double(x), Tag::Double(y), Tag::Double(z)] = items.as_slice() else {
        panic!("expected 3 doubles in Pos, got {tag:?}");
    };
    Vec3::new(f64::from_bits(*x), f64::from_bits(*y), f64::from_bits(*z))
}

fn load_witness_26_3() -> Vec<WitnessCase> {
    let witness: serde_json::Value = serde_json::from_str(WITNESS_26_3).unwrap();
    assert_eq!(witness["minecraft"].as_str(), Some("26.3"));
    let records = witness["records"].as_array().unwrap();
    let mut summoned: Vec<(String, String, [f64; 3], String)> = Vec::new();
    let mut moved_pos: HashMap<String, (Vec3, Tag)> = HashMap::new();
    let mut moved_item: HashMap<String, Tag> = HashMap::new();
    let mut reloaded_item: HashMap<String, Tag> = HashMap::new();
    let mut unloaded = false;
    for rec in records {
        let cmd = rec["cmd"].as_str().unwrap();
        let text = rec["text"].as_str().unwrap();
        if let Some(rest) = cmd.strip_prefix("/summon ") {
            let mut parts = rest.splitn(5, ' ');
            let kind = parts.next().unwrap().to_owned();
            let x: f64 = parts.next().unwrap().parse().unwrap();
            let y: f64 = parts.next().unwrap().parse().unwrap();
            let z: f64 = parts.next().unwrap().parse().unwrap();
            let nbt = parts.next().unwrap().to_owned();
            let tag = parse_exact_snbt(&nbt);
            let uuid = uuid_text(entity_nbt::uuid(&tag).unwrap());
            summoned.push((kind, uuid, [x, y, z], nbt));
        } else if cmd.starts_with("/execute unless loaded ") {
            assert_eq!(rec["results"], serde_json::json!([[true, 1]]));
            unloaded = true;
        } else if let Some(rest) = cmd.strip_prefix("/data get entity ") {
            let (uuid, field) = rest.split_once(' ').unwrap();
            let value = entity_data_snbt(text);
            match (unloaded, field) {
                (false, "Pos") => {
                    let pos = vec3_from_pos_tag(&value);
                    moved_pos.insert(uuid.to_owned(), (pos, value));
                }
                (false, "Item") => {
                    moved_item.insert(uuid.to_owned(), value);
                }
                (true, "Item") => {
                    reloaded_item.insert(uuid.to_owned(), value);
                }
                _ => panic!("unexpected /data get record: {cmd:?}"),
            }
        }
    }
    assert!(unloaded, "witness must record a chunk unload");
    assert!(!summoned.is_empty(), "witness must contain summon cases");
    summoned
        .into_iter()
        .map(|(kind, uuid, summon_pos, nbt)| {
            let (moved_pos, moved_pos_tag) = moved_pos.remove(&uuid).unwrap();
            let moved_item = moved_item.remove(&uuid).unwrap();
            let reloaded_item = reloaded_item.remove(&uuid).unwrap();
            WitnessCase {
                kind,
                uuid,
                summon_pos,
                nbt,
                moved_pos,
                moved_pos_tag,
                moved_item,
                reloaded_item,
            }
        })
        .collect()
}

/// #2691 (26.3): a moved thrown-item projectile matches the vanilla 26.3 save/reload witness.
#[test]
#[ignore = "daily"]
fn a_moved_thrown_item_projectile_saves_its_loaded_item() {
    let cases = load_witness_26_3();
    with_game(|game| {
        for case in &cases {
            game.summon_with_nbt(
                &format!("minecraft:{}", case.kind),
                case.summon_pos,
                &case.nbt,
            )
            .unwrap();
            let i = game.creatures.projectiles.len() - 1;
            assert_ne!(
                case.moved_pos,
                Vec3::new(case.summon_pos[0], case.summon_pos[1], case.summon_pos[2]),
                "{} {} must move from its summon position",
                case.kind,
                case.uuid
            );
            // Move the projectile to the exact post-tick position from the 26.3 witness.
            game.creatures
                .projectiles
                .update(i, |p| p.position = case.moved_pos);
            let at = game.creatures.projectiles[i].position;
            let key = chunk_of(at);
            let keys: HashSet<ChunkKey> = [key].into();
            let (_, saved) = game.save_projectile(i, &keys).unwrap();
            assert_eq!(
                saved.get("Pos"),
                Some(&case.moved_pos_tag),
                "{} {} moved Pos",
                case.kind,
                case.uuid
            );
            assert_eq!(
                saved.get("Item"),
                Some(&case.moved_item),
                "{} {} moved Item",
                case.kind,
                case.uuid
            );
            // Unload and reload the saved compound through the chunk entity load path.
            let id = game.creatures.projectiles[i].id;
            game.creatures.projectiles.remove(i);
            game.forget_entity(id);
            game.world.queue_entity_nbt(key, vec![saved]);
            game.spawn_loaded_entities();
            let j = game.creatures.projectiles.len() - 1;
            let reloaded_id = game.creatures.projectiles[j].id;
            let reloaded_data = game
                .command_target_nbt(Target::Projectile(reloaded_id))
                .unwrap();
            assert_eq!(
                reloaded_data.get("Item"),
                Some(&case.reloaded_item),
                "{} {} reloaded /data get Item",
                case.kind,
                case.uuid
            );
            let (_, resaved) = game.save_projectile(j, &keys).unwrap();
            assert_eq!(
                resaved.get("Item"),
                Some(&case.reloaded_item),
                "{} {} reloaded save Item",
                case.kind,
                case.uuid
            );
            game.creatures.projectiles.remove(j);
            game.forget_entity(reloaded_id);
        }
    });
}
