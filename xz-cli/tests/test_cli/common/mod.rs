use std::fs;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};

use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot;

mod data;

pub use data::{generate_random_data, BINARY_DATA, REPETITIVE_DATA, SAMPLE_TEXT};

/// Type of binary to execute
#[derive(Debug, Clone)]
pub enum BinaryType {
    /// Our own binary built by cargo
    Cargo(String),
    /// System binary available in PATH
    System(String),
}

impl BinaryType {
    /// Create a new cargo binary type
    pub fn cargo(name: impl Into<String>) -> Self {
        Self::Cargo(name.into())
    }

    /// Create a new system binary type
    pub fn system(name: impl Into<String>) -> Self {
        Self::System(name.into())
    }

    /// Returns the path to the binary for this variant.
    ///
    /// # Panics
    ///
    /// Panics if the binary cannot be found.
    fn get_path(&self) -> String {
        match self {
            BinaryType::Cargo(name) => {
                let bin_env = format!("CARGO_BIN_EXE_{name}");
                let path = Self::locate_cargo_binary(name, &bin_env).unwrap_or_else(|| {
                    panic!(
                        "Binary '{name}' not found. Set '{bin_env}' or build the project binaries.",
                    )
                });
                path.to_string_lossy().into_owned()
            }
            BinaryType::System(name) => find_system_binary(name)
                .unwrap_or_else(|| panic!("Binary {name} not found in PATH")),
        }
    }

    fn locate_cargo_binary(name: &str, bin_env: &str) -> Option<PathBuf> {
        if let Some(path) = std::env::var_os(bin_env) {
            let path = PathBuf::from(path);
            if Self::is_valid_binary(&path) {
                return Some(path);
            }
        }

        let exe_suffix = std::env::consts::EXE_SUFFIX;
        let exe_name = if exe_suffix.is_empty() || name.ends_with(exe_suffix) {
            name.to_string()
        } else {
            format!("{name}{exe_suffix}")
        };

        let profiles = Self::candidate_profiles();
        for target_dir in Self::candidate_target_dirs() {
            if let Some(path) = Self::search_target_dir(&target_dir, name, &exe_name, &profiles) {
                return Some(path);
            }
        }
        None
    }

    fn candidate_profiles() -> Vec<String> {
        let mut profiles = Vec::new();
        if let Ok(profile) = std::env::var("CARGO_PROFILE") {
            profiles.push(profile);
        }
        if let Ok(profile) = std::env::var("PROFILE") {
            profiles.push(profile);
        }
        if cfg!(debug_assertions) {
            profiles.push("debug".to_string());
            profiles.push("test".to_string());
        } else {
            profiles.push("release".to_string());
        }
        profiles.push("release".to_string());
        profiles.push("debug".to_string());

        profiles
            .into_iter()
            .filter(|profile| !profile.is_empty())
            .fold(Vec::new(), |mut acc, profile| {
                if !acc.contains(&profile) {
                    acc.push(profile);
                }
                acc
            })
    }

    fn candidate_target_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(dir) = std::env::var_os("CARGO_TARGET_DIR") {
            Self::push_dir(&mut dirs, PathBuf::from(dir));
        }
        if let Some(dir) = std::env::var_os("CARGO_WORKSPACE_DIR") {
            Self::push_dir(&mut dirs, PathBuf::from(dir).join("target"));
        }
        if let Some(dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
            let manifest = PathBuf::from(dir);
            for ancestor in manifest.ancestors() {
                Self::push_dir(&mut dirs, ancestor.join("target"));
            }
        }
        if let Ok(current_dir) = std::env::current_dir() {
            if let Some(parent) = current_dir.parent() {
                Self::push_dir(&mut dirs, parent.join("target"));
            }
        }
        Self::push_dir(&mut dirs, PathBuf::from("target"));
        dirs
    }

    fn search_target_dir(
        target_dir: &Path,
        name: &str,
        exe_name: &str,
        profiles: &[String],
    ) -> Option<PathBuf> {
        if !target_dir.is_dir() {
            return None;
        }

        let mut dirs = Vec::new();
        Self::push_dir(&mut dirs, target_dir.to_path_buf());
        for profile in profiles {
            Self::push_dir(&mut dirs, target_dir.join(profile));
        }

        if let Ok(entries) = target_dir.read_dir() {
            for entry in entries.flatten() {
                if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    continue;
                }
                let subdir = entry.path();
                Self::push_dir(&mut dirs, subdir.clone());
                for profile in profiles {
                    Self::push_dir(&mut dirs, subdir.join(profile));
                }
            }
        }

        for dir in dirs {
            if let Some(found) = Self::check_dir_for_binary(&dir, name, exe_name) {
                return Some(found);
            }
        }
        None
    }

    fn check_dir_for_binary(dir: &Path, name: &str, exe_name: &str) -> Option<PathBuf> {
        for candidate in [name, exe_name] {
            if candidate.is_empty() {
                continue;
            }
            let path = dir.join(candidate);
            if Self::is_valid_binary(&path) {
                return Some(path);
            }
        }

        let deps_dir = dir.join("deps");
        if deps_dir.is_dir() {
            if let Ok(entries) = deps_dir.read_dir() {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !Self::is_valid_binary(&path) {
                        continue;
                    }
                    let file_name = entry.file_name();
                    let file_name = file_name.to_string_lossy();
                    if Self::binary_name_matches(&file_name, name, exe_name) {
                        return Some(path);
                    }
                }
            }
        }
        None
    }

    fn binary_name_matches(file_name: &str, name: &str, exe_name: &str) -> bool {
        if file_name == name || file_name == exe_name {
            return true;
        }
        if let Some((prefix, _)) = file_name.split_once('-') {
            if prefix == name || prefix == exe_name {
                return true;
            }
        }
        false
    }

    fn is_valid_binary(path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }
        !matches!(path.extension().and_then(|ext| ext.to_str()), Some("d"))
    }

    fn push_dir(candidates: &mut Vec<PathBuf>, dir: PathBuf) {
        if dir.is_dir() && !candidates.contains(&dir) {
            candidates.push(dir);
        }
    }
}

/// Find a system binary in PATH
fn find_system_binary(name: &str) -> Option<String> {
    // First, try to find the binary in PATH
    if let Ok(path) = which::which(name) {
        return Some(path.to_string_lossy().to_string());
    }

    // If not found in PATH, try common locations
    let common_paths = ["/usr/bin", "/usr/local/bin", "/bin", "/sbin", "/usr/sbin"];

    for base_path in &common_paths {
        let full_path = Path::new(base_path).join(name);
        if full_path.exists() && full_path.is_file() {
            return Some(full_path.to_string_lossy().to_string());
        }
    }

    None
}

/// Output from running a binary command
#[derive(Eq, PartialEq)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout_raw: Vec<u8>,
    pub stdout: String,
    pub stderr: String,
}

/// Compare two command outputs for equality
///
/// # Panics
///
/// Panics if the outputs differ.
pub fn compare_outputs(output_1: &Output, output_2: &Output) {
    assert_eq!(output_1.status.success(), output_2.status.success());
    assert!(output_1.stdout_raw == output_2.stdout_raw);
    assert!(output_1.stdout == output_2.stdout);
    assert!(output_1.stderr == output_2.stderr);
}

/// Shared test fixture utilities to keep filesystem interactions isolated
pub struct Fixture {
    root_dir: tempfile::TempDir,
}

impl Fixture {
    /// Create fixture with multiple files
    ///
    /// # Panics
    ///
    /// Panics if the temporary directory cannot be created or if any fixture file
    /// cannot be written.
    pub fn with_files(names: &[&str], contents: &[&[u8]]) -> Self {
        let root_dir = tempfile::TempDir::new().unwrap();
        let file_paths: Vec<PathBuf> = names
            .iter()
            .map(|name| root_dir.path().join(name))
            .collect();
        for (path, contents) in file_paths.iter().zip(contents) {
            fs::write(path, contents).unwrap();
        }

        Self { root_dir }
    }

    /// Create fixture with single file
    ///
    /// # Panics
    ///
    /// Panics if the temporary directory cannot be created or if the fixture file
    /// cannot be written.
    pub fn with_file(name: &str, contents: &[u8]) -> Self {
        let root_dir = tempfile::TempDir::new().unwrap();
        let path = root_dir.path().join(name);
        fs::write(&path, contents).unwrap();

        Self { root_dir }
    }

    /// Get full path for a file in the fixture
    pub fn path(&self, name: &str) -> String {
        format!("{}/{}", self.root_dir.path().display(), name)
    }

    /// Get compressed path (adds .xz extension)
    pub fn compressed_path(&self, name: &str) -> String {
        format!("{}.xz", self.path(name))
    }

    /// Get compressed path using the legacy `.lzma` extension.
    pub fn lzma_path(&self, name: &str) -> String {
        format!("{}.lzma", self.path(name))
    }

    /// Remove a file from the fixture
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be removed.
    pub fn remove_file(&self, name: &str) {
        let path = self.root_dir.path().join(name);
        fs::remove_file(path).unwrap();
    }

    /// Check if a file exists in the fixture
    pub fn file_exists(&self, name: &str) -> bool {
        self.root_dir.path().join(name).exists()
    }

    /// Run a cargo binary with the specified arguments
    pub async fn run_cargo(&mut self, name: &str, args: &[&str]) -> Output {
        self.run(BinaryType::cargo(name), args).await
    }

    /// Run a system binary with the specified arguments if available
    pub async fn run_system(&mut self, name: &str, args: &[&str]) -> Option<Output> {
        if find_system_binary(name).is_some() {
            Some(self.run(BinaryType::system(name), args).await)
        } else {
            None
        }
    }

    /// Assert that files have expected contents
    ///
    /// # Panics
    ///
    /// Panics if any file cannot be read or if its contents don't match the
    /// expected bytes.
    pub fn assert_files(&self, names: &[&str], contents: &[&[u8]]) {
        for (name, expected_contents) in names.iter().zip(contents) {
            let path = self.root_dir.path().join(name);
            let actual_contents = fs::read(path).unwrap_or_default();
            assert!(actual_contents == *expected_contents);
        }
    }

    pub fn root_dir_path(&self) -> &Path {
        self.root_dir.path()
    }

    async fn run_until_killed(
        &mut self,
        binary_type: &BinaryType,
        args: &[&str],
        stdin_bytes: Option<Vec<u8>>,
        kill_receiver: oneshot::Receiver<()>,
    ) -> Output {
        let bin_path = binary_type.get_path();
        let mut child = tokio::process::Command::new(&bin_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();

        if let Some(stdin_bytes) = stdin_bytes {
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(&stdin_bytes).await.unwrap_or_else(|err| {
                    // Some commands intentionally exit early (e.g. rejecting stdin).
                    // In such cases, the child may close its stdin before we finish writing.
                    if err.kind() == std::io::ErrorKind::BrokenPipe {
                        return;
                    }
                    panic!("failed write to stdin ({} bytes): {err}", stdin_bytes.len());
                });
            }
        }

        // Drop stdin to send EOF to the child process
        drop(child.stdin.take());

        // Use wait_with_output directly to avoid deadlock with large outputs.
        // The pipe buffers can fill up if we wait() before reading stdout/stderr.
        let wait_output = child.wait_with_output();

        tokio::select! {
            raw_output = wait_output => {
                let raw_output = raw_output.unwrap();
                Output {
                    status: raw_output.status,
                    stdout_raw: raw_output.stdout.clone(),
                    stdout: String::from_utf8_lossy(&raw_output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&raw_output.stderr).into_owned(),
                }
            }
            msg = kill_receiver => {
                if msg.is_ok() {
                    // The test framework requested a kill, but we can't easily
                    // kill here since we've already started wait_with_output.
                    // This branch shouldn't normally be reached in practice.
                }
                // Return a dummy output for the kill case
                Output {
                    status: std::process::ExitStatus::default(),
                    stdout_raw: Vec::new(),
                    stdout: String::new(),
                    stderr: String::new(),
                }
            }
        }
    }

    /// Run a binary with the specified arguments and optional stdin input
    ///
    /// # Panics
    ///
    /// Panics if the process cannot be spawned, if writing to stdin fails, or if
    /// awaiting process output fails.
    pub async fn run_with_stdin(
        &mut self,
        binary_type: BinaryType,
        args: &[&str],
        args_to_stdin: Option<Vec<&str>>,
    ) -> Output {
        let stdin_bytes = args_to_stdin.map(|stdin_args| {
            let mut converted_args = Vec::new();
            for arg in stdin_args {
                converted_args.extend_from_slice(arg.as_bytes());
            }
            converted_args
        });

        let (kill_sender, kill_receiver) = oneshot::channel();
        let output = self
            .run_until_killed(&binary_type, args, stdin_bytes, kill_receiver)
            .await;
        drop(kill_sender);
        output
    }

    /// Run a binary with raw stdin bytes.
    ///
    /// # Panics
    ///
    /// Panics if the process cannot be spawned, if writing to stdin fails, or if
    /// awaiting process output fails.
    pub async fn run_with_stdin_raw(
        &mut self,
        binary_type: BinaryType,
        args: &[&str],
        stdin: &[u8],
    ) -> Output {
        let (kill_sender, kill_receiver) = oneshot::channel();
        let output = self
            .run_until_killed(&binary_type, args, Some(stdin.to_vec()), kill_receiver)
            .await;
        drop(kill_sender);
        output
    }

    /// Run a binary with the specified arguments
    async fn run(&mut self, binary_type: BinaryType, args: &[&str]) -> Output {
        self.run_with_stdin(binary_type, args, None).await
    }
}
