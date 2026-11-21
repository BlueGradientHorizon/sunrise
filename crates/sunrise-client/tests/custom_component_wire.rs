extern crate sunrise_client_lib as sunrise_client;
use sunrise_client::{item_sync, items::ItemStack, lan::guest::stack_to_network};
use sunrise_core::nbt::Tag;
use sunrise_core::vanilla_bytes::Writer;
use sunrise_items::item_nbt::{item_from_nbt, item_to_nbt};
use sunrise_protocol::{
    buf::{Codec, Reader},
    item::{DelimitedSlot, Hashed, Slot, Stack, Template, component, custom},
    registry::{Builtin, Dynamic, Ids, SyncedNames},
};

const FIRST_CUSTOM_ID: i32 = 122;

#[test]
fn custom_components_survive_save_wire_and_nested_stacks() {
    custom::install(
        FIRST_CUSTOM_ID,
        vec!["sample:charge".into(), "sample:mode".into()],
    )
    .unwrap();
    let mut component_names = vec![String::new(); FIRST_CUSTOM_ID as usize];
    component_names[sunrise_protocol::item::component::BUNDLE_CONTENTS as usize] =
        "minecraft:bundle_contents".into();
    component_names[sunrise_protocol::item::component::CONTAINER as usize] =
        "minecraft:container".into();
    let builtin = Builtin {
        registries: [
            (
                "minecraft:item".into(),
                Ids::from_names(vec!["minecraft:air".into(), "minecraft:stone".into()]),
            ),
            (
                "minecraft:data_component_type".into(),
                Ids::from_names(component_names),
            ),
        ]
        .into(),
    };
    let dynamic = Dynamic {
        custom_components: true,
        ..Default::default()
    };
    let names = SyncedNames {
        builtin: &builtin,
        dynamic: &dynamic,
    };
    let value = Tag::compound([("charge", Tag::Int(42))]);
    let mut item = ItemStack::new("minecraft:stone", 1);
    item.data_mut()
        .set_other("sample:charge", Some(value.clone()));
    item.data_mut()
        .set_other("!sample:mode", Some(Tag::Compound(Default::default())));

    let saved = item_from_nbt(&item_to_nbt(&item, &Default::default())).unwrap();
    assert!(item.same_item_same_components(&saved));

    let encode = |item: &ItemStack| {
        stack_to_network(|_| 1, |_, name| custom::id(name), &Some(item.clone())).unwrap()
    };
    let vanilla = stack_to_network(|_| 1, |_, _| None, &Some(saved.clone())).unwrap();
    assert!(vanilla.added.is_empty());
    assert!(vanilla.removed.is_empty());
    let stack = encode(&saved);
    assert!(!custom::permitted(&stack, false).unwrap());
    assert!(custom::permitted(&stack, true).unwrap());
    let mut removed = Stack::new(1, 1);
    removed.removed.push(FIRST_CUSTOM_ID + 1);
    assert!(!custom::permitted(&removed, false).unwrap());
    for kind in [
        component::USE_REMAINDER,
        component::POT_DECORATIONS,
        component::CONTAINER,
        component::SULFUR_CUBE_CONTENT,
    ] {
        let mut bytes = Writer::new();
        if kind == component::CONTAINER {
            bytes.varint(1);
        }
        if kind == component::POT_DECORATIONS || kind == component::CONTAINER {
            bytes.bool(true);
        }
        Template::write(&removed, &mut bytes);
        if kind == component::POT_DECORATIONS {
            for _ in 0..3 {
                bytes.bool(false);
            }
        }
        let mut outer = Stack::new(1, 1);
        outer
            .added
            .push((kind, sunrise_protocol::item::Component::Raw(bytes.data)));
        assert!(!custom::permitted(&outer, false).unwrap(), "kind {kind}");
        assert!(custom::permitted(&outer, true).unwrap());
    }
    let hash = Hashed::create(&stack, &names).unwrap();
    assert_eq!(hash.added.len(), 1);
    assert_eq!(hash.removed, [FIRST_CUSTOM_ID + 1]);
    assert_eq!(
        hash.added[0].1,
        sunrise_protocol::item::Component::Nbt(value)
            .hash(FIRST_CUSTOM_ID, &names)
            .unwrap()
    );

    let mut writer = Writer::new();
    Slot::write(&Some(stack.clone()), &mut writer);
    let received = Slot::read(&mut Reader::new(&writer.data)).unwrap().unwrap();
    let client = item_sync::from_network(&received, &builtin, &dynamic);
    assert!(saved.same_item_same_components(&client));
    assert_eq!(hash, Hashed::create(&encode(&client), &names).unwrap());

    let mut writer = Writer::new();
    DelimitedSlot::write(&Some(encode(&client)), &mut writer);
    let returned = DelimitedSlot::read(&mut Reader::new(&writer.data))
        .unwrap()
        .unwrap();
    assert!(
        saved.same_item_same_components(&item_sync::from_network(&returned, &builtin, &dynamic))
    );

    let mut writer = Writer::new();
    Template::write(&stack, &mut writer);
    let template: Stack = Template::read(&mut Reader::new(&writer.data)).unwrap();
    assert!(
        saved.same_item_same_components(&item_sync::from_network(&template, &builtin, &dynamic))
    );

    let mut bundle = Stack::new(1, 1);
    bundle.added.push((
        sunrise_protocol::item::component::BUNDLE_CONTENTS,
        sunrise_protocol::item::Component::Stacks(vec![stack]),
    ));
    assert!(!custom::permitted(&bundle, false).unwrap());
    let mut writer = Writer::new();
    Slot::write(&Some(bundle), &mut writer);
    let received = Slot::read(&mut Reader::new(&writer.data)).unwrap().unwrap();
    let received = item_sync::from_network(&received, &builtin, &dynamic);
    assert!(saved.same_item_same_components(&received.data().unwrap().bundle_contents[0]));

    let mut container = ItemStack::new("minecraft:stone", 1);
    container.data_mut().container.push((2, saved.clone()));
    let mut writer = Writer::new();
    Slot::write(&Some(encode(&container)), &mut writer);
    let received = Slot::read(&mut Reader::new(&writer.data)).unwrap().unwrap();
    let (added, removed) =
        sunrise_protocol::item::WirePatch::decode(received.wire.0.as_ref().unwrap()).unwrap();
    let cached = Stack {
        added,
        removed,
        ..Stack::new(received.item, received.count)
    };
    assert!(!custom::permitted(&cached, false).unwrap());
    let received = item_sync::from_network(&received, &builtin, &dynamic);
    assert!(saved.same_item_same_components(&received.data().unwrap().container[0].1));

    let mut malformed = Writer::new();
    malformed.varint(1);
    malformed.varint(0);
    malformed.varint(1);
    malformed.varint(0);
    malformed.varint(FIRST_CUSTOM_ID);
    malformed.byte_array(&[0, 0]);
    assert!(DelimitedSlot::read(&mut Reader::new(&malformed.data)).is_err());
}

#[test]
fn registered_consume_effect_uses_its_synced_id_and_wire_codec() {
    use sunrise_client::{chunk::LevelShapes, lan::data::SyncData};
    use sunrise_protocol::item::{Component, consume_effects};
    use sunrise_protocol::packets::configuration::Clientbound;
    use sunrise_world::loot::PackOverlay;

    #[derive(mod_api::Wire)]
    struct Charge {
        points: u16,
    }

    struct Effects;
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Effects {
        fn register(
            &self,
            register: &mut dyn mod_api::Registrar,
            _: &mod_api::RegistrationContext,
        ) {
            register
                .register_consume_effect_type(
                    mod_api::Identifier::parse("effect_test:charge").unwrap(),
                    mod_api::ConsumeEffectType::new::<Charge>(
                        |fields| match fields.get(&"points".into()) {
                            Some(Tag::Int(points)) => u16::try_from(*points)
                                .map(|points| Charge { points })
                                .map_err(|error| error.to_string()),
                            _ => Err("expected points".into()),
                        },
                        |charge| Ok([("points".into(), Tag::Int(i32::from(charge.points)))].into()),
                    ),
                )
                .unwrap();
        }
    }

    let mods: mod_api::Mods = vec![mod_api::ModEntry::new("effect_test", Box::new(Effects))].into();
    mod_platform::testing::with_mods(mods.clone(), || {
        mod_platform::__private::part_tests::install_parts_fixture(&mods).unwrap();
        mod_api::__private::freeze_consume_effect_types().unwrap();

        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        std::fs::create_dir_all(root.join("data/minecraft")).unwrap();
        for (file, value) in [
            ("facts.json", r#"{"states":[]}"#),
            ("items.json", "{}"),
            (
                "registries.json",
                r#"{
                "minecraft:item":{"entries":{"minecraft:stone":{"protocol_id":1}}},
                "minecraft:sound_event":{"entries":{"minecraft:entity.generic.eat":{"protocol_id":1}}},
                "minecraft:data_component_type":{"entries":{"minecraft:consumable":{"protocol_id":24}}}
            }"#,
            ),
        ] {
            std::fs::write(root.join(file), value).unwrap();
        }
        let sync = SyncData::load(
            root,
            &PackOverlay::NONE,
            &sunrise_world::item_defaults::read(root).unwrap(),
            &Default::default(),
        )
        .unwrap();
        assert!(sync.content_identity.is_some());
        let config = sync
            .registry_packets(true, &LevelShapes::VANILLA)
            .into_iter()
            .find(|packet| packet.registry == consume_effects::REGISTRY)
            .expect("consume effect types sync during configuration");
        let encoded = Clientbound::RegistryData(config).encode();
        let mut packet = Reader::new(&encoded);
        let Clientbound::RegistryData(config) =
            Clientbound::decode(packet.varint().unwrap(), packet.rest()).unwrap()
        else {
            panic!("expected registry data");
        };
        let mut client_registries = Dynamic::default();
        client_registries.add(config);
        assert_eq!(
            client_registries.id(consume_effects::REGISTRY, "effect_test:charge"),
            Some(5)
        );

        let mut item = ItemStack::new("minecraft:stone", 1);
        item.data_mut().set_other(
            "minecraft:consumable",
            Some(Tag::compound([(
                "on_consume_effects",
                Tag::List(vec![Tag::compound([
                    ("type", Tag::String("effect_test:charge".into())),
                    ("points", Tag::Int(0x1234)),
                ])]),
            )])),
        );
        let net =
            stack_to_network(|_| 1, |registry, name| sync.id(registry, name), &Some(item)).unwrap();
        let Some(Component::Raw(raw)) = net.component(component::CONSUMABLE) else {
            panic!("expected consumable in the stack patch");
        };
        let mut bytes = Reader::new(raw);
        assert_eq!(bytes.f32().unwrap().to_bits(), 1.6_f32.to_bits());
        assert_eq!(bytes.varint().unwrap(), 1); // Eat
        assert_eq!(bytes.varint().unwrap(), 2); // Registered eat sound, holder ID + 1
        assert!(bytes.bool().unwrap());
        assert_eq!(bytes.varint().unwrap(), 1); // One consume effect
        assert_eq!(bytes.varint().unwrap(), 5); // First modded type after the five built-ins
        assert_eq!(bytes.rest(), [0x34, 0x12]); // Wire for u16, no extra envelope

        let mut packet = Writer::new();
        Slot::write(&Some(net.clone()), &mut packet);
        let received = Slot::read(&mut Reader::new(&packet.data)).unwrap().unwrap();
        let client = item_sync::from_network(&received, &sync.builtin, &client_registries);
        let effects = &client.consumable(None).unwrap().effects;
        let [sunrise_items::items::ConsumeEffect::Native { kind, data }] = effects.as_slice()
        else {
            panic!("expected custom effect in the received stack: {effects:?}");
        };
        assert_eq!(kind, "effect_test:charge");
        assert_eq!(data.get("points"), Some(&Tag::Int(0x1234)));
        let names = SyncedNames {
            builtin: &sync.builtin,
            dynamic: &client_registries,
        };
        let expected_hash = Hashed::create(&net, &names).unwrap();
        assert_eq!(Hashed::create(&received, &names).unwrap(), expected_hash);
    });
}
