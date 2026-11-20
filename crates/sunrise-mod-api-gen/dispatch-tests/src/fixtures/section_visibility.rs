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
