//! The HUD's layer order (NeoForge's `GuiLayerManager`): vanilla's layers in the order Sunrise
//! draws them, with the layers native mods register (`register_gui_layers`) placed among them
//! once at start, as `RegisterGuiLayersEvent` inserts them.

use mod_api::{GuiLayerDraw, GuiLayerPlacement, GuiLayerRegistrar, Identifier};
use std::sync::OnceLock;

/// Vanilla HUD anchors used by the engine's HUD pass (no camera overlays or debug screen).
#[derive(Clone, Copy)]
pub enum VanillaGuiLayer {
    Crosshair,
    Hotbar,
    Effects,
    BossOverlay,
    VehicleHealth,
    ContextualInfoBar,
    PlayerHealth,
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
        Self::ContextualInfoBar,
        Self::PlayerHealth,
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
            Self::ContextualInfoBar => "minecraft:contextual_info_bar",
            Self::PlayerHealth => "minecraft:player_health",
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

// no wire: native slots own process-local draw callbacks.
/// An installed HUD layer: a vanilla anchor or a mod's resolved draw callback.
pub enum GuiLayerSlot {
    Vanilla(VanillaGuiLayer),
    Native {
        id: String,
        registrant: Option<&'static str>,
        draw: Box<GuiLayerDraw>,
    },
}

impl GuiLayerSlot {
    fn id(&self) -> &str {
        match self {
            Self::Vanilla(layer) => layer.id(),
            Self::Native { id, .. } => id,
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
                .map(GuiLayerSlot::Vanilla)
                .collect(),
        }
    }
}

impl GuiLayerRegistrar for GuiLayers {
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
        let position = |key: &str| self.slots.iter().position(|slot| slot.id() == key);
        if position(&id).is_some() {
            log::warn!("HUD layer {id} is registered twice; the first is kept");
            return false;
        }
        let at = match &placement {
            GuiLayerPlacement::BelowAll => Some(0),
            GuiLayerPlacement::AboveAll => Some(self.slots.len()),
            GuiLayerPlacement::Below(other) => position(other),
            GuiLayerPlacement::Above(other) => position(other).map(|i| i + 1),
        };
        let Some(at) = at else {
            log::warn!("HUD layer {id} has no registered anchor for {placement:?}; it is left out");
            return false;
        };
        self.slots.insert(
            at,
            GuiLayerSlot::Native {
                id,
                registrant: mod_api::content::crash::current_native_mod(),
                draw,
            },
        );
        true
    }
}

/// The frozen draw order, with a direct index for each vanilla anchor.
pub struct GuiLayerOrder {
    slots: Vec<GuiLayerSlot>,
    positions: [usize; VanillaGuiLayer::ALL.len()],
}

impl GuiLayers {
    fn into_order(self) -> GuiLayerOrder {
        let mut positions = [0; VanillaGuiLayer::ALL.len()];
        for (at, slot) in self.slots.iter().enumerate() {
            if let GuiLayerSlot::Vanilla(layer) = slot {
                positions[*layer as usize] = at;
            }
        }
        GuiLayerOrder {
            slots: self.slots,
            positions,
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
    if ORDER.set(layers.into_order()).is_err() {
        log::warn!("HUD layers were registered already; these are left out");
    }
}

/// The installed order, if there are any native layers to draw.
pub fn order() -> Option<&'static GuiLayerOrder> {
    ORDER
        .get()
        .filter(|order| order.slots.len() > VanillaGuiLayer::ALL.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_go_where_register_gui_layers_event_inserts_them() {
        use GuiLayerPlacement::*;
        let mut layers = GuiLayers::default();
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
                "minecraft:contextual_info_bar",
                "minecraft:player_health",
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
        let order = layers.into_order();
        let mods: Vec<&str> = order
            .slots()
            .iter()
            .filter_map(|slot| match slot {
                GuiLayerSlot::Native { id, .. } => Some(id.as_str()),
                GuiLayerSlot::Vanilla(_) => None,
            })
            .collect();
        assert_eq!(mods, ["compass:bar", "compass:early", "compass:late"]);
        assert_eq!(
            order.slots()[VanillaGuiLayer::ALL.len()].id(),
            "compass:bar"
        );
    }

    #[test]
    fn no_registration_leaves_vanilla_alone() {
        let order = GuiLayers::default().into_order();
        assert!(
            order
                .slots()
                .iter()
                .all(|slot| matches!(slot, GuiLayerSlot::Vanilla(_)))
        );
        for (at, layer) in VanillaGuiLayer::ALL.into_iter().enumerate() {
            assert_eq!(order.position(layer), at);
            assert_eq!(order.slots()[at].id(), layer.id());
        }
    }
}
