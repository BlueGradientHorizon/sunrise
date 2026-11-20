fn body_yaw(value: f32) -> f32 {
    static SEEN: std::sync::Once = std::sync::Once::new();
    SEEN.call_once(|| fnr_lower::note("MODIFY_EXPRESSION_VALUE mobs::render::base_transform_at"));
    value
}
