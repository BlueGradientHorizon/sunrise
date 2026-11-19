//! `living_breathe` gives listeners independent inputs but sequential answers.

#[test]
fn living_breathe_isolates_the_air_snapshot_but_carries_answers_forward() {
    struct First;
    #[mod_api::__subscriptions]
    impl Hooks for First {
        fn living_breathe(&self, _: &dyn LoopContext, event: &mut LivingBreatheEvent<'_>) {
            assert_eq!(event.air, [-19, 6000]);
            assert_eq!(
                (event.can_breathe, event.consume, event.refill),
                (false, 1, 0)
            );
            event.air = [100, 300];
            event.can_breathe = true;
            event.consume = 2;
            event.refill = 7;
        }
    }

    struct Second;
    #[mod_api::__subscriptions]
    impl Hooks for Second {
        fn living_breathe(&self, _: &dyn LoopContext, event: &mut LivingBreatheEvent<'_>) {
            assert_eq!(event.air, [-19, 6000]);
            assert_eq!(
                (event.can_breathe, event.consume, event.refill),
                (true, 2, 7)
            );
            event.refill = 9;
        }
    }

    install(
        vec![
            ModEntry::new("first", Box::new(First)),
            ModEntry::new("second", Box::new(Second)),
        ]
        .into(),
    );
    let (mut can_breathe, mut consume, mut refill) = (false, 1, 0);
    dispatch_living_breathe(
        &Loop("server"),
        42,
        "minecraft:axolotl",
        [-19, 6000],
        &mut can_breathe,
        &mut consume,
        &mut refill,
    );
    assert_eq!((can_breathe, consume, refill), (true, 2, 9));
}
