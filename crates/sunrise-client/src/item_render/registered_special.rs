//! Baking special model nodes at load and writing their layers atomically at draw time.
use super::{
    ItemRenderer, LayerGeometry, LayerRender, LayerTints, corners, glam_matrix, sprite_name,
};
use glam::{Mat4, Vec3};
use mod_api::{
    SpecialModelBakeContext, SpecialModelConfig, SpecialModelFactory, SpecialModelGeometry,
    SpecialModelPolicy, SpecialModelQuad, SpecialModelRenderer,
};
use std::{
    collections::BTreeMap,
    ops::ControlFlow,
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
        ClientItem, Context, DisplayContext, FoilType, ItemEnv, LayerTransform, PreparedParts,
        RegisteredSpecialModel, SpecialContent, SpecialPart, StackView, identifier,
    },
    math::Mat4 as AssetMatrix,
    model::Material,
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
    base: SpecialLayerBase,
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
        ClientItem::from_json_with_specials(text, &|kind, base, fields| {
            let Some((policy, factory)) = factories.and_then(|factories| factories.get(kind))
            else {
                return Ok(None);
            };
            let renderer = factory
                .bake(SpecialModelConfig::new(fields), &SpecialBaker(self))
                .map_err(|error| format!("special model {kind}: {error}"))?;
            let base = self.bake(base);
            let particle = Arc::from(sprite_name(&base.particle.sprite));
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
                base: SpecialLayerBase {
                    model: base,
                    particle,
                },
                failed,
            }))))
        })
    }

    /// Bake an answer's named geometry once. An unavailable model refuses the answer.
    pub fn prepare_parts(
        &self,
        model: &BakedSpecialModel,
        parts: &[SpecialPart],
    ) -> Option<PreparedParts> {
        let parts = parts
            .iter()
            .map(|part| {
                let content = match &part.content {
                    SpecialContent::Model(id) => {
                        if !self.library.resolves(id) {
                            warn_unknown_part_once(id);
                            return None;
                        }
                        let baked = self.bake(id);
                        PreparedContent::Model {
                            geometry: BakedSpecialGeometry {
                                extents: self.quad_extents(id, &baked),
                                model: baked,
                            },
                            tints: part.tints.as_slice().into(),
                        }
                    }
                    SpecialContent::Stack { stack, display } => PreparedContent::Stack {
                        stack: stack.clone(),
                        display: *display,
                    },
                };
                Some(PreparedPart {
                    content,
                    pose: part.pose,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(PreparedParts(Arc::new(PreparedSpecial {
            base: model.base.clone(),
            parts,
        })))
    }

    /// Submit the prepared answer through the same destination as ordinary item layers.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn push_parts(
        &self,
        layers: &mut Vec<LayerRender>,
        prepared: &PreparedParts,
        (foil, outer_transform, display_transform): (FoilType, AssetMatrix, LayerTransform),
        ctx: &Context<'_>,
        env: &dyn ItemEnv,
        ancestor: &StackAncestor<'_>,
        remaining: &mut usize,
    ) -> ControlFlow<()> {
        let Some(prepared) = prepared.0.downcast_ref::<PreparedSpecial>() else {
            return ControlFlow::Break(());
        };
        let mut writer = LayerWriter {
            checkpoint: layers.len(),
            layers,
            renderer: self,
            base: &prepared.base,
            ctx,
            env,
            ancestor,
            remaining,
            foil,
            outer_transform,
            display_transform,
            local_pose: AssetMatrix::IDENTITY,
            refused: false,
            committed: false,
        };
        for part in &prepared.parts {
            writer.local_pose = part.pose;
            match &part.content {
                PreparedContent::Model { geometry, tints } => {
                    writer.submit_geometry(geometry, tints)
                }
                // The answer is atomic even when the caller does not look at this return value.
                PreparedContent::Stack { stack, display } => {
                    let _ = writer.submit_stack(stack, *display);
                }
            }
        }
        writer.commit()
    }
}

/// The base draw properties shared by all answers for one baked node.
#[derive(Clone)]
struct SpecialLayerBase {
    model: Arc<BakedItemModel>,
    particle: Arc<str>,
}

/// A special model renderer's answer as [`ItemRenderer::prepare_parts`] baked it.
struct PreparedSpecial {
    base: SpecialLayerBase,
    parts: Vec<PreparedPart>,
}
struct PreparedPart {
    content: PreparedContent,
    pose: AssetMatrix,
}
enum PreparedContent {
    Model {
        geometry: BakedSpecialGeometry,
        tints: Arc<[i32]>,
    },
    /// A stack, resolved when the item is drawn, so what it draws moves with its own models.
    Stack {
        stack: StackView,
        display: DisplayContext,
    },
}

/// The active stack path, borrowed from the resolvers and their callers, with no stack copies.
pub(super) struct StackAncestor<'a> {
    pub(super) stack: &'a StackView,
    pub(super) parent: Option<&'a StackAncestor<'a>>,
}

/// A single special layer's writes directly into the item state. A refusal is sticky, and
/// dropping without committing rolls the whole layer back, including on an unwind.
struct LayerWriter<'out, 'draw> {
    layers: &'out mut Vec<LayerRender>,
    checkpoint: usize,
    renderer: &'draw ItemRenderer,
    base: &'draw SpecialLayerBase,
    ctx: &'draw Context<'draw>,
    env: &'draw dyn ItemEnv,
    ancestor: &'draw StackAncestor<'draw>,
    remaining: &'out mut usize,
    foil: FoilType,
    outer_transform: AssetMatrix,
    display_transform: LayerTransform,
    local_pose: AssetMatrix,
    refused: bool,
    committed: bool,
}

impl LayerWriter<'_, '_> {
    fn matrix(&self) -> Mat4 {
        // Under the display transform's half-block offset: poses are about the model's corner.
        glam_matrix(&self.base.model.layer_matrix(
            self.display_transform,
            self.ctx.display,
            &self.outer_transform.mul(&self.local_pose),
        ))
    }

    fn submit_geometry(&mut self, geometry: &BakedSpecialGeometry, tints: &Arc<[i32]>) {
        if !self.refused {
            let matrix = self.matrix();
            self.layers.push(LayerRender {
                geometry: LayerGeometry::Quads(geometry.model.clone()),
                tints: LayerTints::Shared(tints.clone()),
                foil: self.foil,
                matrix,
                uses_block_light: self.base.model.uses_block_light,
                particle: self.base.particle.clone(),
                extents: geometry.extents.clone(),
                display: self.ctx.display,
            });
        }
    }

    fn submit_stack(&mut self, stack: &StackView, display: DisplayContext) -> ControlFlow<()> {
        if self.refused {
            return ControlFlow::Break(());
        }
        if *self.remaining == 0
            || std::iter::successors(Some(self.ancestor), |entry| entry.parent)
                .any(|entry| entry.stack == stack)
        {
            warn_refused_nesting_once(&stack.item);
            self.refused = true;
            return ControlFlow::Break(());
        }
        *self.remaining -= 1;
        let ancestor = StackAncestor {
            stack,
            parent: Some(self.ancestor),
        };
        let ctx = Context {
            display,
            ..*self.ctx
        };
        let start = self.layers.len();
        if self
            .renderer
            .append_render_layers(self.layers, &ancestor, &ctx, self.env, None, self.remaining)
            .is_break()
        {
            self.refused = true;
            return ControlFlow::Break(());
        }
        // Compose after resolving the child: preserve parent * (child * grandchild) order
        // as well as the child's own display, foil and tint state.
        let matrix = self.matrix();
        for layer in &mut self.layers[start..] {
            layer.matrix = matrix * layer.matrix;
        }
        ControlFlow::Continue(())
    }

    fn commit(mut self) -> ControlFlow<()> {
        if self.refused {
            return ControlFlow::Break(());
        }
        self.committed = true;
        ControlFlow::Continue(())
    }
}

impl Drop for LayerWriter<'_, '_> {
    fn drop(&mut self) {
        if !self.committed {
            self.layers.truncate(self.checkpoint);
        }
    }
}

/// Says once per model id that a mod's special model renderer drew a model the packs don't have.
fn warn_unknown_part_once(model: &str) {
    static SEEN: Mutex<std::collections::BTreeSet<String>> =
        Mutex::new(std::collections::BTreeSet::new());
    let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
    if !seen.contains(model) {
        seen.insert(model.to_owned());
        drop(seen);
        log::warn!(
            "Sunrise: a special model renderer drew the model {model}, which no pack has; the special layer draws the missing model"
        );
    }
}
/// Says once per item that a special model renderer's stack part was refused as a cycle or past
/// the budget.
fn warn_refused_nesting_once(item: &str) {
    static SEEN: Mutex<std::collections::BTreeSet<String>> =
        Mutex::new(std::collections::BTreeSet::new());
    let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
    if !seen.contains(item) {
        seen.insert(item.to_owned());
        drop(seen);
        log::warn!(
            "Sunrise: a special model renderer's stack part of {item} is a cycle or past the nesting budget; the special layer draws the missing model"
        );
    }
}
