//! The same typed attachment job compiled natively and as a real wasm guest.
use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, ChunkDiskData, ChunkDiskHost, Compound, Tag,
};

pub const VALUE: AttachmentType<i32> = AttachmentType {
    namespace: "wasm_fixture",
    name: "disk_value",
    default: || 0,
    codec: Some(AttachmentCodec {
        encode: |value| Ok(Compound::from_iter([("counter".into(), Tag::Int(*value))])),
        decode: |tag| match tag.get(&"counter".into()) {
            Some(Tag::Int(value)) => Ok(*value),
            _ => Err(AttachmentError::Decode),
        },
    }),
    copy_on_death: false,
    sync: false,
};

/// A game that loads this fixture saves and loads every chunk, nearly all holding the attachment's
/// default: only a chunk the chunk-disk test filled with a value is this fixture's to check, and a trap
/// on another would stop the whole group.
pub fn save(host: &dyn ChunkDiskHost, data: &mut ChunkDiskData) {
    let value = data.get(&VALUE).unwrap();
    if value == 0 {
        return;
    }
    assert_eq!(value, 7);
    data.set(&VALUE, 8).unwrap();
    if host.job().pos == [99, 99] {
        panic!("guest trap after tentative write");
    }
}

pub fn load(_: &dyn ChunkDiskHost, data: &mut ChunkDiskData) {
    let value = data.get(&VALUE).unwrap();
    if value != 0 {
        data.set(&VALUE, value + 1).unwrap();
    }
}
