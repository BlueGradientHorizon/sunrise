use super::*;
/// A part that lays a screen out.
struct Layout;
#[mod_api::sunrise_part]
impl MenuPart for Layout {
    fn screen_draw(&self, _: &dyn ClientHost, _: &mut ScreenDrawEvent<'_>) {}
}
/// Another, of another kind.
struct Overlay;
#[mod_api::sunrise_part]
impl MenuPart for Overlay {
    fn screen_draw(&self, _: &dyn ClientHost, _: &mut ScreenDrawEvent<'_>) {}
}
/// A part on a menu type that lays out no screen.
struct Rules;
#[mod_api::sunrise_part]
impl MenuPart for Rules {
    fn menu_still_valid(&self, _: &dyn LoopContext, _: &mut MenuStillValidEvent<'_>) {}
}
#[test]
fn a_second_screen_part_for_the_same_screens_is_refused() {
    let mut registrar = PartRegistrar::new("alpha");
    let r: &mut dyn Registrar = &mut registrar;
    r.menu_part("alpha:pack", Layout);
    r.menu_part("minecraft:hopper", Layout);
    // One screen part on a mod's menu type, whoever attaches the second.
    r.menu_part("alpha:pack", Overlay);
    // A part that lays out no screen sits beside it.
    r.menu_part("alpha:pack", Rules);
    registrar.mod_id = "beta";
    let r: &mut dyn Registrar = &mut registrar;
    r.menu_part("alpha:pack", Overlay);
    // A vanilla type takes one per mod: beta's blocks get beta's.
    r.menu_part("minecraft:hopper", Overlay);
    registrar.mod_id = "alpha";
    let r: &mut dyn Registrar = &mut registrar;
    r.menu_part("minecraft:hopper", Overlay);
    // Kinds are named by their full type names.
    let here = concat!(module_path!(), "::");
    let conflicts = [
        ("alpha:pack", "alpha", "alpha"),
        ("alpha:pack", "beta", "alpha"),
        ("minecraft:hopper", "alpha", "alpha"),
    ];
    assert_eq!(registrar.refused.len(), conflicts.len());
    for (refusal, (menu, rival, owner)) in registrar.refused.iter().zip(conflicts) {
        for name in [
            menu,
            &format!("Overlay of {rival}"),
            &format!("Layout of {owner}"),
        ] {
            assert!(refusal.contains(name), "{refusal} does not name {name}");
        }
    }
    let owners = |menu: &str| -> Vec<_> {
        registrar.parts.menus[menu]
            .iter()
            .map(|p| (p.mod_id, p.type_name.trim_start_matches(here)))
            .collect()
    };
    assert_eq!(
        owners("alpha:pack"),
        [("alpha", "Layout"), ("alpha", "Rules")]
    );
    assert_eq!(
        owners("minecraft:hopper"),
        [("alpha", "Layout"), ("beta", "Overlay")]
    );
}

#[test]
fn refused_slot_exclusions_cannot_replace_a_scoped_mask() {
    const MENU: &str = "alpha:pack";
    const STOCK: &str = "minecraft:hopper";
    const BLOCK: &str = "alpha:machine";
    let declare = || {
        let mut registrar = PartRegistrar::new("alpha");
        registrar
            .menu_slot_exclusions(MENU, None, &[3, 1, 3])
            .unwrap();
        registrar
            .menu_slot_exclusions(MENU, Some(BLOCK), &[])
            .unwrap();
        registrar
            .menu_slot_exclusions(STOCK, Some(BLOCK), &[4])
            .unwrap();
        registrar
    };
    let mut registrar = declare();
    let good = registrar.menu_slot_exclusions.clone();
    assert!(registrar.menu_slot_exclusions(MENU, None, &[2]).is_err());
    assert!(registrar.menu_slot_exclusions(STOCK, None, &[2]).is_err());
    assert!(
        registrar
            .menu_slot_exclusions(STOCK, Some("beta:machine"), &[2])
            .is_err()
    );
    assert!(
        registrar
            .menu_slot_exclusions(MENU, Some("alpha:"), &[2])
            .is_err()
    );
    registrar.mod_id = "beta";
    assert!(
        registrar
            .menu_slot_exclusions(MENU, Some("beta:machine"), &[2])
            .is_err()
    );
    assert_eq!(registrar.menu_slot_exclusions, good);

    let schema = super::super::tests::rule_schema();
    let errors = install_registrar(registrar, &schema).unwrap_err();
    assert_eq!(errors.len(), 5);
    assert!(menu_slot_exclusions(MENU, None).is_empty());

    let mut registrar = declare();
    let rollback = registrar.registration_checkpoint();
    registrar
        .menu_slot_exclusions(MENU, Some("beta:machine"), &[2])
        .unwrap();
    rollback(&mut registrar);
    install_registrar(registrar, &schema).unwrap();
    assert_eq!(menu_slot_exclusions(MENU, None), &[1, 3]);
    assert!(menu_slot_exclusions(MENU, Some(BLOCK)).is_empty());
    assert_eq!(menu_slot_exclusions(MENU, Some("beta:machine")), &[1, 3]);
    assert_eq!(menu_slot_exclusions(STOCK, Some(BLOCK)), &[4]);
    assert!(menu_slot_exclusions(STOCK, Some("alpha:other")).is_empty());
    assert!(menu_slot_exclusions(STOCK, None).is_empty());
}
