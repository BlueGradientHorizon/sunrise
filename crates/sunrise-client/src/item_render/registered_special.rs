//! Baking special model nodes at load and preparing their recipes at draw time.
use super::{
    ItemRenderState, ItemRenderer, LayerGeometry, LayerRender, corners, glam_matrix, sprite_name,
};
use glam::Vec3;
use mod_api::{
    SpecialModelBakeContext, SpecialModelConfig, SpecialModelFactory, SpecialModelGeometry,
    SpecialModelPolicy, SpecialModelQuad, SpecialModelRenderer,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, OnceLock, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use sunrise_assets::{
    DecodeError,
    bake::{ModelQuad, facing},
    item_bake::{BakedItemModel, MISSING_SPRITE, SpriteLookup, bake_resolved},
    item_model::{
        ClientItem, DisplayContext, FoilType, ItemEnv, LayerTransform, LevelState, OwnerState,
        PreparedParts, RegisteredSpecialModel, SpecialContent, StackView, identifier,
    },
    model::Material,
    resolve::MISSING_MODEL,
};

pub type SpecialModelFactories = BTreeMap<String, (SpecialModelPolicy, SpecialModelFactory)>;

pub(super) static FACTORIES: OnceLock<SpecialModelFactories> = OnceLock::new();

/// Install the winning factories before the first client resource load. A type may replace a
/// builtin, but must be a fully qualified resource identifier.
pub fn install_special_model_factories(mut factories: SpecialModelFactories) -> bool {
    factories.retain(|kind, _| {
        let valid = identifier(kind).is_ok_and(|canonical| canonical == kind.as_str());
        if !valid {
            log::warn!("special model type {kind:?} is not <namespace>:<path>");
        }
        valid
    });
    FACTORIES.set(factories).is_ok()
}

/// One baked `minecraft:special` node. Every resource load owns its own instances and failures.
pub struct BakedSpecialModel {
    /// A unique answer-cache key, even when a node's storage is reused after a reload.
    pub node: usize,
    pub kind: Box<str>,
    pub policy: SpecialModelPolicy,
    pub renderer: Arc<dyn SpecialModelRenderer>,
    /// A draw failure disables this type in this resource load only.
    pub failed: Arc<AtomicBool>,
}

/// The item geometry and extents retained by an API lease from this bake context.
pub struct BakedSpecialGeometry {
    pub model: Arc<BakedItemModel>,
    pub extents: Arc<Vec<Vec3>>,
}

impl BakedSpecialGeometry {
    fn lease(model: BakedItemModel) -> SpecialModelGeometry {
        SpecialModelGeometry::new(Self {
            extents: Arc::new(corners(&model)),
            model: Arc::new(model),
        })
    }
}

struct SpecialBaker<'a>(&'a ItemRenderer);

impl SpecialModelBakeContext for SpecialBaker<'_> {
    fn bake_model(&self, model: &str) -> Result<SpecialModelGeometry, String> {
        let model = self
            .0
            .library
            .resolve_required(model)
            .map_err(|error| error.to_string())?;
        let model = bake_resolved(&model, &self.0.sprites).map_err(|error| error.to_string())?;
        Ok(BakedSpecialGeometry::lease(model))
    }

    fn bake_geometry(&self, quads: &[SpecialModelQuad]) -> Result<SpecialModelGeometry, String> {
        let mut baked = Vec::with_capacity(quads.len());
        for (index, quad) in quads.iter().enumerate() {
            if quad.positions.iter().flatten().any(|v| !v.is_finite()) {
                return Err(format!("quad {index}: non-finite position"));
            }
            let facing =
                facing(&quad.positions).ok_or_else(|| format!("quad {index}: invalid facing"))?;
            if quad.uv.iter().flatten().any(|v| !(0.0..=1.0).contains(v)) {
                return Err(format!("quad {index}: UV is outside [0, 1]"));
            }
            if quad.light_emission > 15 {
                return Err(format!("quad {index}: light emission is over 15"));
            }
            let tint_index = quad
                .tint_index
                .map_or(Ok(-1), i32::try_from)
                .map_err(|_| format!("quad {index}: tint index is out of range"))?;
            let sprite =
                identifier(&quad.sprite).map_err(|error| format!("quad {index}: {error}"))?;
            if sprite != MISSING_SPRITE && self.0.sprites.mask(&sprite).is_none() {
                return Err(format!("quad {index}: sprite {sprite} is missing"));
            }
            baked.push(ModelQuad {
                positions: quad.positions,
                uv: quad.uv,
                facing,
                cull: None,
                material: Material {
                    sprite,
                    force_translucent: quad.force_translucent,
                },
                tint_index,
                shade_direction_override: None,
                light_emission: quad.light_emission,
                light: [quad.light_emission, 0],
                color: None,
                ambient_occlusion: None,
                render_type: None,
                shade: true,
            });
        }
        let model = BakedItemModel {
            quads: baked,
            uses_block_light: false,
            particle: Material {
                sprite: MISSING_SPRITE.to_owned(),
                force_translucent: false,
            },
            transforms: Default::default(),
        };
        Ok(BakedSpecialGeometry::lease(model))
    }
}

impl ItemRenderer {
    /// Decode an item definition, baking its registered special nodes against this renderer's
    /// selected models and sprites. Any failed decode or bake refuses the enclosing definition.
    pub fn decode_definition(
        &self,
        text: &str,
        factories: Option<&SpecialModelFactories>,
    ) -> Result<ClientItem, DecodeError> {
        static NEXT_NODE: AtomicUsize = AtomicUsize::new(0);
        ClientItem::from_json_with_specials(text, &|kind, _, fields| {
            let Some((policy, factory)) = factories.and_then(|factories| factories.get(kind))
            else {
                return Ok(None);
            };
            let renderer = factory
                .bake(SpecialModelConfig::new(fields), &SpecialBaker(self))
                .map_err(|error| format!("special model {kind}: {error}"))?;
            let mut failures = self
                .special_failures
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let failed = failures.entry(kind.to_owned()).or_default().clone();
            Ok(Some(RegisteredSpecialModel(Arc::new(BakedSpecialModel {
                node: NEXT_NODE.fetch_add(1, Ordering::Relaxed),
                kind: kind.into(),
                policy: *policy,
                renderer,
                failed,
            }))))
        })
    }

    /// A special model renderer's answer `parts` for a model node of `base`, prepared to draw:
    /// `base` and each part's model baked, once, when the answer is accepted, so a draw looks
    /// nothing up by name. A part naming a model the packs don't have refuses them all, and the
    /// layer draws the missing model.
    pub fn prepare_parts(
        &self,
        base: &str,
        parts: &[sunrise_assets::item_model::SpecialPart],
    ) -> PreparedParts {
        let part = |model: &str, tints, pose| {
            let geometry = self.bake(model);
            PreparedPart {
                content: PreparedContent::Model {
                    extents: self.quad_extents(model, &geometry),
                    geometry,
                    tints,
                },
                pose,
            }
        };
        let unknown = parts.iter().find_map(|p| match &p.content {
            SpecialContent::Model(model) if !self.library.resolves(model) => Some(model),
            _ => None,
        });
        let parts = match unknown {
            Some(unknown) => {
                warn_unknown_part_once(unknown);
                vec![part(
                    MISSING_MODEL,
                    Vec::new(),
                    sunrise_assets::math::Mat4::IDENTITY,
                )]
            }
            None => parts
                .iter()
                .map(|p| match &p.content {
                    SpecialContent::Model(model) => part(model, p.tints.clone(), p.pose),
                    SpecialContent::Stack { stack, display } => PreparedPart {
                        content: PreparedContent::Stack {
                            stack: stack.clone(),
                            display: *display,
                        },
                        pose: p.pose,
                    },
                })
                .collect(),
        };
        PreparedParts(Arc::new(PreparedSpecial {
            base: self.bake(base),
            parts,
        }))
    }
    /// The layers of a prepared answer, one for each model part and the
    /// layers of each stack part's own resolve: the part's own model's quads, posed under the
    /// layer's transform, with the base model's display transform, particle and GUI light. A
    /// stack part's layers keep their own foil, tints and display transform, and sit under the
    /// part's pose. Returns the state's animation flag, or `None` (with `layers` as they were)
    /// when a stack part is refused: it meets a stack that is already being expanded above it, or
    /// the draw has expanded [`mod_api::MAX_NESTED_STACK_PARTS`].
    pub(super) fn push_parts(
        &self,
        layers: &mut Vec<LayerRender>,
        display: DisplayContext,
        (foil, transform, display_transform): (
            FoilType,
            sunrise_assets::math::Mat4,
            LayerTransform,
        ),
        prepared: &PreparedParts,
        (outer, owner, level, seed, env): (
            &StackView,
            Option<&OwnerState>,
            Option<&LevelState>,
            i32,
            &dyn ItemEnv,
        ),
    ) -> Option<bool> {
        let Some(prepared) = prepared.0.downcast_ref::<PreparedSpecial>() else {
            // Only `prepare_parts` makes them.
            return Some(false);
        };
        let base = &prepared.base;
        let start = layers.len();
        let mut animated = false;
        for part in &prepared.parts {
            // Under `applyTransform`'s half-block offset, so a part's pose is about the model's
            // corner, as its elements are.
            let matrix = glam_matrix(&base.layer_matrix(
                display_transform,
                display,
                &transform.mul(&part.pose),
            ));
            match &part.content {
                PreparedContent::Model {
                    geometry,
                    extents,
                    tints,
                } => layers.push(LayerRender {
                    geometry: LayerGeometry::Quads(geometry.clone()),
                    tints: tints.clone(),
                    foil,
                    matrix,
                    uses_block_light: base.uses_block_light,
                    particle: sprite_name(&base.particle.sprite).to_owned(),
                    extents: extents.clone(),
                    display,
                }),
                PreparedContent::Stack { stack, display } => {
                    let Some(inner) =
                        self.expand_stack(outer, stack, *display, (owner, level, seed, env))
                    else {
                        layers.truncate(start);
                        return None;
                    };
                    animated |= inner.animated;
                    layers.extend(inner.layers.into_iter().map(|mut layer| {
                        layer.matrix = matrix * layer.matrix;
                        layer
                    }));
                }
            }
        }
        Some(animated)
    }
    /// The resolve of the stack part `stack` of an answer drawn for `outer`, or `None` when it is
    /// refused (see [`Self::push_parts`]).
    fn expand_stack(
        &self,
        outer: &StackView,
        stack: &StackView,
        display: DisplayContext,
        (owner, level, seed, env): (Option<&OwnerState>, Option<&LevelState>, i32, &dyn ItemEnv),
    ) -> Option<ItemRenderState> {
        let pushed = NESTING.with_borrow_mut(|n| {
            if n.expanded >= mod_api::MAX_NESTED_STACK_PARTS
                || n.chain.contains(stack)
                || (n.chain.is_empty() && outer == stack)
            {
                return None;
            }
            n.expanded += 1;
            let root = n.chain.is_empty();
            if root {
                n.chain.push(outer.clone());
            }
            n.chain.push(stack.clone());
            Some(root)
        });
        let Some(root) = pushed else {
            warn_refused_nesting_once(&stack.item);
            return None;
        };
        let state = self.resolve_with_rule(stack, display, owner, level, seed, env, None);
        let refused = NESTING.with_borrow_mut(|n| {
            n.chain.pop();
            if root {
                n.chain.pop();
            }
            n.refused
        });
        (!refused).then_some(state)
    }
}

/// A special model renderer's answer as [`ItemRenderer::prepare_parts`] baked it.
struct PreparedSpecial {
    /// The model node's `base`: its display transforms, particle and GUI light.
    base: Arc<BakedItemModel>,
    parts: Vec<PreparedPart>,
}
struct PreparedPart {
    content: PreparedContent,
    pose: sunrise_assets::math::Mat4,
}
enum PreparedContent {
    /// A model, baked once.
    Model {
        geometry: Arc<BakedItemModel>,
        extents: Arc<Vec<Vec3>>,
        tints: Vec<i32>,
    },
    /// A stack, resolved when the item is drawn, so what it draws moves with its own models.
    Stack {
        stack: StackView,
        display: DisplayContext,
    },
}
/// The stacks being expanded by this thread's draw, outermost first, and how many stack parts
/// the draw has expanded: a stack met again while it is expanding is a cycle, and a draw expands
/// at most [`mod_api::MAX_NESTED_STACK_PARTS`], both refusing the whole answer.
#[derive(Default)]
pub(super) struct Nesting {
    chain: Vec<StackView>,
    expanded: usize,
    /// Resolves in progress on this thread; the budget starts again when the outermost ends.
    pub(super) depth: u32,
    /// Set by a nested resolve that met a refused stack part, until the outermost answer
    /// takes it: the refusal reaches every answer above it.
    pub(super) refused: bool,
}
thread_local! {
    pub(super) static NESTING: std::cell::RefCell<Nesting> = std::cell::RefCell::default();
}
/// Counts a resolve of this thread in progress; the budget resets when the last one ends.
pub(super) struct ResolveDepth;
impl ResolveDepth {
    pub(super) fn enter() -> Self {
        NESTING.with_borrow_mut(|n| n.depth += 1);
        Self
    }
}
impl Drop for ResolveDepth {
    fn drop(&mut self) {
        NESTING.with_borrow_mut(|n| {
            n.depth -= 1;
            if n.depth == 0 {
                n.expanded = 0;
            }
        });
    }
}
/// Says once per model id that a mod's special model renderer drew a model the packs don't have.
fn warn_unknown_part_once(model: &str) {
    static SEEN: Mutex<std::collections::BTreeSet<String>> =
        Mutex::new(std::collections::BTreeSet::new());
    if SEEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(model.to_owned())
    {
        log::warn!(
            "Sunrise: a special model renderer drew the model {model}, which no pack has; the item draws the missing model"
        );
    }
}
/// Says once per item that a special model renderer's stack part was refused as a cycle or past
/// the budget.
fn warn_refused_nesting_once(item: &str) {
    static SEEN: Mutex<std::collections::BTreeSet<String>> =
        Mutex::new(std::collections::BTreeSet::new());
    if SEEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(item.to_owned())
    {
        log::warn!(
            "Sunrise: a special model renderer's stack part of {item} is a cycle or past the nesting budget; the item draws the missing model"
        );
    }
}
