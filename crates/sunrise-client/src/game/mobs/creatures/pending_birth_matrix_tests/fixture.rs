//! The mode is the last decimal digit of the parent entity id. Refusals are written into
//! the first child's yaw as digits: 0 ok, 1 Expired, 2 Ownership, 3 Capacity, 4 Invalid,
//! 5 Budget, 9 anything else.

use mod_api::{
    BabyEntitySpawnEvent, Flow, Identifier, MobSplitEvent, PendingMobSpec, ServerHost,
    UnspawnedMobError,
};

fn code<T>(result: Result<T, UnspawnedMobError>) -> f32 {
    match result {
        Ok(_) => 0.0,
        Err(UnspawnedMobError::Expired) => 1.0,
        Err(UnspawnedMobError::Ownership) => 2.0,
        Err(UnspawnedMobError::Capacity) => 3.0,
        Err(UnspawnedMobError::Invalid) => 4.0,
        Err(UnspawnedMobError::Budget) => 5.0,
        Err(_) => 9.0,
    }
}

fn spec(
    kind: &str,
    dim: &str,
    at: [f64; 3],
    yaw: f32,
    size: Option<u32>,
    baby: bool,
) -> PendingMobSpec {
    let mut spec = PendingMobSpec::new(
        Identifier::parse(kind).unwrap(),
        Identifier::parse(dim).unwrap(),
        at,
        yaw,
        baby,
    );
    spec.size = size;
    spec
}

/// `mob_split`: mode 0 changes nothing, 1 cancels, 2 edits A, removes B and appends D, 3 replaces C
/// with E and reports a second append of E in A's yaw, 4 tries invalid input, 5 reads a removed child.
pub fn mob_split(host: &mut dyn ServerHost, event: &mut MobSplitEvent<'_>) -> Flow {
    let (edits, children, dim) = (
        event.edits,
        event.edits.children().unwrap(),
        event.dimension,
    );
    match event.parent % 10 {
        1 => return Flow::Cancel,
        2 => {
            let a = edits.child(children, 0).unwrap();
            edits.set_yaw(a, 90.0).unwrap();
            edits.remove(children, 1).unwrap();
            let d = edits
                .construct(
                    children,
                    spec(
                        "minecraft:slime",
                        dim,
                        [3.0, 70.0, 4.0],
                        12.0,
                        Some(2),
                        false,
                    ),
                )
                .unwrap();
            edits.append(children, d).unwrap();
        }
        3 => {
            let a = edits.child(children, 0).unwrap();
            let e = edits
                .construct(
                    children,
                    spec(
                        "minecraft:magma_cube",
                        dim,
                        [8.0, 65.0, 9.0],
                        33.0,
                        Some(2),
                        false,
                    ),
                )
                .unwrap();
            edits.replace(children, 2, e).unwrap();
            edits.set_yaw(a, code(edits.append(children, e))).unwrap();
        }
        4 => {
            let a = edits.child(children, 0).unwrap();
            let slime = |at: [f64; 3], d: &str| spec("minecraft:slime", d, at, 0.0, Some(2), false);
            let mut digits = vec![
                code(edits.construct(children, slime([f64::NAN, 64.0, 0.0], dim))),
                code(edits.construct(
                    children,
                    spec(
                        "minecraft:not_a_mob",
                        dim,
                        [0.0, 64.0, 0.0],
                        0.0,
                        None,
                        false,
                    ),
                )),
                code(edits.construct(children, slime([0.0, 64.0, 0.0], "minecraft:the_end"))),
            ];
            let d = edits
                .construct(children, slime([0.0, 64.0, 0.0], dim))
                .unwrap();
            edits.append(children, d).unwrap();
            digits.push(code(edits.append(children, d)));
            while edits.len(children).unwrap() < 64 {
                let more = edits
                    .construct(children, slime([0.0, 64.0, 0.0], dim))
                    .unwrap();
                edits.append(children, more).unwrap();
            }
            let extra = edits
                .construct(children, slime([0.0, 64.0, 0.0], dim))
                .unwrap();
            digits.push(code(edits.append(children, extra)));
            digits.push(code(edits.set_position(a, [f64::NAN, 0.0, 0.0])));
            let packed = digits.iter().rev().fold(0.0, |acc, d| acc * 10.0 + d);
            edits.set_yaw(a, packed).unwrap();
        }
        5 => {
            let a = edits.child(children, 0).unwrap();
            let b = edits.child(children, 1).unwrap();
            edits.remove(children, 1).unwrap();
            let read = b.kind(&*host).map(|_| ());
            edits.set_yaw(a, code(read)).unwrap();
        }
        _ => {}
    }
    Flow::Continue
}

/// `baby_entity_spawn`: mode 0 changes nothing, 1 cancels, 2 replaces the baby with a pig baby
/// (yaw 77.0) and drops an unattached second baby, 3 clears the baby, 4 offers a grown replacement
/// and reports its refusal (`Invalid` is 4) in the kept baby's yaw.
pub fn baby_entity_spawn(_: &mut dyn ServerHost, event: &mut BabyEntitySpawnEvent<'_>) -> Flow {
    let dim = event.dimension;
    let pig = |baby| spec("minecraft:pig", dim, [0.5, 64.0, 0.0], 0.0, None, baby);
    match event.parent % 10 {
        1 => return Flow::Cancel,
        2 => {
            let mut replacement = event.child.construct(pig(true)).unwrap();
            replacement.set_yaw(77.0).unwrap();
            let prev = event.child.replace(replacement).unwrap();
            assert_eq!(prev.unwrap().kind().unwrap().to_string(), "minecraft:cow");
            let _dropped = event.child.construct(pig(true)).unwrap();
        }
        3 => event.child.clear(),
        4 => {
            let grown = code(event.child.construct(pig(false)));
            event.child.get_mut().unwrap().set_yaw(grown).unwrap();
        }
        _ => {}
    }
    Flow::Continue
}
