//! One living entity's draw (`LivingEntityRenderer.submit` and the layers, hooks, outline
//! and beam around it). The ordered entity pass shares one `LivingFrame` across its mob runs.

/// What the mob pass works out once a frame for every living entity it draws.
#[derive(Clone, Copy)]
pub(in crate::game) struct LivingFrame<'a> {
    pub(in crate::game) camera: glam::DVec3,
    pub(in crate::game) alpha: f64,
    /// `Entity.shouldRenderAtSqrDistance`'s view scale.
    pub(in crate::game) scale: f64,
    /// Who watches (`Entity.isInvisibleTo`): the local player's team.
    pub(in crate::game) viewer: Option<&'a sunrise_client_core::scoreboard::Team>,
    pub(in crate::game) skinning: bool,
    pub(in crate::game) listened: bool,
    pub(in crate::game) now: std::time::Instant,
}
