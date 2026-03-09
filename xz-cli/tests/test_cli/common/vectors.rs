use std::fs;
use std::path::{Path, PathBuf};

/// Bundled test vector with inferred type and cached bytes.
pub struct Vector {
    kind: VectorKind,
    name: String,
    data: Vec<u8>,
}

/// Bundled vector file groups under `tests/test_cli/files`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorKind {
    /// `.xz` container test vectors.
    Xz,
    /// `.lz` (lzip) container test vectors.
    Lz,
    /// `.lzma` (`LZMA_Alone`) container test vectors.
    Lzma,
}

impl VectorKind {
    fn subdir(self) -> &'static str {
        match self {
            Self::Xz => "xz",
            Self::Lz => "lz",
            Self::Lzma => "lzma",
        }
    }

    fn from_name(name: &str) -> Self {
        if name.ends_with(".xz") {
            return Self::Xz;
        }
        if name.ends_with(".lz") {
            return Self::Lz;
        }
        if name.ends_with(".lzma") {
            return Self::Lzma;
        }

        panic!("unsupported vector name {name}: expected .xz, .lz, or .lzma suffix");
    }
}

impl Vector {
    /// Loads a bundled test vector and infers its group from the file name.
    pub fn bundled(name: &str) -> Self {
        let kind = VectorKind::from_name(name);
        let path = bundled_vector_path(kind, name);
        let data = fs::read(&path).unwrap_or_else(|err| {
            panic!("failed to read bundled vector {}: {err}", path.display())
        });

        Self {
            kind,
            name: name.to_string(),
            data,
        }
    }

    /// Returns the original bundled file name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the bytes of the bundled vector.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns the absolute path to the bundled source file.
    pub fn source_path(&self) -> PathBuf {
        bundled_vector_path(self.kind, &self.name)
    }

    /// Copies the bundled vector into a fixture directory under its original name.
    pub fn copy_to(&self, root_dir: &Path) -> PathBuf {
        self.copy_to_as(root_dir, &self.name)
    }

    /// Copies the bundled vector into a fixture directory under a custom name.
    pub fn copy_to_as(&self, root_dir: &Path, target_name: &str) -> PathBuf {
        let source = self.source_path();
        let target = root_dir.join(target_name);
        fs::copy(&source, &target).unwrap_or_else(|err| {
            panic!(
                "failed to copy vector {} to {}: {err}",
                source.display(),
                target.display()
            )
        });
        target
    }
}

/// Returns the absolute path to a bundled vector fixture.
fn bundled_vector_path(kind: VectorKind, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("test_cli")
        .join("vectors")
        .join(kind.subdir())
        .join(name)
}
