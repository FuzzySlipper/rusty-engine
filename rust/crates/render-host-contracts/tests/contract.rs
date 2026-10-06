use render_host_contracts::{
    RendererCameraInterpolation, RendererCameraMotion, RendererCameraPose,
    RendererCameraProjection, RendererCompositionCamera, RendererCompositionView,
    RendererViewComposition, RendererViewTarget, RendererViewport,
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
        motion: Some(motion.clone()),
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
    let composition = RendererViewComposition {
        cameras: vec![camera],
        targets: Vec::new(),
        views: vec![RendererCompositionView {
            id: "view".to_owned(),
            camera_id: "camera".to_owned(),
            target: RendererViewTarget::Primary,
            viewport: RendererViewport {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            order: 0,
            viewport_anchor: None,
        }],
        presentations: Vec::new(),
    };
    assert!(composition.validate().is_ok());

    let mut invalid = composition;
    invalid.cameras[0].motion.as_mut().unwrap().delay_seconds = 0.0;
    assert!(invalid.validate().is_err());
}
