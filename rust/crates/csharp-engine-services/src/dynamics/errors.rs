use super::*;

pub(super) use crate::operation_diagnostics::clear_receipt;

/// Returns a Dynamics refusal through its operation receipt. The refusal is
/// the failure's only effect; C# receives the named reason.
pub(super) fn refuse(
    context: *mut c_void,
    error: &CsharpEngineServicesError,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
        .operation_diagnostics
        .retain(error, receipt);
    0
}
