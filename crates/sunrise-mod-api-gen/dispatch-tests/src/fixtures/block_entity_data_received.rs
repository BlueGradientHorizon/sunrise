// no wire: dispatch fixture for a read-only view.
struct BlockEntityView;

impl BlockEntityView {
    fn kind(&self) -> Option<BlockEntityTypeKey> {
        Some(BlockEntityTypeKey(0))
    }
}

impl BlockEntityRef {
    fn with_view<H: ?Sized, R>(self, _: &H, read: impl FnOnce(&BlockEntityView) -> R) -> Option<R> {
        Some(read(&BlockEntityView))
    }
}

mod stubs {
    #[derive(Clone, Copy)]
    pub(super) enum BlockEntityDataSource {
        Chunk,
        UpdatePacket,
    }
}
