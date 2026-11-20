//! Special item model renderers, as a client mod registers them in
//! `register_special_model_renderer` (NeoForge's `RegisterSpecialModelRendererEvent` and
//! vanilla's `SpecialModelRenderer`).
//!
//! A factory decodes and bakes each `minecraft:special` model node that names its type. The
//! resulting renderer answers which resource models or item stacks to draw, each posed and
//! tinted, with the node's `base` display transforms, particle and GUI light.
use crate::{ClientHost, ItemDisplayContext, ItemStack};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::{any::Any, collections::BTreeMap, sync::Arc};

// no wire: a borrowed view of one resource definition, used only during decode.
/// The inline fields of the special model's `model` object, excluding the dispatch key `type`.
#[derive(Clone, Copy)]
pub struct SpecialModelConfig<'a>(&'a Map<String, Value>);

impl<'a> SpecialModelConfig<'a> {
    #[doc(hidden)]
    /// Borrow the object validated by the engine's item definition decoder.
    pub fn new(fields: &'a Map<String, Value>) -> Self {
        Self(fields)
    }

    /// Decode these fields into the factory's owned configuration. Your type's serde rules
    /// determine required fields, defaults and handling of unknown fields.
    pub fn decode<T: DeserializeOwned>(self) -> Result<T, String> {
        let fields = self
            .0
            .iter()
            .filter(|(name, _)| name.as_str() != "type")
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        serde_json::from_value(Value::Object(fields)).map_err(|error| error.to_string())
    }
}

// no wire: process-local decoder and bake callbacks.
/// A decoder paired with a bake function for one special model type.
#[derive(Clone)]
pub struct SpecialModelFactory(Arc<BakeSpecialModel>);

type BakeSpecialModel = dyn for<'a> Fn(
        SpecialModelConfig<'a>,
        &dyn SpecialModelBakeContext,
    ) -> Result<Arc<dyn SpecialModelRenderer>, String>
    + Send
    + Sync;

impl SpecialModelFactory {
    /// Pair a decoder with the bake function consuming its typed output. For each model node
    /// naming this type, the game decodes and then bakes once while loading client resources.
    /// The baked renderer can retain decoded fields or geometry for the lifetime of that load.
    pub fn new<C: 'static, R: SpecialModelRenderer>(
        decode: impl for<'a> Fn(SpecialModelConfig<'a>) -> Result<C, String> + Send + Sync + 'static,
        bake: impl Fn(C, &dyn SpecialModelBakeContext) -> Result<R, String> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(move |config, context| {
            let config = decode(config).map_err(|error| format!("decode: {error}"))?;
            let renderer = bake(config, context).map_err(|error| format!("bake: {error}"))?;
            Ok(Arc::new(renderer) as Arc<dyn SpecialModelRenderer>)
        }))
    }

    #[doc(hidden)]
    /// Run the factory at the engine's item definition load boundary.
    pub fn bake(
        &self,
        config: SpecialModelConfig<'_>,
        context: &dyn SpecialModelBakeContext,
    ) -> Result<Arc<dyn SpecialModelRenderer>, String> {
        (self.0)(config, context)
    }
}

// no wire: a scoped view of the selected client resources, used only during bake.
/// The resources and item quad baker available to a special model factory during baking.
pub trait SpecialModelBakeContext {
    /// Bake a model from the selected packs into a lease, or report that it could not be baked.
    /// This uses the model's geometry, not its item display transforms.
    fn bake_model(&self, model: &str) -> Result<SpecialModelGeometry, String>;

    /// Bake custom quads against the selected packs' item and block atlas sprites. A missing
    /// sprite, non-finite coordinate, invalid facing, tint index or light level rejects the bake.
    fn bake_geometry(&self, quads: &[SpecialModelQuad]) -> Result<SpecialModelGeometry, String>;
}

// no wire: an input to the in-process geometry baker, not a draw recipe.
/// One textured quad to bake for a special item model.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct SpecialModelQuad {
    /// Corners in block units, relative to the model's corner. The first three determine facing
    /// by their winding and must not be collinear.
    pub positions: [[f32; 3]; 4],
    /// Sprite-local UVs for the same four corners, in `[0, 1]`.
    pub uv: [[f32; 2]; 4],
    /// The sprite's resource ID in an item or block atlas.
    pub sprite: String,
    /// Index into the submitting part's tints, if any.
    pub tint_index: Option<u32>,
    /// Block light floor, from 0 to 15.
    pub light_emission: u8,
    /// Draw on the translucent item sheet even when the sprite is opaque.
    pub force_translucent: bool,
}

impl SpecialModelQuad {
    /// An untinted, non-emissive quad whose render sheet follows its sprite.
    pub fn new(positions: [[f32; 3]; 4], uv: [[f32; 2]; 4], sprite: impl Into<String>) -> Self {
        Self {
            positions,
            uv,
            sprite: sprite.into(),
            tint_index: None,
            light_emission: 0,
            force_translucent: false,
        }
    }
}

// no wire: a process-local lease on baked quads and extents for one client resource load.
/// A lease on baked item geometry. Cloning the lease retains the geometry without rebaking it.
#[derive(Clone)]
pub struct SpecialModelGeometry(Arc<dyn Any + Send + Sync>);

impl SpecialModelGeometry {
    #[doc(hidden)]
    /// Store validated, engine-owned geometry produced by a bake context.
    pub fn new(geometry: impl Any + Send + Sync) -> Self {
        Self(Arc::new(geometry))
    }

    #[doc(hidden)]
    /// Borrow the baked payload in the engine that produced it.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }
}

/// What a [`SpecialModelPart`] draws.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, sunrise_mod_api_macros::Wire)]
pub enum SpecialPartContent {
    /// A resource model: the id (`mymod:item/skillet_pan`) of a model file of the packs.
    Model(String),
    /// An item stack, resolved through the item model resolver as any held or displayed item is:
    /// the item's own models, tints and foil, drawn as `display` says. The part's transform
    /// places the item's centre, and its `tints` are not used.
    Stack {
        /// The stack drawn.
        stack: ItemStack,
        /// The display context the item is resolved and drawn with (`Fixed` as in an item frame,
        /// `Ground`, `Gui`): its own display transform applies.
        display: ItemDisplayContext,
    },
}

/// The most nested stack parts one draw of an item may expand, all its renderers' and the
/// stacks' own together: a draw that would expand more, or that meets a stack already being
/// expanded above it, is refused whole.
pub const MAX_NESTED_STACK_PARTS: usize = 64;

/// One thing a special model renderer draws: a resource model or an item stack.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, sunrise_mod_api_macros::Wire)]
pub struct SpecialModelPart {
    /// What is drawn.
    pub content: SpecialPartContent,
    /// Where it is drawn: a row-major 3×4 affine transform in the special model's space, in
    /// blocks with the origin at the model's corner (a model element's coordinates over 16).
    pub transform: [f32; 12],
    /// ARGB per tint index, as an item model's `tints`: a quad with a tint index past them, or
    /// none, draws white.
    pub tints: Vec<u32>,
}

impl SpecialModelPart {
    /// The identity transform.
    pub const IDENTITY: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

    /// `model` where the special model is, untinted.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            content: SpecialPartContent::Model(model.into()),
            transform: Self::IDENTITY,
            tints: Vec::new(),
        }
    }

    /// `stack` drawn as `display` says, centred where the special model is.
    pub fn stack(stack: ItemStack, display: ItemDisplayContext) -> Self {
        Self {
            content: SpecialPartContent::Stack { stack, display },
            transform: Self::IDENTITY,
            tints: Vec::new(),
        }
    }

    /// The part drawn under `transform` (row-major 3×4 affine).
    pub fn with_transform(mut self, transform: [f32; 12]) -> Self {
        self.transform = transform;
        self
    }

    /// The part moved by `by`, in blocks, after its transform.
    pub fn translated(mut self, by: [f32; 3]) -> Self {
        self.transform[3] += by[0];
        self.transform[7] += by[1];
        self.transform[11] += by[2];
        self
    }

    /// The part tinted per tint index (ARGB).
    pub fn with_tints(mut self, tints: Vec<u32>) -> Self {
        self.tints = tints;
        self
    }
}

/// When a special model renderer is asked again: what its answer depends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum SpecialModelPolicy {
    /// The answer is a pure function of the stack and the baked renderer: asked once for an
    /// unchanged stack however many frames and display contexts draw it, and given the neutral
    /// [`SpecialModelContext::STACK`]. A deliberate difference from vanilla, which extracts
    /// every frame.
    Stack,
    /// Asked in every frame the item is drawn, for each display context and game time it is
    /// drawn with, as vanilla's `ItemModel.update` is: for an answer that moves with time. There
    /// is no cap on animated items: the renderer costs one `extract` per drawn item per frame,
    /// for each display context it is drawn in.
    Frame,
}

/// What a [`SpecialModelPolicy::Frame`] renderer is told besides the stack, as `ItemModel.update`
/// reads it: where the item is drawn and when.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub struct SpecialModelContext {
    /// Where the item is drawn (a hand, a screen, the ground, a frame, the head).
    pub display: ItemDisplayContext,
    /// `getGameTime` of the level the item is drawn in; `None` outside a world.
    pub game_time: Option<i64>,
    /// How far the world is between its last tick and the next, in `[0, 1)`: the fraction the
    /// frame draws the world at.
    pub partial_tick: f32,
}

impl SpecialModelContext {
    /// The context of one drawn item; only the game builds one, mods read it.
    // hidden: the game's constructor for a received struct, not a mod authoring surface.
    #[doc(hidden)]
    pub const fn new(
        display: ItemDisplayContext,
        game_time: Option<i64>,
        partial_tick: f32,
    ) -> Self {
        Self {
            display,
            game_time,
            partial_tick,
        }
    }
    /// What a [`SpecialModelPolicy::Stack`] renderer is always given: no display context, no
    /// time.
    pub const STACK: Self = Self {
        display: ItemDisplayContext::None,
        game_time: None,
        partial_tick: 0.0,
    };
}

/// The most parts one answer of a special model renderer may have: more refuse it.
pub const MAX_SPECIAL_MODEL_PARTS: usize = 256;

/// A mod's baked special model renderer: vanilla's `SpecialModelRenderer`. One instance is
/// baked for each model node; nodes of the same type can have different typed configuration.
/// The renderer's [`SpecialModelPolicy`] is chosen when it is registered. Under `Stack`, answers
/// depend only on stack and configuration: host-state changes do not invalidate a continuously
/// drawn unchanged stack, and answers drop after all views stop drawing the stack or resources
/// reload. Under `Frame` an answer is kept for one frame of one view only.
pub trait SpecialModelRenderer: Send + Sync + 'static {
    /// `extractArgument` and `submit` in one: the parts to draw for `stack`, in order. Use the
    /// fields retained by your bake function for this model node. Asked on the client's main
    /// thread when an item of such a model is drawn. Under
    /// [`SpecialModelPolicy::Stack`], when its stack differs from the last one this model was
    /// asked about: an unchanged stack is asked once, however many frames draw it, every display
    /// context draws the same answer, and `context` is [`SpecialModelContext::STACK`]. Under
    /// [`SpecialModelPolicy::Frame`], once per frame for each display context and game time the
    /// item is drawn with, `context` saying which. The `base` model's display transforms apply
    /// to the parts either way. Diverges: vanilla's renderer submits geometry; Sunrise's answers
    /// resource models, so geometry stays in the packs.
    ///
    /// An `Err`, a panic, more than [`MAX_SPECIAL_MODEL_PARTS`] parts or a non-finite
    /// transform refuses the entire answer: the special model draws the missing model, the error is logged once with the type,
    /// and renderers of this type are asked no more in this resource load. A part naming a
    /// model no pack has refuses the answer too, logged once for the model.
    fn extract(
        &self,
        host: &dyn ClientHost,
        stack: &ItemStack,
        context: &SpecialModelContext,
    ) -> Result<Vec<SpecialModelPart>, String>;
}

/// The special model renderer registry `register_special_model_renderer` hands out: NeoForge's
/// `RegisterSpecialModelRendererEvent`.
pub trait SpecialModelRenderers {
    /// `register(location, codec)`: `factory` decodes and bakes every special model node of
    /// type `kind` (`mymod:skillet`), including replacements for builtins. Each baked renderer
    /// is asked as `policy` says. Last registration wins, as in NeoForge; the game logs both
    /// mods' ids. An id that is not `<namespace>:<path>` is logged and left out.
    fn register(&mut self, kind: String, policy: SpecialModelPolicy, factory: SpecialModelFactory);
}

// no wire: a report the game logs at registration; it holds `&'static` mod ids.
/// A registration that replaced an earlier one of the same type.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReplacedSpecialModelRenderer {
    /// The type.
    pub kind: String,
    /// The mod that registered it first, when a mod's dispatch did.
    pub earlier: Option<&'static str>,
    /// The mod whose registration stays.
    pub later: Option<&'static str>,
}

type PolicyFactory = (SpecialModelPolicy, SpecialModelFactory);

// no wire: process-local listener or callback storage; trait objects are invoked through host dispatch, not serialized.
/// The game's [`SpecialModelRenderers`]: the registrations by type.
#[derive(Default)]
// exhaustive: its fields are private; the game builds it
pub struct SpecialModelRendererRegistry {
    factories: BTreeMap<String, (Option<&'static str>, PolicyFactory)>,
    replaced: Vec<ReplacedSpecialModelRenderer>,
    initial: Vec<(String, Option<&'static str>)>,
}

impl SpecialModelRenderers for SpecialModelRendererRegistry {
    fn register(&mut self, kind: String, policy: SpecialModelPolicy, factory: SpecialModelFactory) {
        let later = crate::content::crash::current_native_mod();
        if let Some((earlier, ..)) = self
            .factories
            .insert(kind.clone(), (later, (policy, factory)))
        {
            self.replaced.push(ReplacedSpecialModelRenderer {
                kind,
                earlier,
                later,
            });
        } else {
            self.initial.push((kind, later));
        }
    }
}

impl SpecialModelRendererRegistry {
    /// The first mod registration of each type. The engine uses its own builtin decoder
    /// to distinguish replacement of a game type from a new type, irrespective of namespace.
    pub fn initial_owners(&self) -> impl Iterator<Item = (&str, Option<&'static str>)> {
        self.initial
            .iter()
            .map(|(kind, owner)| (kind.as_str(), *owner))
    }

    /// Each type's policy and factory, the last registration of a type winning, and the
    /// registrations that replaced an earlier one.
    pub fn into_renderers(
        self,
    ) -> (
        BTreeMap<String, PolicyFactory>,
        Vec<ReplacedSpecialModelRenderer>,
    ) {
        let factories = self
            .factories
            .into_iter()
            .map(|(kind, (_, factory))| (kind, factory))
            .collect();
        (factories, self.replaced)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Nothing;
    impl SpecialModelRenderer for Nothing {
        fn extract(
            &self,
            _: &dyn ClientHost,
            _: &ItemStack,
            _: &SpecialModelContext,
        ) -> Result<Vec<SpecialModelPart>, String> {
            Ok(Vec::new())
        }
    }

    /// The last registration of a type wins, vanilla's types included, and each replacement is
    /// reported.
    #[test]
    fn the_last_registration_of_a_type_wins() {
        let factory = || SpecialModelFactory::new(|_| Ok(()), |(), _| Ok(Nothing));
        let last = factory();
        let mut registry = SpecialModelRendererRegistry::default();
        registry.register("a:pan".into(), SpecialModelPolicy::Stack, factory());
        registry.register("a:pan".into(), SpecialModelPolicy::Frame, last.clone());
        registry.register(
            "minecraft:shield".into(),
            SpecialModelPolicy::Stack,
            factory(),
        );
        assert_eq!(
            registry.initial_owners().collect::<Vec<_>>(),
            vec![("a:pan", None), ("minecraft:shield", None)]
        );
        let (renderers, replaced) = registry.into_renderers();
        assert_eq!(renderers.len(), 2);
        assert_eq!(renderers["a:pan"].0, SpecialModelPolicy::Frame);
        assert!(Arc::ptr_eq(&renderers["a:pan"].1.0, &last.0));
        assert_eq!(replaced.len(), 1);
        assert_eq!(replaced[0].kind, "a:pan");
    }

    /// A part's helpers move and tint it without turning it.
    #[test]
    fn a_part_moves_after_its_transform() {
        let part = SpecialModelPart::new("a:item/b")
            .with_transform([2.0, 0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0])
            .translated([0.5, 0.25, 0.0])
            .with_tints(vec![0xFF00_FF00]);
        assert_eq!(
            part.transform,
            [2.0, 0.0, 0.0, 1.5, 0.0, 2.0, 0.0, 0.25, 0.0, 0.0, 2.0, 0.0]
        );
        assert_eq!(part.tints, [0xFF00_FF00]);
    }
}
