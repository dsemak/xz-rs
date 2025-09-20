//! Build script for liblzma-sys
//!
//! This build script handles both system and vendored builds of liblzma,
//! with security checks for known vulnerabilities and support for patches.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents the stability level of a liblzma version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stability {
    Alpha,
    Beta,
    Stable,
}

impl Stability {
    /// Parse a stability token from the version header
    fn from_token(token: &str) -> Result<Self, String> {
        match token {
            "LZMA_VERSION_STABILITY_ALPHA" => Ok(Stability::Alpha),
            "LZMA_VERSION_STABILITY_BETA" => Ok(Stability::Beta),
            "LZMA_VERSION_STABILITY_STABLE" => Ok(Stability::Stable),
            other => Err(format!("unrecognized stability token: {other}")),
        }
    }

    /// Get the string suffix for this stability level
    fn suffix(self) -> &'static str {
        match self {
            Stability::Alpha => "alpha",
            Stability::Beta => "beta",
            Stability::Stable => "",
        }
    }

    /// Get a numeric ordinal for version comparison
    fn ordinal(self) -> u32 {
        match self {
            Stability::Alpha => 0,
            Stability::Beta => 1,
            Stability::Stable => 2,
        }
    }
}

/// Represents a semantic version with stability information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Version {
    major: u32,
    minor: u32,
    patch: u32,
    stability: Stability,
}

impl Version {
    /// Create a new version
    const fn new(major: u32, minor: u32, patch: u32, stability: Stability) -> Self {
        Version {
            major,
            minor,
            patch,
            stability,
        }
    }

    /// Parse a version string (e.g., "5.4.1", "5.6.0alpha", "5.4.1-123-g456")
    fn parse(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("empty version string".to_string());
        }

        // Split off any git suffix (e.g., "5.4.1-123-g456" -> "5.4.1")
        let core = trimmed.split('-').next().unwrap_or(trimmed);

        // Extract stability suffix
        let (body, stability) = if let Some(body) = core.strip_suffix("alpha") {
            (body, Stability::Alpha)
        } else if let Some(body) = core.strip_suffix("beta") {
            (body, Stability::Beta)
        } else {
            (core, Stability::Stable)
        };

        // Find the numeric portion (major.minor.patch)
        let numeric_len = body
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(body.len());
        let numeric = &body[..numeric_len];

        // Parse version components
        let mut parts = numeric.split('.');
        let major = parts
            .next()
            .ok_or_else(|| format!("invalid liblzma version: {raw}"))?
            .parse()
            .map_err(|_| format!("invalid major component in version: {raw}"))?;
        let minor = parts
            .next()
            .ok_or_else(|| format!("invalid liblzma version: {raw}"))?
            .parse()
            .map_err(|_| format!("invalid minor component in version: {raw}"))?;
        let patch = parts
            .next()
            .ok_or_else(|| format!("invalid liblzma version: {raw}"))?
            .parse()
            .map_err(|_| format!("invalid patch component in version: {raw}"))?;

        if parts.next().is_some() {
            return Err(format!("unexpected extra data in version: {raw}"));
        }

        Ok(Version::new(major, minor, patch, stability))
    }

    /// Convert to a numeric ordinal for comparison
    /// Format: MMMMMMMMNNNNPPPS where M=major, N=minor, P=patch, S=stability
    fn ordinal(self) -> u32 {
        self.major * 10_000_000 + self.minor * 10_000 + self.patch * 10 + self.stability.ordinal()
    }

    /// Format version as a display string
    fn display(self) -> String {
        let mut base = format!("{}.{}.{}", self.major, self.minor, self.patch);
        let suffix = self.stability.suffix();
        if !suffix.is_empty() {
            base.push_str(suffix);
        }
        base
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ordinal().cmp(&other.ordinal())
    }
}

/// Complete version information including git metadata
struct VersionInfo {
    version: Version,
    commit_suffix: Option<String>,
    git_commit: Option<String>,
}

/// Information about a system-installed liblzma
struct SystemLibrary {
    include_paths: Vec<String>,
}

/// Information about a vendored liblzma build
struct VendoredBuild {
    include_paths: Vec<String>,
}

/// Information about a known vulnerable commit
struct VulnerableCommit {
    prefix: &'static str,
    cve: &'static str,
    note: &'static str,
}

/// Known vulnerable git commits that should be avoided
const VULNERABLE_COMMITS: &[VulnerableCommit] = &[
    VulnerableCommit {
        prefix: "2d7d862e3ffa",
        cve: "CVE-2024-3094",
        note: "xz 5.6.0 release tarball shipped with a backdoor; avoid building from this commit",
    },
    VulnerableCommit {
        prefix: "1b7a78738112",
        cve: "CVE-2024-3094",
        note: "xz 5.6.1 release tarball shipped with a backdoor; avoid building from this commit",
    },
];

fn main() {
    // Set up cargo rebuild triggers
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=lzma.h");
    println!("cargo:rerun-if-changed=xz/src/liblzma/api/lzma/version.h");
    println!("cargo:rerun-if-env-changed=LIBLZMA_SYS_ALLOW_UNSAFE");
    println!("cargo:rerun-if-env-changed=LIBLZMA_SYS_FORCE_LOCAL");

    if let Err(err) = run() {
        panic!("{err}");
    }
}

/// Main build logic
fn run() -> Result<(), String> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set by Cargo"));
    let allow_unsafe = env::var_os("LIBLZMA_SYS_ALLOW_UNSAFE").is_some();
    let force_local = env::var_os("LIBLZMA_SYS_FORCE_LOCAL").is_some();

    let patches = PatchSet::discover(Path::new("patches"))?;

    let mut include_paths = Vec::new();
    let mut use_system_headers = false;

    // Try system liblzma first, unless patches are present or forced local build
    if patches.is_empty() && !force_local {
        match try_system_liblzma(allow_unsafe)? {
            Some(system) => {
                include_paths = system.include_paths;
                use_system_headers = true;
            }
            None => {
                println!("cargo:warning=pkg-config did not yield a safe liblzma; trying vendored sources");
            }
        }
    } else if !patches.is_empty() {
        println!("cargo:warning=patches detected; forcing vendored liblzma build");
    }

    // Fall back to vendored build if system library wasn't suitable
    if include_paths.is_empty() {
        let vendored = build_vendored_liblzma(&out_dir, &patches, allow_unsafe)?;
        include_paths = vendored.include_paths;
        use_system_headers = false;
    }

    // Generate bindings if the feature is enabled
    #[cfg(feature = "bindgen")]
    generate_bindings(&out_dir, &include_paths, use_system_headers);

    Ok(())
}

/// Attempt to use system-installed liblzma via pkg-config
#[cfg(feature = "pkg-config")]
fn try_system_liblzma(allow_unsafe: bool) -> Result<Option<SystemLibrary>, String> {
    let library = match pkg_config::Config::new().probe("liblzma") {
        Ok(info) => info,
        Err(err) => {
            println!("cargo:warning=pkg-config probe for liblzma failed: {err}");
            return Ok(None);
        }
    };

    // Verify the system library version is safe
    match library.version.as_str() {
        version_str if !version_str.is_empty() => {
            let version = Version::parse(version_str)?;
            if let Err(err) = ensure_version_safe(&version, allow_unsafe, "system liblzma") {
                println!("cargo:warning={err}; falling back to vendored liblzma");
                return Ok(None);
            }
        }
        _ => {
            println!("cargo:warning=pkg-config returned liblzma without version information; skipping CVE check");
        }
    }

    // Export include paths for cargo
    for path in &library.include_paths {
        println!("cargo:include={}", path.display());
    }

    let include_paths = library
        .include_paths
        .iter()
        .map(|p| p.display().to_string())
        .collect();

    Ok(Some(SystemLibrary { include_paths }))
}

/// Stub implementation when pkg-config feature is disabled
#[cfg(not(feature = "pkg-config"))]
fn try_system_liblzma(_allow_unsafe: bool) -> Result<Option<SystemLibrary>, String> {
    println!("cargo:warning=pkg-config feature disabled; skipping system liblzma detection");
    Ok(None)
}

/// Build liblzma from vendored sources
fn build_vendored_liblzma(
    out_dir: &Path,
    patches: &PatchSet,
    allow_unsafe: bool,
) -> Result<VendoredBuild, String> {
    // Apply patches if any are present
    if !patches.is_empty() {
        patches.apply()?;
    }

    // Read and validate version information
    let version_info = read_vendored_version()?;
    ensure_version_safe(&version_info.version, allow_unsafe, "vendored liblzma")?;
    ensure_commit_safe(version_info.git_commit.as_deref(), allow_unsafe)?;

    // Configure linking
    println!("cargo:rustc-link-lib=static=lzma");
    println!("cargo:rustc-link-search=native={}", out_dir.display());

    let sizeof_size_t = env::var("CARGO_CFG_TARGET_POINTER_WIDTH")
        .ok()
        .and_then(|width| match width.as_str() {
            "16" => Some("2".to_string()),
            "32" => Some("4".to_string()),
            "64" => Some("8".to_string()),
            "128" => Some("16".to_string()),
            _ => None,
        })
        .unwrap_or_else(|| std::mem::size_of::<usize>().to_string());

    let package_version = format!("\"{}\"", version_display(&version_info));
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();

    let mut build = cc::Build::new();

    for source in collect_liblzma_sources(Path::new("xz/src/liblzma"))? {
        build.file(source);
    }

    if target_family == "unix" {
        build.define("MYTHREAD_POSIX", "1");
        build.flag_if_supported("-pthread");
        println!("cargo:rustc-link-lib=pthread");
    }

    build
        .file("xz/src/common/tuklib_physmem.c")
        .file("xz/src/common/tuklib_cpucores.c")
        // Include directories for compilation
        .include("xz/src/common")
        .include("xz/src/liblzma/api")
        .include("xz/src/liblzma/common")
        .include("xz/src/liblzma/lz")
        .include("xz/src/liblzma/lzma")
        .include("xz/src/liblzma/rangecoder")
        .include("xz/src/liblzma/check")
        .include("xz/src/liblzma/simple")
        .include("xz/src/liblzma/delta")
        // Package information
        .define("PACKAGE_NAME", "\"XZ Utils\"")
        .define("PACKAGE_VERSION", package_version.as_str())
        .define("LZMA_API_STATIC", "1")
        .define("SIZEOF_SIZE_T", sizeof_size_t.as_str())
        .define("HAVE_STDBOOL_H", "1")
        .define("HAVE__BOOL", "1")
        .define("HAVE_STDINT_H", "1")
        .define("_GNU_SOURCE", "1")
        .flag_if_supported("-std=c99")
        .warnings(false);

    match build.try_compile("lzma") {
        Ok(()) => {}
        Err(err) => return Err(format!("failed to build vendored liblzma: {err}")),
    }

    Ok(VendoredBuild {
        include_paths: vec!["xz/src/liblzma/api".to_string()],
    })
}

/// Format version information for display, including commit suffix if present
fn version_display(info: &VersionInfo) -> String {
    if let Some(suffix) = &info.commit_suffix {
        if !suffix.is_empty() {
            return format!("{}{}", info.version.display(), suffix);
        }
    }

    info.version.display()
}

/// Read version information from the vendored liblzma headers
fn read_vendored_version() -> Result<VersionInfo, String> {
    let header = fs::read_to_string("xz/src/liblzma/api/lzma/version.h")
        .map_err(|err| format!("unable to read liblzma version header: {err}"))?;

    let version = Version::new(
        parse_define_u32(&header, "LZMA_VERSION_MAJOR")?,
        parse_define_u32(&header, "LZMA_VERSION_MINOR")?,
        parse_define_u32(&header, "LZMA_VERSION_PATCH")?,
        Stability::from_token(&parse_define_token(&header, "LZMA_VERSION_STABILITY")?)?,
    );

    let commit_suffix = parse_define_string(&header, "LZMA_VERSION_COMMIT");
    let git_commit = read_git_commit("xz");

    Ok(VersionInfo {
        version,
        commit_suffix,
        git_commit,
    })
}

/// Parse a numeric #define from C header content
fn parse_define_u32(content: &str, name: &str) -> Result<u32, String> {
    let needle = format!("#define {name} ");

    content
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if let Some(value_part) = trimmed.strip_prefix(&needle) {
                value_part.split_whitespace().next()
            } else {
                None
            }
        })
        .ok_or_else(|| format!("missing definition for {name}"))?
        .parse()
        .map_err(|_| format!("failed to parse numeric value for {name}"))
}

/// Parse a token #define from C header content
fn parse_define_token(content: &str, name: &str) -> Result<String, String> {
    let needle = format!("#define {name} ");

    content
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix(&needle)
                .and_then(|remainder| remainder.split_whitespace().next())
                .map(ToString::to_string)
        })
        .ok_or_else(|| format!("missing token for {name}"))
}

/// Parse a string #define from C header content (returns None if not found)
fn parse_define_string(content: &str, name: &str) -> Option<String> {
    let needle = format!("#define {name} ");

    content.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix(&needle)
            .and_then(|remainder| remainder.split_whitespace().next())
            .map(|token| token.trim_matches('"').to_string())
    })
}

/// Read the current git commit hash from a directory
fn read_git_commit(dir: &str) -> Option<String> {
    let status = Command::new("git")
        .args(["-C", dir, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !status.status.success() {
        return None;
    }
    let hash = String::from_utf8_lossy(&status.stdout).trim().to_string();
    if hash.is_empty() {
        None
    } else {
        Some(hash)
    }
}

/// Check if a version is known to be vulnerable and handle accordingly
fn ensure_version_safe(version: &Version, allow_unsafe: bool, origin: &str) -> Result<(), String> {
    let report = |cve: &str, note: &str| {
        let message = format!(
            "{origin} resolved to liblzma {} which is vulnerable to {cve}: {note}",
            version.display()
        );
        if allow_unsafe {
            println!("cargo:warning={message} (allowed by LIBLZMA_SYS_ALLOW_UNSAFE)");
            Ok(())
        } else {
            Err(message)
        }
    };

    // Check for CVE-2025-31115 (multi-threaded decoder use-after-free)
    let cve_2025_start = Version::new(5, 3, 3, Stability::Alpha);
    let cve_2025_fixed = Version::new(5, 8, 1, Stability::Stable);
    if version >= &cve_2025_start && version < &cve_2025_fixed {
        return report(
            "CVE-2025-31115",
            "multi-threaded decoder use-after-free; upgrade to >= 5.8.1 or apply upstream patch",
        );
    }

    // Check for CVE-2024-3094 (backdoored releases)
    let bad_2024_a = Version::new(5, 6, 0, Stability::Stable);
    let bad_2024_b = Version::new(5, 6, 1, Stability::Stable);
    if version == &bad_2024_a || version == &bad_2024_b {
        return report(
            "CVE-2024-3094",
            "5.6.0/5.6.1 release artifacts were backdoored; do not use these builds",
        );
    }

    Ok(())
}

/// Check if a git commit is known to be vulnerable
fn ensure_commit_safe(commit: Option<&str>, allow_unsafe: bool) -> Result<(), String> {
    let Some(hash) = commit else {
        println!(
            "cargo:warning=unable to detect xz git commit; ensure vendored sources are trusted"
        );

        return Ok(());
    };

    for candidate in VULNERABLE_COMMITS {
        if hash.starts_with(candidate.prefix) {
            let message = format!(
                "vendored xz commit {hash} is blacklisted ({}) - {}",
                candidate.cve, candidate.note
            );

            if allow_unsafe {
                println!("cargo:warning={message} (allowed by LIBLZMA_SYS_ALLOW_UNSAFE)");
                return Ok(());
            }

            return Err(message);
        }
    }

    Ok(())
}

/// Manages a set of patch files to apply to vendored sources
struct PatchSet {
    files: Vec<PathBuf>,
}

impl PatchSet {
    /// Discover patch files in the given directory
    fn discover(dir: &Path) -> Result<Self, String> {
        if !dir.exists() {
            return Ok(PatchSet { files: Vec::new() });
        }

        let mut files = Vec::new();
        let entries = fs::read_dir(dir)
            .map_err(|err| format!("unable to read patches directory {}: {err}", dir.display()))?;

        for entry in entries {
            let entry = entry.map_err(|err| format!("error reading patches directory: {err}"))?;
            let path = entry.path();
            println!("cargo:rerun-if-changed={}", path.display());

            if entry
                .file_type()
                .map_err(|err| format!("unable to inspect {}: {err}", path.display()))?
                .is_file()
                && is_patch_file(&path)
            {
                files.push(path);
            }
        }

        // Sort patches to ensure consistent application order
        files.sort();
        Ok(PatchSet { files })
    }

    /// Check if there are no patches to apply
    fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Apply all patches in order
    fn apply(&self) -> Result<(), String> {
        for patch in &self.files {
            println!("cargo:warning=applying patch {}", patch.display());
            let status = Command::new("patch")
                .args(["--forward", "-p1", "-d", "xz", "-i"])
                .arg(patch)
                .status()
                .map_err(|err| format!("failed to invoke patch for {}: {err}", patch.display()))?;

            if !status.success() {
                return Err(format!("patch {} failed", patch.display()));
            }
        }
        Ok(())
    }
}

/// Check if a file appears to be a patch file based on its extension
fn is_patch_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("patch") | Some("diff")
    )
}

fn collect_liblzma_sources(base: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    gather_c_sources(base, &mut files)?;

    files.retain(|path| {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        !matches!(
            name,
            "crc32_tablegen.c"
                | "crc64_tablegen.c"
                | "crc_clmul_consts_gen.c"
                | "fastpos_tablegen.c"
                | "price_tablegen.c"
                | "crc32_small.c"
                | "crc64_small.c"
        )
    });

    files.sort();
    Ok(files)
}

fn gather_c_sources(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?
    {
        let entry =
            entry.map_err(|err| format!("failed to access entry in {}: {err}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            gather_c_sources(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("c") {
            out.push(path);
        }
    }

    Ok(())
}

/// Generate Rust bindings for liblzma using bindgen
#[cfg(feature = "bindgen")]
fn generate_bindings(out_dir: &Path, include_paths: &[String], use_system_headers: bool) {
    let mut builder = bindgen::Builder::default()
        .header("lzma.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Allow only liblzma symbols
        .allowlist_function("lzma_.*")
        .allowlist_type("lzma_.*")
        .allowlist_var("LZMA_.*")
        .allowlist_item("LZMA_.*")
        // Block problematic types
        .blocklist_type("max_align_t")
        // Configuration
        .size_t_is_usize(true)
        .layout_tests(false);

    // Add system header flag if using pkg-config
    if use_system_headers {
        builder = builder.clang_arg("-DPKG_CONFIG");
    }

    // Add include paths
    for path in include_paths {
        builder = builder.clang_arg(format!("-I{path}"));
    }

    let bindings = builder
        .generate()
        .expect("Unable to generate bindings for liblzma");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings.rs to OUT_DIR");
}
