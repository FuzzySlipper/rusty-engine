// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Linux: libcef loaded when CEF is first started, not when the process
//! starts. The `cef` crate calls libcef's C functions by name; this module
//! defines each one it calls and forwards it to the library loaded by
//! [`load`]. With every reference satisfied here, the linker (`--as-needed`)
//! drops `libcef.so` from the executable's dependencies, so a host that never
//! starts Chromium runs where libcef is absent.
//!
//! A new libcef function the `cef` crate starts calling links libcef again,
//! which brings the dependency back rather than failing.

use std::ffi::{c_char, c_int, c_void};
use std::path::Path;
use std::sync::OnceLock;

use cef::sys::*;

static TABLE: OnceLock<Table> = OnceLock::new();

macro_rules! forward {
    ($(fn $name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?;)*) => {
        #[allow(non_snake_case)]
        struct Table {
            $($name: unsafe extern "C" fn($($ty),*) $(-> $ret)?,)*
        }

        impl Table {
            /// # Safety
            /// `library` is libcef, whose functions have these signatures.
            unsafe fn resolve(library: &dl::Library) -> Result<Self, String> {
                // SAFETY: the caller's.
                unsafe {
                    Ok(Self {
                        $($name: *library
                            .get(concat!(stringify!($name), "\0").as_bytes())
                            .map_err(|e| format!("libcef has no {}: {e}", stringify!($name)))?,)*
                    })
                }
            }
        }

        $(
            #[unsafe(no_mangle)]
            #[doc(hidden)]
            pub unsafe extern "C" fn $name($($arg: $ty),*) $(-> $ret)? {
                // SAFETY: the same function in libcef, with the caller's
                // arguments and their guarantees.
                unsafe { (table().$name)($($arg),*) }
            }
        )*
    };
}

forward! {
    fn cef_api_hash(version: c_int, entry: c_int) -> *const c_char;
    fn cef_browser_host_create_browser_sync(
        window_info: *const cef_window_info_t,
        client: *mut _cef_client_t,
        url: *const cef_string_t,
        settings: *const _cef_browser_settings_t,
        extra_info: *mut _cef_dictionary_value_t,
        request_context: *mut _cef_request_context_t
    ) -> *mut cef_browser_t;
    fn cef_do_message_loop_work();
    fn cef_execute_process(
        args: *const cef_main_args_t,
        application: *mut cef_app_t,
        windows_sandbox_info: *mut c_void
    ) -> c_int;
    fn cef_initialize(
        args: *const cef_main_args_t,
        settings: *const _cef_settings_t,
        application: *mut cef_app_t,
        windows_sandbox_info: *mut c_void
    ) -> c_int;
    fn cef_process_message_create(name: *const cef_string_t) -> *mut cef_process_message_t;
    fn cef_request_context_create_context(
        settings: *const _cef_request_context_settings_t,
        handler: *mut _cef_request_context_handler_t
    ) -> *mut cef_request_context_t;
    fn cef_shutdown();
    fn cef_string_list_alloc() -> cef_string_list_t;
    fn cef_string_list_free(list: cef_string_list_t);
    fn cef_string_list_size(list: cef_string_list_t) -> usize;
    fn cef_string_list_value(list: cef_string_list_t, index: usize, value: *mut cef_string_t) -> c_int;
    fn cef_string_userfree_utf16_free(string: cef_string_userfree_utf16_t);
    fn cef_string_utf16_clear(string: *mut cef_string_utf16_t);
    fn cef_string_utf16_set(
        source: *const char16_t,
        length: usize,
        output: *mut cef_string_utf16_t,
        copy: c_int
    ) -> c_int;
    fn cef_string_utf16_to_utf8(
        source: *const char16_t,
        length: usize,
        output: *mut cef_string_utf8_t
    ) -> c_int;
    fn cef_string_utf8_clear(string: *mut cef_string_utf8_t);
    fn cef_string_utf8_to_utf16(
        source: *const c_char,
        length: usize,
        output: *mut cef_string_utf16_t
    ) -> c_int;
}

fn table() -> &'static Table {
    TABLE.get().unwrap_or_else(|| {
        eprintln!("welding: a CEF function was called before libcef was loaded");
        std::process::abort()
    })
}

/// Load `cef_dir/libcef.so`, or `libcef.so` from the dynamic loader's search
/// path when `cef_dir` has none (CEF's subprocesses are started without the
/// host's directory), once per process.
pub(crate) fn load(cef_dir: &Path) -> Result<(), String> {
    if TABLE.get().is_some() {
        return Ok(());
    }
    let beside = cef_dir.join("libcef.so");
    let path = if beside.is_file() {
        beside.into_os_string()
    } else {
        "libcef.so".into()
    };
    // SAFETY: libcef's initialisers are CEF's own; loading it globally is
    // what linking it would have done. It stays loaded for the process.
    let library = unsafe {
        dl::os::unix::Library::open(
            Some(&path),
            dl::os::unix::RTLD_NOW | dl::os::unix::RTLD_GLOBAL,
        )
    }
    .map_err(|e| format!("load {}: {e}", Path::new(&path).display()))?;
    let library = dl::Library::from(library);
    // SAFETY: this is libcef, whose C API these signatures are.
    let table = unsafe { Table::resolve(&library) }?;
    std::mem::forget(library);
    let _ = TABLE.set(table);
    Ok(())
}
