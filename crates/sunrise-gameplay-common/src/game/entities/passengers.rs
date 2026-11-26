#[cfg(any(test, feature = "test-support"))]
pub mod nested_vehicle_discard_tests {

    pub static VEHICLE_VALUE: mod_api::AttachmentType<i32> = mod_api::AttachmentType {
        namespace: "vehicle_boundary",
        name: "value",
        default: || 0,
        codec: None,
        copy_on_death: false,
        sync: false,
    };
}
