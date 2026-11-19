use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

const TEXTURE: &str = "assets/demo/textures/a.png";

/// The game's side of a sprite source's resources: one resource with two layers, answering
/// nothing once `closed` is set, as the reader a source was handed does when its call ends.
struct Resources {
    closed: Arc<AtomicBool>,
}
impl mod_api::ResourceReader for Resources {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        (!self.closed.load(Ordering::SeqCst) && path == TEXTURE).then(|| b"winning".to_vec())
    }
    fn read_stack(&self, path: &str) -> Vec<Vec<u8>> {
        if !self.closed.load(Ordering::SeqCst) && path == TEXTURE {
            vec![b"lower".to_vec(), b"winning".to_vec()]
        } else {
            Vec::new()
        }
    }
    fn list(&self, prefix: &str) -> Vec<String> {
        if self.closed.load(Ordering::SeqCst) {
            Vec::new()
        } else {
            vec![format!("{prefix}a.png")]
        }
    }
}

/// What a reader answered to one round of reads.
#[derive(Debug, PartialEq)]
struct Answers {
    winning: Option<Vec<u8>>,
    missing: Option<Vec<u8>>,
    stack: Vec<Vec<u8>>,
    listed: Vec<String>,
}

/// The reads the guest makes, once while the game's reader answers and once after the game
/// stopped answering (the test closes the reader inside the guest call, standing in for the end
/// of the callback that was handed it).
fn read(reader: &Arc<dyn mod_api::ResourceReader>, closed: &AtomicBool) -> [Answers; 2] {
    let look = || Answers {
        winning: reader.read(TEXTURE),
        missing: reader.read("assets/demo/textures/b.png"),
        stack: reader.read_stack(TEXTURE),
        listed: reader.list("assets/demo/textures/"),
    };
    let open = look();
    closed.store(true, Ordering::SeqCst);
    [open, look()]
}

/// A reader of the load's resources crosses to a guest as a held resource: each call reaches the
/// game's reader, and what the game no longer answers is an empty answer, never a panic.
#[test]
fn wire_loopback_a_guest_reads_a_sprite_sources_resources_through_the_held_reader() {
    mod_api::__private::initialize().unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let closed = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    struct Fixture {
        closed: Arc<AtomicBool>,
        seen: Arc<std::sync::Mutex<Vec<[Answers; 2]>>>,
    }
    impl Hooks for Fixture {
        fn subscribed(&self) -> &'static [Option<mod_api::HookId>] {
            &[]
        }
        fn register(&self, _: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            let module = WasmMod {
                group: 0,
                subscribed: &[],
                listeners: &[],
                corrupt: false,
            };
            let mut cx = Box::new(boundary::Cx::default());
            cx.module = &module;
            cx.frames.push(boundary::Frame::default());
            let pointer = &mut *cx as *mut boundary::Cx;
            // SAFETY: the module and context outlive the synchronous calls and scopes.
            let _resources = unsafe { boundary::resource_scope(pointer, true) };
            let reader: Arc<dyn mod_api::ResourceReader> = Arc::new(Resources {
                closed: self.closed.clone(),
            });
            let mut out = Writer::new();
            out.put(&reader);
            // SAFETY: the fixture owns cx until the transport scope drops.
            let transport = unsafe { host_dispatch::transport(pointer) };
            let _transport = TransportScope(None);
            guest::set_transport(Some(transport));
            let _guest = guest::resource_scope();
            let reader: Arc<dyn mod_api::ResourceReader> = Reader::new(out.bytes()).take().unwrap();
            self.seen.lock().unwrap().push(read(&reader, &self.closed));
        }
    }
    mod_api::__private::install_parts(
        &[mod_api::ModEntry::new(
            "sprite_reader_fixture",
            Box::new(Fixture {
                closed: closed.clone(),
                seen: seen.clone(),
            }),
        )],
        &mod_api::__private::part_tests::rule_schema(),
        |_| false,
    )
    .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "the guest registered once");
    assert_eq!(
        seen[0],
        [
            Answers {
                winning: Some(b"winning".to_vec()),
                missing: None,
                stack: vec![b"lower".to_vec(), b"winning".to_vec()],
                listed: vec!["assets/demo/textures/a.png".to_owned()],
            },
            Answers {
                winning: None,
                missing: None,
                stack: Vec::new(),
                listed: Vec::new(),
            }
        ]
    );
}
