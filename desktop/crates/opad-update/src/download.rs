//! Downloading and installing files without ever leaving a broken one (§U-0.5).
//!
//! Every replacement is: write to a temporary file in the *destination*
//! directory, fsync it, verify its hash against the signed manifest, then
//! rename it over the target. Killing the process at any point leaves either
//! the old file or the new one, never a half-written binary — and the hash is
//! checked before the rename, so a truncated download never becomes the
//! installed version.
//!
//! The temporary file is in the destination directory on purpose: a rename
//! across filesystems is not atomic, and `/tmp` is frequently its own mount.

use crate::verify::check_hash;
use crate::UpdateError;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A downloaded file that has been verified but not yet installed.
///
/// Dropping it without calling [`StagedFile::install_to`] removes the
/// temporary file, so an abandoned or failed update leaves nothing behind.
#[derive(Debug)]
pub struct StagedFile {
    path: PathBuf,
    persisted: bool,
}

impl StagedFile {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Atomically replaces `target`.
    ///
    /// On Unix a rename over an open, running binary is fine: the running
    /// process keeps its inode. On Windows the target must not be open, which
    /// is why a tosu swap stops tosu first (§U-1).
    pub fn install_to(mut self, target: &Path) -> Result<(), UpdateError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // The bundled tosu has to be executable; a downloaded file is 0600.
            let mut perms = std::fs::metadata(&self.path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&self.path, perms)?;
        }
        std::fs::rename(&self.path, target)?;
        self.persisted = true;
        sync_parent(target);
        Ok(())
    }
}

/// Replaces `target` with `bytes` so that a kill at any point leaves the old
/// file or the new one, never a truncated one: temporary file beside it,
/// fsync, rename, then fsync the directory. For files with no hash to check
/// (tosu's VERSION/NOTICE, counter backups); a downloaded artifact goes
/// through [`stage_bytes`] instead.
pub fn write_atomic(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = incoming_path(target)?;
    let result = write_synced(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, target));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return result;
    }
    sync_parent(target);
    Ok(())
}

/// `.<name>.incoming` in the target's own directory (created if missing), so
/// the final rename never crosses a filesystem.
fn incoming_path(target: &Path) -> std::io::Result<PathBuf> {
    let dir = target.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} has no parent directory", target.display()),
        )
    })?;
    std::fs::create_dir_all(dir)?;
    Ok(dir.join(format!(
        ".{}.incoming",
        target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "opad-update".to_string())
    )))
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Makes a rename into `target`'s directory durable, so a power cut after a
/// "successful" write cannot resurrect the old file with the new one gone.
fn sync_parent(target: &Path) {
    if let Some(dir) = target.parent() {
        if let Ok(handle) = std::fs::File::open(dir) {
            let _ = handle.sync_all();
        }
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        if !self.persisted {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Writes `bytes` beside `target`, fsyncs, and verifies the hash.
///
/// Returns an error — and leaves nothing behind — if the bytes do not match
/// the hash the signed manifest gave for this artifact.
pub fn stage_bytes(
    target: &Path,
    bytes: &[u8],
    expected_sha256: &str,
    artifact_name: &str,
) -> Result<StagedFile, UpdateError> {
    let tmp = incoming_path(target)?;
    // Owned by the StagedFile from here, so a failed write is cleaned up too
    let staged = StagedFile {
        path: tmp,
        persisted: false,
    };
    write_synced(&staged.path, bytes)?;
    // Verify before returning: a caller cannot install what never verified,
    // and the Drop above removes the rejected file.
    check_hash(artifact_name, &staged.path, expected_sha256)?;
    Ok(staged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::sha256_bytes;

    #[test]
    fn a_verified_file_replaces_the_target_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("tosu");
        std::fs::write(&target, b"old version").unwrap();

        let new = b"new version";
        let staged = stage_bytes(&target, new, &sha256_bytes(new), "tosu").unwrap();
        // Still the old one until install_to runs
        assert_eq!(std::fs::read(&target).unwrap(), b"old version");
        staged.install_to(&target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), new);
    }

    #[test]
    fn a_corrupted_download_is_rejected_and_the_old_file_survives() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("tosu");
        std::fs::write(&target, b"old version").unwrap();

        let err =
            stage_bytes(&target, b"tampered", &sha256_bytes(b"expected"), "tosu").unwrap_err();
        assert!(matches!(err, UpdateError::HashMismatch { .. }));
        assert_eq!(std::fs::read(&target).unwrap(), b"old version");
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "the rejected download must not be left behind"
        );
    }

    #[test]
    fn abandoning_a_staged_file_cleans_up() {
        // "Killing the app mid-download leaves the previous version intact."
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("tosu");
        std::fs::write(&target, b"old version").unwrap();

        let bytes = b"new version";
        {
            let _staged = stage_bytes(&target, bytes, &sha256_bytes(bytes), "tosu").unwrap();
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
        }
        assert_eq!(std::fs::read(&target).unwrap(), b"old version");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn staging_works_when_the_target_does_not_exist_yet() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("nested").join("tosu");
        let bytes = b"first install";
        stage_bytes(&target, bytes, &sha256_bytes(bytes), "tosu")
            .unwrap()
            .install_to(&target)
            .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
    }

    #[test]
    fn an_atomic_write_replaces_the_file_and_leaves_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("VERSION");
        std::fs::write(&target, b"4.1.0\n").unwrap();
        write_atomic(&target, b"4.2.0\n").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"4.2.0\n");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);

        let nested = dir.path().join("new").join("NOTICE");
        write_atomic(&nested, b"notice").unwrap();
        assert_eq!(std::fs::read(&nested).unwrap(), b"notice");
    }

    #[test]
    fn a_failed_atomic_write_keeps_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        // A directory in the way makes the final rename fail
        let target = dir.path().join("VERSION");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep"), b"x").unwrap();
        assert!(write_atomic(&target, b"4.2.0\n").is_err());
        assert!(target.join("keep").exists());
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "the temporary file must not be left behind"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_installed_binary_is_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("tosu");
        let bytes = b"#!/bin/sh\n";
        stage_bytes(&target, bytes, &sha256_bytes(bytes), "tosu")
            .unwrap()
            .install_to(&target)
            .unwrap();
        let mode = std::fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "a bundled tosu must be runnable");
    }
}
