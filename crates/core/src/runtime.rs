//! Pluggable runtime abstraction over the host system.
//!
//! The install pipeline talks to the host through this trait. Production
//! code wires [`RealRuntime`]; tests wire a [`MockRuntime`] that records
//! every call and returns programmed responses. This is how
//! [`crate::install`]'s ~130 lines of subprocess orchestration become
//! reachable from unit tests on any platform.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// A recorded command invocation for test inspection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    /// The command name (e.g. `"parted"`).
    pub cmd: String,
    /// Argv passed to it.
    pub args: Vec<String>,
}

/// Outcome the [`MockRuntime`] will return for the next matching call.
#[derive(Clone, Debug)]
pub enum MockOutcome {
    /// Return success with optional captured stdout bytes.
    Ok(Vec<u8>),
    /// Return [`crate::CoreError::Io`] with this message.
    Err(String),
}

/// Trait the install pipeline calls into for every side-effecting
/// operation. Implemented by [`RealRuntime`] in production and by
/// [`MockRuntime`] under `#[cfg(test)]`.
pub trait Runtime {
    /// Run a command; fail on non-zero exit. No stdout capture.
    fn run(&self, cmd: &str, args: &[&str]) -> crate::Result<()>;
    /// Run a command and return captured stdout. Fail on non-zero
    /// exit.
    fn run_output(&self, cmd: &str, args: &[&str]) -> crate::Result<Vec<u8>>;
    /// `true` if the command can be located on `$PATH`.
    fn has_cmd(&self, cmd: &str) -> bool;
    /// Look up an environment variable.
    fn env_var(&self, key: &str) -> Option<String>;
    /// `true` if the path exists on the filesystem.
    fn path_exists(&self, path: &Path) -> bool;
    /// Create the directory and all parents. Returns
    /// [`crate::CoreError::Io`] on failure.
    fn create_dir_all(&self, path: &Path) -> crate::Result<()>;
    /// Mountpoint base — production returns `/mnt`; tests return a
    /// scratch directory so the install pipeline doesn't try to
    /// `mkdir /mnt/raidhos-esp` outside a container.
    fn mount_base(&self) -> PathBuf {
        PathBuf::from("/mnt")
    }
}

/// The shipped runtime. Talks to `std::process::Command`, `std::env`,
/// and `std::fs` directly.
#[derive(Default)]
pub struct RealRuntime;

impl Runtime for RealRuntime {
    fn run(&self, cmd: &str, args: &[&str]) -> crate::Result<()> {
        let status = std::process::Command::new(cmd)
            .args(args)
            .status()
            .map_err(|e| crate::CoreError::Io(e.to_string()))?;
        if !status.success() {
            return Err(crate::CoreError::Io(format!("command failed: {cmd}")));
        }
        Ok(())
    }

    fn run_output(&self, cmd: &str, args: &[&str]) -> crate::Result<Vec<u8>> {
        let out = std::process::Command::new(cmd)
            .args(args)
            .output()
            .map_err(|e| crate::CoreError::Io(format!("{cmd}: {e}")))?;
        if !out.status.success() {
            return Err(crate::CoreError::Io(format!(
                "{cmd} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        Ok(out.stdout)
    }

    fn has_cmd(&self, cmd: &str) -> bool {
        std::process::Command::new("sh")
            .args(["-c", &format!("command -v {cmd} >/dev/null 2>&1")])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn env_var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn create_dir_all(&self, path: &Path) -> crate::Result<()> {
        std::fs::create_dir_all(path).map_err(|e| crate::CoreError::Io(e.to_string()))
    }
}

/// In-memory runtime for tests. Records every call and returns
/// programmed responses.
#[derive(Default)]
pub struct MockRuntime {
    /// Commands that should appear "present" to `has_cmd`.
    pub available_cmds: Vec<&'static str>,
    /// Environment variables to expose.
    pub env: Vec<(String, String)>,
    /// Paths to claim exist.
    pub existing_paths: Vec<PathBuf>,
    /// Mount base (so the install pipeline mkdirs inside a scratch
    /// directory).
    pub mount_base: PathBuf,
    /// Stack of programmed outcomes for `run` / `run_output`. Popped
    /// front-to-back.
    pub outcomes: RefCell<Vec<MockOutcome>>,
    /// Every invocation recorded in order.
    pub invocations: RefCell<Vec<Invocation>>,
    /// `true` if every `create_dir_all` should succeed (records the
    /// path but does no I/O).
    pub fake_mkdir: bool,
}

impl MockRuntime {
    /// Construct a new mock with sensible defaults.
    pub fn new() -> Self {
        Self {
            mount_base: std::env::temp_dir().join(format!("raidhos-mock-{}", std::process::id())),
            fake_mkdir: true,
            ..Default::default()
        }
    }

    /// Push a programmed outcome onto the queue. Consumed in FIFO order.
    pub fn push_outcome(&self, outcome: MockOutcome) {
        self.outcomes.borrow_mut().push(outcome);
    }

    fn record(&self, cmd: &str, args: &[&str]) {
        self.invocations.borrow_mut().push(Invocation {
            cmd: cmd.to_string(),
            args: args.iter().map(|s| (*s).to_string()).collect(),
        });
    }

    fn pop_outcome(&self) -> MockOutcome {
        let mut q = self.outcomes.borrow_mut();
        if q.is_empty() {
            MockOutcome::Ok(Vec::new())
        } else {
            q.remove(0)
        }
    }
}

impl Runtime for MockRuntime {
    fn run(&self, cmd: &str, args: &[&str]) -> crate::Result<()> {
        self.record(cmd, args);
        match self.pop_outcome() {
            MockOutcome::Ok(_) => Ok(()),
            MockOutcome::Err(msg) => Err(crate::CoreError::Io(msg)),
        }
    }

    fn run_output(&self, cmd: &str, args: &[&str]) -> crate::Result<Vec<u8>> {
        self.record(cmd, args);
        match self.pop_outcome() {
            MockOutcome::Ok(bytes) => Ok(bytes),
            MockOutcome::Err(msg) => Err(crate::CoreError::Io(msg)),
        }
    }

    fn has_cmd(&self, cmd: &str) -> bool {
        self.available_cmds.contains(&cmd)
    }

    fn env_var(&self, key: &str) -> Option<String> {
        self.env
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    fn path_exists(&self, path: &Path) -> bool {
        self.existing_paths.iter().any(|p| p == path)
    }

    fn create_dir_all(&self, path: &Path) -> crate::Result<()> {
        if self.fake_mkdir {
            self.invocations.borrow_mut().push(Invocation {
                cmd: "mkdir".to_string(),
                args: vec![path.display().to_string()],
            });
            Ok(())
        } else {
            std::fs::create_dir_all(path).map_err(|e| crate::CoreError::Io(e.to_string()))
        }
    }

    fn mount_base(&self) -> PathBuf {
        self.mount_base.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_records_and_returns_programmed_outcomes() {
        let mock = MockRuntime::new();
        mock.push_outcome(MockOutcome::Ok(b"hello".to_vec()));
        mock.push_outcome(MockOutcome::Err("boom".into()));

        let out = mock.run_output("foo", &["bar"]).unwrap();
        assert_eq!(out, b"hello");
        let err = mock.run("baz", &["qux"]).unwrap_err();
        assert!(err.to_string().contains("boom"));

        let inv = mock.invocations.borrow();
        assert_eq!(inv.len(), 2);
        assert_eq!(inv[0].cmd, "foo");
        assert_eq!(inv[0].args, vec!["bar"]);
        assert_eq!(inv[1].cmd, "baz");
    }

    #[test]
    fn mock_has_cmd_uses_available_list() {
        let mut mock = MockRuntime::new();
        mock.available_cmds.push("parted");
        assert!(mock.has_cmd("parted"));
        assert!(!mock.has_cmd("mkfs.exfat"));
    }

    #[test]
    fn mock_env_var_returns_programmed() {
        let mut mock = MockRuntime::new();
        mock.env
            .push(("RAIDHOS_PAYLOAD_DIR".into(), "/tmp/p".into()));
        assert_eq!(
            mock.env_var("RAIDHOS_PAYLOAD_DIR").as_deref(),
            Some("/tmp/p")
        );
        assert!(mock.env_var("NOT_SET").is_none());
    }

    #[test]
    fn mock_path_exists() {
        let mut mock = MockRuntime::new();
        mock.existing_paths.push(PathBuf::from("/some/path"));
        assert!(mock.path_exists(Path::new("/some/path")));
        assert!(!mock.path_exists(Path::new("/other")));
    }

    #[test]
    fn mock_run_empty_queue_returns_ok() {
        let mock = MockRuntime::new();
        // No programmed outcomes — defaults to Ok with empty stdout.
        assert!(mock.run("anything", &[]).is_ok());
        assert_eq!(mock.run_output("anything", &[]).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn mock_fake_mkdir_records_without_io() {
        let mock = MockRuntime::new();
        let result = mock.create_dir_all(Path::new("/should/not/exist/anywhere"));
        assert!(result.is_ok());
        let inv = mock.invocations.borrow();
        assert_eq!(inv[0].cmd, "mkdir");
    }

    #[test]
    fn real_runtime_path_exists_matches_std() {
        let rt = RealRuntime;
        assert!(rt.path_exists(Path::new("/")));
        assert!(!rt.path_exists(Path::new("/this/path/should/not/exist/raidhos")));
    }

    #[test]
    fn real_runtime_env_var_matches_std() {
        std::env::set_var("RAIDHOS_TEST_VAR_RT", "value");
        let rt = RealRuntime;
        assert_eq!(rt.env_var("RAIDHOS_TEST_VAR_RT").as_deref(), Some("value"));
        std::env::remove_var("RAIDHOS_TEST_VAR_RT");
    }

    #[test]
    fn real_runtime_has_cmd_finds_sh() {
        let rt = RealRuntime;
        // `sh` is almost certainly on PATH on any Unix-like host.
        // On Windows the helper isn't compiled anyway.
        #[cfg(unix)]
        assert!(rt.has_cmd("sh"));
        assert!(!rt.has_cmd("absolutely-not-a-real-command-xyz"));
    }

    #[test]
    fn real_runtime_run_succeeds_on_true() {
        let rt = RealRuntime;
        #[cfg(unix)]
        assert!(rt.run("true", &[]).is_ok());
        #[cfg(unix)]
        assert!(rt.run("false", &[]).is_err());
    }

    #[test]
    fn invocation_record_round_trips() {
        let inv = Invocation {
            cmd: "parted".to_string(),
            args: vec!["/dev/sdb".into(), "-s".into(), "mklabel".into()],
        };
        // PartialEq + Eq + Clone hold.
        assert_eq!(inv.clone(), inv);
    }

    #[test]
    fn real_runtime_run_errors_on_missing_command() {
        let rt = RealRuntime;
        let err = rt
            .run("absolutely-not-a-real-command-raidhos-xyz", &[])
            .unwrap_err();
        // Different OSes phrase "no such file" differently; we just
        // confirm it's an Io-flavoured error.
        assert!(matches!(err, crate::CoreError::Io(_)));
    }

    #[test]
    fn real_runtime_run_output_errors_on_missing_command() {
        let rt = RealRuntime;
        let err = rt
            .run_output("absolutely-not-a-real-command-raidhos-xyz", &[])
            .unwrap_err();
        // run_output prefixes errors with the command name —
        // see RealRuntime::run_output.
        let s = format!("{err}");
        assert!(
            s.contains("absolutely-not-a-real-command-raidhos-xyz"),
            "got: {s}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn real_runtime_run_output_returns_stdout() {
        let rt = RealRuntime;
        let out = rt.run_output("echo", &["hello-raidhos"]).unwrap();
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("hello-raidhos"));
    }

    #[test]
    fn real_runtime_create_dir_all_succeeds() {
        let rt = RealRuntime;
        let path = std::env::temp_dir().join(format!(
            "raidhos-rt-mkdir-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(rt.create_dir_all(&path).is_ok());
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn real_runtime_create_dir_all_errors_on_unwritable_parent() {
        let rt = RealRuntime;
        // /nonexistent-root/foo can't be created because /nonexistent-root
        // doesn't exist and we can't mkdir under /.
        // On most systems this fails with EACCES or ENOENT.
        let res = rt.create_dir_all(Path::new("/proc/cannot-create-here/x"));
        assert!(res.is_err());
    }

    #[test]
    fn real_runtime_mount_base_default() {
        let rt = RealRuntime;
        assert_eq!(rt.mount_base(), PathBuf::from("/mnt"));
    }

    #[test]
    fn mock_runtime_create_dir_all_real_io_branch() {
        let mut rt = MockRuntime::new();
        rt.fake_mkdir = false;
        let path = std::env::temp_dir().join(format!(
            "raidhos-mock-real-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(rt.create_dir_all(&path).is_ok());
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn mock_runtime_create_dir_all_real_io_branch_errors() {
        let mut rt = MockRuntime::new();
        rt.fake_mkdir = false;
        let res = rt.create_dir_all(Path::new("/proc/cannot-create-here/x"));
        assert!(res.is_err());
    }

    #[test]
    fn mock_outcome_clone_debug() {
        let a = MockOutcome::Ok(vec![1, 2, 3]);
        let b = a.clone();
        let s = format!("{a:?}");
        assert!(s.contains("Ok"));
        let _ = b;
        let c = MockOutcome::Err("x".into());
        assert!(format!("{c:?}").contains("Err"));
    }
}
