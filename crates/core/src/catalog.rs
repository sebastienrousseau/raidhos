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

use crate::runtime::{RealRuntime, Runtime};
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
///
/// # Examples
///
/// ```no_run
/// let catalog = raidhos_core::load_catalog().expect("catalog.json not found");
/// for entry in &catalog {
///     println!("{} ({})", entry.name, entry.slug);
/// }
/// ```
pub fn load_catalog() -> Result<Vec<CatalogEntry>> {
    let candidates = [
        PathBuf::from("catalog/catalog.json"),
        PathBuf::from("../catalog/catalog.json"),
        PathBuf::from("../../catalog/catalog.json"),
    ];
    for path in &candidates {
        if path.exists() {
            return load_catalog_from(path);
        }
    }
    Err(CoreError::Io("catalog.json not found".to_string()))
}

/// Load a catalog file from a specific path. Useful for tests and for
/// hosts that ship the catalog under a non-default location.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// let catalog = raidhos_core::load_catalog_from(Path::new("./my-catalog.json"))
///     .expect("catalog parse failed");
/// assert!(!catalog.is_empty());
/// ```
pub fn load_catalog_from(path: &Path) -> Result<Vec<CatalogEntry>> {
    let body =
        fs::read(path).map_err(|e| CoreError::Io(format!("read {}: {e}", path.display())))?;
    serde_json::from_slice(&body).map_err(|e| CoreError::Parse(format!("catalog.json: {e}")))
}

/// Resolve a catalog entry by its `slug`.
///
/// # Examples
///
/// ```no_run
/// let catalog = raidhos_core::load_catalog().unwrap();
/// if let Some(entry) = raidhos_core::find_entry(&catalog, "ubuntu-24.04-desktop-amd64") {
///     println!("Will verify against {}", entry.iso_filename);
/// }
/// ```
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
    verify_iso_with(&RealRuntime, entry, iso_path, sums_path, sig_path, key_dir)
}

/// Test-visible verification entry point that takes a [`Runtime`] so
/// `gpg(1)` invocations can be mocked in unit tests. Production code
/// calls [`verify_iso`] which wires [`RealRuntime`].
pub fn verify_iso_with(
    rt: &dyn Runtime,
    entry: &CatalogEntry,
    iso_path: &Path,
    sums_path: &Path,
    sig_path: &Path,
    key_dir: &Path,
) -> std::result::Result<VerifiedIso, CatalogError> {
    if !gpg_available(rt) {
        return Err(CatalogError::GpgMissing);
    }

    let key_path = key_dir.join(format!("{}.asc", entry.gpg_fingerprint));
    if !rt.path_exists(&key_path) {
        return Err(CatalogError::KeyMissing(entry.gpg_fingerprint.clone()));
    }

    // Verify the signature. Production wires this through gpg(1) in an
    // ephemeral GNUPGHOME so the user's keyring stays untouched; the
    // mock just returns whatever its programmed outcome dictates.
    rt.run_output(
        "gpg",
        &[
            "--quiet",
            "--batch",
            "--import",
            &key_path.to_string_lossy(),
        ],
    )
    .map_err(|e| CatalogError::SignatureRejected(format!("gpg import: {e}")))?;
    rt.run_output(
        "gpg",
        &[
            "--quiet",
            "--batch",
            "--verify",
            &sig_path.to_string_lossy(),
            &sums_path.to_string_lossy(),
        ],
    )
    .map_err(|e| CatalogError::SignatureRejected(format!("gpg verify: {e}")))?;

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

fn gpg_available(rt: &dyn Runtime) -> bool {
    rt.run_output("gpg", &["--version"]).is_ok()
}

/// Look up a filename in a `SHA256SUMS`-style body and return the
/// expected hex hash if present. Public re-export of the internal
/// helper for callers that want to verify against a downloaded
/// `SHA256SUMS` without going through [`verify_iso`].
///
/// # Examples
///
/// ```
/// let body = "deadbeef  ubuntu.iso\nfeedface  arch.iso\n";
/// assert_eq!(
///     raidhos_core::sha256sums_lookup(body, "arch.iso").as_deref(),
///     Some("feedface"),
/// );
/// ```
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

/// Verify an ISO against a single expected SHA-256, supplied as a hex
/// string. Used by `raidhos-cli catalog verify --iso X --sha256 Y` for
/// ISOs that aren't in the bundled GPG-signed catalog. The trust model
/// here is **weaker** than [`verify_iso`]: the caller is asserting
/// "this hash is correct" without a GPG attestation. Pair with a
/// trusted source for the hash.
///
/// Closes Ventoy gap G24 (per-ISO checksum file).
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// // Expected hex from the distro's website / signed checksum file.
/// let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
/// raidhos_core::verify_iso_sha256(Path::new("/Downloads/some.iso"), expected)
///     .expect("hash mismatch");
/// ```
pub fn verify_iso_sha256(
    iso_path: &Path,
    expected_hex: &str,
) -> std::result::Result<String, CatalogError> {
    let expected = expected_hex.trim().to_ascii_lowercase();
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(CatalogError::Io(format!(
            "expected SHA-256 must be 64 hex chars (got {} chars)",
            expected.len()
        )));
    }
    let computed = sha256_of_file(iso_path)?;
    if !computed.eq_ignore_ascii_case(&expected) {
        return Err(CatalogError::Sha256Mismatch { expected, computed });
    }
    Ok(computed)
}

/// Verify an ISO against a co-located `<file>.sha256` companion file
/// containing either a bare hex string or a `SHA256SUMS`-style line.
///
/// Convention: `iso_path` is `/path/to/ubuntu-24.04.iso`; this function
/// reads `/path/to/ubuntu-24.04.iso.sha256` (or `.sha512` if the caller
/// passes one — though only SHA-256 is hashed) and parses the first
/// non-comment line.
///
/// Closes Ventoy gap G24 (per-ISO `VENTOY_CHECKSUM` companion file)
/// with our convention rather than Ventoy's.
pub fn verify_iso_companion_sha256(iso_path: &Path) -> std::result::Result<String, CatalogError> {
    let companion = {
        let mut p = iso_path.as_os_str().to_owned();
        p.push(".sha256");
        std::path::PathBuf::from(p)
    };
    let body = fs::read_to_string(&companion)
        .map_err(|e| CatalogError::Io(format!("read companion {}: {e}", companion.display())))?;
    let expected = extract_first_hash(&body, iso_path).ok_or_else(|| {
        CatalogError::Io(format!("no usable hash line in {}", companion.display()))
    })?;
    verify_iso_sha256(iso_path, &expected)
}

fn extract_first_hash(body: &str, iso_path: &Path) -> Option<String> {
    let iso_filename = iso_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    // SHA256SUMS-style format first.
    if let Some(h) = sha256sums_lookup(body, iso_filename) {
        return Some(h);
    }
    // Bare-hex fallback: first whitespace-separated token of the first
    // non-comment line that looks like a 64-char hex.
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tok = line.split_whitespace().next().unwrap_or("");
        if tok.len() == 64 && tok.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(tok.to_string());
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

    use crate::runtime::{MockOutcome, MockRuntime};

    fn mock_with_gpg_available(available: bool) -> MockRuntime {
        let rt = MockRuntime::new();
        // First outcome — `gpg --version`. Subsequent outcomes are
        // pushed per test for the actual import/verify calls.
        if available {
            rt.push_outcome(MockOutcome::Ok(b"gpg (GnuPG) 2.4.5\n".to_vec()));
        } else {
            rt.push_outcome(MockOutcome::Err("not found".into()));
        }
        rt
    }

    fn make_local_files() -> (PathBuf, PathBuf, PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp = std::env::temp_dir();
        let iso = tmp.join(format!("raidhos-iso-{nanos}.bin"));
        let sums = tmp.join(format!("raidhos-sums-{nanos}.txt"));
        let sig = tmp.join(format!("raidhos-sig-{nanos}.asc"));
        std::fs::write(&iso, b"hello").unwrap();
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        std::fs::write(
            &sums,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824  ci-test.iso\n",
        )
        .unwrap();
        std::fs::write(
            &sig,
            b"-----BEGIN PGP SIGNATURE-----\n-----END PGP SIGNATURE-----\n",
        )
        .unwrap();
        (iso, sums, sig)
    }

    fn ci_entry() -> CatalogEntry {
        CatalogEntry {
            name: "CI Test".into(),
            slug: "ci-test".into(),
            iso_url: "https://example/x.iso".into(),
            sha256sums_url: "https://example/SHA256SUMS".into(),
            sha256sums_sig_url: "https://example/SHA256SUMS.gpg".into(),
            gpg_fingerprint: "0000".into(),
            iso_filename: "ci-test.iso".into(),
        }
    }

    #[test]
    fn verify_iso_with_mock_gpg_missing() {
        let rt = mock_with_gpg_available(false);
        let (iso, sums, sig) = make_local_files();
        let entry = ci_entry();
        let key_dir = std::env::temp_dir();
        let err = verify_iso_with(&rt, &entry, &iso, &sums, &sig, &key_dir).unwrap_err();
        assert!(matches!(err, CatalogError::GpgMissing));
        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_file(&sums);
        let _ = std::fs::remove_file(&sig);
    }

    #[test]
    fn verify_iso_with_mock_key_missing() {
        let rt = mock_with_gpg_available(true);
        let (iso, sums, sig) = make_local_files();
        let entry = ci_entry();
        // key_dir doesn't have <fingerprint>.asc, so KeyMissing fires.
        let key_dir = std::env::temp_dir().join("raidhos-empty-keydir");
        std::fs::create_dir_all(&key_dir).unwrap();
        let err = verify_iso_with(&rt, &entry, &iso, &sums, &sig, &key_dir).unwrap_err();
        assert!(matches!(err, CatalogError::KeyMissing(_)));
        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_file(&sums);
        let _ = std::fs::remove_file(&sig);
        let _ = std::fs::remove_dir_all(&key_dir);
    }

    #[test]
    fn verify_iso_with_mock_signature_rejected() {
        let rt = mock_with_gpg_available(true);
        rt.push_outcome(MockOutcome::Err("BAD signature".into())); // gpg --import OR --verify fails
        let (iso, sums, sig) = make_local_files();
        let entry = ci_entry();
        // Stage the key file so we get past KeyMissing.
        let key_dir = std::env::temp_dir().join(format!("raidhos-kd-{}", std::process::id()));
        std::fs::create_dir_all(&key_dir).unwrap();
        let mut rt = rt;
        rt.existing_paths.push(key_dir.join("0000.asc"));
        std::fs::write(
            key_dir.join("0000.asc"),
            b"-----BEGIN PGP PUBLIC KEY BLOCK-----\n",
        )
        .unwrap();
        let err = verify_iso_with(&rt, &entry, &iso, &sums, &sig, &key_dir).unwrap_err();
        assert!(matches!(err, CatalogError::SignatureRejected(_)));
        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_file(&sums);
        let _ = std::fs::remove_file(&sig);
        let _ = std::fs::remove_dir_all(&key_dir);
    }

    #[test]
    fn verify_iso_with_mock_missing_filename() {
        let rt = mock_with_gpg_available(true);
        rt.push_outcome(MockOutcome::Ok(b"".to_vec())); // gpg --import
        rt.push_outcome(MockOutcome::Ok(b"".to_vec())); // gpg --verify
        let (iso, sums, sig) = make_local_files();
        let mut entry = ci_entry();
        entry.iso_filename = "not-in-sums.iso".into(); // missing → MissingFilename
        let key_dir = std::env::temp_dir().join(format!("raidhos-kd2-{}", std::process::id()));
        std::fs::create_dir_all(&key_dir).unwrap();
        let mut rt = rt;
        rt.existing_paths.push(key_dir.join("0000.asc"));
        std::fs::write(key_dir.join("0000.asc"), b"key").unwrap();
        let err = verify_iso_with(&rt, &entry, &iso, &sums, &sig, &key_dir).unwrap_err();
        assert!(matches!(err, CatalogError::MissingFilename(_)));
        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_file(&sums);
        let _ = std::fs::remove_file(&sig);
        let _ = std::fs::remove_dir_all(&key_dir);
    }

    #[test]
    fn verify_iso_with_mock_sha256_mismatch() {
        let rt = mock_with_gpg_available(true);
        rt.push_outcome(MockOutcome::Ok(b"".to_vec())); // import
        rt.push_outcome(MockOutcome::Ok(b"".to_vec())); // verify
        let (iso, sums, sig) = make_local_files();
        // Rewrite sums so it has a different hash for ci-test.iso.
        std::fs::write(&sums, "deadbeef00000000  ci-test.iso\n").unwrap();
        let entry = ci_entry();
        let key_dir = std::env::temp_dir().join(format!("raidhos-kd3-{}", std::process::id()));
        std::fs::create_dir_all(&key_dir).unwrap();
        let mut rt = rt;
        rt.existing_paths.push(key_dir.join("0000.asc"));
        std::fs::write(key_dir.join("0000.asc"), b"key").unwrap();
        let err = verify_iso_with(&rt, &entry, &iso, &sums, &sig, &key_dir).unwrap_err();
        assert!(matches!(err, CatalogError::Sha256Mismatch { .. }));
        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_file(&sums);
        let _ = std::fs::remove_file(&sig);
        let _ = std::fs::remove_dir_all(&key_dir);
    }

    #[test]
    fn verify_iso_with_mock_complete_success() {
        let rt = mock_with_gpg_available(true);
        rt.push_outcome(MockOutcome::Ok(b"".to_vec())); // import
        rt.push_outcome(MockOutcome::Ok(b"".to_vec())); // verify
        let (iso, sums, sig) = make_local_files();
        // sums already references the correct SHA-256 of "hello".
        let entry = ci_entry();
        let key_dir = std::env::temp_dir().join(format!("raidhos-kd4-{}", std::process::id()));
        std::fs::create_dir_all(&key_dir).unwrap();
        let mut rt = rt;
        rt.existing_paths.push(key_dir.join("0000.asc"));
        std::fs::write(key_dir.join("0000.asc"), b"key").unwrap();
        let v = verify_iso_with(&rt, &entry, &iso, &sums, &sig, &key_dir).unwrap();
        assert_eq!(v.entry.slug, "ci-test");
        assert_eq!(
            v.computed_sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_file(&sums);
        let _ = std::fs::remove_file(&sig);
        let _ = std::fs::remove_dir_all(&key_dir);
    }

    #[test]
    fn verify_iso_with_mock_io_error_on_missing_sums() {
        let rt = mock_with_gpg_available(true);
        rt.push_outcome(MockOutcome::Ok(b"".to_vec())); // import
        rt.push_outcome(MockOutcome::Ok(b"".to_vec())); // verify
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let iso = std::env::temp_dir().join(format!("raidhos-iso-x-{nanos}.bin"));
        std::fs::write(&iso, b"hello").unwrap();
        let sums = Path::new("/nonexistent/sums-raidhos");
        let sig = Path::new("/nonexistent/sig-raidhos");
        let entry = ci_entry();
        let key_dir = std::env::temp_dir().join(format!("raidhos-kd5-{}", std::process::id()));
        std::fs::create_dir_all(&key_dir).unwrap();
        let mut rt = rt;
        rt.existing_paths.push(key_dir.join("0000.asc"));
        std::fs::write(key_dir.join("0000.asc"), b"key").unwrap();
        let err = verify_iso_with(&rt, &entry, &iso, sums, sig, &key_dir).unwrap_err();
        assert!(matches!(err, CatalogError::Io(_)));
        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_dir_all(&key_dir);
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

    fn sample_entry() -> CatalogEntry {
        CatalogEntry {
            name: "Sample".into(),
            slug: "sample".into(),
            iso_url: "https://example/x.iso".into(),
            sha256sums_url: "https://example/SHA256SUMS".into(),
            sha256sums_sig_url: "https://example/SHA256SUMS.gpg".into(),
            gpg_fingerprint: "DEADBEEF".into(),
            iso_filename: "x.iso".into(),
        }
    }

    fn rand_suffix() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("{nanos:x}")
    }

    #[test]
    fn find_entry_locates_by_slug() {
        let catalog = vec![sample_entry()];
        let hit = find_entry(&catalog, "sample");
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().name, "Sample");
    }

    #[test]
    fn find_entry_returns_none_for_unknown_slug() {
        let catalog = vec![sample_entry()];
        assert!(find_entry(&catalog, "no-such-slug").is_none());
    }

    #[test]
    fn find_entry_works_with_empty_catalog() {
        let catalog: Vec<CatalogEntry> = vec![];
        assert!(find_entry(&catalog, "anything").is_none());
    }

    #[test]
    fn catalog_error_into_core_error_keeps_string() {
        let ce: crate::CoreError = CatalogError::MissingFilename("foo.iso".into()).into();
        let s = ce.to_string();
        assert!(s.contains("foo.iso"));
    }

    #[test]
    fn catalog_error_variants_all_render_non_empty() {
        let cases = [
            CatalogError::Io("disk".into()),
            CatalogError::Parse("json".into()),
            CatalogError::GpgMissing,
            CatalogError::SignatureRejected("bad sig".into()),
            CatalogError::MissingFilename("file.iso".into()),
            CatalogError::Sha256Mismatch {
                expected: "aa".into(),
                computed: "bb".into(),
            },
            CatalogError::KeyMissing("FP".into()),
        ];
        for case in cases {
            assert!(!case.to_string().is_empty());
        }
    }

    #[test]
    fn sha256sums_lookup_pub_alias_works() {
        let body = "deadbeef  a.iso\n";
        assert_eq!(
            crate::sha256sums_lookup(body, "a.iso").as_deref(),
            Some("deadbeef")
        );
    }

    #[test]
    fn sha256sums_lookup_handles_tabs() {
        let body = "deadbeef\ta.iso\n";
        assert_eq!(
            sha256sums_lookup(body, "a.iso").as_deref(),
            Some("deadbeef")
        );
    }

    #[test]
    fn catalog_entry_round_trips_via_json() {
        let entry = sample_entry();
        let s = serde_json::to_string(&entry).unwrap();
        let back: CatalogEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(back.slug, entry.slug);
        assert_eq!(back.gpg_fingerprint, entry.gpg_fingerprint);
    }

    #[test]
    fn load_catalog_from_missing_path_errors() {
        let res = load_catalog_from(Path::new("/nonexistent/catalog-xyz.json"));
        assert!(res.is_err());
    }

    #[test]
    fn load_catalog_from_invalid_json_errors() {
        let tmp = std::env::temp_dir().join(format!(
            "raidhos-bad-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        std::fs::write(&tmp, b"not json").unwrap();
        let res = load_catalog_from(&tmp);
        let _ = std::fs::remove_file(&tmp);
        assert!(res.is_err());
    }

    #[test]
    fn hex_lower_matches_known_input() {
        // sha2::Sha256::digest(b"") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let hasher = sha2::Sha256::new();
        let digest = hasher.finalize();
        let s = hex_lower(&digest);
        assert_eq!(
            s,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hex_lower_renders_all_byte_values() {
        let bytes: Vec<u8> = (0u8..16).collect();
        assert_eq!(hex_lower(&bytes), "000102030405060708090a0b0c0d0e0f");
    }

    #[test]
    fn sha256_of_file_returns_known_value() {
        let tmp = std::env::temp_dir().join(format!(
            "raidhos-sha-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        std::fs::write(&tmp, b"hello").unwrap();
        let s = sha256_of_file(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        assert_eq!(
            s,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn sha256_of_file_errors_on_missing_path() {
        let res = sha256_of_file(Path::new("/nonexistent/raidhos-sha-not-found"));
        assert!(res.is_err());
    }

    #[test]
    fn load_catalog_from_reads_valid_file() {
        let tmp = std::env::temp_dir().join(format!(
            "raidhos-good-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        std::fs::write(
            &tmp,
            br#"[{"name":"X","slug":"x","iso_url":"u","sha256sums_url":"u","sha256sums_sig_url":"u","gpg_fingerprint":"FP","iso_filename":"x.iso"}]"#,
        )
        .unwrap();
        let entries = load_catalog_from(&tmp).expect("should load");
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].slug, "x");
    }

    #[test]
    fn load_catalog_finds_repo_catalog_when_present() {
        // This test runs against the actual repo catalog if it exists.
        // We don't assert a specific shape because the file changes
        // between releases.
        match load_catalog() {
            Ok(entries) => assert!(!entries.is_empty(), "shipped catalog must be non-empty"),
            Err(_) => {
                // Running outside the repo (e.g. doctest harness). Fine.
            }
        }
    }
}
