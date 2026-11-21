//! The HUD's layer order (NeoForge's `GuiLayerManager`): vanilla's layers in the order Sunrise
//! draws them, with native layers inserted and bodies installed once at client startup.

use mod_api::{
    __private::game::{CallbackOwner, discard_registration},
    GuiLayerDraw, GuiLayerPlacement, GuiLayerRegistrar, GuiLayerWrapper, HookId, Identifier,
    content::crash::current_native_mod,
};
use std::sync::OnceLock;

/// Vanilla HUD anchors used by the engine's HUD pass (no camera overlays or debug screen).
#[derive(Clone, Copy)]
pub enum VanillaGuiLayer {
    Crosshair,
    Hotbar,
    Effects,
    BossOverlay,
    VehicleHealth,
    PlayerHealth,
    ContextualInfoBar,
    SelectedItemName,
    SleepOverlay,
    OverlayMessage,
    Title,
    ScoreboardSidebar,
    SubtitleOverlay,
    Chat,
    TabList,
}

impl VanillaGuiLayer {
    /// Vanilla anchors in the layer registrar's bottom-to-top order.
    pub const ALL: [Self; 15] = [
        Self::Crosshair,
        Self::Hotbar,
        Self::Effects,
        Self::BossOverlay,
        Self::VehicleHealth,
        Self::PlayerHealth,
        Self::ContextualInfoBar,
        Self::SelectedItemName,
        Self::SleepOverlay,
        Self::OverlayMessage,
        Self::Title,
        Self::ScoreboardSidebar,
        Self::SubtitleOverlay,
        Self::Chat,
        Self::TabList,
    ];

    /// The vanilla layer ID reported to HUD observers.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Crosshair => "minecraft:crosshair",
            Self::Hotbar => "minecraft:hotbar",
            Self::Effects => "minecraft:effects",
            Self::BossOverlay => "minecraft:boss_overlay",
            Self::VehicleHealth => "minecraft:vehicle_health",
            Self::PlayerHealth => "minecraft:player_health",
            Self::ContextualInfoBar => "minecraft:contextual_info_bar",
            Self::SelectedItemName => "minecraft:selected_item_name",
            Self::SleepOverlay => "minecraft:sleep_overlay",
            Self::OverlayMessage => "minecraft:overlay_message",
            Self::Title => "minecraft:title",
            Self::ScoreboardSidebar => "minecraft:scoreboard_sidebar",
            Self::SubtitleOverlay => "minecraft:subtitle_overlay",
            Self::Chat => "minecraft:chat",
            Self::TabList => "minecraft:tab_list",
        }
    }
}

// no wire: slots own process-local layer bodies.
/// An installed HUD layer, with an ID and anchor independent of its body.
pub struct GuiLayerSlot {
    id: GuiLayerId,
    pub body: GuiLayerBody,
}

enum GuiLayerId {
    Vanilla(VanillaGuiLayer),
    Native(String),
}

// no wire: native bodies own process-local draw callbacks.
/// The current implementation of one HUD slot.
pub enum GuiLayerBody {
    Vanilla,
    Native {
        owner: Option<CallbackOwner>,
        draw: Box<GuiLayerDraw>,
    },
    Wrapped {
        owner: Option<CallbackOwner>,
        wrap: Box<GuiLayerWrapper>,
        previous: Box<GuiLayerBody>,
    },
}

impl GuiLayerSlot {
    /// The stable slot ID reported by Pre and Post.
    pub fn id(&self) -> &str {
        match &self.id {
            GuiLayerId::Vanilla(layer) => layer.id(),
            GuiLayerId::Native(id) => id,
        }
    }

    /// The engine paint site for a vanilla slot, even when its body is replaced or wrapped.
    pub fn vanilla_anchor(&self) -> Option<VanillaGuiLayer> {
        match &self.id {
            GuiLayerId::Vanilla(layer) => Some(*layer),
            GuiLayerId::Native(_) => None,
        }
    }
}

/// The client startup registrar, seeded with vanilla's layers.
pub struct GuiLayers {
    slots: Vec<GuiLayerSlot>,
}

impl Default for GuiLayers {
    fn default() -> Self {
        Self {
            slots: VanillaGuiLayer::ALL
                .into_iter()
                .map(|layer| GuiLayerSlot {
                    id: GuiLayerId::Vanilla(layer),
                    body: GuiLayerBody::Vanilla,
                })
                .collect(),
        }
    }
}

impl GuiLayerBody {
    /// Drop each callback separately. Detach the tail before running a wrapper's destructor so
    /// another mod destructor cannot run as part of its unwind.
    fn dispose(&mut self, in_listener: bool) {
        loop {
            let (owner, draw, wrap) = match std::mem::replace(self, Self::Vanilla) {
                Self::Vanilla => return,
                Self::Native { owner, draw } => (owner, Some(draw), None),
                Self::Wrapped {
                    owner,
                    wrap,
                    previous,
                } => {
                    *self = *previous;
                    (owner, None, Some(wrap))
                }
            };
            if in_listener {
                // A superseded callback from this batch is still the current listener's work.
                drop((draw, wrap));
            } else {
                // Slots are already published or rejected; cleanup cannot undo another batch.
                discard_registration(
                    (HookId::RegisterGuiLayers, "register_gui_layers"),
                    owner.as_ref().map(CallbackOwner::id),
                    (draw, wrap),
                );
            }
        }
    }
}

enum StagedLayerId {
    Existing(usize),
    Native(String),
}

struct StagedLayer {
    id: StagedLayerId,
    // A Vanilla tail on an existing slot stands for its original body, still in `existing`.
    body: GuiLayerBody,
    // The position of that original body in `retired`, assigned on the first replacement.
    retirement: Option<usize>,
}

struct GuiLayerEdits<'a> {
    existing: &'a [GuiLayerSlot],
    slots: Vec<StagedLayer>,
    obsolete: Vec<GuiLayerBody>,
    retired: Vec<GuiLayerBody>,
}

impl GuiLayerEdits<'_> {
    /// Resolve against the order at this call, including this listener's earlier insertions.
    fn position(&self, id: &str) -> Option<usize> {
        self.slots.iter().position(|slot| match &slot.id {
            StagedLayerId::Existing(at) => self.existing[*at].id() == id,
            StagedLayerId::Native(key) => key == id,
        })
    }

    fn edit_position(&self, id: &str, owner: Option<&str>, action: &str) -> Option<usize> {
        let at = self.position(id);
        if at.is_none() {
            log::warn!(
                "{} cannot {action} HUD layer {id}: it is not registered",
                owner.unwrap_or("unknown mod")
            );
        }
        at
    }
}

impl GuiLayerRegistrar for GuiLayerEdits<'_> {
    fn register(
        &mut self,
        id: String,
        placement: GuiLayerPlacement,
        draw: Box<GuiLayerDraw>,
    ) -> bool {
        if Identifier::parse_namespaced(&id).is_none_or(|id| id.namespace() == "minecraft") {
            log::warn!(
                "HUD layer {id:?} must be <namespace>:<path> outside the minecraft namespace"
            );
            return false;
        }
        if self.position(&id).is_some() {
            log::warn!("HUD layer {id} is registered twice; the first is kept");
            return false;
        }
        let at = match &placement {
            GuiLayerPlacement::BelowAll => Some(0),
            GuiLayerPlacement::AboveAll => Some(self.slots.len()),
            GuiLayerPlacement::Below(other) => self.position(other),
            GuiLayerPlacement::Above(other) => self.position(other).map(|i| i + 1),
        };
        let owner = current_native_mod();
        let Some(at) = at else {
            log::warn!(
                "{} cannot register HUD layer {id}: no registered anchor for {placement:?}",
                owner.unwrap_or("unknown mod")
            );
            return false;
        };
        self.slots.insert(
            at,
            StagedLayer {
                id: StagedLayerId::Native(id),
                body: GuiLayerBody::Native {
                    owner: owner.map(CallbackOwner::for_mod),
                    draw,
                },
                retirement: None,
            },
        );
        true
    }

    fn replace_layer(&mut self, id: &str, draw: Box<GuiLayerDraw>) -> bool {
        let owner = current_native_mod();
        let Some(at) = self.edit_position(id, owner, "replace") else {
            return false;
        };
        let slot = &mut self.slots[at];
        if matches!(slot.id, StagedLayerId::Existing(_)) && slot.retirement.is_none() {
            slot.retirement = Some(self.retired.len());
            self.retired.push(GuiLayerBody::Vanilla);
        }
        let old = std::mem::replace(
            &mut slot.body,
            GuiLayerBody::Native {
                owner: owner.map(CallbackOwner::for_mod),
                draw,
            },
        );
        if !matches!(old, GuiLayerBody::Vanilla) {
            self.obsolete.push(old);
        }
        // Keep both the new callback and any undropped tail in the batch if a destructor unwinds.
        while let Some(body) = self.obsolete.last_mut() {
            body.dispose(true);
            self.obsolete.pop();
        }
        true
    }

    fn wrap_layer(&mut self, id: &str, wrap: Box<GuiLayerWrapper>) -> bool {
        let owner = current_native_mod();
        let Some(at) = self.edit_position(id, owner, "wrap") else {
            return false;
        };
        let body = &mut self.slots[at].body;
        let previous = Box::new(std::mem::replace(body, GuiLayerBody::Vanilla));
        *body = GuiLayerBody::Wrapped {
            owner: owner.map(CallbackOwner::for_mod),
            wrap,
            previous,
        };
        true
    }
}

/// The frozen draw order, with a direct index for each vanilla anchor.
pub struct GuiLayerOrder {
    slots: Vec<GuiLayerSlot>,
    positions: [usize; VanillaGuiLayer::ALL.len()],
    has_native_bodies: bool,
}

impl GuiLayers {
    /// Stage one listener's edits and publish them in call order only if it succeeds.
    pub fn register_listener(&mut self, listen: impl FnOnce(&mut dyn GuiLayerRegistrar) -> bool) {
        let mut pending = GuiLayerEdits {
            existing: &self.slots,
            slots: (0..self.slots.len())
                .map(|at| StagedLayer {
                    id: StagedLayerId::Existing(at),
                    body: GuiLayerBody::Vanilla,
                    retirement: None,
                })
                .collect(),
            obsolete: Vec::new(),
            retired: Vec::new(),
        };
        let accepted = listen(&mut pending);
        let GuiLayerEdits {
            slots,
            obsolete,
            mut retired,
            ..
        } = pending;
        if accepted {
            // Insertions never reorder existing slots. Publish the slots without destructors,
            // then retire their old bodies in the order of the listener's first replacements.
            let mut existing = std::mem::take(&mut self.slots).into_iter();
            let mut published = Vec::with_capacity(slots.len());
            for StagedLayer {
                id,
                mut body,
                retirement,
            } in slots
            {
                let id = match id {
                    StagedLayerId::Native(id) => GuiLayerId::Native(id),
                    StagedLayerId::Existing(_) => {
                        let slot = existing.next().expect("staged existing HUD slot");
                        if let Some(retirement) = retirement {
                            retired[retirement] = slot.body;
                        } else {
                            let mut tail = &mut body;
                            while let GuiLayerBody::Wrapped { previous, .. } = tail {
                                tail = previous;
                            }
                            *tail = slot.body;
                        }
                        slot.id
                    }
                };
                published.push(GuiLayerSlot { id, body });
            }
            self.slots = published;
        } else {
            for mut slot in slots {
                slot.body.dispose(false);
            }
        }
        for mut body in obsolete.into_iter().chain(retired) {
            body.dispose(false);
        }
    }

    fn into_order(self) -> GuiLayerOrder {
        let mut positions = [0; VanillaGuiLayer::ALL.len()];
        let mut has_native_bodies = false;
        for (at, slot) in self.slots.iter().enumerate() {
            if let Some(layer) = slot.vanilla_anchor() {
                positions[layer as usize] = at;
            }
            has_native_bodies |= !matches!(slot.body, GuiLayerBody::Vanilla);
        }
        GuiLayerOrder {
            slots: self.slots,
            positions,
            has_native_bodies,
        }
    }
}

impl GuiLayerOrder {
    /// The position of a vanilla anchor, including anchors skipped this frame.
    pub fn position(&self, layer: VanillaGuiLayer) -> usize {
        self.positions[layer as usize]
    }

    /// Every layer, bottom first.
    pub fn slots(&self) -> &[GuiLayerSlot] {
        &self.slots
    }
}

static ORDER: OnceLock<GuiLayerOrder> = OnceLock::new();

/// Publish the client startup registration once, before any HUD draws.
pub fn freeze(layers: GuiLayers) {
    if let Err(order) = ORDER.set(layers.into_order()) {
        log::warn!("HUD layers were registered already; these are left out");
        for mut slot in order.slots {
            slot.body.dispose(false);
        }
    }
}

/// The installed order, if it has a mod's own layer, replacement or wrapper.
pub fn order() -> Option<&'static GuiLayerOrder> {
    ORDER.get().filter(|order| order.has_native_bodies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_go_where_register_gui_layers_event_inserts_them() {
        use GuiLayerPlacement::*;
        let mut layers = GuiLayers::default();
        layers.register_listener(|layers| {
            for (id, placement) in [
                ("compass:bar", Above("minecraft:hotbar".into())),
                // Below the compass, so between the hotbar and it.
                ("compass:shadow", Below("compass:bar".into())),
                ("compass:first", BelowAll),
                ("compass:last", AboveAll),
                // A second `BelowAll` goes under the first.
                ("compass:very_first", BelowAll),
                ("compass:over_chat", Above("minecraft:chat".into())),
            ] {
                assert!(layers.register(id.into(), placement, Box::new(|_, _| {})));
            }
            true
        });
        let order = layers.into_order();
        let ids: Vec<&str> = order.slots().iter().map(GuiLayerSlot::id).collect();
        assert_eq!(
            ids,
            [
                "compass:very_first",
                "compass:first",
                "minecraft:crosshair",
                "minecraft:hotbar",
                "compass:shadow",
                "compass:bar",
                "minecraft:effects",
                "minecraft:boss_overlay",
                "minecraft:vehicle_health",
                "minecraft:player_health",
                "minecraft:contextual_info_bar",
                "minecraft:selected_item_name",
                "minecraft:sleep_overlay",
                "minecraft:overlay_message",
                "minecraft:title",
                "minecraft:scoreboard_sidebar",
                "minecraft:subtitle_overlay",
                "minecraft:chat",
                "compass:over_chat",
                "minecraft:tab_list",
                "compass:last",
            ]
        );
        for layer in VanillaGuiLayer::ALL {
            assert_eq!(ids[order.position(layer)], layer.id());
        }
    }

    #[test]
    fn a_layer_that_cannot_be_placed_is_left_out() {
        use GuiLayerPlacement::*;
        let mut layers = GuiLayers::default();
        layers.register_listener(|layers| {
            for (id, placement, accepted) in [
                ("minecraft:hotbar", AboveAll, false),
                ("minecraft:mine", AboveAll, false),
                ("no_namespace", AboveAll, false),
                ("compass:bad space", AboveAll, false),
                ("compass:bar", AboveAll, true),
                ("compass:bar", BelowAll, false),
                // An unknown anchor does not reserve the ID: it can be registered again later.
                ("compass:early", Below("compass:late".into()), false),
                ("compass:late", AboveAll, true),
                ("compass:early", Below("compass:late".into()), true),
                (
                    "compass:lost",
                    Above("minecraft:no_such_layer".into()),
                    false,
                ),
            ] {
                assert_eq!(
                    layers.register(id.into(), placement, Box::new(|_, _| {})),
                    accepted,
                    "registration of {id}"
                );
            }
            true
        });
        let order = layers.into_order();
        let mods: Vec<&str> = order
            .slots()
            .iter()
            .filter(|slot| slot.vanilla_anchor().is_none())
            .map(GuiLayerSlot::id)
            .collect();
        assert_eq!(mods, ["compass:bar", "compass:early", "compass:late"]);
        assert_eq!(
            order.slots()[VanillaGuiLayer::ALL.len()].id(),
            "compass:bar"
        );
    }

    #[test]
    fn a_phase_refusal_discards_all_hud_layer_edits_from_one_listener() {
        use mod_api::__private::game::RegistrationContextNew;
        use mod_api::__private::{LoadAccess, ModPhase, PhaseOwner, check_access};
        use mod_api::{Hooks, ModEntry, RegisterGuiLayersEvent, RegistrationContext};

        enum Listener {
            First,
            Refused,
            Last,
        }
        #[mod_api::__subscriptions]
        impl Hooks for Listener {
            fn register_gui_layers(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterGuiLayersEvent<'_>,
            ) {
                use GuiLayerPlacement::{Above, Below};
                let layers = &mut *event.layers;
                match self {
                    Self::First => {
                        assert!(layers.register(
                            "hud:base".into(),
                            Above("minecraft:hotbar".into()),
                            Box::new(|_, _| {}),
                        ));
                        assert!(layers.wrap_layer("minecraft:hotbar", Box::new(|_| {})));
                    }
                    Self::Refused => {
                        assert!(layers.register(
                            "hud:aborted".into(),
                            Above("hud:base".into()),
                            Box::new(|_, _| {}),
                        ));
                        assert!(layers.replace_layer("minecraft:hotbar", Box::new(|_, _| {})));
                        assert!(layers.wrap_layer("hud:base", Box::new(|_| {})));
                        // Returning normally after catching the unwind does not accept the edits.
                        assert!(
                            std::panic::catch_unwind(|| {
                                check_access("entity", LoadAccess::WorldOnly);
                            })
                            .is_err()
                        );
                    }
                    Self::Last => {
                        assert!(!layers.register(
                            "hud:anchor".into(),
                            Above("hud:aborted".into()),
                            Box::new(|_, _| {}),
                        ));
                        assert!(layers.register(
                            "hud:anchor".into(),
                            Above("minecraft:hotbar".into()),
                            Box::new(|_, _| {}),
                        ));
                        assert!(layers.register(
                            "hud:child".into(),
                            Below("hud:anchor".into()),
                            Box::new(|_, _| {}),
                        ));
                        assert!(layers.wrap_layer("hud:child", Box::new(|_| {})));
                        assert!(layers.wrap_layer("hud:anchor", Box::new(|_| {})));
                        assert!(layers.replace_layer("hud:anchor", Box::new(|_, _| {})));
                    }
                }
            }
        }

        let mods = vec![
            ModEntry::new("first", Box::new(Listener::First)),
            ModEntry::new("refused", Box::new(Listener::Refused)),
            ModEntry::new("last", Box::new(Listener::Last)),
        ];
        let mut layers = GuiLayers::default();
        mod_platform::testing::with_mods(mods.into(), || {
            let _phase = PhaseOwner::scope(ModPhase::Registration);
            mod_api::dispatch_register_gui_layers(&RegistrationContext::new(), |listen| {
                layers.register_listener(listen);
            });
        });
        let order = layers.into_order();
        let ids: Vec<_> = order.slots().iter().map(GuiLayerSlot::id).collect();
        assert!(!ids.contains(&"hud:aborted"));
        let hotbar = order.position(VanillaGuiLayer::Hotbar);
        assert_eq!(
            &ids[hotbar..hotbar + 5],
            [
                "minecraft:hotbar",
                "hud:child",
                "hud:anchor",
                "hud:base",
                "minecraft:effects"
            ]
        );
        assert!(matches!(
            &order.slots()[hotbar].body,
            GuiLayerBody::Wrapped { owner: Some("first"), previous, .. }
                if matches!(**previous, GuiLayerBody::Vanilla)
        ));
        assert!(matches!(
            &order.slots()[hotbar + 1].body,
            GuiLayerBody::Wrapped { owner: Some("last"), previous, .. }
                if matches!(**previous, GuiLayerBody::Native { owner: Some("last"), .. })
        ));
        assert!(matches!(
            &order.slots()[hotbar + 2].body,
            GuiLayerBody::Native {
                owner: Some("last"),
                ..
            }
        ));
        assert!(matches!(
            &order.slots()[hotbar + 3].body,
            GuiLayerBody::Native {
                owner: Some("first"),
                ..
            }
        ));
    }

    #[test]
    fn hud_callback_teardown_refusals_are_contained() {
        use mod_api::__private::game::RegistrationContextNew;
        use mod_api::__private::{LoadAccess, ModPhase, PhaseOwner, check_access};
        use mod_api::{Hooks, ModEntry, RegisterGuiLayersEvent, RegistrationContext};
        use std::sync::{Arc, Mutex};

        type Drops = Arc<Mutex<Vec<(&'static str, Option<&'static str>)>>>;

        struct PhaseDrop(&'static str, Drops);
        impl Drop for PhaseDrop {
            fn drop(&mut self) {
                self.1.lock().unwrap().push((self.0, current_native_mod()));
                // A regression should fail this test, not double-panic and abort the test binary.
                if !std::thread::panicking() {
                    check_access("hud_capture_drop", LoadAccess::WorldOnly);
                }
            }
        }

        enum Action {
            Register,
            Refuse,
            Replace,
        }
        struct Listener(Action, Drops);
        #[mod_api::__subscriptions]
        impl Hooks for Listener {
            fn register_gui_layers(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterGuiLayersEvent<'_>,
            ) {
                let layers = &mut *event.layers;
                let draw = |label| -> Box<GuiLayerDraw> {
                    let capture = PhaseDrop(label, Arc::clone(&self.1));
                    Box::new(move |_, _| {
                        std::hint::black_box(&capture);
                    })
                };
                match self.0 {
                    Action::Register => {
                        assert!(layers.register(
                            "hud:original".into(),
                            GuiLayerPlacement::AboveAll,
                            draw("retired"),
                        ));
                    }
                    Action::Refuse => {
                        assert!(layers.register(
                            "hud:aborted".into(),
                            GuiLayerPlacement::AboveAll,
                            draw("discarded inner draw"),
                        ));
                        let outer = PhaseDrop("superseded wrapper", Arc::clone(&self.1));
                        assert!(layers.wrap_layer(
                            "hud:aborted",
                            Box::new(move |_| {
                                std::hint::black_box(&outer);
                            }),
                        ));
                        // The superseded wrapper refuses the listener while this call is active.
                        layers.replace_layer("hud:aborted", draw("discarded replacement"));
                        self.1
                            .lock()
                            .unwrap()
                            .push(("replace returned", current_native_mod()));
                    }
                    Action::Replace => {
                        assert!(layers.replace_layer("hud:original", Box::new(|_, _| {})));
                        assert!(layers.register(
                            "hud:last".into(),
                            GuiLayerPlacement::Above("hud:original".into()),
                            Box::new(|_, _| {}),
                        ));
                    }
                }
            }
        }

        crate::test_log::install();
        crate::test_log::clear();
        let drops = Drops::default();
        let mods = vec![
            ModEntry::new(
                "original",
                Box::new(Listener(Action::Register, drops.clone())),
            ),
            ModEntry::new(
                "refusing_drop",
                Box::new(Listener(Action::Refuse, drops.clone())),
            ),
            ModEntry::new(
                "replacing",
                Box::new(Listener(Action::Replace, drops.clone())),
            ),
        ];
        mod_platform::testing::with_mods(mods.into(), || {
            let _phase = PhaseOwner::scope(ModPhase::Registration);
            // Refusal is not poison: both calls take the same path, but use the shared diagnostic.
            for _ in 0..2 {
                let mut layers = GuiLayers::default();
                mod_api::dispatch_register_gui_layers(&RegistrationContext::new(), |listen| {
                    layers.register_listener(listen);
                });
                let order = layers.into_order();
                let native = &order.slots()[VanillaGuiLayer::ALL.len()..];
                assert_eq!(
                    native.iter().map(GuiLayerSlot::id).collect::<Vec<_>>(),
                    ["hud:original", "hud:last"]
                );
                assert!(matches!(
                    &native[0].body,
                    GuiLayerBody::Native {
                        owner: Some("replacing"),
                        ..
                    }
                ));
            }
        });
        assert_eq!(
            *drops.lock().unwrap(),
            [
                ("superseded wrapper", Some("refusing_drop")),
                ("discarded replacement", Some("refusing_drop")),
                ("discarded inner draw", Some("refusing_drop")),
                ("retired", Some("original")),
            ]
            .repeat(2)
        );
        let logs: Vec<_> = crate::test_log::records()
            .into_iter()
            .map(|(_, message)| message)
            .filter(|message| message.contains("hud_capture_drop"))
            .collect();
        assert_eq!(
            logs,
            [
                "native refusal: refusing_drop hook register_gui_layers: mod API hud_capture_drop is unavailable during Registration",
                "native refusal: refusing_drop hook register_gui_layers: mod API hud_capture_drop is unavailable during Registration",
                "native refusal: refusing_drop hook register_gui_layers: mod API hud_capture_drop is unavailable during Registration",
                "native refusal: original hook register_gui_layers: mod API hud_capture_drop is unavailable during Registration",
            ].repeat(2)
        );
    }

    #[test]
    fn a_panicking_hud_registration_cannot_displace_vanilla_or_later_layers() {
        use mod_api::__private::game::RegistrationContextNew;
        use mod_api::__private::{HookPanicReport, ModPhase, PhaseOwner, take_hook_panics};
        use mod_api::{Hooks, ModEntry, RegisterGuiLayersEvent, RegistrationContext};
        use std::sync::{Arc, Mutex};

        struct PanicOnDrop(Arc<Mutex<Vec<&'static str>>>);
        impl Drop for PanicOnDrop {
            fn drop(&mut self) {
                self.0.lock().unwrap().push(current_native_mod().unwrap());
                if !std::thread::panicking() {
                    panic!("discarded HUD callback");
                }
            }
        }
        struct Register {
            mod_id: &'static str,
            drops: Arc<Mutex<Vec<&'static str>>>,
        }
        #[mod_api::__subscriptions]
        impl Hooks for Register {
            fn register_gui_layers(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterGuiLayersEvent<'_>,
            ) {
                use GuiLayerPlacement::{Above, AboveAll};
                let layers = &mut *event.layers;
                match self.mod_id {
                    "retired" => {
                        let capture = PanicOnDrop(self.drops.clone());
                        assert!(layers.register(
                            "hud:earlier".into(),
                            AboveAll,
                            Box::new(move |_, _| {
                                std::hint::black_box(&capture);
                            })
                        ));
                    }
                    "failing" => {
                        let capture = PanicOnDrop(self.drops.clone());
                        assert!(layers.register(
                            "hud:discarded".into(),
                            AboveAll,
                            Box::new(move |_, _| {
                                std::hint::black_box(&capture);
                            })
                        ));
                        assert!(layers.replace_layer("minecraft:hotbar", Box::new(|_, _| {})));
                        assert!(layers.wrap_layer("minecraft:hotbar", Box::new(|_| {})));
                        panic!("HUD registration failed");
                    }
                    "healthy" => {
                        assert!(layers.replace_layer("hud:earlier", Box::new(|_, _| {})));
                        assert!(layers.register(
                            "hud:next".into(),
                            Above("minecraft:hotbar".into()),
                            Box::new(|_, _| {})
                        ));
                    }
                    _ => unreachable!(),
                }
            }
        }

        let drops = Arc::new(Mutex::new(Vec::new()));
        let mods = ["retired", "failing", "healthy"].map(|mod_id| {
            ModEntry::new(
                mod_id,
                Box::new(Register {
                    mod_id,
                    drops: drops.clone(),
                }),
            )
        });
        mod_platform::testing::with_mods(Vec::from(mods).into(), || {
            let _phase = PhaseOwner::scope(ModPhase::Registration);
            let mut layers = GuiLayers::default();
            mod_api::dispatch_register_gui_layers(&RegistrationContext::new(), |listen| {
                layers.register_listener(listen);
            });
            let order = layers.into_order();
            let hotbar = order.position(VanillaGuiLayer::Hotbar);
            assert!(matches!(order.slots()[hotbar].body, GuiLayerBody::Vanilla));
            assert_eq!(order.slots()[hotbar + 1].id(), "hud:next");
            assert!(
                !order
                    .slots()
                    .iter()
                    .any(|slot| slot.id() == "hud:discarded")
            );
            let replaced = order.slots().last().unwrap();
            assert_eq!(replaced.id(), "hud:earlier");
            assert!(matches!(
                replaced.body,
                GuiLayerBody::Native {
                    owner: Some("healthy"),
                    ..
                }
            ));
            assert_eq!(*drops.lock().unwrap(), ["failing", "retired"]);
            assert_eq!(
                take_hook_panics(),
                [
                    HookPanicReport {
                        mod_id: "failing",
                        hook: "register_gui_layers"
                    },
                    HookPanicReport {
                        mod_id: "retired",
                        hook: "register_gui_layers"
                    },
                ]
            );
        });
    }

    #[test]
    fn earlier_hud_bodies_retire_in_first_replacement_order() {
        use std::sync::{Arc, Mutex};

        struct RecordDrop(&'static str, Arc<Mutex<Vec<&'static str>>>);
        impl Drop for RecordDrop {
            fn drop(&mut self) {
                self.1.lock().unwrap().push(self.0);
            }
        }

        let drops = Arc::new(Mutex::new(Vec::new()));
        let mut layers = GuiLayers::default();
        layers.register_listener(|layers| {
            for id in ["hud:a", "hud:b"] {
                let capture = RecordDrop(id, Arc::clone(&drops));
                assert!(layers.register(
                    id.into(),
                    GuiLayerPlacement::AboveAll,
                    Box::new(move |_, _| {
                        std::hint::black_box(&capture);
                    }),
                ));
            }
            true
        });
        layers.register_listener(|layers| {
            for id in ["hud:b", "hud:a", "hud:b"] {
                assert!(layers.replace_layer(id, Box::new(|_, _| {})));
            }
            assert!(drops.lock().unwrap().is_empty());
            true
        });
        assert_eq!(*drops.lock().unwrap(), ["hud:b", "hud:a"]);
        let order = layers.into_order();
        let slots = &order.slots()[VanillaGuiLayer::ALL.len()..];
        assert_eq!(
            slots.iter().map(GuiLayerSlot::id).collect::<Vec<_>>(),
            ["hud:a", "hud:b"]
        );
    }

    #[test]
    fn no_registration_leaves_vanilla_alone() {
        let order = GuiLayers::default().into_order();
        assert!(
            order
                .slots()
                .iter()
                .all(|slot| matches!(slot.body, GuiLayerBody::Vanilla))
        );
        for (at, layer) in VanillaGuiLayer::ALL.into_iter().enumerate() {
            assert_eq!(order.position(layer), at);
            assert_eq!(order.slots()[at].id(), layer.id());
        }
    }

    #[test]
    fn only_existing_vanilla_layers_are_registration_anchors() {
        let staged_ids = [
            "camera_overlays",
            "after_camera_decorations",
            "contextual_info_bar_background",
            "experience_level",
            "armor_level",
            "food_level",
            "air_level",
            "spectator_tooltip",
            "demo_overlay",
        ];
        let mut layers = GuiLayers::default();
        for id in staged_ids {
            assert!(!layers.register(
                "compass:bar".into(),
                GuiLayerPlacement::Above(format!("minecraft:{id}")),
                Box::new(|_, _| {}),
            ));
        }
        let order = layers.into_order();
        assert_eq!(order.slots().len(), VanillaGuiLayer::ALL.len());
        assert!(
            order
                .slots()
                .iter()
                .all(|slot| slot.vanilla_anchor().is_some())
        );
    }
}
