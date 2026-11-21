#![cfg(test)]

/// Camera angle overrides keep finite components and fall back to vanilla per component.
#[test]
fn camera_angles_keep_finite_components_and_fallback_per_axis() {
    use super::finite_camera_angles;
    let vanilla = (10.0, 20.0, 0.0);
    assert_eq!(
        finite_camera_angles(vanilla, 270.0, -35.0, 12.5),
        (270.0, -35.0, 12.5)
    );
    assert_eq!(
        finite_camera_angles(vanilla, f32::NAN, f32::NEG_INFINITY, f32::INFINITY),
        vanilla
    );
    assert_eq!(
        finite_camera_angles(vanilla, f32::NAN, -15.0, 5.0),
        (10.0, -15.0, 5.0)
    );
}

/// Camera position overrides keep finite axes and fall back to vanilla per axis.
#[test]
fn camera_position_keeps_finite_axes_and_falls_back_per_axis() {
    use super::finite_camera_position;
    let vanilla = [1.0, 2.0, 3.0];
    assert_eq!(
        finite_camera_position(vanilla, [-40.5, 90.0, 1e7]),
        [-40.5, 90.0, 1e7]
    );
    assert_eq!(
        finite_camera_position(vanilla, [f64::NAN, 5.0, f64::INFINITY]),
        [1.0, 5.0, 3.0]
    );
}
