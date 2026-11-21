//! Server-side `menu_slot_changed` and `menu_slot_take` v3 dispatch on `MenuPart`.
struct ProcessingMenuPart;
impl MenuPart for ProcessingMenuPart {
    fn menu_subscribed(&self) -> &'static [Option<HookId>] {
        &[]
    }

    fn menu_slot_changed(&self, _: &mut dyn ServerHost, event: &mut MenuSlotChangedEvent<'_>) {
        assert_eq!(event.menu, "test:processor");
        assert_eq!(event.changed.len(), 1);
        assert_eq!(event.changed[0].container, MenuContainer::Main);
        assert_eq!(event.changed[0].slot, 0);
        assert_eq!(event.context.player, Some(7));
        event.outputs.push(MenuSlotWrite {
            container: MenuContainer::Main,
            slot: 2,
            stack: Some(ItemStack {
                item: "result".into(),
                count: 1,
            }),
        });
    }

    fn menu_slot_take(&self, event: &mut MenuSlotTakeEvent<'_>) {
        assert_eq!(event.menu, "test:processor");
        assert_eq!(event.player, 7);
        assert_eq!(event.slot, 2);
        assert_eq!(event.taken.item.as_ref(), "result");
        // A blockless menu lends no view; a block-backed one lends its block entity.
        let _ = event.subject.with_view(|entity| entity.data.push(42));
    }
}

#[test]
fn menu_slot_changed_writes_outputs_and_slot_take_handles_block_and_blockless() {
    install(Vec::<ModEntry>::new().into());
    install_parts(Parts {
        menus: vec![Box::leak(Box::new(ProcessingMenuPart))],
        ..Parts::default()
    });
    let changed = [MenuSlotChange {
        container: MenuContainer::Main,
        slot: 0,
        before: None,
        after: Some(ItemStack {
            item: "input".into(),
            count: 2,
        }),
    }];
    let main_slots = [
        Some(ItemStack {
            item: "input".into(),
            count: 2,
        }),
        None,
        None,
    ];
    let read = |container, index| match container {
        MenuContainer::Main => main_slots.get(index).cloned().flatten(),
        _ => None,
    };
    let context = MenuSlotContext::new(Some(7), None, &read);
    let mut outputs = Vec::new();
    dispatch_menu_slot_changed(
        &mut TestHost,
        calls::MenuSlotChangedCall {
            menu: "test:processor",
            changed: &changed,
            context: &context,
            outputs: &mut outputs,
        },
    );
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].slot, 2);
    assert_eq!(outputs[0].stack.as_ref().unwrap().item.as_ref(), "result");

    let taken = ItemStack {
        item: "result".into(),
        count: 1,
    };
    let mut entity = ModBlockEntity::new("machine");
    dispatch_menu_slot_take(
        "test:processor",
        generated::mod_api::BlockEntityScope(Some(&mut entity)),
        7,
        2,
        &taken,
        None,
    );
    assert_eq!(entity.data, [42]);

    dispatch_menu_slot_take(
        "test:processor",
        generated::mod_api::BlockEntityScope(None),
        7,
        2,
        &taken,
        None,
    );
}
