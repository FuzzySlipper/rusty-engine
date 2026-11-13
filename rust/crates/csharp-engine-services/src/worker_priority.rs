//! The priority of Engine background workers.

/// On Windows a background worker runs below normal priority, as the
/// heightfield samplers do (#10718), so a level build leaves the update and
/// the window their cores. Elsewhere a lowered thread may not be raised again;
/// the workers end with their work, but they are left as they are.
#[cfg(windows)]
pub(crate) fn below_normal() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
    };
    // SAFETY: the current thread's pseudo-handle is always valid and is not
    // closed. A refusal leaves the thread at normal priority.
    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) };
}

#[cfg(not(windows))]
pub(crate) fn below_normal() {}
