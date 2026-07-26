//! Per-distro persistence-overlay volume labels.
//!
//! Each Linux distribution expects its persistence overlay to live
//! on a filesystem with a specific volume label. The kernel's
//! initramfs hunts for that label at boot — supply the wrong label
//! and persistence silently fails.
//!
//! This table mirrors the relevant parts of Ventoy's persistence
//! plugin (Ventoy gap G19). Each entry maps a **catalog slug** (the
//! identifier we use for an ISO release) to the **volume label**
//! the distro's kernel looks for.
//!
//! Adding a new distro:
//! 1. Look up the distro's persistence label (often documented as
//!    "boot parameter `persistent` requires label X").
//! 2. Add a row to `LABELS` below.
//! 3. Add a test case.
//!
//! Sources cross-checked against:
//! - Ventoy `plugin/persistence/` (BSD-3 licensed, label list only)
//! - Per-distro documentation (Ubuntu Casper, Debian live-boot,
//!   MX Linux Tools, Arch ISO releng, Kali docs)
//! - Empirical: ISO bootloader configs from the most recent stable
//!   release of each distro (verified May 2026).

/// Default label for the RaidhOS-provisioned persistence overlay
/// when the user didn't pick a specific distro from the catalog.
/// Set on a Linux ext4 filesystem; the kernel will mount it iff the
/// distro happens to look for `persistence` (Debian's default).
pub const DEFAULT_LABEL: &str = "persistence";

/// Catalog-slug → persistence-volume-label mapping.
///
/// Order is deliberately stable: lookups use linear search and
/// matching is case-insensitive on the slug.
const LABELS: &[(&str, &str)] = &[
    // Ubuntu family (Casper) — Ubuntu, Lubuntu, Kubuntu, Xubuntu,
    // Ubuntu MATE, Ubuntu Budgie, Ubuntu Studio, Edubuntu.
    ("ubuntu", "casper-rw"),
    ("lubuntu", "casper-rw"),
    ("kubuntu", "casper-rw"),
    ("xubuntu", "casper-rw"),
    ("ubuntu-mate", "casper-rw"),
    ("ubuntu-budgie", "casper-rw"),
    ("ubuntu-studio", "casper-rw"),
    ("edubuntu", "casper-rw"),
    // Ubuntu-derived spins also using Casper.
    ("linuxmint", "casper-rw"),
    ("mint", "casper-rw"),
    ("elementary", "casper-rw"),
    ("zorin", "casper-rw"),
    ("pop-os", "casper-rw"),
    ("pop_os", "casper-rw"),
    ("popos", "casper-rw"),
    // Debian Live (live-boot).
    ("debian", "persistence"),
    ("debian-live", "persistence"),
    // MX Linux — antiX fork of Debian Live with a custom label.
    ("mx-linux", "MX-Persist"),
    ("mx", "MX-Persist"),
    ("antix", "MX-Persist"),
    // Arch family — Ventoy-aware initramfs hook ('vtoy_cow' module).
    ("arch", "vtoycow"),
    ("archlinux", "vtoycow"),
    ("manjaro", "vtoycow"),
    ("endeavouros", "vtoycow"),
    ("garuda", "vtoycow"),
    ("cachyos", "vtoycow"),
    // Kali.
    ("kali", "kali-persistence"),
    ("kali-linux", "kali-persistence"),
    // Fedora workstation/spins.
    ("fedora", "writable"),
    ("fedora-workstation", "writable"),
    ("fedora-kde", "writable"),
    // CloneZilla.
    ("clonezilla", "live-rw"),
    // Kaspersky Rescue Disk.
    ("kaspersky-rescue", "KRD2018_Data"),
    // Tails (uses a different scheme but supports a persistence
    // volume labelled `TailsData` when set up via the GUI).
    ("tails", "TailsData"),
];

/// Look up the persistence-volume label a given catalog slug
/// expects. Returns `None` if the slug isn't in the table — callers
/// should fall back to [`DEFAULT_LABEL`] or refuse to provision
/// persistence for that distro.
///
/// Matching is case-insensitive. Closes Ventoy gap G19.
///
/// # Examples
///
/// ```
/// use raidhos_core::expected_persistence_label;
/// assert_eq!(expected_persistence_label("ubuntu-24.04-desktop-amd64"), Some("casper-rw"));
/// assert_eq!(expected_persistence_label("MX-Linux"), Some("MX-Persist"));
/// assert_eq!(expected_persistence_label("definitely-not-a-distro"), None);
/// ```
pub fn expected_persistence_label(slug: &str) -> Option<&'static str> {
    let slug_lc = slug.to_ascii_lowercase();
    // Try exact match first.
    if let Some((_, label)) = LABELS.iter().find(|(k, _)| *k == slug_lc) {
        return Some(*label);
    }
    // Then prefix match — `ubuntu-24.04-desktop-amd64` starts with
    // `ubuntu`, `linuxmint-22.1-cinnamon` starts with `linuxmint`.
    // Iterate longest-first so multi-segment keys like
    // `kali-linux` win over the shorter `kali`.
    let mut sorted: Vec<&(&str, &str)> = LABELS.iter().collect();
    sorted.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    for (key, label) in sorted {
        if let Some(after) = slug_lc.strip_prefix(key) {
            // Require a separator after the matched prefix so
            // `manjaro-i3` matches `manjaro` but `manjaroX` doesn't
            // match `manjaro`.
            if after.is_empty()
                || after.starts_with('-')
                || after.starts_with('_')
                || after.starts_with('.')
            {
                return Some(*label);
            }
        }
    }
    None
}

/// Return every (slug-prefix, label) pair the table knows. Useful
/// for UIs that want to show the user "this is what RaidhOS will
/// label your persistence file" before they commit.
pub fn all_labels() -> &'static [(&'static str, &'static str)] {
    LABELS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ubuntu_uses_casper_rw() {
        assert_eq!(expected_persistence_label("ubuntu"), Some("casper-rw"));
        assert_eq!(
            expected_persistence_label("ubuntu-24.04-desktop-amd64"),
            Some("casper-rw"),
        );
    }

    #[test]
    fn mx_linux_uses_mx_persist() {
        assert_eq!(expected_persistence_label("mx"), Some("MX-Persist"));
        assert_eq!(expected_persistence_label("MX-Linux"), Some("MX-Persist"));
        assert_eq!(
            expected_persistence_label("mx-linux-23.5-x64"),
            Some("MX-Persist"),
        );
    }

    #[test]
    fn arch_family_uses_vtoycow() {
        assert_eq!(expected_persistence_label("arch"), Some("vtoycow"));
        assert_eq!(expected_persistence_label("archlinux"), Some("vtoycow"));
        assert_eq!(expected_persistence_label("manjaro"), Some("vtoycow"));
        assert_eq!(expected_persistence_label("cachyos"), Some("vtoycow"));
        assert_eq!(expected_persistence_label("endeavouros"), Some("vtoycow"));
    }

    #[test]
    fn debian_uses_persistence() {
        assert_eq!(expected_persistence_label("debian"), Some("persistence"));
        assert_eq!(
            expected_persistence_label("debian-12.5.0-amd64-DVD-1"),
            Some("persistence"),
        );
    }

    #[test]
    fn fedora_uses_writable() {
        assert_eq!(expected_persistence_label("fedora"), Some("writable"));
        assert_eq!(
            expected_persistence_label("fedora-workstation-41-x86_64"),
            Some("writable"),
        );
    }

    #[test]
    fn kali_uses_kali_persistence() {
        assert_eq!(expected_persistence_label("kali"), Some("kali-persistence"));
        assert_eq!(
            expected_persistence_label("kali-linux-2025.1-live-amd64"),
            Some("kali-persistence"),
        );
    }

    #[test]
    fn lookup_is_case_insensitive() {
        assert_eq!(expected_persistence_label("UBUNTU"), Some("casper-rw"));
        assert_eq!(expected_persistence_label("Linuxmint"), Some("casper-rw"),);
    }

    #[test]
    fn returns_none_for_unknown_slug() {
        assert_eq!(expected_persistence_label("freebsd"), None);
        assert_eq!(expected_persistence_label("syslinux-sample"), None);
        assert_eq!(expected_persistence_label(""), None);
    }

    #[test]
    fn prefix_requires_separator() {
        // `manjaroX` must NOT match `manjaro`. Otherwise we'd map
        // a hostile slug to an unintended label.
        assert_eq!(expected_persistence_label("manjaroX"), None);
        assert_eq!(expected_persistence_label("archive"), None);
    }

    #[test]
    fn underscore_and_dot_are_valid_separators() {
        assert_eq!(expected_persistence_label("pop_os"), Some("casper-rw"),);
        assert_eq!(
            expected_persistence_label("ubuntu.24.04"),
            Some("casper-rw"),
        );
    }

    #[test]
    fn all_labels_returns_full_table() {
        let n = all_labels().len();
        assert!(n >= 25, "expected ≥ 25 entries, got {n}");
        // Sanity: no empty keys or values.
        for (k, v) in all_labels() {
            assert!(!k.is_empty(), "empty key");
            assert!(!v.is_empty(), "empty value for {k}");
        }
    }

    #[test]
    fn no_duplicate_keys() {
        let mut keys: Vec<_> = LABELS.iter().map(|(k, _)| *k).collect();
        keys.sort();
        let n_total = keys.len();
        keys.dedup();
        let n_unique = keys.len();
        assert_eq!(n_total, n_unique, "duplicate key in LABELS table");
    }

    #[test]
    fn default_label_is_debian_style() {
        // Until / unless distros standardise on a label, RaidhOS's
        // fallback is the bare `persistence` Debian Live uses.
        assert_eq!(DEFAULT_LABEL, "persistence");
    }

    #[test]
    fn longest_prefix_match_wins() {
        // 'kali-linux' must match the longer key before the shorter
        // 'kali' (both happen to map to the same label here, but
        // the test guards future cases where they diverge).
        assert_eq!(
            expected_persistence_label("kali-linux-2024"),
            Some("kali-persistence"),
        );
    }
}
