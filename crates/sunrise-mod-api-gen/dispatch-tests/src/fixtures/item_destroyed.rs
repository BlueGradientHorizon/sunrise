#[test]
fn item_destroyed_collects_and_validates_drops_per_part() {
    struct SpillPart {
        add_item: &'static str,
        count: u32,
        seen_damage: Arc<Mutex<Vec<String>>>,
    }
    impl ItemPart for SpillPart {
        fn item_subscribed(&self) -> &'static [Option<HookId>] {
            &[hook_ids::item_destroyed]
        }
        fn item_destroyed(&self, _: &mut dyn ServerHost, event: &mut ItemDestroyedEvent<'_>) {
            self.seen_damage
                .lock()
                .unwrap()
                .push(event.damage_type.to_owned());
            event.drops.push(ItemStack {
                item: Arc::from(self.add_item),
                count: self.count,
            });
        }
    }

    let seen = Arc::new(Mutex::new(Vec::new()));
    let valid_part: &dyn ItemPart = leak(SpillPart {
        add_item: "minecraft:diamond",
        count: 2,
        seen_damage: seen.clone(),
    });
    let invalid_part: &dyn ItemPart = leak(SpillPart {
        add_item: "invalid:overstack",
        count: 999,
        seen_damage: seen.clone(),
    });
    install_parts(Parts {
        items: vec![valid_part, invalid_part],
        ..Parts::default()
    });

    let mut host = TestHost;
    let stack = ItemStack {
        item: Arc::from("test:pouch"),
        count: 1,
    };
    let mut drops = Vec::new();
    let mut validate = |_: &Vec<ItemStack>| Ok(());
    dispatch_item_destroyed(
        &mut host,
        &stack,
        ItemEntityRef,
        "minecraft:lava",
        &mut drops,
        &mut validate,
    );
    assert_eq!(
        *seen.lock().unwrap(),
        vec!["minecraft:lava".to_owned(), "minecraft:lava".to_owned()]
    );
    assert_eq!(drops.len(), 2);
    assert_eq!(&*drops[0].item, "minecraft:diamond");
    assert_eq!(drops[0].count, 2);
    assert_eq!(&*drops[1].item, "invalid:overstack");
}
