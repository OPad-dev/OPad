//! Platform IPC transport (§W0-1).
//!
//! The framing in [`crate`] (`[len: u32 LE][json]`) and the request/response
//! enums are transport-agnostic. Only the connection and listener types differ:
//! a Unix domain socket on Linux, a named pipe on Windows. Consumers name them
//! through the aliases re-exported here and never touch a platform type.
//!
//! `IpcStream` is the client end and `IpcServerStream` the accepted server end.
//! They are the same type on Unix and different types on Windows, so the
//! framing helpers are generic over [`crate::IpcTransport`] rather than tied to
//! either one.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::{
    connect, create_listener, get_socket_path, IpcListener, IpcServerStream, IpcStream,
};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{
    connect, create_listener, create_private_pipe, get_socket_path, pipe_security_sddl,
    IpcListener, IpcServerStream, IpcStream,
};
