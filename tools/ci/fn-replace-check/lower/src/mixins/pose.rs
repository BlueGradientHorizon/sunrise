fn pose(value: ()) {
    static SEEN: std::sync::Once = std::sync::Once::new();
    SEEN.call_once(|| fnr_lower::note("RETURN mobs::anim::setup"));
    value
}
