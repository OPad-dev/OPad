//! Named pipe transport.
//!
//! A `NamedPipeServer` is consumed by the client that connects to it, so the
//! accept loop cannot have the Unix shape: every accepted connection has to be
//! replaced by a freshly created instance (§W0-1.4). [`IpcListener`] keeps one
//! unconnected instance ready at all times, which also keeps the pipe name
//! alive between clients.
//!
//! A pipe created with default security is reachable by every process on the
//! machine, across sessions. Every instance is therefore created with a DACL
//! granting the creating user and `SYSTEM` and nobody else, so the 0700/0600
//! guarantee the Unix socket has (§P2-6) is not silently weakened (§W0-2).

use crate::IpcError;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, LocalFree, ERROR_ACCESS_DENIED, ERROR_PIPE_BUSY, HANDLE,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Client end of an IPC connection.
pub type IpcStream = NamedPipeClient;
/// Server end of an accepted IPC connection.
pub type IpcServerStream = NamedPipeServer;

/// How long a client retries while every pipe instance is busy. The daemon
/// creates the replacement instance immediately after a client connects, so
/// the window this covers is microseconds wide.
const PIPE_BUSY_TIMEOUT: Duration = Duration::from_secs(1);
const PIPE_BUSY_RETRY_INTERVAL: Duration = Duration::from_millis(20);

/// Resolves the standard pipe name, one pipe per user (§W0-1.3).
///
/// The SID mirrors the per-uid socket directory on Unix, so two users logged
/// into one machine get separate pipes.
pub fn get_socket_path() -> PathBuf {
    let sid = current_user_sid().unwrap_or_else(|| {
        tracing::warn!(
            "Could not resolve the current user SID; falling back to a shared pipe name"
        );
        "default".to_string()
    });
    PathBuf::from(format!(r"\\.\pipe\osupad-ipc-{}", sid))
}

/// Opens a client connection to the daemon, without handshaking
pub async fn connect<P: AsRef<Path>>(path: P) -> Result<IpcStream, IpcError> {
    let addr = path.as_ref().as_os_str().to_os_string();
    let deadline = Instant::now() + PIPE_BUSY_TIMEOUT;
    loop {
        match ClientOptions::new().open(&addr) {
            Ok(client) => return Ok(client),
            // Every instance is taken and the daemon has not finished creating
            // the next one: that is contention, not an absent daemon.
            Err(e)
                if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
                    && Instant::now() < deadline =>
            {
                tokio::time::sleep(PIPE_BUSY_RETRY_INTERVAL).await;
            }
            Err(e) => {
                return Err(IpcError::NotConnected(format!(
                    "Failed to connect to {}: {}",
                    path.as_ref().display(),
                    e
                )))
            }
        }
    }
}

/// Accepts client connections for the daemon
#[derive(Debug)]
pub struct IpcListener {
    addr: OsString,
    /// The pipe DACL in SDDL form, converted afresh for every instance
    security: Vec<u16>,
    /// The instance the next client will land on, created ahead of time
    next: Mutex<Option<NamedPipeServer>>,
}

impl IpcListener {
    /// Waits for the next client connection
    pub async fn accept(&self) -> Result<IpcServerStream, IpcError> {
        let server = match self.next.lock().expect("pipe instance mutex").take() {
            Some(s) => s,
            None => create_instance(&self.addr, &self.security, false)?,
        };
        server.connect().await?;

        // This instance now belongs to the client; the name needs another one.
        let next = create_instance(&self.addr, &self.security, false)?;
        *self.next.lock().expect("pipe instance mutex") = Some(next);

        Ok(server)
    }
}

/// Creates the listener with a hardened DACL and claims the pipe name (§W0-2)
pub fn create_listener<P: AsRef<Path>>(path: P) -> Result<IpcListener, IpcError> {
    let addr = path.as_ref().as_os_str().to_os_string();
    let security = wide(&pipe_security_sddl()?);
    // `first_pipe_instance(true)` makes a second daemon fail loudly rather than
    // squatting the name: the Windows half of the single-daemon guarantee.
    let first = create_instance(&addr, &security, true)?;
    Ok(IpcListener {
        addr,
        security,
        next: Mutex::new(Some(first)),
    })
}

/// The DACL applied to every pipe instance, in SDDL form.
///
/// `D:P` is a protected DACL (it inherits nothing), followed by one full-access
/// ACE for `SYSTEM` and one for the calling user. No other trustee is named, so
/// no other account — including an administrator on another session — can open
/// the pipe without first taking ownership.
pub fn pipe_security_sddl() -> Result<String, IpcError> {
    let sid = current_user_sid().ok_or_else(|| {
        IpcError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Could not resolve the current user SID, refusing to create an unrestricted pipe",
        ))
    })?;
    Ok(format!("D:P(A;;GA;;;SY)(A;;GA;;;{})", sid))
}

fn create_instance(
    addr: &OsStr,
    security: &[u16],
    first: bool,
) -> Result<NamedPipeServer, IpcError> {
    let descriptor = security_descriptor(security)?;
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };

    // SAFETY: `attributes` is a fully initialised SECURITY_ATTRIBUTES that
    // outlives the call, and its descriptor is valid until the LocalFree below.
    let server = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .create_with_security_attributes_raw(
                addr,
                &mut attributes as *mut SECURITY_ATTRIBUTES as *mut std::ffi::c_void,
            )
    };
    unsafe { LocalFree(descriptor.cast()) };

    server.map_err(|e| match e.raw_os_error() {
        // Only the first instance can lose the name to another process; a
        // failure on a replacement instance is a genuine IO error. This is the
        // Unix path's "a daemon answered on the socket".
        Some(code)
            if first && (code == ERROR_ACCESS_DENIED as i32 || code == ERROR_PIPE_BUSY as i32) =>
        {
            IpcError::AlreadyRunning
        }
        _ => IpcError::Io(e),
    })
}

/// Converts an SDDL string into a security descriptor the caller must LocalFree
fn security_descriptor(sddl: &[u16]) -> Result<PSECURITY_DESCRIPTOR, IpcError> {
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: `sddl` is NUL-terminated by `wide`, and the out-pointer is valid.
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(IpcError::Io(std::io::Error::last_os_error()));
    }
    Ok(descriptor)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Returns the calling process's user SID in string form (`S-1-5-21-...`)
fn current_user_sid() -> Option<String> {
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }

        // First call sizes the buffer, second fills it.
        let mut len: u32 = 0;
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut len);
        if len == 0 {
            CloseHandle(token);
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), len, &mut len);
        CloseHandle(token);
        if ok == 0 {
            return None;
        }

        // `buf` is a Vec<u8>, so it carries no alignment guarantee for TOKEN_USER
        let user: TOKEN_USER = std::ptr::read_unaligned(buf.as_ptr() as *const TOKEN_USER);
        let mut sid_str: *mut u16 = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid_str) == 0 {
            return None;
        }
        let chars = (0..).take_while(|&i| *sid_str.add(i) != 0).count();
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(sid_str, chars));
        LocalFree(sid_str.cast());
        Some(sid)
    }
}
