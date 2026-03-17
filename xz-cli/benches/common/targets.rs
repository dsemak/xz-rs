use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub enum CodecFormat {
    Xz,
    Lzma,
}

impl CodecFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Xz => "xz",
            Self::Lzma => "lzma",
        }
    }

    pub fn compressor_binary(self) -> &'static str {
        match self {
            Self::Xz => "xz",
            Self::Lzma => "lzma",
        }
    }
}

pub struct BenchmarkTarget {
    pub label: &'static str,
    pub binary_path: PathBuf,
}

pub fn locate_cargo_binary(name: &str) -> PathBuf {
    let env_var = format!("CARGO_BIN_EXE_{name}");
    if let Some(path) = std::env::var_os(&env_var) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
    }

    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let exe_name = if exe_suffix.is_empty() || name.ends_with(exe_suffix) {
        name.to_string()
    } else {
        format!("{name}{exe_suffix}")
    };

    let mut candidates = Vec::new();
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest_dir = PathBuf::from(manifest_dir);
        for ancestor in manifest_dir.ancestors() {
            candidates.push(ancestor.join("target/release").join(&exe_name));
            candidates.push(ancestor.join("target/debug").join(&exe_name));
        }
    }
    candidates.push(PathBuf::from("target/release").join(&exe_name));
    candidates.push(PathBuf::from("target/debug").join(&exe_name));

    for candidate in candidates {
        if candidate.is_file() {
            return candidate;
        }
    }

    panic!(
        "unable to locate cargo-built binary '{name}', build it first with `cargo build --release`"
    );
}

pub fn locate_system_binary(name: &str) -> Option<PathBuf> {
    if let Ok(path) = which::which(name) {
        return Some(path);
    }

    let common_paths = ["/usr/bin", "/usr/local/bin", "/bin", "/sbin", "/usr/sbin"];
    for base in common_paths {
        let candidate = Path::new(base).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

pub fn benchmark_targets(binary_name: &str) -> Vec<BenchmarkTarget> {
    let our_binary = locate_cargo_binary(binary_name);
    let mut targets = vec![BenchmarkTarget {
        label: "ours",
        binary_path: our_binary.clone(),
    }];

    if let Some(system_binary) = locate_system_binary(binary_name) {
        if system_binary != our_binary {
            targets.push(BenchmarkTarget {
                label: "upstream",
                binary_path: system_binary,
            });
        }
    }

    targets
}
