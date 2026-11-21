mod stubs {
    pub(super) struct PendingChildrenRef;
    pub(super) trait PendingChildrenEdits {}
    pub(super) struct BabyEntitySpawnChild<'a>(pub std::marker::PhantomData<&'a mut ()>);
}
