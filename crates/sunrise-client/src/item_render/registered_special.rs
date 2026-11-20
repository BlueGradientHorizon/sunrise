//! Preparation and expansion of registered special item renderers' recipes.
use super::{ItemRenderState, ItemRenderer, LayerGeometry, LayerRender, glam_matrix, sprite_name};
use glam::Vec3;
use std::sync::{Arc, Mutex};
use sunrise_assets::{
    item_bake::BakedItemModel,
    item_model::{
        DisplayContext, FoilType, ItemEnv, LayerTransform, LevelState, OwnerState, PreparedParts,
        SpecialContent, StackView,
    },
    resolve::MISSING_MODEL,
};

impl ItemRenderer {
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
    /// The layers of a prepared answer (`LayerContent::Parts`), one for each model part and the
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
