//! Unix domain socket transport. Behaviour is unchanged from the pre-§W0-1
//! code; only the types moved behind the aliases.

use crate::IpcError;
use std::path::{Path, PathBuf};

/// Client end of an IPC connection.
pub type IpcStream = tokio::net::UnixStream;
/// Server end of an accepted IPC connection.
pub type IpcServerStream = tokio::net::UnixStream;

/// Resolves the standard socket path, one directory per uid
pub fn get_socket_path() -> PathBuf {
    socket_path_from(std::env::var("XDG_RUNTIME_DIR").ok())
}

/// An empty `XDG_RUNTIME_DIR` counts as unset: joined as-is it would give a
/// CWD-relative path that differs between daemon and GUI.
fn socket_path_from(xdg_runtime_dir: Option<String>) -> PathBuf {
    if let Some(runtime_dir) = xdg_runtime_dir.filter(|v| !v.is_empty()) {
        PathBuf::from(runtime_dir).join("opad").join("daemon.sock")
    } else {
        let uid = rustix::process::getuid().as_raw();
        let run_user = PathBuf::from(format!("/run/user/{}", uid));
        if run_user.is_dir() {
            run_user.join("opad").join("daemon.sock")
        } else {
            PathBuf::from(format!("/tmp/opad-{}", uid)).join("daemon.sock")
        }
    }
}

/// Opens a client connection to the daemon, without handshaking
pub async fn connect<P: AsRef<Path>>(path: P) -> Result<IpcStream, IpcError> {
    IpcStream::connect(path.as_ref()).await.map_err(|e| {
        IpcError::NotConnected(format!(
            "Failed to connect to {}: {}",
            path.as_ref().display(),
            e
        ))
    })
}

/// Accepts client connections for the daemon
#[derive(Debug)]
pub struct IpcListener {
    inner: tokio::net::UnixListener,
}

impl IpcListener {
    /// Waits for the next client connection
    pub async fn accept(&self) -> Result<IpcServerStream, IpcError> {
        let (stream, _addr) = self.inner.accept().await?;
        Ok(stream)
    }
}

/// Creates and binds the listener with hardened permissions (§P2-6)
pub fn create_listener<P: AsRef<Path>>(path: P) -> Result<IpcListener, IpcError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let p = path.as_ref();
    if let Some(parent) = p.parent() {
        let is_system_tmp_or_root = parent == Path::new("/tmp") || parent == Path::new("/");
        let current_uid = rustix::process::getuid().as_raw();
        if parent.exists() {
            if !is_system_tmp_or_root {
                let meta = std::fs::symlink_metadata(parent)?;
                if !meta.is_dir() {
                    return Err(IpcError::Io(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!("Path {} exists and is not a directory", parent.display()),
                    )));
                }
                if meta.uid() != current_uid {
                    return Err(IpcError::Io(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!(
                            "Directory {} is owned by UID {}, expected UID {}",
                            parent.display(),
                            meta.uid(),
                            current_uid
                        ),
                    )));
                }
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        } else {
            std::fs::create_dir_all(parent)?;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }

    if p.exists() {
        // Before removing an existing socket, try to connect to it.
        // If a daemon answers, exit with "opad-daemon is already running".
        // Only remove it if the connect fails (stale socket).
        if std::os::unix::net::UnixStream::connect(p).is_ok() {
            return Err(IpcError::AlreadyRunning);
        }
        let _ = std::fs::remove_file(p);
    }

    let inner = tokio::net::UnixListener::bind(p)?;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
    Ok(IpcListener { inner })
}

#[cfg(test)]
mod tests {
    use super::socket_path_from;
    use std::path::PathBuf;

    #[test]
    fn an_empty_xdg_runtime_dir_is_treated_as_unset() {
        assert_eq!(
            socket_path_from(Some("/run/user/1000".into())),
            PathBuf::from("/run/user/1000/opad/daemon.sock")
        );
        let unset = socket_path_from(None);
        assert!(unset.is_absolute());
        assert_eq!(socket_path_from(Some(String::new())), unset);
    }
}
