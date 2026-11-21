//! Reload-owned item providers and baked humanoid equipment trees.
use super::*;
use mod_api::__private::game::CallbackOwner;
use mod_api::{
    ArmorModelRef, ArmorTextureRef, ClientItemExtensions, ClientItemExtensionsRegistrar, ItemKey,
    ModelPart,
};
use std::{
    collections::hash_map::Entry,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sunrise_client::{
    fast_hash::FxHashMap,
    mobs::{Part, Polygon},
};

/// One callback owner per mod and item, shared by all generations of that provider.
#[derive(Default)]
pub(super) struct ItemExtensionOwners(FxHashMap<(&'static str, ItemKey), CallbackOwner>);
impl ItemExtensionOwners {
    pub(super) fn owner_for(
        &mut self,
        mod_id: &'static str,
        item: ItemKey,
    ) -> Option<CallbackOwner> {
        match self.0.entry((mod_id, item)) {
            Entry::Occupied(entry) => Some(entry.get().clone()),
            Entry::Vacant(entry) => Some(entry.insert(CallbackOwner::new(mod_id)?).clone()),
        }
    }
}

pub(super) struct ItemExtensionsPart {
    pub owner: CallbackOwner,
    pub provider: Arc<dyn ClientItemExtensions>,
}

pub(super) struct ArmorModel {
    pub name: String,
    pub slots: [Arc<Part>; 4],
}

pub(super) struct ArmorExtensions {
    pub generation: u64,
    pub models: Vec<ArmorModel>,
    pub textures: Vec<String>,
    pub texture_keys: FxHashMap<String, u32>,
    pub items: FxHashMap<ItemKey, ItemExtensionsPart>,
}
impl Default for ArmorExtensions {
    fn default() -> Self {
        static GENERATION: AtomicU64 = AtomicU64::new(1);
        Self {
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
            models: Vec::new(),
            textures: Vec::new(),
            texture_keys: FxHashMap::default(),
            items: FxHashMap::default(),
        }
    }
}

impl ArmorExtensions {
    pub(super) fn dispose_item_providers(&mut self) {
        for ItemExtensionsPart { owner, provider } in std::mem::take(&mut self.items).into_values()
        {
            owner.dispose(
                "client item extensions",
                "client_item_extensions_drop",
                provider,
            );
        }
    }
}

struct Registration<'a> {
    state: ArmorExtensions,
    sprite_exists: &'a dyn Fn(&str) -> bool,
    owners: &'a mut ItemExtensionOwners,
}
impl ClientItemExtensionsRegistrar for Registration<'_> {
    fn register_model(
        &mut self,
        name: &str,
        model: &ModelPart,
    ) -> std::result::Result<ArmorModelRef, String> {
        mod_api::Identifier::parse(name).map_err(|why| format!("armour model {name}: {why}"))?;
        if self.state.models.iter().any(|model| model.name == name) {
            return Err(format!("duplicate armour model {name}"));
        }
        let baked =
            bake_model("root", model).map_err(|why| format!("armour model {name}: {why}"))?;
        let index = u32::try_from(self.state.models.len())
            .map_err(|_| format!("armour model {name}: model table exhausted"))?;
        use super::equipment_piece::ArmorSlot;
        let slots = [
            ArmorSlot::Head,
            ArmorSlot::Chest,
            ArmorSlot::Legs,
            ArmorSlot::Feet,
        ]
        .map(|slot| Arc::new(hide_other_slots(&baked, slot)));
        self.state.models.push(ArmorModel {
            name: name.to_owned(),
            slots,
        });
        Ok(ArmorModelRef::__new(self.state.generation, index))
    }
    fn register_texture(&mut self, name: &str) -> std::result::Result<ArmorTextureRef, String> {
        if !(self.sprite_exists)(name) {
            return Err(format!("unknown armour texture {name}"));
        }
        if let Some(&index) = self.state.texture_keys.get(name) {
            return Ok(ArmorTextureRef::__new(self.state.generation, index));
        }
        let index = u32::try_from(self.state.textures.len())
            .map_err(|_| format!("armour texture {name}: texture table exhausted"))?;
        self.state.textures.push(name.to_owned());
        self.state.texture_keys.insert(name.to_owned(), index);
        Ok(ArmorTextureRef::__new(self.state.generation, index))
    }
    fn client_item_extensions(
        &mut self,
        item: ItemKey,
        provider: Arc<dyn ClientItemExtensions>,
    ) -> std::result::Result<(), String> {
        let mod_id = mod_api::content::crash::current_native_mod()
            .ok_or_else(|| "client item extensions: no registering provider".to_owned())?;
        let name = mod_api::__private::item_key_id(item)
            .ok_or_else(|| format!("unknown item for extension provider {mod_id}"))?;
        if let Some(previous) = self.state.items.get(&item) {
            return Err(format!(
                "duplicate client item extensions for {name}: {} and {mod_id}",
                previous.owner.id()
            ));
        }
        let owner = self
            .owners
            .owner_for(mod_id, item)
            .ok_or_else(|| format!("unknown item extension provider {mod_id}"))?;
        self.state
            .items
            .insert(item, ItemExtensionsPart { owner, provider });
        Ok(())
    }
}

// The bake's flat preorder lets slot filtering copy polygons without recursively cloning a tree.
struct BakedPart<'a> {
    parent: usize,
    name: &'a str,
    pose: [f32; 9],
    polygons: Vec<Polygon>,
}

// All unknown geometry is validated once while baking, never during a frame.
fn bake_model<'a>(
    name: &'a str,
    model: &'a ModelPart,
) -> std::result::Result<Vec<BakedPart<'a>>, &'static str> {
    let mut baked = Vec::new();
    let mut pending = vec![(0, name, model)];
    while let Some((parent, name, model)) = pending.pop() {
        let pose = [
            model.x,
            model.y,
            model.z,
            model.x_rot,
            model.y_rot,
            model.z_rot,
            model.x_scale,
            model.y_scale,
            model.z_scale,
        ];
        if !pose.iter().all(|x| x.is_finite()) {
            return Err("non-finite model pose");
        }
        let mut polygons = Vec::new();
        for polygon in &model.polygons {
            if !polygon.normal.iter().all(|x| x.is_finite())
                || !polygon
                    .vertices
                    .iter()
                    .all(|v| v.pos.iter().chain([&v.u, &v.v]).all(|x| x.is_finite()))
            {
                return Err("non-finite model geometry");
            }
            if !model.skip_draw && model.visible {
                polygons.push(Polygon {
                    normal: polygon.normal,
                    vertices: polygon
                        .vertices
                        .map(|v| [v.pos[0], v.pos[1], v.pos[2], v.u, v.v]),
                    cull_back: false,
                });
            }
        }
        if model.visible {
            let index = baked.len();
            let start = pending.len();
            pending.extend(model.children().map(|(name, child)| (index, name, child)));
            pending[start..].reverse();
        }
        baked.push(BakedPart {
            parent,
            name,
            pose,
            polygons,
        });
    }
    Ok(baked)
}

fn hide_other_slots(baked: &[BakedPart<'_>], slot: super::equipment_piece::ArmorSlot) -> Part {
    let mut selected: Vec<Option<(Part, bool)>> = Vec::with_capacity(baked.len());
    for (index, part) in baked.iter().enumerate() {
        // Keep the bind pose of a hidden part, but neither its polygons nor its subtree.
        let keep = index == 0 || matches!(&selected[part.parent], Some((_, true)));
        selected.push(keep.then(|| {
            let shows = slot.shows(part.name);
            let part = Part {
                name: part.name.to_owned(),
                pose: part.pose,
                polygons: if shows {
                    part.polygons.clone()
                } else {
                    Vec::new()
                },
                children: Vec::new(),
            };
            (part, shows)
        }));
    }

    // Assemble from the leaves up; reverse each sibling list to restore model order.
    for node in baked[1..].iter().rev() {
        if let Some((mut part, _)) = selected.pop().flatten() {
            part.children.reverse();
            let (parent, _) = selected[node.parent]
                .as_mut()
                .expect("a retained part has a retained parent");
            parent.children.push(part);
        }
    }
    let (mut root, _) = selected.pop().flatten().expect("bake retains the root");
    root.children.reverse();
    root
}

impl Game<'_> {
    pub(super) fn resolve_client_item_extensions(&mut self) {
        use mod_api::__private::game::RegistrationContextNew;
        self.armor_stand_assets.extensions.dispose_item_providers();
        let mut registration = Registration {
            state: ArmorExtensions::default(),
            sprite_exists: &|name| self.gui.atlas.sprites.contains_key(name),
            owners: &mut self.client_item_extension_owners,
        };
        // Prebind every default texture once, so frame callbacks receive only integer references.
        for name in self.gui.atlas.sprites.keys() {
            let _ = registration.register_texture(name);
        }
        Self::native_register_client_item_extensions(
            &mod_api::RegistrationContext::new(),
            &mut registration,
        );
        self.armor_stand_assets.extensions = registration.state;
    }
}

impl Game<'_> {
    pub(super) fn extended_equipment_materials<'a>(
        &'a self,
        piece: &super::equipment_piece::EquipmentPiece<'_>,
        stack: &sunrise_client::items::ItemStack,
        trim: Option<super::equipment_piece::MaterialBinding<'a>>,
        mut texture: impl FnMut(&str) -> Option<super::equipment_piece::MaterialBinding<'a>>,
        mut emissive: impl FnMut(&str) -> Option<super::equipment_piece::MaterialBinding<'a>>,
    ) -> Option<Vec<super::equipment_piece::EquipmentMaterial<'a>>> {
        use super::api_values::TryIntoApi;
        let state = &self.armor_stand_assets.extensions;
        if state.items.is_empty() {
            return None;
        }
        let item = mod_api::__private::item_key(&stack.item)?;
        let part = state.items.get(&item)?;
        if !part.owner.is_enabled() {
            return None;
        }
        let stack = match stack.try_into_api() {
            Ok(stack) => stack,
            Err(why) => {
                log::warn!(
                    "Sunrise: client item extensions for {}: invalid equipped stack: {why:?}",
                    part.owner.id()
                );
                return None;
            }
        };
        let bind = |name: &str| {
            self.gui.atlas.sprites.get_key_value(name).map(|(name, r)| {
                super::equipment_piece::MaterialBinding {
                    name,
                    uv: [r.u0, r.v0, r.u1, r.v1],
                }
            })
        };
        dispatch_equipment(
            self,
            state,
            part,
            &stack,
            piece,
            EquipmentBindings {
                trim,
                default: &mut texture,
                binding: &bind,
                emissive: &mut emissive,
            },
        )
    }
}

type Binding<'a> = super::equipment_piece::MaterialBinding<'a>;
struct EquipmentBindings<'a, 'call> {
    trim: Option<Binding<'a>>,
    default: &'call mut dyn FnMut(&str) -> Option<Binding<'a>>,
    binding: &'call dyn Fn(&str) -> Option<Binding<'a>>,
    emissive: &'call mut dyn FnMut(&str) -> Option<Binding<'a>>,
}
fn dispatch_equipment<'a>(
    host: &dyn mod_api::ClientHost,
    state: &'a ArmorExtensions,
    part: &ItemExtensionsPart,
    stack: &mod_api::ItemStack,
    piece: &super::equipment_piece::EquipmentPiece<'_>,
    lookups: EquipmentBindings<'a, '_>,
) -> Option<Vec<super::equipment_piece::EquipmentMaterial<'a>>> {
    if !part.owner.is_enabled() {
        return None;
    }
    use super::equipment_piece::ResolvedEquipmentLayer;
    use mod_api::{EquipmentLayer, EquipmentLayerType};
    let EquipmentBindings {
        trim,
        default: texture,
        binding: bind,
        emissive,
    } = lookups;
    let provider = &part.provider;
    let report_stale = |kind: &str| {
        log::warn!(
            "Sunrise: stale armour {kind} reference from {}; using the vanilla binding",
            part.owner.id()
        );
    };
    // A callback refusal or panic drops the whole result, even if an earlier layer succeeded.
    part.owner
        .call("client item extensions", "client_item_extensions", || {
            let dye = provider.default_dye_color(host, stack);
            let layer_type = match piece.kind {
                "humanoid_baby" => EquipmentLayerType::HumanoidBaby,
                "humanoid_leggings" => EquipmentLayerType::HumanoidLeggings,
                _ => EquipmentLayerType::Humanoid,
            };
            piece.resolved_materials(
                trim,
                |index, layer| {
                    let original_ref = state
                        .texture_keys
                        .get(&layer.texture)
                        .map_or(ArmorTextureRef::__new(0, 0), |&index| {
                            ArmorTextureRef::__new(state.generation, index)
                        });
                    let snapshot = EquipmentLayer::__new(
                        original_ref,
                        layer.dyeable,
                        layer.color_when_undyed,
                        layer.use_player_texture,
                    );
                    let color =
                        provider.armor_layer_tint_color(host, stack, &snapshot, index as u32, dye);
                    if color == 0 {
                        return None;
                    }
                    let default = texture(&layer.texture);
                    let default_ref = default
                        .and_then(|b| state.texture_keys.get(b.name))
                        .map_or(ArmorTextureRef::__new(0, 0), |&index| {
                            ArmorTextureRef::__new(state.generation, index)
                        });
                    let selected =
                        provider.armor_texture(host, stack, layer_type, &snapshot, default_ref);
                    let mut selected_texture_final = selected != default_ref;
                    let binding = if selected == default_ref {
                        default
                    } else {
                        let (generation, index) = selected.__parts();
                        let name = (generation == state.generation)
                            .then(|| state.textures.get(index as usize))
                            .flatten();
                        if let Some(name) = name {
                            bind(name)
                        } else {
                            report_stale("texture");
                            selected_texture_final = false;
                            default
                        }
                    }?;
                    let selected = provider.armor_layer_model(
                        host,
                        stack,
                        layer_type,
                        &snapshot,
                        index as u32,
                    );
                    let model = if selected == ArmorModelRef::VANILLA {
                        None
                    } else {
                        let (generation, index) = selected.__parts();
                        let model = (generation == state.generation)
                            .then(|| state.models.get(index as usize))
                            .flatten();
                        if let Some(model) = model {
                            Some(&model.slots[piece.slot.index()])
                        } else {
                            report_stale("model");
                            None
                        }
                    };
                    let color = (color & 0xFF00_FF00)
                        | ((color & 0x00FF_0000) >> 16)
                        | ((color & 0x0000_00FF) << 16);
                    Some(ResolvedEquipmentLayer {
                        binding,
                        color,
                        model,
                        final_texture: selected_texture_final,
                    })
                },
                emissive,
            )
        })
        .ok()
}

#[cfg(test)]
mod tests {
    use super::super::equipment_piece::{ArmorSlot, EquipmentAssetLayer, EquipmentPiece};
    use super::*;
    use mod_api::{
        ClientHost, EquipmentLayer, EquipmentLayerType, ModelPolygon, ModelVertex, PartPose,
    };
    struct Empty;
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Empty {}
    fn owner(id: &'static str) -> mod_api::content::crash::ModGuard {
        mod_api::content::crash::enter_native_mod(mod_api::content::crash::ModCall {
            id,
            hook: "register_client_item_extensions",
            part: None,
        })
    }
    fn tree(x: f32) -> ModelPart {
        let face = ModelPolygon::new(
            [
                ModelVertex::new([x, 0.0, 0.0], 0.0, 0.0),
                ModelVertex::new([x + 1.0, 0.0, 0.0], 1.0, 0.0),
                ModelVertex::new([x + 1.0, 1.0, 0.0], 1.0, 1.0),
                ModelVertex::new([x, 1.0, 0.0], 0.0, 1.0),
            ],
            [0.0, 0.0, 1.0],
        );
        ModelPart::new(
            Vec::new(),
            vec![(
                "body".into(),
                ModelPart::new(vec![face], Vec::new(), PartPose::ZERO),
            )],
            PartPose::ZERO,
        )
    }
    #[test]
    fn a_deep_armour_model_bakes_with_slot_visibility_intact() {
        let mut model = tree(0.0);
        for _ in 0..64 {
            model = ModelPart::new(Vec::new(), vec![("child".into(), model)], PartPose::ZERO);
        }
        let baked = bake_model("root", &model).unwrap();
        for (slot, faces) in [(ArmorSlot::Chest, 1), (ArmorSlot::Head, 0)] {
            let tree = hide_other_slots(&baked, slot);
            let mut part = &tree;
            for _ in 0..65 {
                assert_eq!(part.children.len(), 1);
                part = &part.children[0];
            }
            assert_eq!(part.name, "body");
            assert!(part.children.is_empty());
            assert_eq!(part.polygons.len(), faces);
        }
    }

    struct TwoShapes {
        models: [ArmorModelRef; 2],
        textures: [ArmorTextureRef; 2],
        original_first: ArmorTextureRef,
        calls: Arc<std::sync::Mutex<Vec<String>>>,
    }
    impl ClientItemExtensions for TwoShapes {
        fn default_dye_color(&self, _: &dyn ClientHost, _: &mod_api::ItemStackView<'_>) -> u32 {
            self.calls.lock().unwrap().push("dye".into());
            0
        }
        fn armor_layer_tint_color(
            &self,
            _: &dyn ClientHost,
            _: &mod_api::ItemStackView<'_>,
            _: &EquipmentLayer<'_>,
            index: u32,
            _: u32,
        ) -> u32 {
            self.calls.lock().unwrap().push(format!("tint{index}"));
            match index {
                0 => 0xFF12_3456,
                1 => 0xFFFF_FFFF,
                _ => 0,
            }
        }
        fn armor_texture(
            &self,
            _: &dyn ClientHost,
            _: &mod_api::ItemStackView<'_>,
            _: EquipmentLayerType,
            layer: &EquipmentLayer<'_>,
            _: &str,
        ) -> ArmorTextureRef {
            let index = usize::from(layer.texture != self.original_first);
            self.calls.lock().unwrap().push(format!("texture{index}"));
            self.textures[index]
        }
        fn armor_layer_model(
            &self,
            _: &dyn ClientHost,
            _: &mod_api::ItemStackView<'_>,
            _: EquipmentLayerType,
            _: &EquipmentLayer<'_>,
            index: u32,
        ) -> ArmorModelRef {
            self.calls.lock().unwrap().push(format!("model{index}"));
            self.models[index as usize]
        }
    }
    #[test]
    fn two_shapes_two_textures_tint_skip_and_one_trim_use_the_real_dispatch_boundary() {
        mod_platform::testing::with_mods(
            vec![mod_api::ModEntry::new("armor_test", Box::new(Empty))].into(),
            || {
                use mod_api::prelude::ItemsHost;
                let mut host = mod_api::testing::FakeHost::new();
                let id = mod_api::Identifier::parse("armor_test:chest").unwrap();
                host.registries.resolve_items([id.clone()]).unwrap();
                let item = host.item_key(&id).unwrap();
                let stack = mod_api::ItemStack::of(item);
                let mut owners = ItemExtensionOwners::default();
                let mut registry = Registration {
                    state: Default::default(),
                    sprite_exists: &|_| true,
                    owners: &mut owners,
                };
                let _owner = owner("armor_test");
                let models = [
                    registry
                        .register_model("armor_test:inner", &tree(0.0))
                        .unwrap(),
                    registry
                        .register_model("armor_test:outer", &tree(16.0))
                        .unwrap(),
                ];
                let original_first = registry.register_texture("inner").unwrap();
                registry.register_texture("outer").unwrap();
                let textures = [
                    registry.register_texture("custom_inner").unwrap(),
                    registry.register_texture("custom_outer").unwrap(),
                ];
                let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
                registry
                    .client_item_extensions(
                        item,
                        Arc::new(TwoShapes {
                            models,
                            textures,
                            original_first,
                            calls: calls.clone(),
                        }),
                    )
                    .unwrap();
                let layers = [
                    EquipmentAssetLayer {
                        texture: "inner".into(),
                        dyeable: false,
                        use_player_texture: false,
                        color_when_undyed: None,
                    },
                    EquipmentAssetLayer {
                        texture: "outer".into(),
                        dyeable: false,
                        use_player_texture: false,
                        color_when_undyed: None,
                    },
                    EquipmentAssetLayer {
                        texture: "skipped".into(),
                        dyeable: false,
                        use_player_texture: false,
                        color_when_undyed: None,
                    },
                ];
                let piece = EquipmentPiece {
                    asset: "armor_test:chest".into(),
                    slot: ArmorSlot::Chest,
                    kind: "humanoid",
                    layers: &layers,
                    overlay: 36,
                };
                let bind = |name: &str| match name {
                    "inner" => Some(Binding {
                        name: "inner",
                        uv: [0.75, 0.75, 1.0, 1.0],
                    }),
                    "outer" => Some(Binding {
                        name: "outer",
                        uv: [0.75, 0.75, 1.0, 1.0],
                    }),
                    "custom_inner" => Some(Binding {
                        name: "custom_inner",
                        uv: [0.0, 0.0, 0.25, 0.25],
                    }),
                    "custom_outer" => Some(Binding {
                        name: "custom_outer",
                        uv: [0.25, 0.25, 0.5, 0.5],
                    }),
                    _ => None,
                };
                let mut default = bind;
                let mut glow = |_: &str| None;
                let materials = dispatch_equipment(
                    &host,
                    &registry.state,
                    &registry.state.items[&item],
                    &mod_api::ItemStackView::from_stack(&stack),
                    &piece,
                    EquipmentBindings {
                        trim: Some(Binding {
                            name: "trim",
                            uv: [0.5, 0.5, 1.0, 1.0],
                        }),
                        default: &mut default,
                        binding: &bind,
                        emissive: &mut glow,
                    },
                )
                .unwrap();
                assert_eq!(
                    *calls.lock().unwrap(),
                    [
                        "dye", "tint0", "texture0", "model0", "tint1", "texture1", "model1",
                        "tint2"
                    ]
                );
                assert_eq!(
                    materials
                        .iter()
                        .map(|m| (m.binding.name, m.overlay))
                        .collect::<Vec<_>>(),
                    [("custom_inner", 0), ("custom_outer", 0), ("trim", 36)]
                );
                assert_eq!(
                    materials[0].color, 0xFF56_3412,
                    "ARGB becomes renderer ABGR"
                );
                assert!(materials[0].final_texture && materials[1].final_texture);
                let selected = materials
                    .iter()
                    .map(|m| m.model.unwrap())
                    .collect::<Vec<_>>();
                assert!(Arc::ptr_eq(selected[0], &registry.state.models[0].slots[1]));
                assert!(Arc::ptr_eq(selected[1], &registry.state.models[1].slots[1]));
                assert!(
                    Arc::ptr_eq(selected[2], selected[1]),
                    "one trim on the last material's tree"
                );
                let mut vertices = Vec::new();
                for m in materials {
                    let model = m.model.unwrap();
                    sunrise_client::mobs::render::draw(
                        model,
                        glam::Mat4::IDENTITY,
                        &sunrise_client::mobs::anim::Poses::new(model),
                        m.binding.uv,
                        m.color,
                        0xF0F0,
                        m.overlay,
                        &mut vertices,
                    );
                }
                assert_eq!(
                    vertices.len(),
                    12,
                    "one face per material and one trim, no double painting"
                );
                assert_eq!(vertices[0].pos[0], 0.0);
                assert_eq!(vertices[4].pos[0], 1.0);
                assert_eq!(vertices[8].pos[0], 1.0);
                assert_eq!(vertices[0].uv, [0.0, 0.0]);
                assert_eq!(vertices[4].uv, [0.25, 0.25]);
                assert_eq!(vertices[8].uv, [0.5, 0.5]);
            },
        );
    }
    #[test]
    fn model_registration_accepts_other_namespaces() {
        mod_platform::testing::with_mods(
            vec![mod_api::ModEntry::new("armor_test", Box::new(Empty))].into(),
            || {
                let mut owners = ItemExtensionOwners::default();
                let mut registry = Registration {
                    state: Default::default(),
                    sprite_exists: &|_| false,
                    owners: &mut owners,
                };
                let _owner = owner("armor_test");
                registry.register_model("other:chest", &tree(0.0)).unwrap();
                assert_eq!(registry.state.models[0].name, "other:chest");
            },
        );
    }

    struct DefaultProvider;
    impl ClientItemExtensions for DefaultProvider {}

    #[test]
    #[ignore = "daily"]
    fn invalid_registrations_do_not_discard_valid_reload_entries() {
        use mod_api::{Hooks, RegisterClientItemExtensionsEvent, RegistrationContext};
        use std::sync::Mutex;

        #[derive(Default)]
        struct Results {
            accepted_model: Option<ArmorModelRef>,
            errors: Vec<String>,
        }
        struct Registers(Arc<Mutex<Results>>);
        #[mod_api::__subscriptions]
        impl Hooks for Registers {
            fn register_client_item_extensions(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterClientItemExtensionsEvent<'_>,
            ) {
                let registrar = &mut event.extensions;
                let model = registrar
                    .register_model("armor_test:valid", &tree(0.0))
                    .unwrap();
                let mut invalid = tree(0.0);
                invalid.x = f32::NAN;
                let errors = vec![
                    registrar
                        .register_model("armor_test:invalid", &invalid)
                        .unwrap_err(),
                    registrar
                        .register_model("armor_test:valid", &tree(0.0))
                        .unwrap_err(),
                    registrar
                        .register_texture("armor_test:missing")
                        .unwrap_err(),
                ];
                let item = mod_api::__private::item_key("minecraft:iron_chestplate").unwrap();
                registrar
                    .client_item_extensions(item, Arc::new(DefaultProvider))
                    .unwrap();
                *self.0.lock().unwrap() = Results {
                    accepted_model: Some(model),
                    errors,
                };
            }
        }
        struct Duplicate(Arc<Mutex<Results>>);
        #[mod_api::__subscriptions]
        impl Hooks for Duplicate {
            fn register_client_item_extensions(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterClientItemExtensionsEvent<'_>,
            ) {
                let item = mod_api::__private::item_key("minecraft:iron_chestplate").unwrap();
                let error = event
                    .extensions
                    .client_item_extensions(item, Arc::new(DefaultProvider))
                    .unwrap_err();
                self.0.lock().unwrap().errors.push(error);
            }
        }

        let results = Arc::new(Mutex::new(Results::default()));
        mod_platform::testing::with_mods(
            vec![
                mod_api::ModEntry::new("armor_test", Box::new(Registers(results.clone()))),
                mod_api::ModEntry::new("other", Box::new(Duplicate(results.clone()))),
            ]
            .into(),
            || {
                crate::game::native_entity_refs::tests::with_client_game(|game| {
                    game.resolve_client_item_extensions();

                    let results = results.lock().unwrap();
                    assert_eq!(
                        results.errors,
                        [
                            "armour model armor_test:invalid: non-finite model pose",
                            "duplicate armour model armor_test:valid",
                            "unknown armour texture armor_test:missing",
                            "duplicate client item extensions for minecraft:iron_chestplate: armor_test and other",
                        ]
                    );
                    let extensions = &game.armor_stand_assets.extensions;
                    let (generation, index) = results.accepted_model.unwrap().__parts();
                    assert_eq!(extensions.generation, generation);
                    assert_eq!(extensions.models[index as usize].name, "armor_test:valid");
                    let item = mod_api::__private::item_key("minecraft:iron_chestplate").unwrap();
                    assert_eq!(extensions.items[&item].owner.id(), "armor_test");
                });
            },
        );
    }

    #[test]
    fn stale_refs_fall_back_to_vanilla_bindings_without_disabling_the_provider() {
        mod_platform::testing::with_mods(
            vec![mod_api::ModEntry::new("armor_test", Box::new(Empty))].into(),
            || {
                use mod_api::prelude::ItemsHost;
                struct Stale(u64);
                impl ClientItemExtensions for Stale {
                    fn armor_texture(
                        &self,
                        _: &dyn ClientHost,
                        _: &mod_api::ItemStackView<'_>,
                        _: EquipmentLayerType,
                        _: &EquipmentLayer<'_>,
                        _: &str,
                    ) -> ArmorTextureRef {
                        ArmorTextureRef::__new(self.0, 0)
                    }
                    fn armor_layer_model(
                        &self,
                        _: &dyn ClientHost,
                        _: &mod_api::ItemStackView<'_>,
                        _: EquipmentLayerType,
                        _: &EquipmentLayer<'_>,
                        _: u32,
                    ) -> ArmorModelRef {
                        ArmorModelRef::__new(self.0, 0)
                    }
                }
                let mut host = mod_api::testing::FakeHost::new();
                let id = mod_api::Identifier::parse("armor_test:chest").unwrap();
                host.registries.resolve_items([id.clone()]).unwrap();
                let item = host.item_key(&id).unwrap();
                let old = ArmorExtensions::default();
                let mut owners = ItemExtensionOwners::default();
                let mut registry = Registration {
                    state: Default::default(),
                    sprite_exists: &|_| true,
                    owners: &mut owners,
                };
                let _owner = owner("armor_test");
                registry.register_texture("original").unwrap();
                registry
                    .client_item_extensions(item, Arc::new(Stale(old.generation)))
                    .unwrap();
                let layers = [EquipmentAssetLayer {
                    texture: "original".into(),
                    dyeable: false,
                    use_player_texture: false,
                    color_when_undyed: None,
                }];
                let piece = EquipmentPiece {
                    asset: "armor_test:chest".into(),
                    slot: ArmorSlot::Chest,
                    kind: "humanoid",
                    layers: &layers,
                    overlay: 0,
                };
                let binding = Binding {
                    name: "original",
                    uv: [0.0, 0.0, 1.0, 1.0],
                };
                let part = &registry.state.items[&item];
                for _ in 0..2 {
                    let mut default = |_: &str| Some(binding);
                    let bind = |_: &str| Some(binding);
                    let mut glow = |_: &str| None;
                    let out = dispatch_equipment(
                        &host,
                        &registry.state,
                        part,
                        &mod_api::ItemStackView::from_stack(&mod_api::ItemStack::of(item)),
                        &piece,
                        EquipmentBindings {
                            trim: None,
                            default: &mut default,
                            binding: &bind,
                            emissive: &mut glow,
                        },
                    )
                    .unwrap();
                    assert_eq!(out.len(), 1);
                    assert_eq!(out[0].binding.name, "original");
                    assert!(out[0].model.is_none());
                    assert!(!out[0].final_texture);
                }
                assert!(part.owner.is_enabled());
            },
        );
    }
    #[test]
    fn callback_panic_discards_the_piece_and_disables_only_its_mod_and_item_across_reloads() {
        mod_platform::testing::with_mods(
            vec![
                mod_api::ModEntry::new("armor_test", Box::new(Empty)),
                mod_api::ModEntry::new("other", Box::new(Empty)),
            ]
            .into(),
            || {
                use mod_api::prelude::ItemsHost;
                struct FailsOnSecond(Arc<std::sync::atomic::AtomicUsize>);
                impl ClientItemExtensions for FailsOnSecond {
                    fn armor_layer_tint_color(
                        &self,
                        _: &dyn ClientHost,
                        _: &mod_api::ItemStackView<'_>,
                        _: &EquipmentLayer<'_>,
                        index: u32,
                        _: u32,
                    ) -> u32 {
                        self.0.fetch_add(1, Ordering::Relaxed);
                        assert_ne!(index, 1, "provider panic");
                        0xFFFFFFFF
                    }
                }
                let mut host = mod_api::testing::FakeHost::new();
                let id = mod_api::Identifier::parse("armor_test:chest").unwrap();
                let other_id = mod_api::Identifier::parse("armor_test:helmet").unwrap();
                host.registries
                    .resolve_items([id.clone(), other_id.clone()])
                    .unwrap();
                let item = host.item_key(&id).unwrap();
                let other_item = host.item_key(&other_id).unwrap();
                let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let layers = std::array::from_fn::<_, 2, _>(|_| EquipmentAssetLayer {
                    texture: "original".into(),
                    dyeable: false,
                    use_player_texture: false,
                    color_when_undyed: None,
                });
                let piece = EquipmentPiece {
                    asset: "armor_test:chest".into(),
                    slot: ArmorSlot::Chest,
                    kind: "humanoid",
                    layers: &layers,
                    overlay: 0,
                };
                let binding = Binding {
                    name: "original",
                    uv: [0.0, 0.0, 1.0, 1.0],
                };
                let mut owners = ItemExtensionOwners::default();
                for mod_id in ["armor_test", "armor_test", "other"] {
                    let mut registry = Registration {
                        state: Default::default(),
                        sprite_exists: &|_| true,
                        owners: &mut owners,
                    };
                    registry.register_texture("original").unwrap();
                    {
                        let _owner = owner("armor_test");
                        registry
                            .client_item_extensions(other_item, Arc::new(DefaultProvider))
                            .unwrap();
                    }
                    {
                        let _owner = owner(mod_id);
                        let provider: Arc<dyn ClientItemExtensions> = if mod_id == "armor_test" {
                            Arc::new(FailsOnSecond(calls.clone()))
                        } else {
                            Arc::new(DefaultProvider)
                        };
                        registry.client_item_extensions(item, provider).unwrap();
                    }
                    let draw = |item| {
                        dispatch_equipment(
                            &host,
                            &registry.state,
                            &registry.state.items[&item],
                            &mod_api::ItemStackView::from_stack(&mod_api::ItemStack::of(item)),
                            &piece,
                            EquipmentBindings {
                                trim: None,
                                default: &mut |_| Some(binding),
                                binding: &|_| Some(binding),
                                emissive: &mut |_| None,
                            },
                        )
                    };
                    if mod_id == "armor_test" {
                        assert!(draw(item).is_none(), "a panic discards even earlier layers");
                        assert!(draw(item).is_none(), "the callback stays disabled");
                    } else {
                        assert_eq!(draw(item).unwrap().len(), 2, "another mod is unaffected");
                    }
                    assert_eq!(
                        draw(other_item).unwrap().len(),
                        2,
                        "another item is unaffected"
                    );
                    assert_eq!(
                        calls.load(Ordering::Relaxed),
                        2,
                        "only the first draw entered the panicking provider"
                    );
                }
            },
        );
    }

    /// Asks the Host for a level in its tint callback.
    struct Gated(Arc<std::sync::atomic::AtomicUsize>);
    impl ClientItemExtensions for Gated {
        fn armor_layer_tint_color(
            &self,
            host: &dyn ClientHost,
            _: &mod_api::ItemStackView<'_>,
            _: &EquipmentLayer<'_>,
            _: u32,
            _: u32,
        ) -> u32 {
            let _ = host.level("minecraft:overworld");
            self.0.fetch_add(1, Ordering::Relaxed);
            0xFFFF_FFFF
        }
    }

    /// A world read during Setup or Reloading refuses the current piece without poisoning the
    /// provider. The same registered callback can answer a subsequent draw in World.
    #[test]
    #[ignore = "daily"]
    fn client_item_extensions_outside_the_world_are_refused_not_a_panic() {
        use mod_api::__private::{ModPhase, PhaseOwner};
        mod_platform::testing::with_mods(
            vec![mod_api::ModEntry::new("gated", Box::new(Empty))].into(),
            || {
                crate::game::native_entity_refs::tests::with_client_game(|game| {
                    let item = mod_api::__private::item_key("minecraft:iron_chestplate").unwrap();
                    let layers = [EquipmentAssetLayer {
                        texture: "original".into(),
                        dyeable: false,
                        use_player_texture: false,
                        color_when_undyed: None,
                    }];
                    let piece = EquipmentPiece {
                        asset: "minecraft:iron".into(),
                        slot: ArmorSlot::Chest,
                        kind: "humanoid",
                        layers: &layers,
                        overlay: 0,
                    };
                    let binding = Binding {
                        name: "original",
                        uv: [0.0, 0.0, 1.0, 1.0],
                    };
                    let answered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    let mut owners = ItemExtensionOwners::default();
                    let mut registry = Registration {
                        state: Default::default(),
                        sprite_exists: &|_| true,
                        owners: &mut owners,
                    };
                    {
                        let _owner = owner("gated");
                        registry.register_texture("original").unwrap();
                        registry
                            .client_item_extensions(item, Arc::new(Gated(answered.clone())))
                            .unwrap();
                    }
                    let part = &registry.state.items[&item];
                    let draw = || {
                        dispatch_equipment(
                            &*game,
                            &registry.state,
                            part,
                            &mod_api::ItemStackView::from_stack(&mod_api::ItemStack::of(item)),
                            &piece,
                            EquipmentBindings {
                                trim: None,
                                default: &mut |_| Some(binding),
                                binding: &|_| Some(binding),
                                emissive: &mut |_| None,
                            },
                        )
                    };
                    {
                        let setup = PhaseOwner::scope(ModPhase::Setup);
                        assert!(draw().is_none(), "Setup refusal selects the vanilla draw");
                        let _reload = setup.reloading().unwrap();
                        assert!(
                            draw().is_none(),
                            "Reloading refusal selects the vanilla draw"
                        );
                        assert_eq!(
                            answered.load(Ordering::Relaxed),
                            0,
                            "both reads were refused"
                        );
                    }
                    let _world = PhaseOwner::scope(ModPhase::World);
                    assert_eq!(
                        draw().unwrap().len(),
                        1,
                        "the same provider still draws in World"
                    );
                    assert_eq!(answered.load(Ordering::Relaxed), 1);
                });
            },
        );
    }
}
