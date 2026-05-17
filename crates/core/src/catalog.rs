//! Curated ISO catalog with GPG signature verification.
//!
//! Each `CatalogEntry` describes a published Linux distribution release
//! with three URLs and a GPG signing-key reference:
//!
//! - `iso_url` — the ISO itself.
//! - `sha256sums_url` — the distro's published `SHA256SUMS` file (or
//!   equivalent), which contains one line per file with `<sha256>  <name>`.
//! - `sha256sums_sig_url` — the detached GPG signature over that file.
//! - `gpg_fingerprint` — the long-form fingerprint of the signing key,
//!   pre-validated. The key itself is shipped under
//!   `catalog/keys/<fingerprint>.asc` in the repo so verification is
//!   offline-capable.
//!
//! Verification steps:
//!
//! 1. Import the named public key into a temporary keyring.
//! 2. `gpg --verify` the detached signature over the SHA256SUMS file.
//! 3. Look up the ISO filename in the verified SHA256SUMS.
//! 4. Compute the SHA-256 of the local ISO and compare.
//!
//! The implementation shells out to `gpg(1)` rather than pulling a
//! pure-Rust OpenPGP stack, keeping the dependency surface small.
//! Hosts without `gpg` get a clear error.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{CoreError, Result};

/// A single distro release in the bundled catalog.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CatalogEntry {
    /// Display name (e.g. `Ubuntu 24.04 LTS Desktop`).
    pub name: String,
    /// Short slug used as a lookup key (e.g. `ubuntu-24.04-desktop`).
    pub slug: String,
    /// HTTPS URL to the ISO file.
    pub iso_url: String,
    /// HTTPS URL to the SHA256SUMS file (or equivalent).
    pub sha256sums_url: String,
    /// HTTPS URL to the detached signature over SHA256SUMS.
    pub sha256sums_sig_url: String,
    /// Fingerprint (long form, no spaces) of the key that signs
    /// `sha256sums_sig_url`. The corresponding ASCII-armoured public
    /// key must live at `catalog/keys/<fingerprint>.asc` in the repo.
    pub gpg_fingerprint: String,
    /// ISO filename as it appears in SHA256SUMS. Required because the
    /// SHA256SUMS line is matched on filename, not URL.
    pub iso_filename: String,
}

/// Outcome of a verification run.
#[derive(Debug)]
pub struct VerifiedIso {
    /// The catalog entry that matched.
    pub entry: CatalogEntry,
    /// Computed SHA-256 of the local ISO, lowercase hex.
    pub computed_sha256: String,
}

/// Errors specific to catalog verification.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    /// I/O while reading inputs.
    #[error("io: {0}")]
    Io(String),
    /// Catalog JSON could not be parsed.
    #[error("catalog parse: {0}")]
    Parse(String),
    /// No `gpg(1)` binary on `$PATH`.
    #[error("gpg(1) not found on PATH")]
    GpgMissing,
    /// `gpg --verify` rejected the detached signature.
    #[error("signature verification failed: {0}")]
    SignatureRejected(String),
    /// SHA256SUMS contained no line matching the catalog's
    /// `iso_filename`.
    #[error("filename {0} not present in SHA256SUMS")]
    MissingFilename(String),
    /// Computed SHA-256 did not match the entry in SHA256SUMS.
    #[error("sha256 mismatch: expected {expected} computed {computed}")]
    Sha256Mismatch {
        /// Hash recorded in SHA256SUMS.
        expected: String,
        /// Hash we computed from the local ISO.
        computed: String,
    },
    /// The named key fingerprint is not bundled in `catalog/keys/`.
    #[error("public key for fingerprint {0} is not bundled")]
    KeyMissing(String),
}

impl From<CatalogError> for CoreError {
    fn from(value: CatalogError) -> Self {
        CoreError::Validation(value.to_string())
    }
}

/// Load the bundled catalog from `catalog/catalog.json` (relative to
/// the current working directory or the repo root, whichever resolves
/// first).
pub fn load_catalog() -> Result<Vec<CatalogEntry>> {
    let candidates = [
        PathBuf::from("catalog/catalog.json"),
        PathBuf::from("../catalog/catalog.json"),
        PathBuf::from("../../catalog/catalog.json"),
    ];
    for path in candidates {
        if let Ok(body) = fs::read(&path) {
            let parsed: Vec<CatalogEntry> = serde_json::from_slice(&body)
                .map_err(|e| CoreError::Parse(format!("catalog.json: {e}")))?;
            return Ok(parsed);
        }
    }
    Err(CoreError::Io("catalog.json not found".to_string()))
}

/// Resolve a catalog entry by its `slug`.
pub fn find_entry<'a>(catalog: &'a [CatalogEntry], slug: &str) -> Option<&'a CatalogEntry> {
    catalog.iter().find(|e| e.slug == slug)
}

/// Verify a local ISO against a catalog entry.
///
/// `iso_path` — the locally downloaded ISO.
/// `sums_path` — the locally downloaded SHA256SUMS file.
/// `sig_path` — the locally downloaded detached signature.
/// `key_dir` — directory containing `<fingerprint>.asc` keys.
pub fn verify_iso(
    entry: &CatalogEntry,
    iso_path: &Path,
    sums_path: &Path,
    sig_path: &Path,
    key_dir: &Path,
) -> std::result::Result<VerifiedIso, CatalogError> {
    if !gpg_available() {
        return Err(CatalogError::GpgMissing);
    }

    let key_path = key_dir.join(format!("{}.asc", entry.gpg_fingerprint));
    if !key_path.exists() {
        return Err(CatalogError::KeyMissing(entry.gpg_fingerprint.clone()));
    }

    // Create an ephemeral GPG home so we don't touch the user's keyring.
    let gnupghome = tempdir()?;
    let out = Command::new("gpg")
        .env("GNUPGHOME", &gnupghome)
        .args(["--quiet", "--batch", "--import"])
        .arg(&key_path)
        .output()
        .map_err(|e| CatalogError::Io(format!("gpg import: {e}")))?;
    if !out.status.success() {
        return Err(CatalogError::SignatureRejected(
            String::from_utf8_lossy(&out.stderr).to_string(),
        ));
    }

    let out = Command::new("gpg")
        .env("GNUPGHOME", &gnupghome)
        .args(["--quiet", "--batch", "--verify"])
        .arg(sig_path)
        .arg(sums_path)
        .output()
        .map_err(|e| CatalogError::Io(format!("gpg verify: {e}")))?;
    let _ = fs::remove_dir_all(&gnupghome);
    if !out.status.success() {
        return Err(CatalogError::SignatureRejected(
            String::from_utf8_lossy(&out.stderr).to_string(),
        ));
    }

    let sums = fs::read_to_string(sums_path).map_err(|e| CatalogError::Io(e.to_string()))?;
    let expected = sha256sums_lookup(&sums, &entry.iso_filename)
        .ok_or_else(|| CatalogError::MissingFilename(entry.iso_filename.clone()))?;

    let computed = sha256_of_file(iso_path)?;
    if !computed.eq_ignore_ascii_case(&expected) {
        return Err(CatalogError::Sha256Mismatch { expected, computed });
    }

    Ok(VerifiedIso {
        entry: entry.clone(),
        computed_sha256: computed,
    })
}

fn gpg_available() -> bool {
    Command::new("gpg")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn tempdir() -> std::result::Result<PathBuf, CatalogError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("raidhos-gpg-{nanos:x}"));
    fs::create_dir_all(&dir).map_err(|e| CatalogError::Io(e.to_string()))?;
    Ok(dir)
}

/// Public re-export of [`sha256sums_lookup`] for callers that want to
/// look up a filename in a `SHA256SUMS` body directly (e.g. integration
/// tests, CI tooling).
pub fn sha256sums_lookup_pub(body: &str, filename: &str) -> Option<String> {
    sha256sums_lookup(body, filename)
}

pub(crate) fn sha256sums_lookup(body: &str, filename: &str) -> Option<String> {
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Format: `<hex>  <filename>` (two-space separator) or
        // `<hex> *<filename>` (binary mode marker). Tolerate both.
        let mut parts = line.splitn(2, |c: char| c.is_ascii_whitespace());
        let hex = parts.next()?.to_string();
        let rest = parts.next()?.trim_start();
        let rest = rest.strip_prefix('*').unwrap_or(rest);
        if rest == filename {
            return Some(hex);
        }
    }
    None
}

fn sha256_of_file(path: &Path) -> std::result::Result<String, CatalogError> {
    let mut f = fs::File::open(path).map_err(|e| CatalogError::Io(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| CatalogError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256sums_lookup_matches_basic_line() {
        let body = "deadbeef  alice.iso\nfeedface  bob.iso\n";
        assert_eq!(
            sha256sums_lookup(body, "bob.iso").as_deref(),
            Some("feedface")
        );
        assert_eq!(
            sha256sums_lookup(body, "alice.iso").as_deref(),
            Some("deadbeef")
        );
    }

    #[test]
    fn sha256sums_lookup_handles_binary_marker() {
        let body = "deadbeef *alice.iso\n";
        assert_eq!(
            sha256sums_lookup(body, "alice.iso").as_deref(),
            Some("deadbeef")
        );
    }

    #[test]
    fn sha256sums_lookup_returns_none_for_missing() {
        let body = "deadbeef  alice.iso\n";
        assert!(sha256sums_lookup(body, "bob.iso").is_none());
    }

    #[test]
    fn sha256sums_lookup_ignores_comments_and_blanks() {
        let body = "# header\n\ndeadbeef  alice.iso\n";
        assert_eq!(
            sha256sums_lookup(body, "alice.iso").as_deref(),
            Some("deadbeef")
        );
    }

    #[test]
    fn verify_iso_reports_missing_key() {
        let entry = CatalogEntry {
            name: "Test".into(),
            slug: "test".into(),
            iso_url: "https://example/test.iso".into(),
            sha256sums_url: "https://example/SHA256SUMS".into(),
            sha256sums_sig_url: "https://example/SHA256SUMS.gpg".into(),
            gpg_fingerprint: "0000000000000000000000000000000000000000".into(),
            iso_filename: "test.iso".into(),
        };
        let tmp = std::env::temp_dir();
        let iso = tmp.join("test.iso");
        let sums = tmp.join("SHA256SUMS");
        let sig = tmp.join("SHA256SUMS.gpg");
        let keydir = tmp.clone();
        let _ = fs::write(&iso, b"x");
        let _ = fs::write(&sums, b"deadbeef  test.iso\n");
        let _ = fs::write(
            &sig,
            b"-----BEGIN PGP SIGNATURE-----\n-----END PGP SIGNATURE-----\n",
        );
        let err = verify_iso(&entry, &iso, &sums, &sig, &keydir).unwrap_err();
        // Either gpg is missing (CI minimal image) or the key is missing.
        assert!(matches!(
            err,
            CatalogError::GpgMissing | CatalogError::KeyMissing(_)
        ));
    }
}
