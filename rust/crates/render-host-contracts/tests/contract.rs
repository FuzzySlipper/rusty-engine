use render_host_contracts::{
    validate_target_pixels, RendererCameraInterpolation, RendererCameraMotion, RendererCameraPose,
    RendererCameraProjection, RendererCompositionCamera, RendererCompositionTarget,
    RendererHostContractError, RendererTargetColor, RendererTargetDepth, RendererTargetSampling,
    MAX_RENDERER_TARGET_DIMENSION,
};

#[test]
fn camera_motion_contract_carries_renderer_sampling_facts() {
    let motion = RendererCameraMotion {
        sample_id: "7".to_owned(),
        sample_time_seconds: 12.5,
        delay_seconds: 1.0 / 60.0,
        interpolation: RendererCameraInterpolation::Pose,
        cut: true,
    };
    let camera = RendererCompositionCamera {
        id: "camera".to_owned(),
        pose: RendererCameraPose {
            position: [1.0, 2.0, 3.0],
            pitch_degrees: 0.0,
            yaw_degrees: 0.0,
        },
        basis: None,
        projection: RendererCameraProjection::Perspective {
            fov_y_degrees: 70.0,
            near: 0.1,
            far: 1_000.0,
        },
        motion: Some(motion),
        viewmodel_fov_y_degrees: 0.0,
    };
    assert_eq!(
        serde_json::to_value(&camera).unwrap()["motion"],
        serde_json::json!({
            "sampleId": "7",
            "sampleTimeSeconds": 12.5,
            "delaySeconds": 1.0 / 60.0,
            "interpolation": "pose",
            "cut": true,
        })
    );
}

#[test]
fn a_target_takes_any_side_up_to_the_texture_limit_within_the_pixel_budget() {
    let target = |width, height, revision| RendererCompositionTarget {
        id: "inset".to_owned(),
        revision,
        width,
        height,
        color: RendererTargetColor::Rgba8Srgb,
        depth: RendererTargetDepth::Depth24,
        sampling: RendererTargetSampling::Linear,
    };
    let wide = target(4_096, 512, 1 << 60);
    assert_eq!(wide.validate(), Ok(()));
    assert_eq!(validate_target_pixels([&wide]), Ok(()));
    assert_eq!(
        target(MAX_RENDERER_TARGET_DIMENSION, 1, 1).validate(),
        Ok(())
    );
    assert_eq!(
        target(MAX_RENDERER_TARGET_DIMENSION + 1, 1, 1).validate(),
        Err(RendererHostContractError::InvalidDimension)
    );
    assert_eq!(
        validate_target_pixels([&target(4_096, 4_096, 1)]),
        Err(RendererHostContractError::LimitExceeded)
    );
}
