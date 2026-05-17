//! Defence-in-depth seccomp filter.
//!
//! Linux only. Applied **after** argv parsing and **before**
//! `raidhos_core::install` does any privileged work. The filter is a
//! **denylist** of syscalls that legitimate install operations
//! (`parted`, `mkfs.vfat`, `mkfs.exfat`, `mount`, `cp`, `umount`,
//! `wipefs`) never need, but which are popular escalation primitives
//! if the helper itself is compromised.
//!
//! Denylist (returns `EPERM`):
//! - `ptrace` and friends — debugger attach
//! - `bpf` — eBPF program load
//! - `perf_event_open` — perf side channels
//! - `userfaultfd` — userspace fault handling for race-window
//!   widening
//! - `modify_ldt` — local descriptor table tricks (x86)
//! - `kexec_load`, `kexec_file_load` — replace running kernel
//! - `init_module`, `finit_module`, `delete_module` — load/unload
//!   kernel modules
//! - `create_module` — pre-2.6 module loader
//! - `process_vm_readv`, `process_vm_writev` — cross-process memory
//!   access
//!
//! Filter installation failures are logged but **not fatal** — we
//! prefer to keep the install working over refusing if seccomp isn't
//! available on this kernel.

#![cfg(target_os = "linux")]

use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, SeccompRule, TargetArch};
use std::collections::BTreeMap;

fn target_arch() -> Result<TargetArch, String> {
    if cfg!(target_arch = "x86_64") {
        Ok(TargetArch::x86_64)
    } else if cfg!(target_arch = "aarch64") {
        Ok(TargetArch::aarch64)
    } else {
        Err("unsupported architecture for seccomp filter".to_string())
    }
}

/// Build and install the denylist filter on the current thread.
/// Inherited across `clone`/`fork`/`exec`, so child processes started
/// after this call run under the same restrictions.
pub fn install_denylist() -> Result<(), String> {
    // Each entry: syscall name. We map each to "return EPERM" so the
    // syscall fails predictably rather than killing the process — a
    // SIGKILL on `parted` mid-install would corrupt the partition
    // table.
    let denied = denied_syscalls();
    let arch = target_arch()?;

    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    for name in denied {
        let Some(num) = lookup_syscall(name, arch) else {
            // Unknown on this arch — skip silently.
            continue;
        };
        rules.insert(num, Vec::new());
    }

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        arch,
    )
    .map_err(|e| format!("build seccomp filter: {e}"))?;

    let prog: BpfProgram = filter
        .try_into()
        .map_err(|e: seccompiler::BackendError| format!("compile seccomp BPF: {e}"))?;

    seccompiler::apply_filter(&prog).map_err(|e| format!("apply seccomp BPF: {e}"))?;
    Ok(())
}

fn denied_syscalls() -> &'static [&'static str] {
    &[
        "ptrace",
        "bpf",
        "perf_event_open",
        "userfaultfd",
        "modify_ldt",
        "kexec_load",
        "kexec_file_load",
        "init_module",
        "finit_module",
        "delete_module",
        "create_module",
        "process_vm_readv",
        "process_vm_writev",
    ]
}

/// Hand-rolled syscall name → number table for the two architectures
/// we support. Mirrors `arch/<arch>/include/uapi/asm/unistd_64.h` in
/// the kernel tree as of Linux 6.x. Adding a new arch is a one-line
/// branch; the seccompiler crate does not expose a name lookup itself.
fn lookup_syscall(name: &str, arch: TargetArch) -> Option<i64> {
    match arch {
        TargetArch::x86_64 => x86_64_syscall(name),
        TargetArch::aarch64 => aarch64_syscall(name),
        // Any other target_arch supported by seccompiler — currently
        // riscv64 — isn't a primary build target for RaidhOS. Skip;
        // the filter just won't apply on those hosts.
        _ => None,
    }
}

fn x86_64_syscall(name: &str) -> Option<i64> {
    Some(match name {
        "ptrace" => 101,
        "perf_event_open" => 298,
        "process_vm_readv" => 310,
        "process_vm_writev" => 311,
        "kexec_load" => 246,
        "kexec_file_load" => 320,
        "init_module" => 175,
        "finit_module" => 313,
        "delete_module" => 176,
        "create_module" => 174,
        "modify_ldt" => 154,
        "bpf" => 321,
        "userfaultfd" => 323,
        _ => return None,
    })
}

fn aarch64_syscall(name: &str) -> Option<i64> {
    Some(match name {
        // Generic ABI (asm-generic/unistd.h) numbering used by aarch64.
        "ptrace" => 117,
        "perf_event_open" => 241,
        "process_vm_readv" => 270,
        "process_vm_writev" => 271,
        "kexec_load" => 104,
        "kexec_file_load" => 294,
        "init_module" => 105,
        "finit_module" => 273,
        "delete_module" => 106,
        // create_module and modify_ldt are not available on aarch64.
        "bpf" => 280,
        "userfaultfd" => 282,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denylist_contains_expected_kernel_primitives() {
        let denied = denied_syscalls();
        for must in ["ptrace", "bpf", "kexec_load", "init_module"] {
            assert!(
                denied.contains(&must),
                "denylist missing critical syscall {must}",
            );
        }
    }

    #[test]
    fn denylist_is_non_empty_and_unique() {
        let denied = denied_syscalls();
        assert!(!denied.is_empty());
        let mut sorted = denied.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), denied.len(), "duplicate in denylist");
    }

    #[test]
    fn x86_64_syscall_resolves_every_denied_name() {
        for name in denied_syscalls() {
            // Every name in denied_syscalls() must resolve on x86_64.
            assert!(
                x86_64_syscall(name).is_some(),
                "x86_64 mapping missing for {name}",
            );
        }
    }

    #[test]
    fn x86_64_syscall_returns_none_for_unknown_name() {
        assert_eq!(x86_64_syscall("definitely-not-a-syscall"), None);
        assert_eq!(x86_64_syscall(""), None);
    }

    #[test]
    fn aarch64_syscall_resolves_most_denied_names() {
        // aarch64 generic ABI doesn't have create_module or modify_ldt,
        // which is fine — install_denylist skips them. Every other
        // denied name must resolve.
        let absent_on_aarch64 = ["create_module", "modify_ldt"];
        for name in denied_syscalls() {
            if absent_on_aarch64.contains(name) {
                assert!(
                    aarch64_syscall(name).is_none(),
                    "{name} should not be mapped on aarch64",
                );
                continue;
            }
            assert!(
                aarch64_syscall(name).is_some(),
                "aarch64 mapping missing for {name}",
            );
        }
    }

    #[test]
    fn x86_64_syscall_numbers_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for name in denied_syscalls() {
            if let Some(num) = x86_64_syscall(name) {
                assert!(seen.insert(num), "duplicate x86_64 syscall number {num}");
            }
        }
    }

    #[test]
    fn lookup_syscall_dispatches_per_arch() {
        // ptrace: x86_64 = 101, aarch64 = 117
        assert_eq!(lookup_syscall("ptrace", TargetArch::x86_64), Some(101));
        assert_eq!(lookup_syscall("ptrace", TargetArch::aarch64), Some(117));
    }

    #[test]
    fn install_denylist_does_not_panic() {
        // We don't assert success — in containerised CI the kernel may
        // reject the filter — but install_denylist() must never panic
        // and must return a Result either way.
        let _ = install_denylist();
    }

    #[test]
    fn target_arch_returns_supported_arch_on_test_host() {
        // CI runs on x86_64 / aarch64. target_arch() returns Ok on both.
        let arch = target_arch();
        assert!(arch.is_ok(), "unexpected unsupported arch in test");
    }
}
