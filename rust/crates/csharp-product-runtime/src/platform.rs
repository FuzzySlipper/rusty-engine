//! What the supervisor and the runtime it starts do differently on Unix and
//! Windows: how the runtime inherits the supervisor's listener, how a runtime
//! is kept from terminal Ctrl+C and killed, how the supervisor waits for a
//! connection while no runtime serves, and, on Windows, the private copy a
//! runtime loads its assemblies from.

#[cfg(unix)]
pub(crate) use unix::*;
#[cfg(windows)]
pub(crate) use windows::*;

#[cfg(unix)]
mod unix {
    use std::{
        net::TcpListener,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::process::CommandExt,
        },
        process::{Child, Command},
    };

    /// The listener a runtime inherits: a file descriptor.
    pub(crate) type RawListener = std::os::fd::RawFd;

    pub(crate) fn shared_listener(listener: &TcpListener) -> Result<RawListener, String> {
        Ok(listener.as_raw_fd())
    }

    /// Terminal Ctrl+C and foreground-group signals reach only the
    /// supervisor; CoreCLR stops through its stdin instead. The runtime
    /// inherits the listener across exec.
    pub(crate) fn runtime_command(command: &mut Command, listener: RawListener) {
        command.process_group(0);
        // SAFETY: only the async-signal-safe fcntl runs between fork and
        // exec. It clears close-on-exec so the runtime inherits the listener.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(listener, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    /// Kills the runtime and the process group it leads.
    pub(crate) fn kill_runtime(child: &mut Child) {
        // SAFETY: killpg only sends a signal to that group.
        unsafe {
            libc::killpg(child.id() as libc::pid_t, libc::SIGKILL);
        }
    }

    pub(crate) fn runtime_ended(_pid: u32) {}

    pub(crate) fn wait_readable(listener: &TcpListener, timeout_ms: i32) -> bool {
        let mut poll = libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd for the duration of the call.
        let ready = unsafe { libc::poll(&mut poll, 1, timeout_ms) };
        ready > 0 && poll.revents & libc::POLLIN != 0
    }

    /// Takes sole use of the inherited listener, keeping it from anything
    /// this runtime starts.
    pub(crate) fn take_listener(listener: RawListener) -> Result<TcpListener, String> {
        // SAFETY: fcntl on a descriptor this process owns.
        if unsafe { libc::fcntl(listener, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(format!(
                "PRODUCT_HOST_BIND: could not keep the listener private: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: the supervisor bound this listener and hands this process
        // sole use of the descriptor number.
        Ok(unsafe { TcpListener::from_raw_fd(listener) })
    }
}

#[cfg(windows)]
mod windows {
    use std::{
        net::TcpListener,
        os::windows::{
            io::{AsRawSocket, FromRawSocket},
            process::CommandExt,
        },
        path::{Path, PathBuf},
        process::{Child, Command},
    };

    use windows_sys::Win32::{
        Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT},
        Networking::WinSock::{WSAPoll, WSAStartup, POLLRDNORM, SOCKET, WSADATA, WSAPOLLFD},
    };

    /// The listener a runtime inherits: a socket handle.
    pub(crate) type RawListener = std::os::windows::io::RawSocket;

    /// Keeps Ctrl+C in the console from reaching the runtime; it stops
    /// through its stdin instead.
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

    fn set_inherit(listener: RawListener, inherit: bool) -> Result<(), String> {
        let flags = if inherit { HANDLE_FLAG_INHERIT } else { 0 };
        // SAFETY: changes only the inherit flag of a socket this process owns.
        if unsafe { SetHandleInformation(listener as HANDLE, HANDLE_FLAG_INHERIT, flags) } == 0 {
            return Err(format!(
                "PRODUCT_HOST_BIND: could not set the listener's inheritance: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    /// Lets every runtime the supervisor starts inherit the listener.
    pub(crate) fn shared_listener(listener: &TcpListener) -> Result<RawListener, String> {
        let socket = listener.as_raw_socket();
        set_inherit(socket, true)?;
        Ok(socket)
    }

    pub(crate) fn runtime_command(command: &mut Command, _listener: RawListener) {
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    /// Kills the runtime. Chromium's helpers for a window end with it.
    pub(crate) fn kill_runtime(child: &mut Child) {
        let _ = child.kill();
    }

    /// Polls rather than switching the socket to non-blocking, which the
    /// runtime's inherited copy would share.
    pub(crate) fn wait_readable(listener: &TcpListener, timeout_ms: i32) -> bool {
        let mut poll = WSAPOLLFD {
            fd: listener.as_raw_socket() as SOCKET,
            events: POLLRDNORM,
            revents: 0,
        };
        // SAFETY: one valid WSAPOLLFD for the duration of the call.
        let ready = unsafe { WSAPoll(&mut poll, 1, timeout_ms) };
        ready > 0 && poll.revents & POLLRDNORM != 0
    }

    /// Takes sole use of the inherited listener, keeping it from anything
    /// this runtime starts.
    pub(crate) fn take_listener(listener: RawListener) -> Result<TcpListener, String> {
        // The standard library starts Winsock when it creates a socket, and
        // this process creates none before it uses the inherited one.
        // SAFETY: WSAStartup only writes the WSADATA it is given.
        let started = unsafe {
            let mut data: WSADATA = std::mem::zeroed();
            WSAStartup(0x0202, &mut data)
        };
        if started != 0 {
            return Err(format!(
                "PRODUCT_HOST_BIND: could not start Winsock: error {started}"
            ));
        }
        set_inherit(listener, false)?;
        // SAFETY: the supervisor bound this listener and hands this process
        // sole use of the socket handle.
        Ok(unsafe { TcpListener::from_raw_socket(listener) })
    }

    /// Removes the private copy an exited runtime loaded from.
    pub(crate) fn runtime_ended(pid: u32) {
        let _ = std::fs::remove_dir_all(private_copy_root().join(pid.to_string()));
    }

    fn private_copy_root() -> PathBuf {
        std::env::temp_dir().join("rusty-product-runtime")
    }

    /// Windows keeps a loaded assembly's file from being replaced, so a
    /// supervised runtime loads its CoreCLR artifacts from a private copy and
    /// `rusty dev` can restage the Product while it runs. Returns the copied
    /// directory, which replaces `directory` in the artifact paths.
    pub(crate) fn private_loader_copy(directory: &Path) -> Result<PathBuf, String> {
        let root = private_copy_root();
        // Copies left by runtimes that ended with their supervisor; a running
        // one's loaded files refuse removal.
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
        let copy = root.join(std::process::id().to_string());
        copy_directory(directory, &copy).map_err(|error| {
            format!(
                "PRODUCT_HOST_RUNTIME_COPY: could not copy `{}` to `{}`: {error}",
                directory.display(),
                copy.display()
            )
        })?;
        Ok(copy)
    }

    fn copy_directory(from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let target = to.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_directory(&entry.path(), &target)?;
            } else {
                std::fs::copy(entry.path(), target)?;
            }
        }
        Ok(())
    }
}
