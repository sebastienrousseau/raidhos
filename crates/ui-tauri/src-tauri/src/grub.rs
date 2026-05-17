//! GRUB configuration generator.
//!
//! Produces a `grub.cfg` from the user-supplied `BootConfig`. Every
//! string interpolated into the output passes through [`sanitize`],
//! which strips characters with meaning to the GRUB scripting language.
//! Without this, a hostile ISO filename (or a hostile boot entry
//! title) could escape its quoted context and execute GRUB commands
//! before the kernel even runs.
//!
//! See `docs/THREAT_MODEL.md` for context.

use crate::{BootConfig, BootEntryConfig};

/// Render a complete `grub.cfg`.
pub fn render_grub_cfg(config: &BootConfig, data_label: &str) -> String {
    let mut out = String::new();
    out.push_str("set timeout=5\n");
    if let Some(default) = &config.default_entry {
        out.push_str(&format!("set default=\"{}\"\n", sanitize(default)));
    }
    // Ventoy gap G13: superuser + PBKDF2 password gate. Both must
    // be set together; a hash-without-superuser would be silently
    // ignored by GRUB, and a superuser-without-hash would lock the
    // user out. The hash format check rejects plaintext passwords
    // (the most common foot-gun).
    if !config.grub_superuser.is_empty() && is_grub_pbkdf2_hash(&config.grub_password_pbkdf2) {
        let user = sanitize_username(&config.grub_superuser);
        out.push_str(&format!("set superusers=\"{user}\"\n"));
        // The hash is the literal output of grub-mkpasswd-pbkdf2
        // — it contains '.' '/' '$' but no characters that escape
        // GRUB's string parsing, so it goes through unmodified.
        // Sanitising it would corrupt the hash.
        out.push_str(&format!(
            "password_pbkdf2 {user} {}\n",
            config.grub_password_pbkdf2.trim()
        ));
    }
    out.push_str("insmod part_gpt\n");
    out.push_str("insmod fat\n");
    out.push_str("insmod exfat\n");
    // Ventoy gaps G8/G9 — keep boot-time readable when the DATA
    // partition is one of the non-default filesystems.
    out.push_str("insmod ntfs\n");
    out.push_str("insmod ext2\n");
    out.push_str("insmod btrfs\n");
    out.push_str("insmod xfs\n");
    // Ventoy gap G10 — UDF for Blu-ray rescue / install images.
    out.push_str("insmod udf\n");
    out.push_str("insmod iso9660\n");
    out.push_str("insmod loopback\n");
    out.push_str("insmod search\n");
    out.push_str(&format!(
        "search --no-floppy --label {} --set=root\n",
        sanitize(data_label)
    ));
    out.push_str("set isopath=/boot/isos\n");
    out.push_str("export root\n");
    out.push_str("export isopath\n");

    if config.tree_view {
        // Ventoy gap G20: TreeView. Group entries by sanitised
        // `class`. Entries with no class render at the top level
        // (so power-user entries stay one keypress from boot).
        let mut by_class: std::collections::BTreeMap<String, Vec<&BootEntryConfig>> =
            std::collections::BTreeMap::new();
        let mut flat: Vec<&BootEntryConfig> = Vec::new();
        for entry in &config.entries {
            if entry.hidden {
                continue;
            }
            let class = sanitize(&entry.class);
            if class.is_empty() {
                flat.push(entry);
            } else {
                by_class.entry(class).or_default().push(entry);
            }
        }
        for entry in flat {
            out.push_str(&menuentry(entry));
        }
        for (class, entries) in by_class {
            out.push_str(&format!("submenu \"{class}\" {{\n"));
            for entry in entries {
                // Indent two spaces inside the submenu block for
                // readability; GRUB doesn't care about whitespace.
                for line in menuentry(entry).lines() {
                    out.push_str("  ");
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out.push_str("}\n");
        }
    } else {
        for entry in &config.entries {
            // Ventoy gap G17: image_blacklist. Hidden entries are
            // skipped before sanitisation; they never appear in
            // the rendered output at all.
            if entry.hidden {
                continue;
            }
            out.push_str(&menuentry(entry));
        }
    }
    if config.enable_disk_browser {
        out.push_str(&disk_browser_menuentry());
    }
    out
}

/// Ventoy gap G22 — F2-hotkeyed "Browse local disks" menuentry.
/// Walks every detected `(hd*,*)` partition, looks for a
/// `/boot/grub/grub.cfg` (or `/EFI/BOOT/grub.cfg`) on each, and
/// chains into the first match. Lets users boot ISOs that live
/// on an internal drive rather than on the USB.
///
/// All literal strings here are static; nothing user-controlled is
/// interpolated, so no sanitisation is needed in this function.
fn disk_browser_menuentry() -> String {
    let mut out = String::new();
    out.push_str("menuentry \"Browse local disks (F2)\" --hotkey=f2 {\n");
    out.push_str("  insmod regexp\n");
    out.push_str("  for dev in (hd*,*); do\n");
    out.push_str("    if [ -f $dev/boot/grub/grub.cfg ]; then\n");
    out.push_str("      echo \"Chaining into $dev/boot/grub/grub.cfg\"\n");
    out.push_str("      configfile $dev/boot/grub/grub.cfg\n");
    out.push_str("    elif [ -f $dev/EFI/BOOT/grub.cfg ]; then\n");
    out.push_str("      echo \"Chaining into $dev/EFI/BOOT/grub.cfg\"\n");
    out.push_str("      configfile $dev/EFI/BOOT/grub.cfg\n");
    out.push_str("    fi\n");
    out.push_str("  done\n");
    out.push_str("  echo \"No bootable grub.cfg found on local disks.\"\n");
    out.push_str("  echo \"Press any key to return to the menu.\"\n");
    out.push_str("  read\n");
    out.push_str("}\n");
    out
}

fn menuentry(entry: &BootEntryConfig) -> String {
    let title = sanitize(&entry.title);
    let path = sanitize(&entry.path);
    let params = sanitize(&entry.params);
    let initrd = sanitize(&entry.initrd);
    let kargs = sanitize(&entry.kargs);
    let class = sanitize(&entry.class);
    let tip = sanitize(&entry.tip);
    // Ventoy gap G18: per-ISO persistence backend. The kernel
    // command-line gets `persistent persistent-path=<value>`
    // appended when this field is set on the entry.
    let persistence = sanitize(&entry.persistence_backend);
    let persistence_kargs = if persistence.is_empty() {
        String::new()
    } else {
        format!(" persistent persistent-path={persistence}")
    };

    let mut out = String::new();
    if !tip.is_empty() {
        out.push_str(&format!("# tip: {tip}\n"));
    }
    if class.is_empty() {
        out.push_str(&format!("menuentry \"{title}\" {{\n"));
    } else {
        out.push_str(&format!("menuentry \"{title}\" --class {class} {{\n"));
    }
    // Ventoy gap G7: chainload a `.efi` binary directly. Anything
    // that doesn't end with `.iso` (case-insensitive) is treated
    // as a raw UEFI binary; this is the route memtest86+ and most
    // firmware-updater images expect.
    if is_efi_binary(&path) {
        out.push_str(&format!(
            "  chainloader \"($root){}\"\n",
            path_prefix(&path)
        ));
    } else if is_raw_disk_image(&path) {
        // Ventoy gap G6: `.img` / `.raw` raw disk image. Loopback-
        // mount the image and chainload its embedded boot sector.
        // Suits OpenWrt / floppy rescue / small embedded images
        // that ship their own MBR. Persistence kargs don't apply
        // — the image isn't a Linux live ISO.
        out.push_str(&format!(
            "  set imgfile=\"($root){}\"\n",
            path_prefix(&path)
        ));
        out.push_str("  loopback loop $imgfile\n");
        out.push_str("  chainloader (loop)\n");
    } else {
        out.push_str(&format!(
            "  set isofile=\"($root){}\"\n",
            path_prefix(&path)
        ));
        out.push_str("  loopback loop $isofile\n");
        out.push_str("  if [ -f (loop)/boot/grub/grub.cfg ]; then\n");
        out.push_str("    configfile (loop)/boot/grub/grub.cfg\n");
        out.push_str("  elif [ -f (loop)/casper/vmlinuz ]; then\n");
        out.push_str(&format!(
            "    linux (loop)/casper/vmlinuz {params} {kargs} iso-scan/filename=$isofile{persistence_kargs}\n"
        ));
        if !initrd.is_empty() {
            out.push_str(&format!("    initrd {initrd}\n"));
        } else {
            out.push_str("    initrd (loop)/casper/initrd\n");
        }
        out.push_str("  elif [ -f (loop)/live/vmlinuz ]; then\n");
        out.push_str(&format!(
            "    linux (loop)/live/vmlinuz {params} {kargs} boot=live findiso=$isofile{persistence_kargs}\n"
        ));
        if !initrd.is_empty() {
            out.push_str(&format!("    initrd {initrd}\n"));
        } else {
            out.push_str("    initrd (loop)/live/initrd.img\n");
        }
        out.push_str("  else\n");
        out.push_str("    echo \"No known kernel path found in ISO.\"\n");
        out.push_str("  fi\n");
    }
    out.push_str("}\n");
    out
}

/// Heuristic: does the (already-sanitised) path look like a UEFI
/// binary? Used by Ventoy gap G7 to switch between `loopback`-ISO
/// mounting and direct `chainloader` invocation.
fn is_efi_binary(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".efi")
}

/// Heuristic: does the (already-sanitised) path look like a raw
/// disk image rather than a Linux live ISO? Used by Ventoy gap G6
/// to route `.img` / `.raw` through a `loopback` + `chainloader
/// (loop)` boot rather than the kernel-search loopback flow.
fn is_raw_disk_image(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".img") || lower.ends_with(".raw")
}

/// Strip every character that has meaning to the GRUB scripting
/// language or that would let attacker-controlled strings (e.g. ISO
/// filenames, boot-entry titles) escape their quoted context.
///
/// Forbidden:
/// - `"` and `\` would break double-quoted strings.
/// - `$` and `` ` `` introduce variable / command substitution.
/// - `;`, `{`, `}` are statement separators / block delimiters.
/// - `\n` and `\r` would inject new commands.
/// - `\0` and other C0 control bytes have undefined behaviour.
/// - `=`, `(`, `)`, `[`, `]`, `*`, `?`, `<`, `>`, `&`, `|`, `#`, `!`
///   are filtered as defence in depth even where they may be
///   syntactically harmless in some positions — GRUB versions differ.
pub fn sanitize(input: &str) -> String {
    const FORBIDDEN: &[char] = &[
        '"', '\\', '$', '`', ';', '{', '}', '\n', '\r', '\t', '=', '(', ')', '[', ']', '*', '?',
        '<', '>', '&', '|', '#', '!',
    ];
    input
        .chars()
        .filter(|c| {
            // Drop control bytes 0x00..0x1F (we already listed the
            // common ones above; this is the catch-all).
            if (*c as u32) < 0x20 {
                return false;
            }
            !FORBIDDEN.contains(c)
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Ensure a path begins with `/` so `($root)<path>` resolves at the
/// filesystem root rather than the GRUB working directory.
pub fn path_prefix(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

/// Strictly validate that a string looks like the output of
/// `grub-mkpasswd-pbkdf2`. Format:
///
///   `grub.pbkdf2.sha512.<rounds>.<salt-hex>.<hash-hex>`
///
/// Rejects empty strings, plaintext passwords, and anything that
/// doesn't carry the four-segment shape. Defence against a user
/// mistakenly pasting their actual password into the field.
fn is_grub_pbkdf2_hash(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with("grub.pbkdf2.sha512.") {
        return false;
    }
    // Split on '.' — expect exactly 6 segments
    //   ["grub", "pbkdf2", "sha512", "<rounds>", "<salt>", "<hash>"]
    let segs: Vec<&str> = s.split('.').collect();
    if segs.len() != 6 {
        return false;
    }
    // Rounds is a decimal integer ≥ 1. Avoid `is_none_or` (stable
    // since 1.82) to keep the workspace's 1.78 MSRV.
    match segs[3].parse::<u32>() {
        Ok(n) if n > 0 => {}
        _ => return false,
    }
    // Salt + hash are hex; PBKDF2-SHA512 produces 128-hex-char (64-
    // byte) output. We don't pin the salt length — grub accepts
    // any positive even-length hex. But both must be hex.
    segs[4].chars().all(|c| c.is_ascii_hexdigit())
        && !segs[4].is_empty()
        && segs[5].chars().all(|c| c.is_ascii_hexdigit())
        && !segs[5].is_empty()
}

/// Sanitise a GRUB username: alphanumerics, underscore, hyphen
/// only. Empty input → "admin" so the rendered output remains
/// valid grub.cfg syntax even if the caller passes whitespace.
fn sanitize_username(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if cleaned.is_empty() {
        "admin".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_quotes_and_newlines() {
        let s = "\"hello\nworld\"";
        assert_eq!(sanitize(s), "helloworld");
    }

    #[test]
    fn sanitize_strips_dollar_and_backtick() {
        assert_eq!(sanitize("a$b`c"), "abc");
    }

    #[test]
    fn sanitize_strips_braces_and_semicolons() {
        assert_eq!(sanitize("a{b}c;d"), "abcd");
    }

    #[test]
    fn sanitize_strips_backslash() {
        assert_eq!(sanitize(r"a\b\nc"), "abnc");
    }

    #[test]
    fn sanitize_strips_control_bytes() {
        assert_eq!(sanitize("a\x01b\x1fc\x07d"), "abcd");
    }

    #[test]
    fn sanitize_strips_pipe_and_amp() {
        assert_eq!(sanitize("a|b&c"), "abc");
    }

    #[test]
    fn sanitize_preserves_printable_text() {
        assert_eq!(sanitize("Ubuntu 24.04 LTS"), "Ubuntu 24.04 LTS");
    }

    #[test]
    fn sanitize_blocks_grub_command_injection() {
        // Realistic attacker payload that previously could break out:
        //   "  } echo PWNED ; menuentry "
        let payload = "evil\" } echo PWNED ; menuentry \"x";
        let out = sanitize(payload);
        assert!(!out.contains('"'));
        assert!(!out.contains('}'));
        assert!(!out.contains(';'));
        assert!(!out.contains('{'));
    }

    #[test]
    fn path_prefix_adds_slash() {
        assert_eq!(path_prefix("/boot/isos/a.iso"), "/boot/isos/a.iso");
        assert_eq!(path_prefix("boot/isos/a.iso"), "/boot/isos/a.iso");
    }

    #[test]
    fn render_contains_search_label() {
        let config = BootConfig {
            default_entry: None,
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
            entries: vec![],
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("search --no-floppy --label DATA --set=root"));
    }

    #[test]
    fn render_menuentry_contains_loopback() {
        let config = BootConfig {
            default_entry: None,
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
            entries: vec![BootEntryConfig {
                title: "Test".to_string(),
                path: "/boot/isos/test.iso".to_string(),
                params: "quiet".to_string(),
                initrd: "".to_string(),
                kargs: "".to_string(),
                class: String::new(),
                tip: String::new(),
                hidden: false,
                persistence_backend: String::new(),
            }],
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("loopback loop $isofile"));
        assert!(out.contains("menuentry \"Test\""));
    }

    #[test]
    fn render_neutralises_hostile_title() {
        let config = BootConfig {
            default_entry: None,
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
            entries: vec![BootEntryConfig {
                title: "evil\" } echo PWNED { ".to_string(),
                path: "test.iso".to_string(),
                params: String::new(),
                initrd: String::new(),
                kargs: String::new(),
                class: String::new(),
                tip: String::new(),
                hidden: false,
                persistence_backend: String::new(),
            }],
        };
        let out = render_grub_cfg(&config, "DATA");
        // The rendered menuentry must still be balanced: exactly one
        // `{` and exactly one `}` for this entry (the opening
        // `menuentry "..." {` and the closing `}`), with no injected
        // statements between.
        let opens = out.matches('{').count();
        let closes = out.matches('}').count();
        assert_eq!(opens, closes, "unbalanced braces: {out}");
        assert!(!out.contains("PWNED { "));
    }

    // ---------------------------------------------------------------
    // Ventoy gaps G11 (menu_class / menu_tip) and G17 (image_blacklist)
    // ---------------------------------------------------------------

    fn entry(title: &str, path: &str) -> BootEntryConfig {
        BootEntryConfig {
            title: title.to_string(),
            path: path.to_string(),
            params: String::new(),
            initrd: String::new(),
            kargs: String::new(),
            class: String::new(),
            tip: String::new(),
            hidden: false,
            persistence_backend: String::new(),
        }
    }

    #[test]
    fn render_emits_class_when_set() {
        let mut e = entry("Ubuntu", "ubuntu.iso");
        e.class = "linux".to_string();
        let config = BootConfig {
            default_entry: None,
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
            entries: vec![e],
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(
            out.contains("menuentry \"Ubuntu\" --class linux {"),
            "missing class in: {out}",
        );
    }

    #[test]
    fn render_omits_class_when_empty() {
        let config = BootConfig {
            default_entry: None,
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
            entries: vec![entry("Plain", "p.iso")],
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("menuentry \"Plain\" {"));
        assert!(!out.contains("--class"));
    }

    #[test]
    fn render_sanitises_class_field() {
        let mut e = entry("Evil", "e.iso");
        e.class = "linux; rm -rf /".to_string();
        let config = BootConfig {
            default_entry: None,
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
            entries: vec![e],
        };
        let out = render_grub_cfg(&config, "DATA");
        // The rendered `--class …` token must NOT carry the
        // attacker-supplied semicolon. The output as a whole still
        // contains semicolons from the GRUB boilerplate (`if [ … ];
        // then` etc.), so we check the menuentry line specifically.
        let menuentry_line = out
            .lines()
            .find(|l| l.starts_with("menuentry"))
            .expect("a menuentry line");
        assert!(
            !menuentry_line.contains(';'),
            "menuentry line contains semicolon: {menuentry_line}",
        );
        // The class still appears, just without the dangerous chars.
        assert!(out.contains("--class linux rm -rf"));
    }

    #[test]
    fn render_emits_tip_as_comment() {
        let mut e = entry("Arch", "arch.iso");
        e.tip = "Rolling-release Linux".to_string();
        let config = BootConfig {
            default_entry: None,
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
            entries: vec![e],
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(
            out.contains("# tip: Rolling-release Linux"),
            "missing tip in: {out}",
        );
    }

    #[test]
    fn render_omits_tip_when_empty() {
        let config = BootConfig {
            default_entry: None,
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
            entries: vec![entry("Plain", "p.iso")],
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(!out.contains("# tip:"));
    }

    #[test]
    fn render_skips_hidden_entries_entirely() {
        let mut hidden = entry("Hidden", "secret.iso");
        hidden.hidden = true;
        let config = BootConfig {
            default_entry: None,
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
            entries: vec![entry("Shown", "shown.iso"), hidden],
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("menuentry \"Shown\""));
        assert!(!out.contains("Hidden"));
        assert!(!out.contains("secret.iso"));
    }

    #[test]
    fn render_with_no_visible_entries_produces_no_menuentries() {
        let mut hidden_a = entry("A", "a.iso");
        hidden_a.hidden = true;
        let mut hidden_b = entry("B", "b.iso");
        hidden_b.hidden = true;
        let config = BootConfig {
            default_entry: None,
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
            entries: vec![hidden_a, hidden_b],
        };
        let out = render_grub_cfg(&config, "DATA");
        // Boilerplate is still there but no menuentry blocks.
        assert!(out.contains("search --no-floppy --label DATA"));
        assert!(!out.contains("menuentry"));
    }

    // ---------------------------------------------------------------
    // Ventoy gap G13: GRUB password protection
    // ---------------------------------------------------------------

    const VALID_HASH: &str =
        "grub.pbkdf2.sha512.10000.aabbccdd.eeff00112233445566778899aabbccddeeff";

    #[test]
    fn is_grub_pbkdf2_hash_accepts_canonical_output() {
        assert!(is_grub_pbkdf2_hash(VALID_HASH));
    }

    #[test]
    fn is_grub_pbkdf2_hash_trims_whitespace() {
        assert!(is_grub_pbkdf2_hash(&format!("  {VALID_HASH}  \n")));
    }

    #[test]
    fn is_grub_pbkdf2_hash_rejects_plaintext_password() {
        assert!(!is_grub_pbkdf2_hash("hunter2"));
        assert!(!is_grub_pbkdf2_hash("password123"));
        assert!(!is_grub_pbkdf2_hash(""));
    }

    #[test]
    fn is_grub_pbkdf2_hash_rejects_wrong_prefix() {
        assert!(!is_grub_pbkdf2_hash("grub.pbkdf2.sha256.10000.aabb.ccdd"));
        assert!(!is_grub_pbkdf2_hash("md5.deadbeef"));
    }

    #[test]
    fn is_grub_pbkdf2_hash_rejects_wrong_segment_count() {
        // 5 segments instead of 6
        assert!(!is_grub_pbkdf2_hash("grub.pbkdf2.sha512.10000.aabbcc"));
        // 7 segments instead of 6
        assert!(!is_grub_pbkdf2_hash("grub.pbkdf2.sha512.10000.aa.bb.cc"));
    }

    #[test]
    fn is_grub_pbkdf2_hash_rejects_zero_rounds() {
        assert!(!is_grub_pbkdf2_hash(
            "grub.pbkdf2.sha512.0.aabbccdd.eeff00112233"
        ));
    }

    #[test]
    fn is_grub_pbkdf2_hash_rejects_non_hex_payload() {
        assert!(!is_grub_pbkdf2_hash(
            "grub.pbkdf2.sha512.10000.NOTHEX.eeff00112233"
        ));
        assert!(!is_grub_pbkdf2_hash(
            "grub.pbkdf2.sha512.10000.aabbccdd.NOTHEX"
        ));
    }

    #[test]
    fn sanitize_username_filters_non_ident_chars() {
        assert_eq!(sanitize_username("admin"), "admin");
        assert_eq!(sanitize_username("op-1"), "op-1");
        assert_eq!(sanitize_username("op_2"), "op_2");
        assert_eq!(sanitize_username("admin'; DROP"), "adminDROP");
        assert_eq!(sanitize_username(""), "admin");
        // Whitespace-only also collapses to the default "admin".
        assert_eq!(sanitize_username("   "), "admin");
    }

    #[test]
    fn render_emits_password_lines_when_both_set() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("Plain", "p.iso")],
            grub_superuser: "admin".to_string(),
            grub_password_pbkdf2: VALID_HASH.to_string(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("set superusers=\"admin\""));
        assert!(out.contains(&format!("password_pbkdf2 admin {VALID_HASH}")));
    }

    #[test]
    fn render_omits_password_lines_when_superuser_missing() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("Plain", "p.iso")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: VALID_HASH.to_string(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(!out.contains("set superusers"));
        assert!(!out.contains("password_pbkdf2"));
    }

    #[test]
    fn render_omits_password_lines_when_hash_invalid() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("Plain", "p.iso")],
            grub_superuser: "admin".to_string(),
            grub_password_pbkdf2: "hunter2".to_string(), // plaintext — rejected
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(!out.contains("set superusers"));
        assert!(!out.contains("password_pbkdf2"));
        // The plaintext password must NOT leak into the rendered output.
        assert!(!out.contains("hunter2"));
    }

    #[test]
    fn render_sanitises_superuser_name() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("Plain", "p.iso")],
            // Hostile superuser name containing GRUB metachars.
            grub_superuser: "admin\"; echo bad".to_string(),
            grub_password_pbkdf2: VALID_HASH.to_string(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        // The injection metachars must not survive in any
        // password-related directive line.
        let superusers_line = out
            .lines()
            .find(|l| l.starts_with("set superusers="))
            .expect("superusers line");
        let password_line = out
            .lines()
            .find(|l| l.starts_with("password_pbkdf2 "))
            .expect("password_pbkdf2 line");
        for line in [superusers_line, password_line] {
            assert!(!line.contains(';'), "semicolon survived: {line}");
            // The closing quote of `superusers=\"X\"` is legitimate;
            // an attacker-injected `\"` would be a second one. The
            // sanitised username has no `\"` of its own.
            let inner_quotes = line.matches('"').count();
            assert!(
                inner_quotes <= 2,
                "extra quotes in {line} (count={inner_quotes})",
            );
        }
        // The sanitised username flows through, sans metachars.
        assert!(out.contains("set superusers=\"adminechobad\""));
        assert!(out.contains("password_pbkdf2 adminechobad"));
    }

    // ---------------------------------------------------------------
    // Ventoy gap G7: .EFI binary direct chainload
    // ---------------------------------------------------------------

    #[test]
    fn is_efi_binary_recognises_efi_extension() {
        assert!(is_efi_binary("bootx64.efi"));
        assert!(is_efi_binary("memtest.efi"));
        assert!(is_efi_binary("/path/to/SHELL.EFI"));
        // Case-insensitive.
        assert!(is_efi_binary("Bootmgr.eFi"));
    }

    #[test]
    fn is_efi_binary_rejects_iso_and_other() {
        assert!(!is_efi_binary("ubuntu.iso"));
        assert!(!is_efi_binary("openwrt.img"));
        assert!(!is_efi_binary(""));
        // Substring 'efi' in the path mustn't match — needs to end
        // with the extension.
        assert!(!is_efi_binary("/uefi/disk.iso"));
    }

    #[test]
    fn render_chainloads_efi_binary() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("Memtest", "/boot/efi/memtest86.efi")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        // The .efi path uses chainloader, not loopback.
        assert!(
            out.contains("chainloader \"($root)/boot/efi/memtest86.efi\""),
            "missing chainloader in: {out}",
        );
        // None of the ISO boilerplate should appear for this entry.
        assert!(!out.contains("loopback loop"));
        assert!(!out.contains("(loop)/casper"));
    }

    #[test]
    fn render_keeps_iso_path_for_iso_entries() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("Ubuntu", "/boot/isos/ubuntu.iso")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        // ISO entries still use the loopback flow.
        assert!(out.contains("loopback loop $isofile"));
        assert!(!out.contains("chainloader"));
    }

    #[test]
    fn render_mixes_iso_and_efi_entries() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![
                entry("Memtest", "/memtest86.efi"),
                entry("Ubuntu", "/boot/isos/ubuntu.iso"),
            ],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("chainloader \"($root)/memtest86.efi\""));
        assert!(out.contains("set isofile=\"($root)/boot/isos/ubuntu.iso\""));
        // Each entry has its own balanced { } block.
        assert_eq!(out.matches('{').count(), out.matches('}').count());
    }

    // ---------------------------------------------------------------
    // Ventoy gap G18: per-ISO persistence backend file
    // ---------------------------------------------------------------

    #[test]
    fn render_omits_persistence_kargs_when_unset() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("Ubuntu", "/boot/isos/ubuntu.iso")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(!out.contains("persistent persistent-path"));
    }

    #[test]
    fn render_appends_persistence_kargs_to_casper_and_live() {
        let mut e = entry("Ubuntu", "/boot/isos/ubuntu.iso");
        e.persistence_backend = "/persistence/ubuntu.dat".to_string();
        let config = BootConfig {
            default_entry: None,
            entries: vec![e],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        // Both the casper and the live kernel command lines pick up
        // the persistence kargs at the end.
        assert!(
            out.contains(
                "linux (loop)/casper/vmlinuz   iso-scan/filename=$isofile persistent persistent-path=/persistence/ubuntu.dat"
            ),
            "missing casper persistence in: {out}",
        );
        assert!(
            out.contains(
                "linux (loop)/live/vmlinuz   boot=live findiso=$isofile persistent persistent-path=/persistence/ubuntu.dat"
            ),
            "missing live persistence in: {out}",
        );
    }

    #[test]
    fn render_sanitises_persistence_backend_field() {
        let mut e = entry("Ubuntu", "/u.iso");
        e.persistence_backend = "/safe; rm -rf /".to_string();
        let config = BootConfig {
            default_entry: None,
            entries: vec![e],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        // Look at the linux line specifically. The sanitiser strips ;.
        let linux_line = out
            .lines()
            .find(|l| l.contains("linux (loop)/casper/vmlinuz"))
            .expect("casper line");
        assert!(!linux_line.contains(';'), "semicolon escaped: {linux_line}");
        assert!(linux_line.contains("persistent persistent-path=/safe rm -rf /"));
    }

    #[test]
    fn render_skips_persistence_for_efi_chainload() {
        // EFI binaries don't take a kernel command line, so the
        // persistence kargs must not appear in the chainloader path.
        let mut e = entry("Memtest", "/memtest86.efi");
        e.persistence_backend = "/persistence.dat".to_string();
        let config = BootConfig {
            default_entry: None,
            entries: vec![e],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("chainloader \"($root)/memtest86.efi\""));
        assert!(!out.contains("persistent persistent-path"));
    }

    // ---------------------------------------------------------------
    // Ventoy gap G20: ListView ↔ TreeView toggle
    // ---------------------------------------------------------------

    fn classed_entry(title: &str, path: &str, class: &str) -> BootEntryConfig {
        let mut e = entry(title, path);
        e.class = class.to_string();
        e
    }

    #[test]
    fn render_tree_view_off_renders_flat_list() {
        // Default (tree_view = false) must keep the existing flat
        // layout — no `submenu` directive in the output.
        let config = BootConfig {
            default_entry: None,
            entries: vec![
                classed_entry("Ubuntu 24.04", "/u.iso", "linux"),
                classed_entry("Fedora 41", "/f.iso", "linux"),
                entry("Memtest", "/memtest.efi"),
            ],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(
            !out.contains("submenu "),
            "tree_view=false must not emit submenu: {out}"
        );
        // All three entries appear as top-level menuentries.
        assert_eq!(out.matches("menuentry \"").count(), 3);
    }

    #[test]
    fn render_tree_view_on_groups_by_class() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![
                classed_entry("Ubuntu", "/u.iso", "linux"),
                classed_entry("Fedora", "/f.iso", "linux"),
                classed_entry("Win10", "/w.iso", "windows"),
            ],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: true,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        // Each class produces one submenu block.
        assert!(
            out.contains("submenu \"linux\" {"),
            "missing linux submenu: {out}"
        );
        assert!(
            out.contains("submenu \"windows\" {"),
            "missing windows submenu: {out}"
        );
        // All three menuentries still appear inside the submenus.
        assert_eq!(out.matches("menuentry \"").count(), 3);
        // Braces still balance with the extra submenu wrappers.
        assert_eq!(out.matches('{').count(), out.matches('}').count());
    }

    #[test]
    fn render_tree_view_classless_entries_float_to_top() {
        // Power-user entries with no class stay one keypress from
        // boot; classed entries get tucked into submenus.
        let config = BootConfig {
            default_entry: None,
            entries: vec![
                entry("Memtest", "/memtest.efi"),
                classed_entry("Ubuntu", "/u.iso", "linux"),
            ],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: true,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        // The top-level menuentry must appear before the submenu line.
        let memtest_pos = out
            .find("menuentry \"Memtest\"")
            .expect("memtest entry missing");
        let submenu_pos = out
            .find("submenu \"linux\"")
            .expect("linux submenu missing");
        assert!(
            memtest_pos < submenu_pos,
            "classless entry should appear before submenu (memtest at {memtest_pos}, submenu at {submenu_pos})",
        );
    }

    #[test]
    fn render_tree_view_skips_hidden_entries() {
        // Ventoy gap G17 still applies under TreeView — hidden
        // entries must not appear in any submenu.
        let mut hidden = classed_entry("Secret", "/secret.iso", "linux");
        hidden.hidden = true;
        let config = BootConfig {
            default_entry: None,
            entries: vec![hidden, classed_entry("Ubuntu", "/u.iso", "linux")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: true,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(!out.contains("Secret"), "hidden entry leaked: {out}");
        assert!(out.contains("menuentry \"Ubuntu\""));
    }

    #[test]
    fn render_tree_view_sanitises_class_in_submenu_label() {
        // Attacker-controlled class names must not escape the
        // submenu quote context.
        let config = BootConfig {
            default_entry: None,
            entries: vec![classed_entry("X", "/x.iso", "linux\"; echo bad")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: true,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        let submenu_line = out
            .lines()
            .find(|l| l.starts_with("submenu \""))
            .expect("submenu line");
        assert!(
            !submenu_line.contains(';'),
            "semicolon survived: {submenu_line}"
        );
        assert_eq!(
            submenu_line.matches('"').count(),
            2,
            "submenu must have exactly one quoted label: {submenu_line}",
        );
        // The sanitised class name still flows through.
        assert!(out.contains("submenu \"linux echo bad\" {"));
    }

    // ---------------------------------------------------------------
    // Ventoy gap G22: Browse local disks (F2 hotkey)
    // ---------------------------------------------------------------

    #[test]
    fn render_disk_browser_off_by_default() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("X", "/x.iso")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(!out.contains("Browse local disks"));
        assert!(!out.contains("--hotkey=f2"));
    }

    #[test]
    fn render_disk_browser_when_enabled() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("X", "/x.iso")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: true,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(
            out.contains("menuentry \"Browse local disks (F2)\" --hotkey=f2 {"),
            "missing disk-browser menuentry: {out}",
        );
        // Looks at both BIOS and EFI grub.cfg locations.
        assert!(out.contains("$dev/boot/grub/grub.cfg"));
        assert!(out.contains("$dev/EFI/BOOT/grub.cfg"));
        // configfile is the chain mechanism.
        assert!(out.contains("configfile $dev/boot/grub/grub.cfg"));
        // The graceful fallback message is present.
        assert!(out.contains("No bootable grub.cfg found"));
        // Brace balance preserved.
        assert_eq!(out.matches('{').count(), out.matches('}').count());
    }

    #[test]
    fn render_disk_browser_appears_after_user_entries() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("Ubuntu", "/u.iso")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: true,
        };
        let out = render_grub_cfg(&config, "DATA");
        let ubuntu_pos = out.find("menuentry \"Ubuntu\"").expect("ubuntu missing");
        let browser_pos = out
            .find("menuentry \"Browse local disks")
            .expect("browser missing");
        assert!(
            ubuntu_pos < browser_pos,
            "browser must come after user entries (ubuntu at {ubuntu_pos}, browser at {browser_pos})",
        );
    }

    #[test]
    fn render_disk_browser_works_under_tree_view() {
        // Tree-view branch also emits the browser entry at the end.
        let config = BootConfig {
            default_entry: None,
            entries: vec![{
                let mut e = entry("Ubuntu", "/u.iso");
                e.class = "linux".to_string();
                e
            }],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: true,
            enable_disk_browser: true,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("submenu \"linux\" {"));
        assert!(out.contains("menuentry \"Browse local disks (F2)\""));
        assert_eq!(out.matches('{').count(), out.matches('}').count());
    }

    // ---------------------------------------------------------------
    // Ventoy gap G6: IMG / raw disk image boot
    // ---------------------------------------------------------------

    #[test]
    fn is_raw_disk_image_recognises_img_and_raw() {
        assert!(is_raw_disk_image("openwrt.img"));
        assert!(is_raw_disk_image("/boot/floppy.IMG"));
        assert!(is_raw_disk_image("disk.raw"));
        assert!(is_raw_disk_image("/x/Y.Raw"));
    }

    #[test]
    fn is_raw_disk_image_rejects_iso_and_efi() {
        assert!(!is_raw_disk_image("ubuntu.iso"));
        assert!(!is_raw_disk_image("bootx64.efi"));
        assert!(!is_raw_disk_image(""));
        // Substring 'img' / 'raw' mustn't match — it must be an
        // extension at the end of the path.
        assert!(!is_raw_disk_image("/image/ubuntu.iso"));
        assert!(!is_raw_disk_image("/braw/x.iso"));
    }

    #[test]
    fn render_loopback_chainloads_img_entries() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("OpenWrt", "/boot/isos/openwrt.img")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        // The img path goes through loopback + chainload-on-loop.
        assert!(
            out.contains("set imgfile=\"($root)/boot/isos/openwrt.img\""),
            "missing imgfile: {out}",
        );
        assert!(out.contains("loopback loop $imgfile"));
        assert!(out.contains("chainloader (loop)"));
        // None of the ISO 9660 kernel-search boilerplate should appear.
        assert!(!out.contains("(loop)/casper/vmlinuz"));
        assert!(!out.contains("(loop)/live/vmlinuz"));
        assert!(!out.contains("No known kernel path found"));
    }

    #[test]
    fn render_keeps_iso_path_for_iso_entries_under_g6() {
        // G6 must not regress G7's ISO routing.
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("Ubuntu", "/boot/isos/ubuntu.iso")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("set isofile="));
        assert!(out.contains("(loop)/casper/vmlinuz"));
        assert!(!out.contains("chainloader (loop)"));
    }

    #[test]
    fn render_keeps_efi_path_for_efi_entries_under_g6() {
        // G6 must not regress G7's EFI routing either.
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("Memtest", "/memtest86.efi")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("chainloader \"($root)/memtest86.efi\""));
        assert!(!out.contains("loopback loop"));
    }

    #[test]
    fn render_mixes_iso_efi_and_img_entries() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![
                entry("Memtest", "/m.efi"),
                entry("Ubuntu", "/u.iso"),
                entry("OpenWrt", "/o.img"),
            ],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("chainloader \"($root)/m.efi\""));
        assert!(out.contains("set isofile=\"($root)/u.iso\""));
        assert!(out.contains("set imgfile=\"($root)/o.img\""));
        assert!(out.contains("chainloader (loop)"));
        // Brace balance preserved across all three branches.
        assert_eq!(out.matches('{').count(), out.matches('}').count());
    }

    // ---------------------------------------------------------------
    // Ventoy gap G10: UDF + multi-fs insmod coverage
    // ---------------------------------------------------------------

    #[test]
    fn render_loads_udf_and_data_partition_filesystems() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("X", "/x.iso")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: false,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        // UDF for Blu-ray rescue ISOs (G10).
        assert!(out.contains("insmod udf\n"), "missing insmod udf");
        // NTFS / ext / btrfs / xfs for the corresponding --data-fs
        // choices (G8 / G9).
        for module in ["ntfs", "ext2", "btrfs", "xfs"] {
            assert!(
                out.contains(&format!("insmod {module}\n")),
                "missing insmod {module}",
            );
        }
        // The existing default modules are still loaded.
        for module in ["part_gpt", "fat", "exfat", "iso9660", "loopback", "search"] {
            assert!(
                out.contains(&format!("insmod {module}\n")),
                "regressed: missing insmod {module}",
            );
        }
    }

    #[test]
    fn render_tree_view_with_no_classed_entries_emits_no_submenu() {
        // If every visible entry is classless, the tree view
        // degenerates into a flat list — no empty submenu blocks.
        let config = BootConfig {
            default_entry: None,
            entries: vec![entry("A", "/a.iso"), entry("B", "/b.iso")],
            grub_superuser: String::new(),
            grub_password_pbkdf2: String::new(),
            tree_view: true,
            enable_disk_browser: false,
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(
            !out.contains("submenu "),
            "empty tree view should not emit submenu: {out}"
        );
        assert_eq!(out.matches("menuentry \"").count(), 2);
    }
}
