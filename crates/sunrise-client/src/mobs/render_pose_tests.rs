fn pose_test_cube(x: f32) -> serde_json::Value {
    serde_json::json!({ "pose": [x, 0.0, 0.0], "polygons": [[[x, 0.0, 0.0, 0.0, 0.0], [x + 1.0, 0.0, 0.0, 1.0, 0.0], [x + 1.0, 1.0, 0.0, 1.0, 1.0], [x, 1.0, 0.0, 0.0, 1.0]]] })
}

fn quadruped_fixture() -> Mob {
    let cube = pose_test_cube;
    let mut head = cube(0.0);
    head["pose"] = serde_json::json!([3.5, -2.5, 1.0, 0.0, 0.0, 0.25, 2.0, 0.5, 1.0]);
    head["children"] = serde_json::json!({"jaw": cube(1.0)});
    let quadruped = serde_json::json!({ "children": {
        "head": head, "body": cube(0.0), "right_hind_leg": cube(-2.0),
        "left_hind_leg": cube(2.0), "right_front_leg": cube(-2.0), "left_front_leg": cube(2.0),
    }});
    let mut kind = (*crate::mobs::sensor_scan_tests::kind()).clone();
    kind.species = Species::Cow;
    kind.anim = super::super::Anim::Quadruped;
    kind.model = std::sync::Arc::new(Part::parse("root", &quadruped));
    let mut cow = Mob::new(
        2,
        std::sync::Arc::new(kind),
        sunrise_core::geometry::Vec3::ZERO,
        0.0,
    );
    cow.age = 37;
    cow
}

#[test]
fn frame_model_lends_the_drawn_pose_without_changing_geometry() {
    use std::f32::consts::FRAC_PI_2;

    let mut cow = quadruped_fixture();
    cow.head_pitch = 90.0;
    cow.previous_head_pitch = 90.0;
    let (_, mut data) = humanoid_zombie();
    data.frame_models = BakedFrameModels::bake([cow.kind.as_ref()]);
    let atlas = |_: &str| Some([0.0, 0.0, 1.0, 1.0]);
    let check = |model: &dyn mod_api::FrameModel| {
        // The empty root still leads to its children. The head's pitch is animated on this
        // draw, while its offset, roll and scale are the fixture's literal local values.
        let root = model.root();
        assert_eq!((root.index, root.name, root.parent), (0, "root", None));
        assert_eq!((root.first_child, root.next_sibling), (Some(1), None));
        assert!(root.visible);
        let body = model.part_at(1).unwrap();
        assert_eq!(
            (body.name, body.parent, body.next_sibling),
            ("body", Some(0), Some(2))
        );
        let head = model.part("head").unwrap();
        assert_eq!(
            (head.index, head.parent, head.first_child, head.next_sibling),
            (2, Some(0), Some(3), Some(4))
        );
        let expected = [3.5, -2.5, 1.0, FRAC_PI_2, 0.0, 0.25, 2.0, 0.5, 1.0];
        assert_eq!(*head.local_pose, expected);
        let pose = head.pose();
        assert_eq!(
            [
                pose.x,
                pose.y,
                pose.z,
                pose.x_rot,
                pose.y_rot,
                pose.z_rot,
                pose.x_scale,
                pose.y_scale,
                pose.z_scale
            ],
            expected,
        );
        let jaw = model.part_at(3).unwrap();
        assert_eq!(
            (jaw.name, jaw.parent, jaw.first_child, jaw.next_sibling),
            ("jaw", Some(2), None, None)
        );
        assert_eq!(jaw.pose(), mod_api::PartPose::offset(1.0, 0.0, 0.0));
        assert!(jaw.visible);
        assert!(model.part("left_sleeve").is_none());
        assert!(model.part_at(8).is_none());
    };

    let cpu = |capture_model, layers_only| {
        let mut frame = FrameDraw::new(capture_model);
        let mut out = Vec::new();
        if layers_only {
            let extra = [(
                &*cow.kind.model,
                [0.0, 0.0, 1.0, 1.0],
                0xFFFF_FFFF,
                0,
                false,
            )];
            render_layers(
                &cow,
                0.5,
                glam::DVec3::ZERO,
                &atlas,
                0xf0f0,
                &data,
                &extra,
                &mut frame,
                &mut out,
            );
        } else {
            render_with_frame(
                &cow,
                0.5,
                glam::DVec3::ZERO,
                &atlas,
                0xf0f0,
                &data,
                &[],
                &mut frame,
                &mut out,
            );
        }
        (out, frame)
    };
    for layers_only in [false, true] {
        let (before, uncaptured) = cpu(false, layers_only);
        let (after, captured) = cpu(true, layers_only);
        assert!(uncaptured.model().is_none());
        assert!(!before.is_empty());
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&before),
            bytemuck::cast_slice::<_, u8>(&after)
        );
        check(captured.model().unwrap());
    }

    // The skinned path keeps the same local pose and submits the same GPU jobs and matrices.
    let gpu = |capture_model| {
        let mut models = super::super::skin::SkinModels::default();
        let mut skin_frame = crate::gfx::entity_skin::SkinFrame::default();
        let mut skin = super::super::skin::Skin::new(&mut models, &mut skin_frame);
        let mut frame = FrameDraw::new(capture_model);
        let mut out = Vec::new();
        render_skinned(
            &cow,
            0.5,
            glam::DVec3::ZERO,
            &atlas,
            0xf0f0,
            &data,
            &mut frame,
            &mut out,
            &mut skin,
        );
        assert!(out.is_empty());
        (skin_frame, frame)
    };
    let (before, uncaptured) = gpu(false);
    let (after, captured) = gpu(true);
    assert!(uncaptured.model().is_none());
    assert!(!before.jobs.is_empty(), "the cow takes the skinned path");
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&before.jobs),
        bytemuck::cast_slice::<_, u8>(&after.jobs)
    );
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&before.parts),
        bytemuck::cast_slice::<_, u8>(&after.parts)
    );
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&before.new_model),
        bytemuck::cast_slice::<_, u8>(&after.new_model)
    );
    check(captured.model().unwrap());
}

#[test]
fn named_hidden_parts_leave_the_body_and_its_overlays_but_not_armour() {
    let cow = quadruped_fixture();
    let cube = pose_test_cube;
    let (_, data) = humanoid_zombie();
    let atlas = |_: &str| Some([0.0, 0.0, 1.0, 1.0]);
    // `render_living_pre` hides a part of any model by its name, on the CPU and on the GPU
    // (#2957): a leg's one face goes; an unknown name is refused.
    let mut leg = mod_api::HiddenParts::default();
    leg.hide_named("right_hind_leg").unwrap();
    let mut absent = mod_api::HiddenParts::default();
    absent.hide_named("nose").unwrap();
    let cpu = |mob: &Mob, hidden| {
        let mut out = Vec::new();
        render_with_frame(
            mob,
            0.5,
            glam::DVec3::ZERO,
            &atlas,
            0xf0f0,
            &data,
            &[],
            &mut FrameDraw {
                hidden,
                ..Default::default()
            },
            &mut out,
        );
        out.len()
    };
    let whole = cpu(&cow, Default::default());
    assert_eq!(
        cpu(&cow, leg),
        whole - 4,
        "the leg's face leaves the CPU draw"
    );
    let mut poses = Poses::pooled();
    let name = hide_parts(&mut poses, &absent).unwrap_err();
    assert_eq!(name, "nose");
    assert_eq!(cpu(&cow, absent), 0, "unknown names refuse the draw");
    let gpu = |mob: &Mob, hidden| {
        let mut models = super::super::skin::SkinModels::default();
        let mut frame = crate::gfx::entity_skin::SkinFrame::default();
        let mut skin = super::super::skin::Skin::new(&mut models, &mut frame);
        render_skinned(
            mob,
            0.5,
            glam::DVec3::ZERO,
            &atlas,
            0xf0f0,
            &data,
            &mut FrameDraw {
                hidden,
                ..Default::default()
            },
            &mut Vec::new(),
            &mut skin,
        );
        frame.jobs.iter().map(|job| job.count).sum::<u32>()
    };
    let whole = gpu(&cow, Default::default());
    assert!(whole > 0, "the cow takes the GPU path");
    assert_eq!(
        gpu(&cow, leg),
        whole - 4,
        "the leg's face leaves the GPU draw"
    );
    // Villager hat_rim is a child of hat; overlays and armour retain the full tree.
    let mut hat = cube(0.0);
    hat["children"] = serde_json::json!({"hat_rim": cube(0.0)});
    let mut head = cube(0.0);
    head["children"] = serde_json::json!({"hat": hat, "nose": cube(0.0)});
    let tree = serde_json::json!({"children": {"head": head, "body": cube(0.0)}});
    let mut kind = (*cow.kind).clone();
    kind.id = "minecraft:villager".into();
    kind.species = Species::Villager;
    kind.anim = super::super::Anim::Villager;
    kind.model = std::sync::Arc::new(Part::parse("root", &tree));
    let villager = Mob::new(
        3,
        std::sync::Arc::new(kind),
        sunrise_core::geometry::Vec3::ZERO,
        0.0,
    );
    let mut hidden = mod_api::HiddenParts::default();
    hidden.hide_named("hat_rim").unwrap();
    hidden.hide_named("nose").unwrap();
    assert_eq!(
        cpu(&villager, hidden),
        cpu(&villager, Default::default()) - 8
    );
    assert_eq!(
        gpu(&villager, hidden),
        gpu(&villager, Default::default()) - 8
    );
    let armour = [(
        &*villager.kind.model,
        [0.0, 0.0, 1.0, 1.0],
        0xf0f0,
        0,
        false,
    )];
    for gpu_path in [false, true] {
        let draw_armoured = |hidden, extra: &[(&Part, [f32; 4], u32, u32, bool)]| {
            let mut out = Vec::new();
            let mut models = super::super::skin::SkinModels::default();
            let mut frame = crate::gfx::entity_skin::SkinFrame::default();
            let mut skin = super::super::skin::Skin::new(&mut models, &mut frame);
            render_visible(
                &villager,
                0.5,
                glam::Mat4::IDENTITY,
                &atlas,
                0xf0f0,
                &data,
                extra,
                Some(body_color(&villager, 0.5)),
                &mut FrameDraw {
                    hidden,
                    ..Default::default()
                },
                &mut out,
                gpu_path.then_some(&mut skin),
            );
            out.len() as u32 + frame.jobs.iter().map(|job| job.count).sum::<u32>()
        };
        let bare = draw_armoured(Default::default(), &[]);
        let armoured = draw_armoured(Default::default(), &armour);
        assert!(armoured > bare);
        assert_eq!(
            draw_armoured(hidden, &armour),
            armoured - 8,
            "armour retains every part on CPU and GPU"
        );
    }
    let mut parent = mod_api::HiddenParts::default();
    parent.hide_named("hat").unwrap();
    assert_eq!(
        cpu(&villager, parent),
        cpu(&villager, Default::default()) - 8
    );
    assert_eq!(
        gpu(&villager, parent),
        gpu(&villager, Default::default()) - 8
    );
    // VillagerProfessionLayer draws the type, profession and level on `getParentModel()`:
    // a part hidden on the body leaves them too.
    let mut kind = (*villager.kind).clone();
    kind.layers
        .insert("villager#main".into(), kind.model.clone());
    let mut layered = Mob::new(
        4,
        std::sync::Arc::new(kind),
        sunrise_core::geometry::Vec3::ZERO,
        0.0,
    );
    let table = super::super::villager::ResolvedProfessions::vanilla();
    super::villager_client::install(&table, &Default::default());
    layered.extra.profession = super::super::villager::ProfessionState::Vanilla(5);
    assert_eq!(layers(&layered, 0.0).len(), 3, "type, profession and level");
    let mut nose = mod_api::HiddenParts::default();
    nose.hide_named("nose").unwrap();
    assert_eq!(
        cpu(&layered, nose),
        cpu(&layered, Default::default()) - 16,
        "the nose leaves the body and every layer on it"
    );
    assert_eq!(gpu(&layered, nose), gpu(&layered, Default::default()) - 16,);
    // A full profession hat makes the type overlay use the no-hat poses.
    // Its head (including nose) is absent even before the mod hides the nose;
    // the body, profession and level each lose one four-vertex polygon.
    use sunrise_assets::resource_source::{Resource, ResourceSource};
    let source = ResourceSource::indexed(std::collections::BTreeMap::from([
        (
            "assets/minecraft/textures/entity/villager/profession/farmer.png".into(),
            Resource::bytes(vec![]),
        ),
        (
            "assets/minecraft/textures/entity/villager/profession/farmer.png.mcmeta".into(),
            Resource::bytes(br#"{"villager":{"hat":"full"}}"#.to_vec()),
        ),
    ]));
    super::villager_client::install(
        &table,
        &super::super::villager_hats::VillagerHats::load(&source),
    );
    assert!(!super::villager_client::type_hat_shown(
        &layered.extra.profession,
        false,
        layered.extra.variant
    ));
    assert_eq!(cpu(&layered, nose), cpu(&layered, Default::default()) - 12);
    assert_eq!(gpu(&layered, nose), gpu(&layered, Default::default()) - 12);
    super::villager_client::install(&table, &Default::default());
}

#[test]
fn equipment_model_override_draws_with_body_poses_and_inventory_does_not_dispatch_it() {
    struct Source {
        data: MobData,
        model: std::sync::Arc<Part>,
        calls: std::cell::Cell<usize>,
    }
    impl RenderDataSource for Source {
        fn mob_data(&self) -> &MobData {
            &self.data
        }
        fn equipment_layers(&self, _: &Mob) -> Vec<Layer> {
            self.calls.set(self.calls.get() + 1);
            let mut layer = Layer::equipment(
                "no_vanilla_layer",
                "test:equipment",
                0xFFFFFFFF,
                ARMOR_GLINT,
            );
            layer.equipment_model = Some(self.model.clone());
            vec![layer]
        }
        fn equipment_inventory(&self, _: &Mob) -> Vec<Layer> {
            Vec::new()
        }
    }
    let (mob, data) = humanoid_zombie();
    let source = Source {
        model: mob.kind.model.clone(),
        data,
        calls: std::cell::Cell::new(0),
    };
    let uv = [0.25, 0.25, 0.5, 0.5];
    let atlas = |name: &str| {
        Some(if name == "test:equipment" {
            uv
        } else {
            [0.0, 0.0, 1.0, 1.0]
        })
    };
    assert!(
        !drawn_textures(&mob, &source)
            .iter()
            .any(|name| name == "test:equipment")
    );
    assert_eq!(
        source.calls.get(),
        0,
        "inventory must not invoke frame providers"
    );
    let mut actual = Vec::new();
    render_with(
        &mob,
        0.5,
        glam::DVec3::ZERO,
        &atlas,
        0xF0F0,
        &source,
        &[],
        &mut actual,
    );
    let mut expected = Vec::new();
    render_with(
        &mob,
        0.5,
        glam::DVec3::ZERO,
        &atlas,
        0xF0F0,
        &source.data,
        &[(&source.model, uv, 0xFFFFFFFF, ARMOR_GLINT, false)],
        &mut expected,
    );
    assert_eq!(source.calls.get(), 1);
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&actual),
        bytemuck::cast_slice::<_, u8>(&expected)
    );
}
