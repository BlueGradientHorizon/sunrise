//! The `pickup_xp` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0212

pub trait Hooks {
    /// Before `player` picks up orb `orb` in `dimension` (`ExperienceOrb.playerTouch`,
    /// NeoForge's `PlayerXpEvent.PickupXp`). Fires only when `takeXpDelay` is 0;
    /// canceling leaves the orb and pickup delay unchanged. Otherwise, edited `value`
    /// and `count` apply to the live orb before the pickup runs: `value` keeps vanilla's
    /// signed `int` semantics (zero and negative values repair nothing and award no XP,
    /// and a surviving orb persists `Value` as a signed short); `count` is not floored
    /// to one—setting it to `0` discards the orb without taking a pickup or setting the
    /// 2-tick delay, while `count >= 1` sets `takeXpDelay` to 2, repairs Mending and
    /// awards remaining positive XP, then decrements `count` and discards at 0.
    /// Server-side. Diverges from upstream: renamed from `experience_orb_pickup`, to
    /// NeoForge's `PlayerXpEvent.PickupXp`.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_pickup_xp",
        dispatch = "flat",
        side = "server",
        contract = "cancel",
        out = "canceled",
        neoforge = "PlayerXpEvent.PickupXp",
        cancellable = true,
        version = 2
    )]
    fn pickup_xp(
        player: u128,
        dimension: &str,
        orb: u32,
        value: &mut i32,
        count: &mut u32,
        canceled: &mut bool,
    );
}
