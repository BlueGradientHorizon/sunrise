//! Read-only keys and views of the engine-owned baked model cache (#861 part 1).
//!
//! Every successful resource load bakes each block state's models and the standalone models
//! once into one immutable store for that resource generation. A mod retains keys into the store;
//! looking up a key borrows the model's geometry and sprites.
use crate::{Direction, ModelRef};
use std::{fmt, iter::FusedIterator};

/// One baked model of the published model store: a block state's variant model or a standalone
/// model, for one resource generation. Get it from
/// [`RenderAssets::block_state_models`](crate::RenderAssets::block_state_models) or from a
/// [`ModelRef`] (`BakedModelKey::from(model_ref)`), and read it with
/// [`RenderAssets::baked_model`](crate::RenderAssets::baked_model). A successful resource reload
/// supersedes it: the old key then answers [`ModelError::StaleModel`](crate::ModelError::StaleModel)
/// and a new one is asked for in `model_baking_completed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustive: private fields; only the engine issues keys
pub struct BakedModelKey(pub(crate) u64, pub(crate) u32);

impl BakedModelKey {
    pub(crate) fn from_parts(generation: u64, slot: u32) -> Result<Self, &'static str> {
        if generation == 0 {
            return Err("unpublished model generation");
        }
        Ok(Self(generation, slot))
    }
}

impl From<ModelRef> for BakedModelKey {
    /// The baked model a standalone model handle names, in the same generation.
    fn from(model: ModelRef) -> Self {
        Self(model.0, model.1)
    }
}

/// A block state's baked model alternatives, borrowed from the published resource generation.
/// Keep the keys produced by [`Self::iter`], not this view, across host calls.
// no wire: borrows the published resource generation's slot list.
#[derive(Clone, Copy, Debug)]
pub struct BakedModelKeys<'a> {
    generation: u64,
    slots: &'a [u32],
}

impl<'a> BakedModelKeys<'a> {
    /// The keys in bake order, without repeats. No random model selection is performed.
    pub fn iter(self) -> impl ExactSizeIterator<Item = BakedModelKey> + 'a {
        let Self { generation, slots } = self;
        slots
            .iter()
            .map(move |&slot| BakedModelKey(generation, slot))
    }
}

/// Immutable geometry and metadata borrowed from one published baked model. Keep its
/// [`BakedModelKey`] to borrow the model again; this view cannot outlive the asset borrow.
// no wire: borrows model geometry and sprite names from the published resource generation.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct BakedModelView<'a> {
    /// Whether the model allows smooth lighting (`ambientocclusion`).
    pub ambient_occlusion: bool,
    /// The block atlas sprite its particles use (`minecraft:block/stone`), when it names one.
    pub particle: Option<&'a str>,
    geometry: &'a dyn BakedQuadSource,
}

impl<'a> BakedModelView<'a> {
    /// Its quads in bake order, already turned and UV-locked as the variant asks. Traversal
    /// borrows geometry and sprite names and does not resolve a block tint or draw material.
    pub fn quads(self) -> impl FusedIterator<Item = BakedQuad<'a>> {
        let geometry = self.geometry;
        (0..).map_while(move |index| geometry.quad(index)).fuse()
    }
}

impl PartialEq for BakedModelView<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.ambient_occlusion == other.ambient_occlusion
            && self.particle == other.particle
            && self.quads().eq(other.quads())
    }
}

impl fmt::Debug for BakedModelView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BakedModelView")
            .field("ambient_occlusion", &self.ambient_occlusion)
            .field("particle", &self.particle)
            .finish_non_exhaustive()
    }
}

/// One quad of a baked model ([`BakedModelView::quads`]), not a resolved draw command.
// no wire: borrows a baked quad's geometry and sprite name from the published resource generation.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedQuad<'a> {
    /// Corners in block units (0 to 1 across the block for a unit cube), counter-clockwise seen
    /// from the side the quad faces.
    pub positions: &'a [[f32; 3]; 4],
    /// Each corner's texture coordinates, 0 to 1 across the sprite.
    pub uvs: &'a [[f32; 2]; 4],
    /// The block atlas sprite named by the baked quad (`minecraft:block/stone`).
    pub sprite: &'a str,
    /// The side it faces, which determines its usual directional shading.
    pub facing: Direction,
    /// The face whose solid neighbour hides the quad; `None` never culls.
    pub cull: Option<Direction>,
    /// The tint index a block tint source colours it by; -1 is none. Independent of `baked_argb`.
    pub tint_index: i32,
    /// The quad's own `0xAARRGGBB` colour, if set. This does not include tint-source colours,
    /// lighting, or any other draw-time state.
    pub baked_argb: Option<u32>,
    /// Whether the baked quad asks for directional shading.
    pub shade: bool,
    /// Its `light_emission`, 0 to 15.
    pub light_emission: u8,
}

/// Renderer-side backing for a model's immutable quads. Indices are contiguous in bake order.
pub trait BakedQuadSource: Sync {
    /// Borrow the quad at `index`, or return `None` past the last quad.
    fn quad(&self, index: usize) -> Option<BakedQuad<'_>>;
}

/// Borrow a state's precomputed slots from a published model store.
pub fn baked_model_keys(generation: u64, slots: &[u32]) -> BakedModelKeys<'_> {
    BakedModelKeys { generation, slots }
}

/// Borrow the renderer's model and its particle sprite without exposing engine types.
pub fn baked_model_view<'a>(
    ambient_occlusion: bool,
    particle: Option<&'a str>,
    geometry: &'a dyn BakedQuadSource,
) -> BakedModelView<'a> {
    BakedModelView {
        ambient_occlusion,
        particle,
        geometry,
    }
}

/// Lend one quad's geometry and sprite name, alongside its own baked metadata.
pub fn baked_quad<'a>(
    positions: &'a [[f32; 3]; 4],
    uvs: &'a [[f32; 2]; 4],
    sprite: &'a str,
    facing: Direction,
    cull: Option<Direction>,
    tint_index: i32,
    baked_argb: Option<u32>,
    shade: bool,
    light_emission: u8,
) -> BakedQuad<'a> {
    BakedQuad {
        positions,
        uvs,
        sprite,
        facing,
        cull,
        tint_index,
        baked_argb,
        shade,
        light_emission,
    }
}
