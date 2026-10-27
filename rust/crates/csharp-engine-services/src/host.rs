//! The `Host` service: whether the host can end at the product's request,
//! the request itself, and what a setup screen asks of the machine. A host
//! that can end (one with a window) enables it and watches the returned flag,
//! stopping as closing the window does. Folder pickers and tool runs are
//! `svc-host` workers; each product call begins with a fresh snapshot of
//! every one, so a call reads one consistent state.

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use csharp_engine_abi::{
    NativeByteResult, NativeFolderChoiceHandle, NativeFolderChoiceReadout,
    NativeFolderChoiceRequest, NativeFolderChoiceState, NativeHostApi, NativeHostExitRequest,
    NativeHostReadout, NativeInstalledContentRequest, NativeOperationErrorReceipt,
    NativeToolRunHandle, NativeToolRunReadout, NativeToolRunRequest, NativeToolRunState,
    NativeUtf8Slice,
};
use svc_host::{FolderChoice, FolderChoiceState, ToolRun, ToolRunSnapshot, ToolRunState};

use crate::composition::{borrowed_utf8, ABI_OK};
use crate::operation_diagnostics::{clear_receipt, refuse, BorrowedResult};
use crate::CsharpEngineServicesError;

struct Folder {
    choice: FolderChoice,
    snapshot: FolderChoiceState,
}

struct Run {
    run: ToolRun,
    snapshot: ToolRunSnapshot,
}

#[derive(Default)]
pub(crate) struct RuntimeHostBridge {
    /// Present once the host has enabled exit.
    exit: Option<Arc<AtomicBool>>,
    /// Where tools run and install containers.
    installed_content: Option<Arc<str>>,
    /// Each declared tool's executable by its project name.
    tools: BTreeMap<String, PathBuf>,
    folders: BTreeMap<u64, Folder>,
    runs: BTreeMap<u64, Run>,
    next: u64,
    borrowed: BorrowedResult,
}

fn error(code: &'static str, message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, message)
}

impl RuntimeHostBridge {
    /// Makes exit available and returns the flag a request sets.
    pub(crate) fn enable_exit(&mut self) -> Arc<AtomicBool> {
        Arc::clone(self.exit.get_or_insert_with(Arc::default))
    }

    pub(crate) fn bind_installed_content(&mut self, directory: &Path) {
        self.installed_content = Some(Arc::from(ordinary_path(directory)));
    }

    /// The tools the product declares, each by name with its executable.
    pub(crate) fn bind_tools(&mut self, tools: BTreeMap<String, PathBuf>) {
        self.tools = tools;
    }

    pub(crate) fn begin_call(&mut self) {
        for folder in self.folders.values_mut() {
            if folder.snapshot == FolderChoiceState::Pending {
                folder.snapshot = folder.choice.snapshot();
            }
        }
        for run in self.runs.values_mut() {
            if run.snapshot.state == ToolRunState::Running {
                run.snapshot = run.run.snapshot();
            }
        }
    }

    fn handle(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn choose_folder(
        &mut self,
        request: &NativeFolderChoiceRequest,
    ) -> Result<NativeFolderChoiceHandle, CsharpEngineServicesError> {
        let choice = FolderChoice::start(utf8(request.title, "folder choice title")?);
        let snapshot = choice.snapshot();
        let value = self.handle();
        self.folders.insert(value, Folder { choice, snapshot });
        Ok(NativeFolderChoiceHandle { value })
    }

    fn read_folder_choice(
        &mut self,
        handle: NativeFolderChoiceHandle,
    ) -> Result<NativeFolderChoiceReadout, CsharpEngineServicesError> {
        let folder = self
            .folders
            .get(&handle.value)
            .ok_or_else(|| error("ENGINE_HOST_FOLDER_CHOICE", "unknown folder choice"))?;
        let state = match &folder.snapshot {
            FolderChoiceState::Pending => NativeFolderChoiceState::Pending,
            FolderChoiceState::Cancelled => NativeFolderChoiceState::Cancelled,
            FolderChoiceState::Chosen(_) => NativeFolderChoiceState::Chosen,
            FolderChoiceState::Unavailable(_) => NativeFolderChoiceState::Unavailable,
        };
        Ok(NativeFolderChoiceReadout { state })
    }

    fn read_folder_choice_text(
        &mut self,
        handle: NativeFolderChoiceHandle,
        diagnostic: bool,
    ) -> Result<NativeByteResult, CsharpEngineServicesError> {
        let folder = self
            .folders
            .get(&handle.value)
            .ok_or_else(|| error("ENGINE_HOST_FOLDER_CHOICE", "unknown folder choice"))?;
        let text = match (&folder.snapshot, diagnostic) {
            // The portal and the Windows dialog hand back UTF-8 paths.
            (FolderChoiceState::Chosen(path), false) => path.to_string_lossy().into_owned(),
            (FolderChoiceState::Unavailable(why), true) => why.clone(),
            _ => String::new(),
        };
        Ok(self.hold_text(Arc::from(text)))
    }

    fn hold_text(&mut self, text: Arc<str>) -> NativeByteResult {
        let result = NativeByteResult {
            bytes: text.as_ptr(),
            len: text.len(),
        };
        self.borrowed.hold(text);
        result
    }

    fn run_tool(
        &mut self,
        request: &NativeToolRunRequest,
    ) -> Result<NativeToolRunHandle, CsharpEngineServicesError> {
        let name = utf8(request.tool, "tool name")?;
        let executable = self.tools.get(name).ok_or_else(|| {
            let declared = self.tools.keys().map(String::as_str).collect::<Vec<_>>();
            error(
                "ENGINE_HOST_TOOL_UNDECLARED",
                format!(
                    "`{name}` is not one of the product's RustyEngineProductTool tools ({})",
                    if declared.is_empty() {
                        "it declares none".to_owned()
                    } else {
                        declared.join(", ")
                    }
                ),
            )
        })?;
        let arguments = if request.arguments_len == 0 {
            &[][..]
        } else if request.arguments.is_null() {
            return Err(error(
                "ENGINE_HOST_TOOL_ARGUMENTS",
                "tool arguments had length without values",
            ));
        } else {
            // SAFETY: the caller's array is valid for the duration of this call.
            unsafe { std::slice::from_raw_parts(request.arguments, request.arguments_len) }
        };
        let arguments = arguments
            .iter()
            .map(|argument| utf8(argument.value, "tool argument").map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        let working_directory = match &self.installed_content {
            Some(directory) => PathBuf::from(&**directory),
            None => executable.parent().map(PathBuf::from).unwrap_or_default(),
        };
        // Under `rusty dev` the directory a tool installs into may not exist
        // until the first install; a failure here shows as the run failing
        // to start.
        let _ = std::fs::create_dir_all(&working_directory);
        let run = ToolRun::start(executable, &arguments, &working_directory);
        let snapshot = run.snapshot();
        let value = self.handle();
        self.runs.insert(value, Run { run, snapshot });
        Ok(NativeToolRunHandle { value })
    }

    fn run(&self, handle: NativeToolRunHandle) -> Result<&Run, CsharpEngineServicesError> {
        self.runs
            .get(&handle.value)
            .ok_or_else(|| error("ENGINE_HOST_TOOL_RUN", "unknown tool run"))
    }

    fn read_tool_run(
        &self,
        handle: NativeToolRunHandle,
    ) -> Result<NativeToolRunReadout, CsharpEngineServicesError> {
        let snapshot = &self.run(handle)?.snapshot;
        let (state, exit_code) = match snapshot.state {
            ToolRunState::Running => (NativeToolRunState::Running, 0),
            ToolRunState::Exited(code) => (NativeToolRunState::Exited, code),
            ToolRunState::Cancelled => (NativeToolRunState::Cancelled, 0),
            ToolRunState::FailedToStart => (NativeToolRunState::FailedToStart, 0),
        };
        Ok(NativeToolRunReadout {
            state,
            exit_code,
            output_lines: snapshot.lines,
        })
    }

    fn read_tool_run_output(
        &mut self,
        handle: NativeToolRunHandle,
    ) -> Result<NativeByteResult, CsharpEngineServicesError> {
        let output = Arc::clone(&self.run(handle)?.snapshot.output);
        Ok(self.hold_text(output))
    }
}

/// A canonical Windows path is verbatim (`\\?\C:\…`); a tool's argument and
/// the text a player reads take the ordinary form.
fn ordinary_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Some(share) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{share}");
    }
    match text.strip_prefix(r"\\?\") {
        Some(drive) if drive.as_bytes().get(1) == Some(&b':') => drive.to_owned(),
        _ => text.into_owned(),
    }
}

/// The borrowed text, valid only during the ABI call that supplied it.
fn utf8<'call>(
    value: NativeUtf8Slice,
    field: &'static str,
) -> Result<&'call str, CsharpEngineServicesError> {
    unsafe { borrowed_utf8(value.bytes, value.len, field) }
}

pub(crate) fn api(bridge: &mut RuntimeHostBridge) -> NativeHostApi {
    NativeHostApi {
        context: (bridge as *mut RuntimeHostBridge).cast(),
        read,
        request_exit,
        read_installed_content,
        choose_folder,
        read_folder_choice,
        read_folder_choice_path,
        read_folder_choice_diagnostic,
        destroy_folder_choice,
        run_tool,
        read_tool_run,
        read_tool_run_output,
        cancel_tool_run,
        destroy_tool_run,
    }
}

/// Runs one refusable operation: writes its result or reports its refusal
/// through the receipt.
unsafe fn refusable<Input, Output>(
    context: *mut c_void,
    input: Input,
    result: *mut Output,
    receipt: *mut NativeOperationErrorReceipt,
    operation: impl FnOnce(&mut RuntimeHostBridge, Input) -> Result<Output, CsharpEngineServicesError>,
) -> i32 {
    clear_receipt(receipt);
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeHostBridge>() };
    match operation(bridge, input) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => refuse(&error, receipt),
    }
}

/// Dereferences a borrowed request, or refuses a null one.
unsafe fn request<'a, T>(request: *const T) -> Result<&'a T, CsharpEngineServicesError> {
    unsafe { request.as_ref() }.ok_or_else(|| error("ENGINE_HOST_REQUEST", "the request was null"))
}

unsafe extern "C" fn read(
    context: *mut c_void,
    output: *mut NativeHostReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, (), output, receipt, |bridge, ()| {
            Ok(NativeHostReadout {
                exit_available: bridge.exit.is_some(),
            })
        })
    }
}

unsafe extern "C" fn read_installed_content(
    context: *mut c_void,
    _request: *const NativeInstalledContentRequest,
    result: *mut NativeByteResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, (), result, receipt, |bridge, ()| {
            let directory = bridge
                .installed_content
                .clone()
                .unwrap_or_else(|| Arc::from(""));
            Ok(bridge.hold_text(directory))
        })
    }
}

unsafe extern "C" fn request_exit(
    context: *mut c_void,
    _request: *const NativeHostExitRequest,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &*context.cast::<RuntimeHostBridge>() };
    match &bridge.exit {
        Some(exit) => {
            exit.store(true, Ordering::Release);
            ABI_OK
        }
        None => refuse(
            &CsharpEngineServicesError::new(
                "ENGINE_HOST_EXIT_UNAVAILABLE",
                "this output has no window to close; streamed output stops when `rusty dev` does",
            ),
            error,
        ),
    }
}

unsafe extern "C" fn choose_folder(
    context: *mut c_void,
    input: *const NativeFolderChoiceRequest,
    result: *mut NativeFolderChoiceHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, input, result, receipt, |bridge, input| {
            bridge.choose_folder(request(input)?)
        })
    }
}

unsafe extern "C" fn read_folder_choice(
    context: *mut c_void,
    handle: NativeFolderChoiceHandle,
    result: *mut NativeFolderChoiceReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, handle, result, receipt, |bridge, handle| {
            bridge.read_folder_choice(handle)
        })
    }
}

unsafe extern "C" fn read_folder_choice_path(
    context: *mut c_void,
    handle: NativeFolderChoiceHandle,
    result: *mut NativeByteResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, handle, result, receipt, |bridge, handle| {
            bridge.read_folder_choice_text(handle, false)
        })
    }
}

unsafe extern "C" fn read_folder_choice_diagnostic(
    context: *mut c_void,
    handle: NativeFolderChoiceHandle,
    result: *mut NativeByteResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, handle, result, receipt, |bridge, handle| {
            bridge.read_folder_choice_text(handle, true)
        })
    }
}

/// Forgets a choice. An open dialog stays until the player answers it.
unsafe extern "C" fn destroy_folder_choice(
    context: *mut c_void,
    handle: NativeFolderChoiceHandle,
) -> i32 {
    match unsafe { context.cast::<RuntimeHostBridge>().as_mut() } {
        Some(bridge) => {
            if bridge.folders.remove(&handle.value).is_some() {
                ABI_OK
            } else {
                0
            }
        }
        None => 0,
    }
}

unsafe extern "C" fn run_tool(
    context: *mut c_void,
    input: *const NativeToolRunRequest,
    result: *mut NativeToolRunHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, input, result, receipt, |bridge, input| {
            bridge.run_tool(request(input)?)
        })
    }
}

unsafe extern "C" fn read_tool_run(
    context: *mut c_void,
    handle: NativeToolRunHandle,
    result: *mut NativeToolRunReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, handle, result, receipt, |bridge, handle| {
            bridge.read_tool_run(handle)
        })
    }
}

unsafe extern "C" fn read_tool_run_output(
    context: *mut c_void,
    handle: NativeToolRunHandle,
    result: *mut NativeByteResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, handle, result, receipt, |bridge, handle| {
            bridge.read_tool_run_output(handle)
        })
    }
}

unsafe extern "C" fn cancel_tool_run(context: *mut c_void, handle: NativeToolRunHandle) -> i32 {
    match unsafe { context.cast::<RuntimeHostBridge>().as_mut() } {
        Some(bridge) => match bridge.runs.get_mut(&handle.value) {
            Some(run) => {
                run.run.cancel();
                run.snapshot = run.run.snapshot();
                ABI_OK
            }
            None => 0,
        },
        None => 0,
    }
}

/// Releases a run, cancelling it if it is still going.
unsafe extern "C" fn destroy_tool_run(context: *mut c_void, handle: NativeToolRunHandle) -> i32 {
    match unsafe { context.cast::<RuntimeHostBridge>().as_mut() } {
        Some(bridge) => {
            if bridge.runs.remove(&handle.value).is_some() {
                ABI_OK
            } else {
                0
            }
        }
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation_diagnostics::{empty_receipt, receipt_codes};
    use std::time::{Duration, Instant};

    fn slice(bytes: &[u8]) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: bytes.as_ptr(),
            len: bytes.len(),
        }
    }

    fn read_host(api: &NativeHostApi) -> NativeHostReadout {
        let mut readout = NativeHostReadout::default();
        assert_eq!(
            unsafe { (api.read)(api.context, &mut readout, std::ptr::null_mut()) },
            ABI_OK
        );
        readout
    }

    /// Streamed output reports exit unavailable and refuses the request; once
    /// the host enables it (window output), a request sets the flag it watches.
    #[test]
    fn exit_is_refused_until_the_host_enables_it_and_then_sets_its_flag() {
        let mut bridge = RuntimeHostBridge::default();
        let api = api(&mut bridge);
        let request = |api: &NativeHostApi| {
            let mut receipt = empty_receipt();
            let status = unsafe {
                (api.request_exit)(api.context, &NativeHostExitRequest::default(), &mut receipt)
            };
            (status, receipt_codes(&receipt))
        };
        assert!(!read_host(&api).exit_available);
        assert_eq!(
            request(&api),
            (0, vec!["ENGINE_HOST_EXIT_UNAVAILABLE".to_owned()])
        );

        let exit = bridge.enable_exit();
        let api = super::api(&mut bridge);
        assert!(read_host(&api).exit_available);
        assert!(!exit.load(Ordering::Acquire));
        assert_eq!(request(&api), (ABI_OK, Vec::new()));
        assert!(exit.load(Ordering::Acquire));
    }

    fn run_tool(api: &NativeHostApi, tool: &str, arguments: &[&str]) -> (i32, Vec<String>, u64) {
        let arguments = arguments
            .iter()
            .map(|value| csharp_engine_abi::NativeToolArgument {
                value: slice(value.as_bytes()),
            })
            .collect::<Vec<_>>();
        let request = NativeToolRunRequest {
            tool: slice(tool.as_bytes()),
            arguments: arguments.as_ptr(),
            arguments_len: arguments.len(),
        };
        let mut handle = NativeToolRunHandle::default();
        let mut receipt = empty_receipt();
        let status = unsafe { (api.run_tool)(api.context, &request, &mut handle, &mut receipt) };
        (status, receipt_codes(&receipt), handle.value)
    }

    unsafe fn text<'a>(bytes: *const u8, len: usize) -> &'a str {
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(bytes, len) }).unwrap()
    }

    #[test]
    fn a_verbatim_windows_directory_reads_in_its_ordinary_form() {
        let read = |text: &str| ordinary_path(Path::new(text));
        assert_eq!(read(r"\\?\C:\Games\Fixture"), r"C:\Games\Fixture");
        assert_eq!(
            read(r"\\?\UNC\server\share\Fixture"),
            r"\\server\share\Fixture"
        );
        assert_eq!(read("/home/player/Fixture"), "/home/player/Fixture");
    }

    /// Only a declared tool runs. It runs in the installed-content directory
    /// the readout names, and a call reads the snapshot taken when the call
    /// began.
    #[cfg(unix)]
    #[test]
    fn only_a_declared_tool_runs_and_it_runs_where_containers_install() {
        let root = tempfile::tempdir().unwrap();
        // Created by the first run, as under `rusty dev` before any install.
        let installed = root.path().join("installed");
        let mut bridge = RuntimeHostBridge::default();
        bridge.bind_installed_content(&installed);
        bridge.bind_tools(BTreeMap::from([(
            "Shell".to_owned(),
            PathBuf::from("/bin/sh"),
        )]));
        let api = api(&mut bridge);

        let mut directory = NativeByteResult {
            bytes: std::ptr::null(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                (api.read_installed_content)(
                    api.context,
                    &NativeInstalledContentRequest::default(),
                    &mut directory,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe { text(directory.bytes, directory.len) },
            installed.to_str().unwrap()
        );

        let (status, codes, _) = run_tool(&api, "sh", &["-c", "touch escaped"]);
        assert_eq!(
            (status, codes),
            (0, vec!["ENGINE_HOST_TOOL_UNDECLARED".to_owned()])
        );

        let (status, codes, value) =
            run_tool(&api, "Shell", &["-c", "pwd; touch installed; exit 4"]);
        assert_eq!((status, codes), (ABI_OK, Vec::new()));
        let handle = NativeToolRunHandle { value };
        let read = || {
            let mut readout = NativeToolRunReadout {
                state: NativeToolRunState::Running,
                exit_code: 0,
                output_lines: 0,
            };
            assert_eq!(
                unsafe {
                    (api.read_tool_run)(api.context, handle, &mut readout, std::ptr::null_mut())
                },
                ABI_OK
            );
            readout
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut readout = read();
        while readout.state == NativeToolRunState::Running && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            bridge.begin_call();
            readout = read();
        }
        assert_eq!(readout.state, NativeToolRunState::Exited);
        assert_eq!(readout.exit_code, 4);
        assert_eq!(readout.output_lines, 1);
        let mut output = NativeByteResult {
            bytes: std::ptr::null(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                (api.read_tool_run_output)(api.context, handle, &mut output, std::ptr::null_mut())
            },
            ABI_OK
        );
        assert_eq!(
            unsafe { text(output.bytes, output.len) }.trim_end(),
            installed.canonicalize().unwrap().to_str().unwrap()
        );
        assert!(installed.join("installed").exists());
        assert!(!installed.join("escaped").exists());
        assert_eq!(
            unsafe { (api.destroy_tool_run)(api.context, handle) },
            ABI_OK
        );
    }
}
