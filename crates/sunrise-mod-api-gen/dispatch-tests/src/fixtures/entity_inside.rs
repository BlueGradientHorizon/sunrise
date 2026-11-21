mod stubs {
    pub(super) struct LandingWrite;
    #[derive(Default)]
    pub(super) struct InsideEffects;
    impl InsideEffects {
        pub(crate) fn staged_for(_: &'static str) -> Self {
            Self
        }

        pub(crate) fn absorb_step(&mut self, _: Self) {}
    }
}
