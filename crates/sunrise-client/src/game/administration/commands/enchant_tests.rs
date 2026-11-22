use super::{Game, Text};
use crate::game::{
    lan::LanHost,
    lan_particles::tests::test_hub,
    native_entity_refs::tests::{fixture_root, run_fresh_cases, with_client_game, with_game},
};
use std::sync::Arc;
use sunrise_client::{
    items::ItemStack,
    lan::{Out, data::SyncData},
    scoreboard::{comp_text, tag_to_json},
};
use sunrise_protocol::{
    buf::Reader,
    connection::Pipe,
    packets::play::{ChatCommand, Clientbound as C, Serverbound as S},
    registry::Builtin,
};

const TRIDENT: &str = "minecraft:trident";
const RIPTIDE: &str = "minecraft:riptide";

#[test]
#[ignore = "daily"]
fn integrated_enchant_updates_the_clients_selected_slot() {
    const CASE: &str = "SUNRISE_TEST_INTEGRATED_ENCHANT_TARGET";
    if run_fresh_cases(
        "game::commands::enchant_tests::integrated_enchant_updates_the_clients_selected_slot",
        CASE,
        &["NearPlayer", "@s", "@a[name=NearPlayer]", "console"],
    ) {
        return;
    }
    let target = std::env::var(CASE).unwrap();
    let console = target == "console";
    let target = if console { "NearPlayer" } else { &target };
    let mut sync = SyncData::empty();
    sync.builtin = Builtin::load(&Builtin::find(&fixture_root()).unwrap()).unwrap();
    sync.dynamic
        .registries
        .insert("minecraft:enchantment".into(), vec![(RIPTIDE.into(), None)]);
    let sync = Arc::new(sync);
    let (mut hub, rx, _other) = test_hub("Enchant", Vec::new(), Some(sync.clone()));
    hub.config.integrated = true;
    let index = hub.guests.connection_index(1).unwrap();
    let selected = 3;
    hub.guests.update(index, |g| {
        g.permission = 4;
        g.selected = selected;
        g.inventory[selected] = Some(ItemStack::new(TRIDENT, 1));
    });
    let mut slot = None;
    with_game(|game| {
        game.player = None;
        game.build_registries();
        game.lan = Some(LanHost::for_test(hub));
        let command = format!("enchant {target} {RIPTIDE} 3");
        if console {
            let (ok, result) = game.recording_command_outcome(|game| {
                game.as_console(|game| game.run_command(&command))
            });
            assert!(ok, "{command}: server source");
            assert_eq!(result, Some((true, 1)));
        } else {
            game.lan_guest_packet(1, S::ChatCommand(ChatCommand { command }));
            game.run_guest_commands();
        }
        let hub = &mut game.lan.as_mut().unwrap().hub;
        hub.sync_guest_after_tick(1);
        let mut feedback = Vec::new();
        for out in rx.try_iter() {
            let bodies = match out {
                Out::Packet(body) => vec![body],
                Out::Packets(bodies) => bodies,
                Out::Disconnect(reason) => panic!("player disconnected: {reason:?}"),
            };
            for body in bodies {
                let mut reader = Reader::new(&body);
                let id = reader.varint().unwrap();
                match C::decode(id, reader.rest()).unwrap() {
                    C::SystemChat(chat) => feedback.push(tag_to_json(&chat.content)),
                    C::ContainerSetSlot(packet)
                        if packet.container == 0 && packet.slot == 36 + selected as i16 =>
                    {
                        slot = Some(packet);
                    }
                    _ => {}
                }
            }
        }
        if !console {
            assert!(
                feedback
                    .iter()
                    .any(|line| line["translate"] == "commands.enchant.success.single"),
                "{target}: {feedback:?}"
            );
        }
        assert_eq!(riptide(hub.guests[index].inventory[selected].as_ref()), 3);
    });
    with_client_game(|client| {
        let (wire, _server) = Pipe::pair();
        client.connect_local(wire, "NearPlayer").unwrap();
        client.set_registries(Arc::new(sync.dynamic.clone()));
        client.selected = selected;
        client.inventory[selected] = Some(ItemStack::new(TRIDENT, 1));
        client
            .handle_packet(C::ContainerSetSlot(
                slot.expect("the client must get its hotbar update"),
            ))
            .unwrap();
        assert_eq!(riptide(client.inventory[selected].as_ref()), 3);
    });
}

fn riptide(stack: Option<&ItemStack>) -> u32 {
    let stack = stack.expect("held stack");
    assert_eq!(&*stack.item, TRIDENT);
    stack.enchantment_level(RIPTIDE)
}

#[test]
#[ignore = "daily"]
fn enchant_reports_single_target_errors_and_counts_only_compatible_living_targets() {
    if run_fresh_cases(
        "game::commands::enchant_tests::enchant_reports_single_target_errors_and_counts_only_compatible_living_targets",
        "SUNRISE_TEST_ENCHANT_LIVING_TARGETS",
        &["living"],
    ) {
        return;
    }
    with_game(|game| {
        game.player = None;
        game.build_registries();
        game.set_allow_commands(true);
        for command in [
            "summon zombie 4 64 4 {NoAI:1b,equipment:{mainhand:{id:\"minecraft:trident\",count:1}}}",
            "summon armor_stand 5 64 4 {ShowArms:1b,equipment:{mainhand:{id:\"minecraft:trident\",count:1}}}",
            "summon marker 6 64 4",
            "summon armor_stand 7 64 4",
            "summon zombie 8 64 4 {NoAI:1b,equipment:{mainhand:{id:\"minecraft:stick\",count:1,components:{\"minecraft:custom_name\":\"Not a trident\"}}}}",
        ] {
            assert!(game.as_console(|g| g.run_command(command)), "{command}");
        }
        for (target, level, key, arg) in [
            ("@n[type=marker]", 4, "level", Some("4")),
            ("@n[type=marker]", 3, "entity", None),
            ("@n[type=armor_stand,x=7,y=64,z=4]", 3, "itemless", None),
            (
                "@n[type=zombie,x=8,y=64,z=4]",
                3,
                "incompatible",
                Some("Not a trident"),
            ),
        ] {
            enchant_outcome(game, target, level, false, 0, &format!("failed.{key}"), arg);
        }
        enchant_outcome(game, "@e", 3, true, 2, "success.multiple", None);
        assert_eq!(riptide(game.mobs[0].equipment_stacks()[0].as_ref()), 3);
        assert_eq!(riptide(game.armor_stands[0].equipment.get(0)), 3);
        enchant_outcome(game, "@e", 3, false, 0, "failed", None);
    });
}

fn enchant_outcome(
    game: &mut Game<'_>,
    target: &str,
    level: u32,
    success: bool,
    count: i32,
    key: &str,
    first_arg: Option<&str>,
) {
    let command = format!("enchant {target} {RIPTIDE} {level}");
    game.chat_capture = Some(Vec::new());
    let (ok, outcome) =
        game.recording_command_outcome(|g| g.as_console(|g| g.run_command(&command)));
    let feedback = game.chat_capture.take().unwrap();
    assert_eq!(
        (ok, outcome),
        (success, Some((success, count))),
        "{command}: {feedback:?}"
    );
    let key = format!("commands.enchant.{key}");
    let component = feedback
        .iter()
        .find_map(|line: &Text| {
            let component = line.component();
            if component["translate"] == key {
                return Some(component);
            }
            component["extra"]
                .as_array()?
                .iter()
                .find(|part| part["translate"] == key)
                .cloned()
        })
        .unwrap_or_else(|| panic!("{command}: expected {key}, got {feedback:?}"));
    if let Some(first_arg) = first_arg {
        assert_eq!(comp_text(&component["with"][0]).plain(), first_arg);
    }
}
