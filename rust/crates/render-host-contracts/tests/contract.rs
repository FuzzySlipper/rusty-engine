use render_host_contracts::{
    RendererCameraInterpolation, RendererCameraMotion, RendererCameraPose,
    RendererCameraProjection, RendererCompositionCamera, RendererCompositionView,
    RendererPhysicalInputReadout, RendererPickRay, RendererPickRequest, RendererPointerReadout,
    RendererViewComposition, RendererViewTarget, RendererViewport, RendererWheelReadout,
    RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
};

#[test]
fn camera_and_pick_contracts_match_the_typescript_border() {
    let pose = RendererCameraPose {
        position: [1.0, 2.0, 3.0],
        pitch_degrees: -10.0,
        yaw_degrees: 45.0,
    };
    assert_eq!(
        serde_json::to_value(pose).unwrap(),
        serde_json::json!({
            "position": [1.0, 2.0, 3.0],
            "pitchDegrees": -10.0,
            "yawDegrees": 45.0,
        })
    );

    let request = RendererPickRequest {
        filter: None,
        max_distance: Some(20.0),
        ray: RendererPickRay::Viewport { point: [0.0, 0.0] },
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({
            "maxDistance": 20.0,
            "ray": { "kind": "viewport", "point": [0.0, 0.0] },
        })
    );
}

#[test]
fn physical_input_is_a_typed_observation_not_a_semantic_action() {
    let readout: RendererPhysicalInputReadout = serde_json::from_value(serde_json::json!({
        "pressedCodes": ["KeyW"],
        "pointer": { "xPixels": 12.0, "yPixels": 18.0, "buttons": 1 },
        "wheel": { "deltaX": 0.0, "deltaY": -2.0 },
    }))
    .unwrap();
    assert_eq!(readout.pressed_codes, ["KeyW"]);
    assert_eq!(
        readout.pointer,
        RendererPointerReadout {
            x_pixels: 12.0,
            y_pixels: 18.0,
            buttons: 1,
        }
    );
    assert_eq!(
        readout.wheel,
        RendererWheelReadout {
            delta_x: 0.0,
            delta_y: -2.0,
        }
    );
}

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
        schema_version: RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
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
        }],
        presentations: Vec::new(),
    };
    assert!(composition.validate().is_ok());

    let mut invalid = composition;
    invalid.cameras[0].motion.as_mut().unwrap().delay_seconds = 0.0;
    assert!(invalid.validate().is_err());
}
