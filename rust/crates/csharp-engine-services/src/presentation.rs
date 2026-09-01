//! Direct NativeAOT callbacks for renderer-neutral presentation facts.

use std::ffi::c_void;

use csharp_engine_abi::*;

use crate::{appearance::RuntimeAppearanceBridge, composition::ABI_OK};

unsafe fn bridge<'call>(context: *mut c_void) -> Option<&'call mut RuntimeAppearanceBridge> {
    if context.is_null() {
        None
    } else {
        Some(unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() })
    }
}

pub(crate) unsafe extern "C" fn create_billboard(
    context: *mut c_void,
    request: *const NativePresentationBillboardDescriptor,
    result: *mut NativePresentationBillboardHandle,
) -> i32 {
    if request.is_null() || result.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_create_billboard(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn update_billboard(
    context: *mut c_void,
    owner: NativePresentationBillboardHandle,
    request: *const NativePresentationBillboardDescriptor,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_update_billboard(owner, unsafe { &*request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn create_structured_billboard(
    context: *mut c_void,
    request: *const NativePresentationStructuredBillboardDescriptor,
    result: *mut NativePresentationBillboardHandle,
) -> i32 {
    if request.is_null() || result.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_create_structured_billboard(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn update_structured_billboard(
    context: *mut c_void,
    owner: NativePresentationBillboardHandle,
    request: *const NativePresentationStructuredBillboardDescriptor,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_update_structured_billboard(owner, unsafe { &*request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn destroy_billboard(
    context: *mut c_void,
    owner: NativePresentationBillboardHandle,
) -> i32 {
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_destroy_billboard(owner) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn emit_particles(
    context: *mut c_void,
    request: *const NativePresentationParticleDescriptor,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    let request = unsafe { &*request };
    match bridge.presentation_emit_particles(request.signal_id, request) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn create_emitter(
    context: *mut c_void,
    request: *const NativePresentationParticleDescriptor,
    result: *mut NativePresentationEmitterHandle,
) -> i32 {
    if request.is_null() || result.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_create_emitter(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn update_emitter(
    context: *mut c_void,
    owner: NativePresentationEmitterHandle,
    request: *const NativePresentationParticleDescriptor,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_update_emitter(owner, unsafe { &*request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn destroy_emitter(
    context: *mut c_void,
    owner: NativePresentationEmitterHandle,
) -> i32 {
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_destroy_emitter(owner) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read(
    context: *mut c_void,
    result: *mut NativePresentationFactsReadout,
) -> i32 {
    if result.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    unsafe { *result = bridge.presentation_readout() };
    ABI_OK
}

pub(crate) unsafe extern "C" fn read_diagnostic_at(
    context: *mut c_void,
    request: NativePresentationDiagnosticAtRequest,
    result: *mut NativePresentationDiagnosticAtReceipt,
) -> i32 {
    if result.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    unsafe { *result = bridge.presentation_diagnostic(request) };
    ABI_OK
}

pub(crate) unsafe extern "C" fn create_ghost_plate(
    context: *mut c_void,
    request: *const NativeCreateGhostPlatePresentationRequest,
    result: *mut NativeGhostPlatePresentationHandle,
) -> i32 {
    if request.is_null() || result.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_create_ghost_plate(unsafe { *request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn update_ghost_plate(
    context: *mut c_void,
    request: *const NativeUpdateGhostPlatePresentationRequest,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_update_ghost_plate(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn recapture_ghost_plate(
    context: *mut c_void,
    request: *const NativeRecaptureGhostPlatePresentationRequest,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_recapture_ghost_plate(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_ghost_plate(
    context: *mut c_void,
    presentation: NativeGhostPlatePresentationHandle,
    result: *mut NativeGhostPlatePresentationReadout,
) -> i32 {
    if result.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_read_ghost_plate(presentation) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn destroy_ghost_plate(
    context: *mut c_void,
    presentation: NativeGhostPlatePresentationHandle,
) -> i32 {
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match bridge.presentation_destroy_ghost_plate(presentation) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.record_callback_error(error);
            0
        }
    }
}
