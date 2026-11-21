#![cfg(test)]
//! The health replacement on the real `minecraft:player_health` layer.
use super::{Screen, native_entity_refs::tests::with_client_game};
use mod_api::{
    ClientHost, Flow, GuiLayerPostEvent, GuiLayerPreEvent, HealthHudEvent, HealthHudReplacement,
    Hooks, ModEntry, content::crash::current_native_call,
};
use mod_platform::testing::with_mods;
use std::sync::{Arc, Mutex};
use sunrise_client::motion::PlayerMotion;
use sunrise_core::geometry::Vec3;

const HEALTH_LAYER: &str = "minecraft:player_health";
// Greys are the same in API ARGB and batch ABGR.
const PRE: u32 = 0xFF12_1212;
const CUSTOM: u32 = 0xFF34_3434;
const POST: u32 = 0xFF56_5656;
const CUSTOM_TOP: i32 = 17;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Frame {
    #[default]
    Vanilla,
    Custom,
    Refused,
    Empty,
    HiddenCustom,
    HiddenVanilla,
}

#[derive(Default)]
struct Trace {
    frame: Frame,
    calls: Vec<&'static str>,
}

struct Selector {
    call: &'static str,
    trace: Arc<Mutex<Trace>>,
    renderer: HealthHudReplacement,
}

#[mod_api::__subscriptions]
impl Hooks for Selector {
    fn health_hud(
        &self,
        _: &dyn ClientHost,
        _: &mut HealthHudEvent<'_>,
    ) -> Option<HealthHudReplacement> {
        let mut trace = self.trace.lock().unwrap();
        trace.calls.push(self.call);
        match trace.frame {
            Frame::Vanilla | Frame::HiddenVanilla => None,
            Frame::Custom | Frame::Refused => {
                Some(self.renderer.clone().with_status_top(CUSTOM_TOP))
            }
            Frame::Empty => Some(self.renderer.clone()),
            Frame::HiddenCustom => Some(self.renderer.clone().with_status_top(i32::MIN)),
        }
    }
}

struct LayerObserver(Arc<Mutex<Trace>>);

#[mod_api::__subscriptions]
impl Hooks for LayerObserver {
    fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
        if event.layer == HEALTH_LAYER {
            let mut trace = self.0.lock().unwrap();
            trace.calls.push("pre");
            event.draw.fill([2.0, 2.0, 1.0, 1.0], PRE);
            if matches!(trace.frame, Frame::HiddenCustom | Frame::HiddenVanilla) {
                return Flow::Cancel;
            }
        }
        Flow::Continue
    }

    fn gui_layer_post(&self, _: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
        if event.layer == HEALTH_LAYER {
            self.0.lock().unwrap().calls.push("post");
            event.draw.fill([2.0, 2.0, 1.0, 1.0], POST);
        }
    }
}

#[test]
#[ignore = "daily"]
fn health_hud_runs_the_first_renderer_at_the_layer_and_falls_back_on_refusal() {
    let trace = Arc::new(Mutex::new(Trace::default()));
    let draw_trace = Arc::clone(&trace);
    let renderer = HealthHudReplacement::new(move |_, inputs, canvas| {
        let call = current_native_call().expect("the renderer has an owning mod scope");
        assert_eq!((call.id, call.hook), ("first", "health_hud"));
        assert_eq!(inputs.health, 20.0);
        let frame = {
            let mut trace = draw_trace.lock().unwrap();
            trace.calls.push("custom");
            trace.frame
        };
        if frame == Frame::Empty {
            return Ok(());
        }
        canvas.fill([2.0, 2.0, 1.0, 1.0], CUSTOM)?;
        if frame == Frame::Refused {
            canvas.fill([f32::NAN, 2.0, 1.0, 1.0], CUSTOM)?;
        }
        Ok(())
    });
    let mut mods: Vec<_> = [("first", "first select"), ("late", "late select")]
        .into_iter()
        .map(|(id, call)| {
            ModEntry::new(
                id,
                Box::new(Selector {
                    call,
                    trace: Arc::clone(&trace),
                    renderer: renderer.clone(),
                }),
            )
        })
        .collect();
    mods.push(ModEntry::new(
        "observer",
        Box::new(LayerObserver(trace.clone())),
    ));
    with_mods(mods.into(), || {
        with_client_game(|game| {
            game.title = None;
            game.screen = Screen::Playing;
            game.ui.hide_gui = false;
            game.set_mode_id(0);
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(2.0, 64.0, 2.0)).unwrap());
            let mut vanilla = Vec::new();
            for (frame, expected_calls) in [
                (
                    Frame::Vanilla,
                    &["first select", "late select", "pre", "post"][..],
                ),
                (Frame::Custom, &["first select", "pre", "custom", "post"]),
                (Frame::Refused, &["first select", "pre", "custom", "post"]),
                (Frame::Empty, &["first select", "pre", "custom", "post"]),
                (Frame::HiddenCustom, &["first select", "pre", "post"]),
                (
                    Frame::HiddenVanilla,
                    &["first select", "late select", "pre", "post"],
                ),
            ] {
                trace.lock().unwrap().frame = frame;
                game.health_hud = Default::default();
                let batch = game.hud_layers([640, 480]);
                let calls = std::mem::take(&mut trace.lock().unwrap().calls);
                assert_eq!(calls.as_slice(), expected_calls, "{frame:?}");
                let marker = |color| {
                    batch
                        .vertices
                        .iter()
                        .position(|v| v.color == color)
                        .expect("layer marker")
                };
                let body = &batch.vertices[marker(PRE) + 6..marker(POST)];
                match frame {
                    Frame::Vanilla => {
                        assert!(!body.is_empty());
                        vanilla = bytemuck::cast_slice::<_, u8>(body).to_vec();
                    }
                    Frame::Custom => {
                        assert_eq!(body.len(), 6);
                        assert!(body.iter().all(|v| v.color == CUSTOM));
                    }
                    Frame::Refused => {
                        assert_eq!(bytemuck::cast_slice::<_, u8>(body), vanilla.as_slice());
                        assert!(!batch.vertices.iter().any(|v| v.color == CUSTOM));
                    }
                    Frame::Empty | Frame::HiddenCustom | Frame::HiddenVanilla => {
                        assert!(body.is_empty())
                    }
                }
                let used_vanilla = matches!(
                    frame,
                    Frame::Vanilla | Frame::Refused | Frame::HiddenVanilla
                );
                assert_eq!(
                    game.health_hud.last_health,
                    if used_vanilla { 20 } else { 0 },
                    "{frame:?}"
                );
                let expected_top = match frame {
                    Frame::Vanilla | Frame::Refused | Frame::HiddenVanilla => batch.height - 39.0,
                    Frame::Custom => CUSTOM_TOP as f32,
                    Frame::Empty => batch.height,
                    Frame::HiddenCustom => i32::MIN as f32,
                };
                assert_eq!(game.ui.room.bottom(), expected_top, "{frame:?}");
            }
        });
    });
}
