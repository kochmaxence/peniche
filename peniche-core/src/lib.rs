use anyhow::Context as _;
use std::env::current_dir;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

pub mod config;
pub mod krate;
pub mod log;
pub mod workspace;

pub fn resolve_manifest_path(path: &PathBuf) -> (PathBuf, PathBuf) {
    let is_manifest = path.ends_with("Cargo.toml");

    let manifest_path = if is_manifest {
        path.clone()
    } else {
        if !path.to_string_lossy().len() > 0 {
            path.join("Cargo.toml")
        } else {
            path.clone()
        }
    };

    let path = if is_manifest {
        path.parent().unwrap_or_else(|| Path::new("")).to_path_buf()
    } else {
        path.to_path_buf()
    };

    (path, manifest_path)
}

pub fn resolve_path(input_path: &str) -> anyhow::Result<PathBuf> {
    let path = PathBuf::from(input_path);
    // Check if the path is already absolute
    if path.is_absolute() {
        Ok(path)
    } else {
        // Combine with the current directory to make it absolute
        let base = current_dir().with_context(|| "Failed to get current directory")?;
        let absolute_path = base.join(path);
        Ok(absolute_path)
    }
}

pub fn mkdirp(input_path: &str) -> anyhow::Result<PathBuf> {
    let path = resolve_path(input_path)?;

    create_dir_all(&path).with_context(|| format!("Failed to create directory at {:?}", path))?;

    Ok(path)
}
