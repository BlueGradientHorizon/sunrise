use super::*;
use mod_api::__private::game::Fields;
use mod_api::{
    DrawContext, Gui3dError, Gui3dSubject, Gui3dView, IDENTITY_TRANSFORM, ItemStack, ModelError,
};

/// A screen's drawing that keeps each subject it is asked to draw: it refuses a view that is not
/// drawable, an item it does not know and a model of generation 9.
#[derive(Default)]
struct Draw(Vec<(Gui3dSubject, Gui3dView)>);
impl DrawContext for Draw {
    fn model_3d(&mut self, subject: Gui3dSubject, view: Gui3dView) -> Result<(), Gui3dError> {
        self.0.push((subject.clone(), view));
        if !view.is_drawable() {
            return Err(Gui3dError::NotFinite);
        }
        match subject {
            Gui3dSubject::Item(stack) if stack.count() == 7 => Err(Gui3dError::UnknownItem),
            Gui3dSubject::Model(model) if model.fields().generation == 9 => {
                Err(Gui3dError::Model(ModelError::StaleModel))
            }
            _ => Ok(()),
        }
    }
}

/// The items a stack can name, resolved for this process.
fn resolve() {
    mod_api::__private::part_tests::install_parts_fixture(&[]).unwrap();
    mod_api::__private::resolve_items(["minecraft:diamond_sword"]).unwrap();
}

/// What a data model's screen draws: a wand turned about all three axes, a model under a quarter
/// turn, and three the context refuses.
fn asks() -> Vec<(Gui3dSubject, Gui3dView, Result<(), Gui3dError>)> {
    let item = |count| {
        Gui3dSubject::Item(
            ItemStack::new(
                mod_api::__private::item_key("minecraft:diamond_sword").unwrap(),
                count,
            )
            .unwrap(),
        )
    };
    let turn = [
        0.0, -1.0, 0.0, 0.0, //
        1.0, 0.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.5,
    ];
    vec![
        (item(1), Gui3dView::new([178.5, 448.0], 64.0, turn), Ok(())),
        (
            Gui3dSubject::Model(mod_api::__private::built::model_ref(3, 9)),
            Gui3dView::new([26.0, 39.0], 16.0, IDENTITY_TRANSFORM),
            Ok(()),
        ),
        (
            item(7),
            Gui3dView::new([0.0, 0.0], 16.0, IDENTITY_TRANSFORM),
            Err(Gui3dError::UnknownItem),
        ),
        (
            item(1),
            Gui3dView::new([0.0, 0.0], -1.0, IDENTITY_TRANSFORM),
            Err(Gui3dError::NotFinite),
        ),
        (
            Gui3dSubject::Model(mod_api::__private::built::model_ref(9, 1)),
            Gui3dView::new([0.0, 0.0], 16.0, IDENTITY_TRANSFORM),
            Err(Gui3dError::Model(ModelError::StaleModel)),
        ),
    ]
}

/// A native mod's screen draw: the hook's draw context is the game's, called directly.
struct Screen;
#[mod_api::__subscriptions]
impl Hooks for Screen {
    fn gui_render_post(
        &self,
        _: &dyn mod_api::ClientHost,
        event: &mut mod_api::GuiRenderPostEvent<'_>,
    ) {
        for (subject, view, answer) in asks() {
            assert_eq!(event.batch.model_3d(subject, view), answer);
        }
    }
}

struct Client;
impl mod_api::Host for Client {}
mod_api::impl_host_areas!(Client);
impl mod_api::ClientHost for Client {}
mod_api::impl_client_host_areas!(Client);

#[test]
fn a_native_mods_screen_draw_reaches_the_draw_contexts_model_3d() {
    let mods = vec![mod_api::ModEntry::new("wire_test", Box::new(Screen))];
    mod_api::testing::with_mods(mods.into(), || {
        resolve();
        let mut draw = Draw::default();
        mod_api::dispatch_gui_render_post(&Client, &mut draw, false, true);
        let sent: Vec<_> = asks().into_iter().map(|(s, v, _)| (s, v)).collect();
        assert_eq!(draw.0, sent);
    });
}

#[test]
fn a_wasm_mods_model_3d_crosses_whole_and_its_refusal_comes_back() {
    mod_api::__private::initialize().unwrap();
    mod_api::testing::with_mods(vec![].into(), || {
        resolve();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut draw = Draw::default();
        let mut cx = Box::new(boundary::Cx::default());
        let mut frame = boundary::Frame::default();
        frame.set_draw_context_mut(&mut draw);
        cx.frames.push(frame);
        let pointer = &mut *cx as *mut boundary::Cx;
        guest::set_transport(Some(Rc::new(move |id, bytes| {
            let mut out = Writer::new();
            // SAFETY: cx and its frame's draw remain live for this synchronous transport.
            unsafe { boundary::serve(id, pointer, bytes, &mut out) }.unwrap();
            out.into_bytes()
        })));
        let mut sent = Vec::new();
        for (subject, view, answer) in asks() {
            assert_eq!(
                DrawContext::model_3d(&mut guest::Guest, subject.clone(), view),
                answer
            );
            sent.push((subject, view));
        }
        guest::set_transport(None);
        drop(cx);
        // The host was handed exactly what the guest drew: subject, handle, matrix and view.
        assert_eq!(draw.0, sent);
    });
}
