use super::*;
use crate::game::{
    native_entity_refs::tests::with_client_game,
    native_guest_turn_tests::with_guests,
    stations::{Kind, Station},
};
use sunrise_client::lan::{Out, PlayerSender};
use sunrise_mod_package::content::RECIPE_PRIORITIES_PATH;
use sunrise_protocol::{buf::Reader, packets::recipes::UpdateRecipes};

#[test]
#[ignore = "daily"]
fn guest_stonecutter_uses_packet_tags_components_and_authoritative_buttons() {
    let _publication = sunrise_client::datapack_net::isolate_server_publication();
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    let pack = root.join("datapacks/cuts");
    let write = |rel: &str, json: &str| {
        let path = pack.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, json).unwrap();
    };
    write(
        "pack.mcmeta",
        r#"{"pack":{"description":"fixture","min_format":94,"max_format":94}}"#,
    );
    std::fs::write(
        root.join("datapacks.json"),
        r#"{"enabled":["vanilla","file/cuts"],"disabled":[]}"#,
    )
    .unwrap();
    write(
        RECIPE_PRIORITIES_PATH,
        r#"{"entries":{"fixture:z_tag":10}}"#,
    );
    write(
        "data/fixture/tags/item/cuttable.json",
        r#"{"values":["minecraft:stick"]}"#,
    );
    write(
        "data/fixture/recipe/z_tag.json",
        r##"{
        "type":"minecraft:stonecutting","ingredient":"#fixture:cuttable",
        "result":{"id":"minecraft:leather_helmet","components":{"minecraft:dyed_color":1122867}}
    }"##,
    );
    write(
        "data/fixture/recipe/a_named.json",
        r#"{
        "type":"minecraft:stonecutting",
        "ingredient":{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:custom_name":"special"}},
        "result":{"id":"minecraft:diamond"}
    }"#,
    );
    write(
        "data/fixture/recipe/b_plain.json",
        r#"{
        "type":"minecraft:stonecutting","ingredient":"minecraft:stick",
        "result":{"id":"minecraft:emerald","count":2}
    }"#,
    );
    with_guests(|server| {
        let _serving = crate::game::integrated::tests::install_serving(server, false);
        server.world_root = root.to_owned();
        let (tx, rx) = std::sync::mpsc::channel();
        let host = server.lan.as_mut().unwrap();
        host.play.entry(12).or_default();
        host.hub.update_guest(12, |guest| {
            guest.sender = PlayerSender::new(tx, Arc::default());
        });
        server.reload_datapacks(true, &[]).unwrap();
        server
            .world
            .chunks
            .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
        let state = server
            .pack
            .assets
            .registry
            .default_state("minecraft:stonecutter")
            .unwrap();
        server.world.set(3, 64, 4, state).unwrap();
        assert!(server.lan_open_station(12, &Station::new(Kind::Stonecutter, [3, 64, 4])));
        let window = server.lan.as_ref().unwrap().menus[&12]
            .station
            .as_ref()
            .unwrap()
            .window;
        let click = |slot, input| {
            S::ContainerClick(play::ContainerClick {
                state: 0,
                changed: Vec::new(),
                carried: None,
                container: window,
                slot,
                button: 0,
                input,
            })
        };
        // Opening the station returns anything already on the cursor to inventory.
        server.lan.as_mut().unwrap().hub.update_guest(12, |guest| {
            guest.carried = Some(ItemStack::new("minecraft:stick", 3));
        });
        server.lan_guest_packet(12, click(0, 0));
        with_client_game(|client| {
            let (mut peer, _writer) = super::super::tests::connect_menu_test(client);
            let deliver = |client: &mut Game<'_>, expect_closed: bool| {
                let mut recipes = None;
                for out in rx.try_iter() {
                    let bodies = match out {
                        Out::Packet(body) => vec![body],
                        Out::Packets(bodies) => bodies,
                        Out::Disconnect(_) => panic!("guest disconnected"),
                    };
                    for body in bodies {
                        let mut reader = Reader::new(&body);
                        let id = reader.varint().unwrap();
                        match C::decode(id, reader.rest()).unwrap() {
                            packet @ (C::UpdateTags(_)
                            | C::OpenScreen(_)
                            | C::ContainerClose(_)
                            | C::ContainerSetContent(_)
                            | C::ContainerSetData(_)) => client.handle_packet(packet).unwrap(),
                            C::UpdateRecipes(packet) => {
                                if expect_closed {
                                    assert!(client.station.is_none(), "close before new recipes");
                                }
                                recipes = Some(packet.clone());
                                client.handle_packet(C::UpdateRecipes(packet)).unwrap();
                            }
                            _ => {}
                        }
                    }
                }
                recipes
            };
            let packet = deliver(client, false).expect("server recipe packet");
            let input = client.station.as_ref().unwrap().inputs[0]
                .as_ref()
                .expect("the guest placed its stack");
            assert_eq!((input.item.as_ref(), input.count), ("minecraft:stick", 3));
            assert!(
                client.remote.as_ref().unwrap().tags.is_none(),
                "no native tag view"
            );
            let shown = |client: &Game<'_>| client.station_info()["recipe_results"].clone();
            assert_eq!(
                shown(client),
                serde_json::json!([
                    "minecraft:leather_helmet",
                    "minecraft:diamond",
                    "minecraft:emerald"
                ])
            );
            let first = client.station.as_ref().unwrap().stone.recipes[0];
            let preview = &client.station_recipes().stonecutting[first].result;
            assert_eq!(
                preview.data().and_then(|data| data.dyed_color),
                Some(0x112233)
            );

            let mut press = |client: &mut Game<'_>, server: &mut Game<'_>, button| {
                assert!(client.menu_session(|menu, env, _| Some(menu.button(env, button))));
                assert!(client.remote_button_click(button));
                let (id, payload) = peer.recv().unwrap();
                let packet = S::decode(id, &payload).unwrap();
                assert!(
                    matches!(&packet, S::ContainerButtonClick(p) if p.container == window && p.button == button)
                );
                server.lan_guest_packet(12, packet);
                deliver(client, false);
            };
            press(client, server, 1); // The packet's item matches, but the actual component does not.
            assert!(client.station.as_ref().unwrap().result.is_none());
            assert_eq!(client.station.as_ref().unwrap().selected, None);
            press(client, server, 2); // The ineligible row must not shift this one's number.
            assert_eq!(client.station.as_ref().unwrap().selected, Some(2));
            assert_eq!(
                client
                    .station
                    .as_ref()
                    .unwrap()
                    .result
                    .as_ref()
                    .unwrap()
                    .item
                    .as_ref(),
                "minecraft:emerald"
            );
            server.lan_guest_packet(12, click(1, 1)); // Quick-move all three cuts into inventory.
            deliver(client, false);
            assert_eq!(
                server.lan.as_ref().unwrap().hub.guests[0]
                    .inventory
                    .iter()
                    .flatten()
                    .filter(|s| s.item.as_ref() == "minecraft:emerald")
                    .map(|s| s.count)
                    .sum::<u32>(),
                6
            );

            server.lan.as_mut().unwrap().hub.update_guest(12, |guest| {
                guest.carried = Some(ItemStack::new("minecraft:stick", 1))
            });
            server.lan_guest_packet(12, click(0, 0));
            deliver(client, false);
            press(client, server, 2);
            // Other servers can replace UpdateRecipes before the screen closes. Supply a single
            // different row without changing the input; the cached old indices cannot survive.
            let replaced = UpdateRecipes {
                stonecutter: vec![packet.stonecutter[first].clone()],
                ..packet
            };
            client.handle_packet(C::UpdateRecipes(replaced)).unwrap();
            assert_eq!(
                shown(client),
                serde_json::json!(["minecraft:leather_helmet"])
            );
            assert_eq!(
                client.station.as_ref().unwrap().inputs[0]
                    .as_ref()
                    .unwrap()
                    .item
                    .as_ref(),
                "minecraft:stick"
            );
            assert!(client.station_info()["selected"].is_null());
            assert!(client.station.as_ref().unwrap().result.is_none());

            // A Sunrise reload uses the vanilla close packet before the new recipe generation.
            write(
                RECIPE_PRIORITIES_PATH,
                r#"{"entries":{"fixture:b_plain":10}}"#,
            );
            server.reload_datapacks(true, &[]).unwrap();
            deliver(client, true).expect("new server recipe generation");
            assert!(client.station.is_none());
            server.lan_guest_packet(
                12,
                S::ContainerButtonClick(play::ContainerButtonClick {
                    container: window,
                    button: 0,
                }),
            );
            assert!(server.lan.as_ref().unwrap().menus[&12].station.is_none());
        });
    });
}
