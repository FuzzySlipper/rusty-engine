use std::ffi::c_void;

use core_math::Vec2;
use csharp_engine_abi::*;
use engine_spatial::{
    FirstPersonLookCommand, FirstPersonLookConfig, FirstPersonLookDiagnostic,
    FirstPersonLookService, FirstPersonLookState,
};

use crate::composition::{native_quat, native_vec3, ABI_OK};

pub(crate) fn api() -> NativeLookApi {
    NativeLookApi {
        context: std::ptr::null_mut(),
        integrate: integrate_look,
        reset: reset_look,
        rebase: rebase_look,
        diagnose: diagnose_look,
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
    let config = look_config(request.config);
    let result = FirstPersonLookService.integrate(
        &config,
        look_state(request.state),
        FirstPersonLookCommand {
            delta: Vec2::new(request.delta.x, request.delta.y),
        },
    );
    match result {
        Ok(result) => {
            // SAFETY: null was rejected above; the receipt is borrowed for this call only.
            unsafe {
                *receipt = native_receipt(result);
            }
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn reset_look(
    _context: *mut c_void,
    request: NativeLookResetRequest,
    receipt: *mut NativeLookReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    let result = FirstPersonLookService.reset(look_state(request.state));
    // SAFETY: null was rejected above; the receipt is borrowed for this call only.
    unsafe { *receipt = native_receipt(result) };
    ABI_OK
}

unsafe extern "C" fn rebase_look(
    _context: *mut c_void,
    request: NativeLookRebaseRequest,
    receipt: *mut NativeLookReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    match FirstPersonLookService.rebase(
        &look_config(request.config),
        look_state(request.state),
        look_state(request.target),
    ) {
        Ok(result) => {
            // SAFETY: null was rejected above; the receipt is borrowed for this call only.
            unsafe { *receipt = native_receipt(result) };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn diagnose_look(
    _context: *mut c_void,
    request: NativeLookRequest,
    diagnostic: *mut NativeLookDiagnostic,
) -> i32 {
    if diagnostic.is_null() {
        return 0;
    }
    let result = FirstPersonLookService.diagnose(
        &look_config(request.config),
        look_state(request.state),
        FirstPersonLookCommand {
            delta: Vec2::new(request.delta.x, request.delta.y),
        },
    );
    // SAFETY: null was rejected above; the diagnostic is borrowed for this call only.
    unsafe { *diagnostic = native_diagnostic(result) };
    ABI_OK
}

fn look_config(value: NativeLookConfig) -> FirstPersonLookConfig {
    let mut config = FirstPersonLookConfig::default();
    config.horizontal_radians_per_unit = value.horizontal_radians_per_unit;
    config.vertical_radians_per_unit = value.vertical_radians_per_unit;
    config.minimum_pitch_radians = value.minimum_pitch_radians;
    config.maximum_pitch_radians = value.maximum_pitch_radians;
    config.invert_horizontal = value.invert_horizontal;
    config.invert_vertical = value.invert_vertical;
    config.wrap_yaw = value.wrap_yaw;
    config.maximum_delta_radians = value.maximum_delta_radians;
    config
}

fn look_state(value: NativeLookState) -> FirstPersonLookState {
    FirstPersonLookState {
        yaw_radians: value.yaw_radians,
        pitch_radians: value.pitch_radians,
    }
}

fn native_receipt(value: engine_spatial::FirstPersonLookReceipt) -> NativeLookReceipt {
    NativeLookReceipt {
        before: native_state(value.before),
        after: native_state(value.after),
        orientation: native_quat(value.orientation),
        forward: native_vec3(value.forward),
        right: native_vec3(value.right),
        up: native_vec3(value.up),
    }
}

fn native_state(value: FirstPersonLookState) -> NativeLookState {
    NativeLookState {
        yaw_radians: value.yaw_radians,
        pitch_radians: value.pitch_radians,
    }
}

fn native_diagnostic(value: FirstPersonLookDiagnostic) -> NativeLookDiagnostic {
    match value {
        FirstPersonLookDiagnostic::Accepted => NativeLookDiagnostic::Accepted,
        FirstPersonLookDiagnostic::InvalidConfig => NativeLookDiagnostic::InvalidConfig,
        FirstPersonLookDiagnostic::InvalidState => NativeLookDiagnostic::InvalidState,
        FirstPersonLookDiagnostic::InvalidCommand => NativeLookDiagnostic::InvalidCommand,
        FirstPersonLookDiagnostic::DeltaLimitExceeded => NativeLookDiagnostic::DeltaLimitExceeded,
    }
}
