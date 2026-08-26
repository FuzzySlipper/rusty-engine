use std::ffi::c_void;

use core_math::Vec2;
use csharp_engine_abi::*;
use engine_spatial::{
    FirstPersonLookCommand, FirstPersonLookConfig, FirstPersonLookService, FirstPersonLookState,
};

use crate::composition::{native_quat, native_vec3, ABI_OK};

pub(crate) fn api() -> NativeLookApi {
    NativeLookApi {
        context: std::ptr::null_mut(),
        integrate: integrate_look,
    }
}

unsafe extern "C" fn integrate_look(
    _context: *mut c_void,
    request: NativeLookRequest,
    receipt: *mut NativeLookReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    let mut config = FirstPersonLookConfig::default();
    config.horizontal_radians_per_unit = request.config.horizontal_radians_per_unit;
    config.vertical_radians_per_unit = request.config.vertical_radians_per_unit;
    config.minimum_pitch_radians = request.config.minimum_pitch_radians;
    config.maximum_pitch_radians = request.config.maximum_pitch_radians;
    config.invert_horizontal = request.config.invert_horizontal != 0;
    config.invert_vertical = request.config.invert_vertical != 0;
    config.wrap_yaw = request.config.wrap_yaw != 0;
    config.maximum_delta_radians = request.config.maximum_delta_radians;
    let result = FirstPersonLookService.integrate(
        &config,
        FirstPersonLookState {
            yaw_radians: request.state.yaw_radians,
            pitch_radians: request.state.pitch_radians,
        },
        FirstPersonLookCommand {
            delta: Vec2::new(request.delta.x, request.delta.y),
        },
    );
    match result {
        Ok(result) => {
            // SAFETY: null was rejected above; the receipt is borrowed for this call only.
            unsafe {
                *receipt = NativeLookReceipt {
                    state: NativeLookState {
                        yaw_radians: result.after.yaw_radians,
                        pitch_radians: result.after.pitch_radians,
                    },
                    orientation: native_quat(result.orientation),
                    forward: native_vec3(result.forward),
                    right: native_vec3(result.right),
                    up: native_vec3(result.up),
                };
            }
            ABI_OK
        }
        Err(_) => 0,
    }
}
