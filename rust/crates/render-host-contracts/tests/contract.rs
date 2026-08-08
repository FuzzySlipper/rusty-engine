use render_host_contracts::{
    RendererCameraPose, RendererPhysicalInputReadout, RendererPickRay, RendererPickRequest,
    RendererPointerReadout, RendererWheelReadout,
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
