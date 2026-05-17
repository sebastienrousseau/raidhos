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
    out.push_str("insmod part_gpt\n");
    out.push_str("insmod fat\n");
    out.push_str("insmod exfat\n");
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

    for entry in &config.entries {
        out.push_str(&menuentry(entry));
    }
    out
}

fn menuentry(entry: &BootEntryConfig) -> String {
    let title = sanitize(&entry.title);
    let path = sanitize(&entry.path);
    let params = sanitize(&entry.params);
    let initrd = sanitize(&entry.initrd);
    let kargs = sanitize(&entry.kargs);

    let mut out = String::new();
    out.push_str(&format!("menuentry \"{title}\" {{\n"));
    out.push_str(&format!(
        "  set isofile=\"($root){}\"\n",
        path_prefix(&path)
    ));
    out.push_str("  loopback loop $isofile\n");
    out.push_str("  if [ -f (loop)/boot/grub/grub.cfg ]; then\n");
    out.push_str("    configfile (loop)/boot/grub/grub.cfg\n");
    out.push_str("  elif [ -f (loop)/casper/vmlinuz ]; then\n");
    out.push_str(&format!(
        "    linux (loop)/casper/vmlinuz {params} {kargs} iso-scan/filename=$isofile\n"
    ));
    if !initrd.is_empty() {
        out.push_str(&format!("    initrd {initrd}\n"));
    } else {
        out.push_str("    initrd (loop)/casper/initrd\n");
    }
    out.push_str("  elif [ -f (loop)/live/vmlinuz ]; then\n");
    out.push_str(&format!(
        "    linux (loop)/live/vmlinuz {params} {kargs} boot=live findiso=$isofile\n"
    ));
    if !initrd.is_empty() {
        out.push_str(&format!("    initrd {initrd}\n"));
    } else {
        out.push_str("    initrd (loop)/live/initrd.img\n");
    }
    out.push_str("  else\n");
    out.push_str("    echo \"No known kernel path found in ISO.\"\n");
    out.push_str("  fi\n");
    out.push_str("}\n");
    out
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
            entries: vec![],
        };
        let out = render_grub_cfg(&config, "DATA");
        assert!(out.contains("search --no-floppy --label DATA --set=root"));
    }

    #[test]
    fn render_menuentry_contains_loopback() {
        let config = BootConfig {
            default_entry: None,
            entries: vec![BootEntryConfig {
                title: "Test".to_string(),
                path: "/boot/isos/test.iso".to_string(),
                params: "quiet".to_string(),
                initrd: "".to_string(),
                kargs: "".to_string(),
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
            entries: vec![BootEntryConfig {
                title: "evil\" } echo PWNED { ".to_string(),
                path: "test.iso".to_string(),
                params: String::new(),
                initrd: String::new(),
                kargs: String::new(),
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
}
