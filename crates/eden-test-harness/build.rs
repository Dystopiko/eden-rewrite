//! Build script for `eden-test-harness`.
//!
//! Computes a deterministic SHA-256 hash of all files in the workspace `migrations/`
//! directory and exposes it at compile time via the `EDEN_DB_MIGRATIONS_HASH` environment
//! variable.
//!
//! This hash allows test harnesses to detect database schema changes and trigger schema
//! recreation or cache invalidation when migrations are updated.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

fn main() {
    let root = Path::new(env!("CARGO_WORKSPACE_DIR"));
    let migrations_dir = root.join("migrations");
    println!("cargo::rerun-if-changed={}", migrations_dir.display());

    let hash = compute_migrations_hash(&migrations_dir);
    println!("cargo::rustc-env=EDEN_DB_MIGRATIONS_HASH={hash}");
}

/// Computes a deterministic SHA-256 hash of all files within `dir`.
fn compute_migrations_hash(dir: &Path) -> String {
    let mut files = Vec::new();
    collect_files(dir, &mut files);
    files.sort();

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    for path in files {
        println!("cargo::rerun-if-changed={}", path.display());

        if let Ok(relative) = path.strip_prefix(dir) {
            hasher.update(relative.to_string_lossy().as_bytes());
        }

        if let Ok(mut file) = fs::File::open(&path) {
            while let Ok(count) = file.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
            }
        }
    }

    hex::encode(hasher.finalize())
}

/// Recursively collects all regular files in a directory.
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files);
        } else if path.is_file() {
            files.push(path);
        }
    }
}
