//! The host the product runs in: what it offers the product, the product's
//! request to end it (a title menu's Quit), and what a setup screen asks of
//! the machine: the platform's folder picker and runs of the product's own
//! declared tools. Pickers and tool runs go on Engine threads; their outcome
//! reaches the product only as readouts refreshed at the start of each
//! product call.
use crate::{NativeByteResult, NativeOperationErrorReceipt, NativeUtf8Slice};
use std::ffi::c_void;

/// `exit_available` is true when the product has its own window (window
/// output): a requested exit closes it and stops the host, as closing the
/// window does. Streamed output has no window to close, so a product hides
/// its Quit there.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeHostReadout {
    pub exit_available: bool,
}

pub type NativeReadHost = unsafe extern "C" fn(
    *mut c_void,
    *mut NativeHostReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;

/// Explicit empty request, as the generated direct API takes one input.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeHostExitRequest {
    pub reserved: u32,
}

/// Explicit empty request, as the generated direct API takes one input.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeInstalledContentRequest {
    pub reserved: u32,
}

pub type NativeReadInstalledContent = unsafe extern "C" fn(
    *mut c_void,
    *const NativeInstalledContentRequest,
    *mut NativeByteResult,
    *mut NativeOperationErrorReceipt,
) -> i32;

/// Closes the window and stops the host once the current product call
/// returns. Refuses with `ENGINE_HOST_EXIT_UNAVAILABLE` where
/// `exit_available` is false.
pub type NativeRequestHostExit = unsafe extern "C" fn(
    *mut c_void,
    *const NativeHostExitRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeFolderChoiceHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeFolderChoiceRequest {
    /// The dialog's title.
    pub title: NativeUtf8Slice,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFolderChoiceState {
    /// The dialog is open.
    Pending = 0,
    /// The player chose a folder; read its path.
    Chosen = 1,
    /// The player closed the dialog without choosing.
    Cancelled = 2,
    /// No picker could be shown (on Linux, no xdg-desktop-portal answered).
    Unavailable = 3,
}

/// The choice as of the start of the current product call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeFolderChoiceReadout {
    pub state: NativeFolderChoiceState,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeToolRunHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeToolArgument {
    pub value: NativeUtf8Slice,
}

/// Runs one of the product's declared tools (`RustyEngineProductTool`) by
/// its project name, with the product's arguments, in the installed-content
/// directory. Only declared tools run.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeToolRunRequest {
    pub tool: NativeUtf8Slice,
    pub arguments: *const NativeToolArgument,
    pub arguments_len: usize,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeToolRunState {
    /// The tool is still going.
    Running = 0,
    /// The tool ended by itself; `exit_code` says how.
    Exited = 1,
    /// The product cancelled it and the tool has stopped.
    Cancelled = 2,
    /// The executable could not be started; its output says why.
    FailedToStart = 3,
}

/// The run as of the start of the current product call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeToolRunReadout {
    pub state: NativeToolRunState,
    /// Once `Exited`: the tool's exit code (-1 when a signal ended it).
    pub exit_code: i32,
    /// Every line the tool has written so far, on standard output or error.
    /// It grows when there is new output to show.
    pub output_lines: u64,
}

pub type NativeChooseFolder = unsafe extern "C" fn(
    *mut c_void,
    *const NativeFolderChoiceRequest,
    *mut NativeFolderChoiceHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadFolderChoice = unsafe extern "C" fn(
    *mut c_void,
    NativeFolderChoiceHandle,
    *mut NativeFolderChoiceReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadFolderChoiceText = unsafe extern "C" fn(
    *mut c_void,
    NativeFolderChoiceHandle,
    *mut NativeByteResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyFolderChoice =
    unsafe extern "C" fn(*mut c_void, NativeFolderChoiceHandle) -> i32;
pub type NativeRunTool = unsafe extern "C" fn(
    *mut c_void,
    *const NativeToolRunRequest,
    *mut NativeToolRunHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadToolRun = unsafe extern "C" fn(
    *mut c_void,
    NativeToolRunHandle,
    *mut NativeToolRunReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadToolRunOutput = unsafe extern "C" fn(
    *mut c_void,
    NativeToolRunHandle,
    *mut NativeByteResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCancelToolRun = unsafe extern "C" fn(*mut c_void, NativeToolRunHandle) -> i32;
pub type NativeDestroyToolRun = unsafe extern "C" fn(*mut c_void, NativeToolRunHandle) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHostApi {
    pub context: *mut c_void,
    pub read: NativeReadHost,
    pub request_exit: NativeRequestHostExit,
    /// The absolute directory a relative `Content.OpenContainer` path
    /// resolves in and tools run in, as UTF-8: beside `product.rpak` in a
    /// release, the project's installed-content directory under `rusty dev`.
    /// Empty where the host names none (the test host).
    pub read_installed_content: NativeReadInstalledContent,
    /// Opens the platform's folder picker; the answer arrives in a later call.
    pub choose_folder: NativeChooseFolder,
    pub read_folder_choice: NativeReadFolderChoice,
    /// The chosen folder's absolute path as UTF-8 once `Chosen`; empty
    /// otherwise.
    pub read_folder_choice_path: NativeReadFolderChoiceText,
    /// Why no picker could be shown, as UTF-8, once `Unavailable`; empty
    /// otherwise.
    pub read_folder_choice_diagnostic: NativeReadFolderChoiceText,
    pub destroy_folder_choice: NativeDestroyFolderChoice,
    /// Starts a declared tool. An undeclared name is refused with
    /// `ENGINE_HOST_TOOL_UNDECLARED`.
    pub run_tool: NativeRunTool,
    pub read_tool_run: NativeReadToolRun,
    /// The tool's recent output lines as UTF-8 text, oldest first, borrowed
    /// until the next Host call.
    pub read_tool_run_output: NativeReadToolRunOutput,
    /// Asks a running tool to end. The run reads `Running` until the tool
    /// has stopped, then `Cancelled`; a tool that exited first keeps its
    /// `Exited` code.
    pub cancel_tool_run: NativeCancelToolRun,
    /// Disposing a run still going cancels it.
    pub destroy_tool_run: NativeDestroyToolRun,
}
