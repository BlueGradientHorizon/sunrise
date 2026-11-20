mod stubs {
    #[derive(Debug, PartialEq)]
    pub(super) struct SectionVisibilityInput(pub(super) [i32; 3]);
    #[derive(Debug, PartialEq)]
    pub(super) enum SectionVisibility {
        Keep(u8),
        HideTerrain,
    }
}

/// The generated dispatch of `section_visibility` lends each listening mod the same raw input and
/// an empty batch of its own, and names that mod on the answer it pushed, in mod order; a mod that
/// pushes nothing leaves no entry.
#[test]
fn each_section_visibility_answer_names_the_mod_that_gave_it() {
    struct Answers(Option<fn() -> SectionVisibility>);
    #[mod_api::__subscriptions]
    impl Hooks for Answers {
        fn section_visibility(&self, _: &dyn ClientHost, event: &mut SectionVisibilityEvent<'_>) {
            assert_eq!(*event.input, SectionVisibilityInput([16, -64, 32]));
            assert!(event.answers.is_empty(), "each mod starts from its own batch");
            event.answers.extend(self.0.map(|answer| answer()));
        }
    }
    install(
        vec![
            ModEntry::new("xray", Box::new(Answers(Some(|| SectionVisibility::Keep(61))))),
            ModEntry::new("quiet", Box::new(Answers(None))),
            ModEntry::new(
                "cutaway",
                Box::new(Answers(Some(|| SectionVisibility::HideTerrain))),
            ),
        ]
        .into(),
    );
    let input = SectionVisibilityInput([16, -64, 32]);
    let mut answers = Vec::new();
    dispatch_section_visibility(
        &TestHost,
        calls::SectionVisibilityCall {
            input: &input,
            answers: &mut answers,
        },
    );
    assert_eq!(
        answers,
        [
            Answered {
                answer: SectionVisibility::Keep(61),
                mod_id: "xray"
            },
            Answered {
                answer: SectionVisibility::HideTerrain,
                mod_id: "cutaway"
            }
        ]
    );
}

/// A listener that unwinds loses its own batch and is skipped by the hook for the session; the
/// listeners around it still answer, in this dispatch and the next (isolated collector).
#[test]
fn a_section_visibility_listener_that_unwinds_loses_only_its_own_answer() {
    struct Answers {
        answer: Option<fn() -> SectionVisibility>,
        unwinds: bool,
        asked: Arc<Mutex<u32>>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for Answers {
        fn section_visibility(&self, _: &dyn ClientHost, event: &mut SectionVisibilityEvent<'_>) {
            *self.asked.lock().unwrap() += 1;
            event.answers.extend(self.answer.map(|answer| answer()));
            if self.unwinds {
                panic!("the fixture's listener unwinds");
            }
        }
    }
    let asked = Arc::new(Mutex::new(0));
    let failing = Arc::new(Mutex::new(0));
    let mod_entry = |id, answer, unwinds, asked: &Arc<Mutex<u32>>| {
        ModEntry::new(
            id,
            Box::new(Answers {
                answer,
                unwinds,
                asked: Arc::clone(asked),
            }),
        )
    };
    install(
        vec![
            mod_entry("xray", Some(|| SectionVisibility::Keep(61)), false, &asked),
            mod_entry("broken", Some(|| SectionVisibility::Keep(1)), true, &failing),
            mod_entry("cutaway", Some(|| SectionVisibility::HideTerrain), false, &asked),
        ]
        .into(),
    );
    let input = SectionVisibilityInput([0, 0, 0]);
    let healthy = [
        Answered {
            answer: SectionVisibility::Keep(61),
            mod_id: "xray",
        },
        Answered {
            answer: SectionVisibility::HideTerrain,
            mod_id: "cutaway",
        },
    ];
    for dispatch in 0..2 {
        let mut answers = Vec::new();
        dispatch_section_visibility(
            &TestHost,
            calls::SectionVisibilityCall {
                input: &input,
                answers: &mut answers,
            },
        );
        assert_eq!(answers, healthy, "dispatch {dispatch}");
    }
    assert_eq!(*failing.lock().unwrap(), 1, "the failed listener is not asked again");
    assert_eq!(*asked.lock().unwrap(), 4, "the others answer both times");
}
