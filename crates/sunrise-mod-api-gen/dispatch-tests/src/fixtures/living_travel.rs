//! The `living_travel` fixture: the isolated dispatch's per-listener output check
//! (`output_validator`) — a refused pair is dropped alone, the preceding accepted pair
//! stands for the next listener, and later listeners still run.

/// What a fixture listener heard of one travel: the input and velocity it was handed.
type TravelHeard = Vec<([f64; 3], [f64; 3])>;

/// A listener that records what it hears, then writes `input` and `velocity`.
struct Setter {
    input: [f64; 3],
    velocity: [f64; 3],
    heard: Arc<Mutex<TravelHeard>>,
}

#[mod_api::__subscriptions]
impl Hooks for Setter {
    fn living_travel(&self, _: &dyn LoopContext, event: &mut LivingTravelEvent<'_>) -> Flow {
        self.heard
            .lock()
            .unwrap()
            .push((*event.input, *event.velocity));
        *event.input = self.input;
        *event.velocity = self.velocity;
        Flow::Continue
    }
}

fn setter(
    id: &'static str,
    input: [f64; 3],
    velocity: [f64; 3],
) -> (Arc<Mutex<TravelHeard>>, ModEntry) {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let entry = ModEntry::new(
        id,
        Box::new(Setter {
            input,
            velocity,
            heard: Arc::clone(&heard),
        }),
    );
    (heard, entry)
}

/// A refused pair is dropped whole; the earlier accepted pair stands, and the listener after
/// it hears that accepted pair, never the refused proposal.
#[test]
fn a_refused_pair_is_dropped_alone_and_later_listeners_hear_the_accepted_one() {
    let (_, accepted) = setter("accepted", [0.5, 0.0, 0.0], [3.0, 0.0, 0.0]);
    let (_, refused) = setter("refused", [2.0, 0.0, 0.0], [7.0, 0.0, 0.0]);
    let (later_heard, later) = setter("later", [0.5, 0.0, 0.0], [3.0, 0.0, 0.0]);
    install(vec![accepted, refused, later].into());
    let mut input = [0.1, 0.0, 0.0];
    let mut velocity = [1.0, 0.0, 0.0];
    let mut canceled = false;
    mod_api::dispatch_living_travel(
        &Loop("server"),
        7,
        "minecraft:zombie",
        TravelMode::Air,
        &mut input,
        &mut velocity,
        &mut canceled,
    );
    assert_eq!(
        (input, velocity),
        ([0.5, 0.0, 0.0], [3.0, 0.0, 0.0]),
        "the accepted pair stands; the refused one, velocity included, is gone"
    );
    assert_eq!(
        *later_heard.lock().unwrap(),
        vec![([0.5, 0.0, 0.0], [3.0, 0.0, 0.0])],
        "the later listener hears the accepted pair"
    );
    assert!(!canceled);
}

mod crate_root {
    mod entity {
        pub(crate) mod types {
            /// The fixture's validator: input.x past 1.0 is refused. The real budgets are
            /// tested in the vocabulary crate.
            pub(crate) fn refused_travel_output(
                input: &[f64; 3],
                _: &[f64; 3],
            ) -> Result<(), &'static str> {
                if input[0] > 1.0 { Err("input") } else { Ok(()) }
            }
        }
    }
}
