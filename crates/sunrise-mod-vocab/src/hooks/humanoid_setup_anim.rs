//! The mid-setupAnim humanoid animation moment.

pub trait Hooks {
    /// Once per actual `HumanoidModel.setupAnim` invocation, after head/walk/riding
    /// setup and before arm priority/posing. `parts` lends the live [`HumanoidPose`]
    /// for this draw. The hook is skipped if an incoming base-part position or
    /// rotation is non-finite. Setters apply finite components immediately, leaving
    /// refused components unchanged. Later listeners see these edits. Later vanilla
    /// ITEM, attack, crouch, bob and swim animation still apply. Armor stands then
    /// replace rotations with their saved pose, preserving position edits.
    /// `humanoid` contains this invocation's held stacks and use state. Time is
    /// captured for its view; `game_time` is absent outside a level, never wall time.
    /// The Host is read-only. No frame deduplication: layers that actually animate
    /// again get another invocation; copying parent poses does not dispatch.
    #[hook(
        side = "client",
        contract = "listen",
        version = 3,
        event_per_listener = true,
        neoforge = "none (vanilla HumanoidModel.setupAnim; NeoForge has no mid-setupAnim event)"
    )]
    fn humanoid_setup_anim(
        entity_id: u32,
        entity_type: &str,
        humanoid: &HumanoidRenderState<'_>,
        partial_tick: f32,
        game_time: Option<i64>,
        parts: &mut dyn HumanoidPose,
    );
}
