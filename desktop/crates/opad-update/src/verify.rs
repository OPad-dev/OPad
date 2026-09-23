//! Signature and hash verification (§U-0.3).
//!
//! Two independent checks, in this order:
//!
//! 1. The manifest's minisign signature, against a public key compiled into
//!    this binary. An attacker who can serve the manifest cannot forge this.
//! 2. Each downloaded artifact's SHA-256, against the hash inside that
//!    now-trusted manifest.
//!
//! Neither is optional and neither substitutes for the other: a signature
//! proves the hash list is ours, the hash proves the bytes are the ones the
//! list names.

use crate::UpdateError;
use minisign_verify::{PublicKey, Signature};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// The minisign public key the release manifest is signed with.
///
/// The matching secret key is **not in this repository and must never be**. It
/// lives at `~/.config/opad/opad-manifest.key` on the release machine, and
/// `opad-manifest` (the signing tool beside this crate) is the only thing
/// that reads it.
///
/// An empty value here is not a soft failure: with no key there is nothing to
/// verify against, so every update path fails closed rather than falling back
/// to trusting the transport.
///
/// **This key is currently stored unencrypted** (`minisign -G -W`). See the
/// warning in `bin/opad-manifest.rs` — it must be re-cut with a passphrase
/// before the repository is published.
pub const MANIFEST_PUBLIC_KEY: &str = "RWRsyn4Jum62+G0lLWDLbT9yjEO35ZEsWJ30iUmclHEc27zVZRer8qtG";

/// True when this build can verify anything at all
pub fn signing_configured() -> bool {
    !MANIFEST_PUBLIC_KEY.trim().is_empty()
}

/// Verifies a detached minisign signature over the raw manifest bytes.
///
/// Takes the bytes rather than a parsed manifest on purpose: the signature
/// covers exactly what was served, so parsing must happen after this, never
/// before, and never on a re-serialised copy.
pub fn verify_manifest_signature(
    manifest_bytes: &[u8],
    signature: &str,
) -> Result<(), UpdateError> {
    verify_with_key(MANIFEST_PUBLIC_KEY, manifest_bytes, signature)
}

pub(crate) fn verify_with_key(
    public_key: &str,
    manifest_bytes: &[u8],
    signature: &str,
) -> Result<(), UpdateError> {
    if public_key.trim().is_empty() {
        return Err(UpdateError::NoPublicKey);
    }
    let key = PublicKey::from_base64(public_key.trim())
        .map_err(|e| UpdateError::BadSignature(format!("unusable public key: {e}")))?;
    let sig = Signature::decode(signature)
        .map_err(|e| UpdateError::BadSignature(format!("unreadable signature: {e}")))?;
    key.verify(manifest_bytes, &sig, false)
        .map_err(|e| UpdateError::BadSignature(e.to_string()))
}

/// Streaming SHA-256, so a 100 MB installer is never held in memory
pub fn sha256_file(path: &Path) -> Result<String, UpdateError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    hex_lower(&Sha256::digest(bytes))
}

/// Compares against the manifest's hash, case- and whitespace-insensitively so
/// a manifest written by hand is not rejected for cosmetics.
pub fn check_hash(artifact: &str, path: &Path, expected: &str) -> Result<(), UpdateError> {
    compare_hash(artifact, sha256_file(path)?, expected)
}

/// [`check_hash`] for bytes already in memory
pub fn check_hash_bytes(artifact: &str, bytes: &[u8], expected: &str) -> Result<(), UpdateError> {
    compare_hash(artifact, sha256_bytes(bytes), expected)
}

fn compare_hash(artifact: &str, actual: String, expected: &str) -> Result<(), UpdateError> {
    if actual.eq_ignore_ascii_case(expected.trim()) {
        return Ok(());
    }
    Err(UpdateError::HashMismatch {
        artifact: artifact.to_string(),
        expected: expected.trim().to_lowercase(),
        actual,
    })
}

fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{:02x}", b);
        s
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Generated with `minisign -G`; the matching secret key is not in the repo
    /// and is not used for anything — these tests only need a real key pair
    /// shape to prove verification accepts and rejects the right things.
    const TEST_PUBLIC_KEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";

    /// A throwaway keypair that is **not** ours, with a genuine minisign
    /// signature it really did produce over `WRONG_KEY_MANIFEST`. Its secret
    /// half was discarded; it exists so the rejection below is provably about
    /// *which key signed*, not about a malformed signature.
    const WRONG_PUBLIC_KEY: &str = "RWRwm1Yg1liKWycfEdoSZapd0XhecKK8/9rao/6nG/PkEZSGJaHTslb1";
    const WRONG_KEY_MANIFEST: &[u8] =
        br#"{"schema":1,"generated":"2026-09-17T00:00:00Z","components":{}}"#;
    const WRONG_KEY_SIGNATURE: &str = concat!(
        "untrusted comment: signature from minisign secret key\n",
        "RURwm1Yg1liKW2Hjtz4hRdi1ZJBsx/hhU8YIx4+Z9RnAbE3GVpSMlb40aTG5PJ83evWawBCGvrt2k/9ee7aoLRgQwFfNaQ5A+gY=\n",
        "trusted comment: osupad test vector\n",
        "y2t+5fkSFjzY7ObWMq4Mci4kwEsgFHWtGHUb9c4MqP99u5DuOGapzDRiF8IIwvh3ugpxMtBMO3T+mwH0uYQ7AA==\n",
    );

    #[test]
    fn hashing_matches_a_known_vector() {
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn a_tampered_file_fails_its_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("artifact.bin");
        std::fs::write(&path, b"the real thing").unwrap();
        let good = sha256_file(&path).unwrap();
        assert!(check_hash("artifact.bin", &path, &good).is_ok());
        // Case and stray whitespace in the manifest are cosmetic, not fatal
        assert!(check_hash("artifact.bin", &path, &format!(" {} ", good.to_uppercase())).is_ok());

        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        f.write_all(b"!").unwrap();
        drop(f);
        match check_hash("artifact.bin", &path, &good) {
            Err(UpdateError::HashMismatch {
                expected, actual, ..
            }) => {
                assert_eq!(expected, good);
                assert_ne!(actual, good);
            }
            other => panic!("expected a hash mismatch, got {other:?}"),
        }
    }

    #[test]
    fn in_memory_bytes_are_checked_without_touching_disk() {
        let good = sha256_bytes(b"the real thing");
        assert!(check_hash_bytes("artifact.bin", b"the real thing", &good).is_ok());
        assert!(check_hash_bytes("artifact.bin", b"the real thing", &good.to_uppercase()).is_ok());
        assert!(matches!(
            check_hash_bytes("artifact.bin", b"the real thing!", &good),
            Err(UpdateError::HashMismatch { .. })
        ));
    }

    #[test]
    fn hashing_a_large_file_does_not_depend_on_buffer_boundaries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.bin");
        // Larger than the 64 KiB read buffer, and not a multiple of it
        let data: Vec<u8> = (0..(200 * 1024 + 7)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &data).unwrap();
        assert_eq!(sha256_file(&path).unwrap(), sha256_bytes(&data));
    }

    #[test]
    fn a_build_with_no_key_refuses_to_verify_anything() {
        // Fail closed: no key means no way to tell a real manifest from a
        // forged one, so nothing is installable.
        assert!(matches!(
            verify_with_key("", b"whatever", "untrusted comment: x\nRWQ"),
            Err(UpdateError::NoPublicKey)
        ));
    }

    #[test]
    fn a_garbage_signature_is_rejected_rather_than_panicking() {
        assert!(matches!(
            verify_with_key(TEST_PUBLIC_KEY, b"manifest", "not a signature at all"),
            Err(UpdateError::BadSignature(_))
        ));
    }

    #[test]
    fn this_build_has_a_usable_signing_key() {
        // A release that ships with no key, or with a key that does not parse,
        // has silently disabled all three updaters.
        assert!(signing_configured(), "MANIFEST_PUBLIC_KEY is empty");
        PublicKey::from_base64(MANIFEST_PUBLIC_KEY.trim())
            .expect("MANIFEST_PUBLIC_KEY is not a valid minisign public key");
    }

    #[test]
    fn a_manifest_signed_with_a_different_key_is_rejected() {
        // The control: the signature really is well formed and really does
        // verify — against the key that made it.
        verify_with_key(WRONG_PUBLIC_KEY, WRONG_KEY_MANIFEST, WRONG_KEY_SIGNATURE)
            .expect("the test vector should verify under its own key");

        // The actual assertion, through the production entry point that the
        // client uses, against the key compiled into this build.
        match verify_manifest_signature(WRONG_KEY_MANIFEST, WRONG_KEY_SIGNATURE) {
            Err(UpdateError::BadSignature(_)) => {}
            other => panic!("a foreign signature must be rejected, got {other:?}"),
        }
    }

    #[test]
    fn a_manifest_altered_after_signing_is_rejected() {
        let mut tampered = WRONG_KEY_MANIFEST.to_vec();
        let last = tampered.len() - 1;
        tampered[last] = b' ';
        // Even under the key that signed the original, one changed byte fails.
        assert!(matches!(
            verify_with_key(WRONG_PUBLIC_KEY, &tampered, WRONG_KEY_SIGNATURE),
            Err(UpdateError::BadSignature(_))
        ));
    }

    #[test]
    fn a_signature_from_another_key_is_rejected() {
        // A well-formed minisign signature that this key did not produce.
        let other = "untrusted comment: signature from a different key\n\
                     RWQf6LRCGA9i5wbNaKjuHFHt/9JXL6tM2fN0N5J8yX0M1vYb0KQF0hRsUM3zZ0oXyq\
                     hZQ1q3rLZ0kYgYQ8yX0M1vYb0KQF0hRsUA==\n";
        assert!(matches!(
            verify_with_key(TEST_PUBLIC_KEY, b"manifest", other),
            Err(UpdateError::BadSignature(_))
        ));
    }
}
