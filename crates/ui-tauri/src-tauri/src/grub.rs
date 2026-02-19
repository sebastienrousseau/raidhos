use crate::{BootConfig, BootEntryConfig};

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
    out.push_str(&format!("menuentry \"{}\" {{\n", title));
    out.push_str(&format!("  set isofile=\"($root){}\"\n", path_prefix(&path)));
    out.push_str("  loopback loop $isofile\n");
    out.push_str("  if [ -f (loop)/boot/grub/grub.cfg ]; then\n");
    out.push_str("    configfile (loop)/boot/grub/grub.cfg\n");
    out.push_str("  elif [ -f (loop)/casper/vmlinuz ]; then\n");
    out.push_str(&format!(
        "    linux (loop)/casper/vmlinuz {} {} iso-scan/filename=$isofile\n",
        params, kargs
    ));
    if !initrd.is_empty() {
        out.push_str(&format!("    initrd {}\n", initrd));
    } else {
        out.push_str("    initrd (loop)/casper/initrd\n");
    }
    out.push_str("  elif [ -f (loop)/live/vmlinuz ]; then\n");
    out.push_str(&format!(
        "    linux (loop)/live/vmlinuz {} {} boot=live findiso=$isofile\n",
        params, kargs
    ));
    if !initrd.is_empty() {
        out.push_str(&format!("    initrd {}\n", initrd));
    } else {
        out.push_str("    initrd (loop)/live/initrd.img\n");
    }
    out.push_str("  else\n");
    out.push_str("    echo \"No known kernel path found in ISO.\"\n");
    out.push_str("  fi\n");
    out.push_str("}\n");
    out
}

pub fn sanitize(input: &str) -> String {
    input.replace('"', "").replace('\n', " ").trim().to_string()
}

pub fn path_prefix(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_quotes_and_newlines() {
        let s = "\"hello\nworld\"";
        assert_eq!(sanitize(s), "hello world");
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
}
