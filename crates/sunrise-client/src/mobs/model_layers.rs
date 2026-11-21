//! Engine registration and vanilla resource conversion for model layers.
use super::{EntityModelDocument, Part};
use mod_api::{
    __private::game::{CallbackOwner, LayerDefinitionRegistration},
    Answered, Identifier, LayerDefinition, ModelPart, ModelPolygon, ModelVertex, PartPose,
};
use std::{collections::HashMap, sync::OnceLock};

/// A vanilla layer's part tree from `entity-models.json`.
pub fn from_part(part: &Part) -> ModelPart {
    let p = part.pose;
    let mut pose = PartPose::offset_and_rotation(p[0], p[1], p[2], p[3], p[4], p[5]);
    pose.x_scale = p[6];
    pose.y_scale = p[7];
    pose.z_scale = p[8];
    let polygons = part
        .polygons
        .iter()
        .map(|poly| {
            ModelPolygon::new(
                poly.vertices
                    .map(|v| ModelVertex::new([v[0], v[1], v[2]], v[3], v[4])),
                poly.normal,
            )
        })
        .collect();
    let children = part
        .children
        .iter()
        .map(|c| (c.name.clone(), from_part(c)))
        .collect();
    ModelPart::new(polygons, children, pose)
}

static REGISTERED: OnceLock<HashMap<String, LayerDefinition>> = OnceLock::new();

/// Realize each winning mod supplier once, after checking the prepared vanilla layer keys.
pub fn register(layers: Vec<Answered<LayerDefinitionRegistration>>, vanilla: &EntityModelDocument) {
    let mut assembled = false;
    REGISTERED.get_or_init(|| {
        assembled = true;
        assemble(&layers, vanilla)
    });
    if !assembled {
        log::warn!("model layers were registered already; these are left out");
    }
}

fn assemble(
    layers: &[Answered<LayerDefinitionRegistration>],
    vanilla: &EntityModelDocument,
) -> HashMap<String, LayerDefinition> {
    let mut winners = HashMap::new();
    for (index, entry) in layers.iter().enumerate() {
        let id = &entry.answer.id;
        let mod_id = entry.mod_id;
        if !is_valid_layer_id(id) {
            log::warn!("Mod {mod_id}: model layer {id:?} is not <namespace>:<path>#<layer>");
            continue;
        }
        if vanilla.value().get(id.as_str()).is_some() {
            log::warn!("Mod {mod_id}: model layer {id} is a built-in vanilla layer; left out");
            continue;
        }
        winners.insert(id.as_str(), (index, entry));
    }
    let mut winners: Vec<_> = winners.into_values().collect();
    winners.sort_unstable_by_key(|(index, _)| *index);
    let mut definitions = HashMap::new();
    for (_, entry) in winners {
        let LayerDefinitionRegistration { id, supplier, .. } = &entry.answer;
        let mod_id = entry.mod_id;
        let Some(owner) = CallbackOwner::new(mod_id) else {
            log::warn!("Model layer {id} has no installed owner {mod_id}");
            continue;
        };
        match owner.call("model layer supplier", "build_layer_definition", supplier) {
            Ok(definition) => {
                definitions.insert(id.clone(), definition);
            }
            Err(error) => log::warn!("Model layer {id}: {error}"),
        }
    }
    definitions
}

/// The realized definition of the layer a native mod registered as `id`.
pub fn registered(id: &str) -> Option<&'static LayerDefinition> {
    REGISTERED.get()?.get(id)
}

fn is_valid_layer_id(id: &str) -> bool {
    let Some((location, layer)) = id.split_once('#') else {
        return false;
    };
    Identifier::parse_namespaced(location).is_some()
        && !layer.is_empty()
        && !layer.contains(['#', ':'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{CubeDeformation, CubeListBuilder, MeshDefinition};

    #[test]
    fn only_winning_suppliers_run_and_builtin_layers_stay_reserved() {
        use mod_api::__private::game::RegistrationContextNew;
        use mod_api::{Hooks, ModEntry, RegisterLayerDefinitionsEvent, RegistrationContext};
        use std::sync::{Arc, Mutex};

        struct Layers(&'static [(&'static str, i32)], Arc<Mutex<Vec<i32>>>);
        #[mod_api::__subscriptions]
        impl Hooks for Layers {
            fn register_layer_definitions(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterLayerDefinitionsEvent<'_>,
            ) {
                for &(id, offset) in self.0 {
                    let calls = self.1.clone();
                    event.register(id, move || {
                        calls.lock().unwrap().push(offset);
                        let mut mesh = MeshDefinition::new();
                        mesh.add_or_replace_child(
                            mesh.root(),
                            "head",
                            CubeListBuilder::create(),
                            PartPose::offset(0.0, offset as f32, 0.0),
                        );
                        LayerDefinition::create(mesh, 64, 32)
                    });
                }
            }
        }

        let vanilla_root =
            std::env::temp_dir().join(format!("sunrise-model-layers-test-{}", std::process::id()));
        std::fs::create_dir_all(&vanilla_root).unwrap();
        std::fs::write(
            vanilla_root.join("entity-models.json"),
            r#"{"minecraft:zombie#main": {"pose": [0,0,0,0,0,0,1,1,1], "children": {}}}"#,
        )
        .unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        mod_platform::testing::with_mods(
            vec![
                ModEntry::new(
                    "first",
                    Box::new(Layers(
                        &[
                            ("minecraft:zombie#main", 1),
                            ("minecraft:golem#main", 4),
                            ("first:golem#main", 6),
                        ],
                        calls.clone(),
                    )),
                ),
                ModEntry::new(
                    "second",
                    Box::new(Layers(
                        &[("first:golem#main", 9), ("second:golem#main", 12)],
                        calls.clone(),
                    )),
                ),
            ]
            .into(),
            || {
                let mut suppliers = Vec::new();
                mod_api::dispatch_register_layer_definitions(
                    &RegistrationContext::new(),
                    &mut suppliers,
                );
                assert!(calls.lock().unwrap().is_empty());
                let vanilla = EntityModelDocument::load(&vanilla_root).unwrap();
                let definitions = assemble(&suppliers, &vanilla);
                assert_eq!(*calls.lock().unwrap(), [4, 9, 12]);
                assert!(!definitions.contains_key("minecraft:zombie#main"));
                assert_eq!(
                    definitions["minecraft:golem#main"]
                        .bake_root()
                        .get_child("head")
                        .unwrap()
                        .y,
                    4.0
                );
                assert_eq!(
                    definitions["first:golem#main"]
                        .bake_root()
                        .get_child("head")
                        .unwrap()
                        .y,
                    9.0
                );
            },
        );
        let _ = std::fs::remove_dir_all(&vanilla_root);
    }

    #[test]
    fn layer_ids_require_valid_location_and_layer() {
        assert!(is_valid_layer_id("mymod:golem#main"));
        assert!(is_valid_layer_id("minecraft:golem#main"));
        assert!(is_valid_layer_id("my_mod:mobs/golem.v2#outer_armor"));
        for bad in [
            "mymod:golem",
            "golem#main",
            "mymod:#main",
            "mymod:golem#",
            "MyMod:golem#main",
            "mymod:golem#a#b",
            "mymod:golem#a:b",
        ] {
            assert!(!is_valid_layer_id(bad), "{bad}");
        }
    }
    /// `HumanoidModel.createMesh(CubeDeformation.NONE, 0)` built with these
    /// builders bakes what vanilla baked for the zombie
    /// (`minecraft:zombie#main` in the prepared `entity-models.json`):
    /// `cargo test -p sunrise-client-lib --lib model_layers -- --ignored`
    /// with SUNRISE_TEST_RESOURCES set to the prepared resources.
    #[test]
    #[ignore = "daily"]
    fn the_humanoid_mesh_bakes_as_vanillas_zombie() {
        let root = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("SUNRISE_TEST_RESOURCES"),
        );
        let vanilla =
            super::super::load_model(&root, "minecraft:zombie#main").expect("zombie#main");
        let vanilla = from_part(&vanilla);

        // `HumanoidModel.createMesh`: (part, parent, texOffs, mirror, origin, size, grow, pose).
        let half = CubeDeformation::new(0.5);
        let none = CubeDeformation::NONE;
        let (z4, limb) = ([-4.0, -8.0, -4.0], [4.0, 12.0, 4.0]);
        #[rustfmt::skip]
        let parts = [
            ("head", "", (0, 0), false, z4, [8.0; 3], none, [0.0; 3]),
            ("hat", "head", (32, 0), false, z4, [8.0; 3], half, [0.0; 3]),
            ("body", "", (16, 16), false, [-4.0, 0.0, -2.0], [8.0, 12.0, 4.0], none, [0.0; 3]),
            ("right_arm", "", (40, 16), false, [-3.0, -2.0, -2.0], limb, none, [-5.0, 2.0, 0.0]),
            ("left_arm", "", (40, 16), true, [-1.0, -2.0, -2.0], limb, none, [5.0, 2.0, 0.0]),
            ("right_leg", "", (0, 16), false, [-2.0, 0.0, -2.0], limb, none, [-1.9, 12.0, 0.0]),
            ("left_leg", "", (0, 16), true, [-2.0, 0.0, -2.0], limb, none, [1.9, 12.0, 0.0]),
        ];
        let mut mesh = MeshDefinition::new();
        for (name, parent, (u, v), mirror, origin, size, grow, [x, y, z]) in parts {
            let parent = mesh.child(mesh.root(), parent).unwrap_or(mesh.root());
            let cubes = CubeListBuilder::create()
                .tex_offs(u, v)
                .set_mirror(mirror)
                .add_box_with(origin, size, grow);
            mesh.add_or_replace_child(parent, name, cubes, PartPose::offset(x, y, z));
        }
        let ours = LayerDefinition::create(mesh, 64, 64).bake_root();

        fn same(path: &str, ours: &ModelPart, vanilla: &ModelPart) {
            assert_eq!(ours.store_pose(), vanilla.store_pose(), "{path} pose");
            assert_eq!(ours.polygons, vanilla.polygons, "{path} faces");
            let mut names: Vec<_> = ours.children().map(|(n, _)| n).collect();
            let mut theirs: Vec<_> = vanilla.children().map(|(n, _)| n).collect();
            names.sort_unstable();
            theirs.sort_unstable();
            assert_eq!(names, theirs, "{path} children");
            for (name, child) in ours.children() {
                same(
                    &format!("{path}/{name}"),
                    child,
                    vanilla.get_child(name).unwrap(),
                );
            }
        }
        same("root", &ours, &vanilla);
    }
}
