use sunrise_core::nbt::Tag;

pub(in crate::game) fn saved_silent(extra: Option<&Tag>) -> bool {
    sunrise_sim::save::number(extra.and_then(|tag| tag.get("Silent"))).unwrap_or(0) != 0
}
