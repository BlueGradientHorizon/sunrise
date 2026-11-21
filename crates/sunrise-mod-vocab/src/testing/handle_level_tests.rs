use crate::testing::FakeHost;

#[test]
fn fake_level_declines_to_lend_a_random_stream() {
    let mut host = FakeHost::new();
    let level = host.level.level();
    assert_eq!(level.with_random(&mut host, |_| true), None);
}

#[test]
fn player_level_and_dimension_reads() {
    use crate::host::entities::EntitiesHost;
    use crate::host::players::PlayersHost;
    let mut host = FakeHost::new();
    let overworld = host.level("minecraft:overworld").unwrap();
    let nether = crate::LevelRef::new(overworld.epoch(), crate::DimensionKey::NETHER);
    host.join(1, 10);
    host.join(2, 20).level = nether;
    let p1 = host.player(1).unwrap();
    let p2 = host.player(2).unwrap();
    let unknown = crate::__private::game::player_ref(p1.entity(), 99);
    let stale = crate::__private::game::player_ref(
        crate::EntityRef::new(
            crate::WorldEpoch::new(p1.entity().epoch().get() + 1),
            p1.entity().id(),
        ),
        p1.uuid(),
    );
    let replaced = crate::__private::game::player_ref(p2.entity(), p1.uuid());
    for invalid in [unknown, stale, replaced] {
        assert_eq!(invalid.level(&host), None);
        assert_eq!(invalid.dimension(&host), None);
    }
    assert_eq!(p1.level(&host), Some(overworld));
    assert_eq!(
        p1.dimension(&host),
        Some(crate::Identifier::parse("minecraft:overworld").unwrap())
    );
    assert_eq!(p2.level(&host), Some(nether));
    assert_eq!(
        p2.dimension(&host),
        Some(crate::Identifier::parse("minecraft:the_nether").unwrap())
    );
    host.players = Default::default();
    for disconnected in [p1, p2] {
        assert_eq!(disconnected.level(&host), None);
        assert_eq!(disconnected.dimension(&host), None);
    }
}

#[test]
fn fresh_fake_player_kind_has_player_id() {
    use crate::host::players::PlayersHost;
    let mut host = FakeHost::new();
    host.join(1, 10);
    host.resolve_entity_types().unwrap();
    host.level.entity(20, "minecraft:player");
    let player = host.player(1).unwrap();
    let kind = player
        .entity()
        .with_view(&host, |view| view.kind())
        .flatten()
        .unwrap();
    assert_eq!(kind.id(&host).as_deref(), Some("minecraft:player"));
}

#[test]
#[should_panic(
    expected = "fake entity kind test:late was not declared before entity-type resolution"
)]
fn an_undeclared_fake_entity_kind_after_resolution_is_a_setup_error() {
    crate::parts::install_input(crate::__private::parts_installation::InstalledInput::default());
    let mut host = FakeHost::new();
    host.level.entity(20, "test:declared");
    host.resolve_entity_types().unwrap();
    host.level.entity(21, "test:declared");
    // An existing id keeps its kind; the unused argument does not declare a new one.
    assert_eq!(host.level.entity(20, "test:unused").kind, "test:declared");
    host.level.entity(22, "test:late");
}

#[test]
fn fake_block_entity_generation_invalidates_replaced_handles() {
    use crate::host::block_entities::BlockEntitiesHost;
    let mut host = FakeHost::new();
    let level = host.level.level();
    let pos = [4, 64, 5];

    host.level.block_entity(pos);
    let first = host.block_entity(level, pos).unwrap();
    // Mutating the same standing placement keeps its generation and lends a view.
    host.level.block_entity(pos).with_items(1);
    assert_eq!(host.block_entity(level, pos), Some(first));
    assert_eq!(
        first.with_view(&host, |view| (view.level(), view.position(), view.handle())),
        Some((level, pos, first))
    );

    // Removing the block entity invalidates the first handle.
    assert!(host.level.remove_block_entity(pos).is_some());
    assert_eq!(host.block_entity(level, pos), None);
    assert_eq!(first.with_view(&host, |view| view.position()), None);

    // Replacing at the same position mints a new generation; the old handle stays stale
    // while the new handle resolves and lends a view.
    host.level.block_entity(pos);
    let second = host.block_entity(level, pos).unwrap();
    assert_ne!(first, second);
    assert_eq!(first.with_view(&host, |view| view.position()), None);
    assert_eq!(
        second.with_view(&host, |view| (view.level(), view.position(), view.handle())),
        Some((level, pos, second))
    );
}
