//! A hook's `Host` reads merged tags through the same wasm imports as a level view or loot context.
use super::*;
use mod_api::__private::{ModPhase, PhaseOwner};
use mod_api::prelude::TagsHost;
use mod_api::{BlockKey, Host, HostSide, Identifier, ItemKey, ServerHost, TagKey, Tags};
use std::cell::Cell;

struct TagTable;
impl Tags for TagTable {
    fn key(&self, id: &Identifier) -> Option<TagKey> {
        mod_api::__private::tag_key(id)
    }

    fn is_in_tag(&self, item: ItemKey, tag: TagKey) -> Option<bool> {
        mod_api::__private::with_tag_name(tag, |name| {
            Some(name? == "test:ingredients" && Some(item) == REGISTERED_ITEM.get())
        })
    }

    fn blocks(&self, tag: TagKey) -> Option<Vec<BlockKey>> {
        mod_api::__private::with_tag_name(tag, |name| {
            Some(if name? == "test:ingredients" {
                vec![REGISTERED_BLOCK.get().unwrap()]
            } else {
                Vec::new()
            })
        })
    }
}

#[derive(Default)]
struct World(Cell<usize>);
impl Host for World {}
mod_api::impl_host_areas!(World, except(tags));
impl ServerHost for World {}
mod_api::impl_server_host_areas!(World);
impl TagsHost for World {
    fn tags(&self) -> &dyn Tags {
        self.0.set(self.0.get() + 1);
        &TagTable
    }
}

fn check_tags(host: &dyn Host) {
    let tags = host.tags();
    let key = tags
        .key(&Identifier::parse("test:ingredients").unwrap())
        .unwrap();
    assert_eq!(
        tags.is_in_tag(REGISTERED_ITEM.get().unwrap(), key),
        Some(true)
    );
    assert_eq!(
        tags.blocks(key),
        Some(vec![REGISTERED_BLOCK.get().unwrap()])
    );
}

pub(super) struct HostTagsReadsHooks;
impl HostTagsReadsHooks {
    pub(super) fn tags_updated(&self, host: mod_api::HostSide, _: &mut mod_api::TagsUpdatedEvent) {
        let HostSide::Server(host) = host else {
            panic!("expected the server's Host");
        };
        check_tags(host);
    }
}

#[test]
fn a_host_answers_the_merged_tags_on_wasm_as_natively() {
    install_counter();
    let _phase = PhaseOwner::scope(ModPhase::Reloading);
    let mut world = World::default();
    check_tags(&world);
    let native_reads = world.0.get();
    WasmMod {
        group: 0,
        subscribed: &[Some(mod_api::HookId::TagsUpdated)],
        listeners: &[],
        corrupt: false,
    }
    .tags_updated(
        HostSide::Server(&mut world),
        &mut mod_api::TagsUpdatedEvent {
            cause: mod_api::TagsUpdateCause::ServerDataLoad { reload: true },
        },
    );
    assert!(
        world.0.get() > native_reads,
        "guest never borrowed the Host's tags"
    );
}

#[test]
fn a_config_host_refuses_tag_imports_without_calling_the_accessor() {
    mod_api::__private::initialize().unwrap();
    let _phase = PhaseOwner::scope(ModPhase::Config);
    let world = World::default();
    let mut frame = boundary::Frame::default();
    frame.set_host(&world);
    let mut cx = boundary::Cx::default();
    cx.frames.push(frame);
    let id = boundary::import_id("Tags", "key").unwrap();
    let mut input = Writer::new();
    input.put(&Identifier::parse("test:ingredients").unwrap());
    let mut out = Writer::new();
    // SAFETY: the Host lives until this synchronous call returns.
    assert!(matches!(
        unsafe { boundary::serve(id, &mut cx, input.bytes(), &mut out) },
        Err(boundary::ServeError::Phase {
            operation: "tags",
            phase: ModPhase::Config,
        })
    ));
    assert!(out.bytes().is_empty());
    assert_eq!(world.0.get(), 0);
}
