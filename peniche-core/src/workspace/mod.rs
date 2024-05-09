use anyhow::Context as _;
use cargo::util::important_paths::find_root_manifest_for_wd;
use cargo::GlobalContext;
use cargo_util::paths::write_atomic;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::{collections::HashMap, fs::File, path::PathBuf};
use toml_edit::DocumentMut;

use crate::krate::{KrateKind, KrateSource};
use crate::{krate::Krate, mkdirp, resolve_manifest_path};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Workspace {
    pub path: PathBuf,
    pub manifest_path: PathBuf,
    pub crates: HashMap<String, Krate>,
}

impl Workspace {
    pub fn new(path: PathBuf) -> Self {
        let (path, manifest_path) = resolve_manifest_path(&path);

        Self {
            path,
            manifest_path,
            crates: HashMap::new(),
        }
    }

    pub fn initialize(path: &str, name: &str) -> anyhow::Result<Self> {
        let canonical_path = mkdirp(path)?;
        let (path, manifest_path) = resolve_manifest_path(&canonical_path);

        if !manifest_path.exists() {
            let mut file = File::create(&manifest_path)?;
            writeln!(file, "[workspace]")?;
            writeln!(file, "resolver = \"{}\"", 2)?;
            writeln!(file, "name = \"{}\"", name)?;
            writeln!(file, "members = []")?;
        }

        Ok(Self::from_path(&path.to_string_lossy().to_string())?)
    }

    pub fn create_member_crate(
        &self,
        name: String,
        path: PathBuf,
        kind: KrateKind,
    ) -> anyhow::Result<Krate> {
        Krate::create_in_workspace(kind, name, path)
    }
    pub fn remove_member_crate(&mut self, name: &str, delete_files: bool) -> anyhow::Result<bool> {
        // First, locate the manifest and parse it to ensure everything else can proceed.
        let root_manifest_path =
            find_root_manifest_for_wd(&self.manifest_path).with_context(|| {
                format!(
                    "Failed to find root manifest from path {:?}",
                    &self.manifest_path
                )
            })?;
        let root_manifest_content =
            std::fs::read_to_string(&root_manifest_path).with_context(|| {
                format!(
                    "Failed to read the root manifest at {:?}",
                    root_manifest_path
                )
            })?;
        let mut workspace_document = root_manifest_content
            .parse::<DocumentMut>()
            .with_context(|| "Failed to parse the root Cargo.toml into a TOML document")?;

        // Check if the crate is actually part of the workspace
        if !self.crates.contains_key(name) {
            return Ok(false);
        }

        // Prepare to update the TOML document but do not modify self.crates yet
        let workspace = workspace_document
            .get_mut("workspace")
            .ok_or_else(|| anyhow::anyhow!("Workspace section not found in Cargo.toml"))?;
        let members = workspace
            .get_mut("members")
            .and_then(|m| m.as_array_mut())
            .ok_or_else(|| {
                anyhow::anyhow!("Members array not found in the workspace section of Cargo.toml")
            })?;

        let initial_len = members.len();
        members.retain(|v| v.as_str() != Some(name));

        // If members are modified, proceed with file operations
        if initial_len != members.len() {
            write_atomic(
                root_manifest_path.clone(),
                workspace_document.to_string().to_string().as_bytes(),
            )
            .with_context(|| {
                format!(
                    "Failed to write updated Cargo.toml to {:?}",
                    root_manifest_path
                )
            })?;

            // Now safely remove the crate from the map
            if let Some(krate) = self.crates.remove(name) {
                if delete_files {
                    if let KrateSource::Path(path) = &krate.path {
                        std::fs::remove_dir_all(path).with_context(|| {
                            format!("Failed to delete the crate directory for {}", name)
                        })?;
                    }
                }
                Ok(true)
            } else {
                // This branch should logically never be hit due to the earlier contains_key check
                Err(anyhow::anyhow!(
                    "Failed to find crate in workspace during removal process"
                ))
            }
        } else {
            Ok(false)
        }
    }

    pub fn from_path(path: &str) -> anyhow::Result<Self> {
        let (path, _) = resolve_manifest_path(&PathBuf::from(path));

        // let source_id = SourceId::for_path(&manifest_path)?;
        let manifest_path = cargo::util::important_paths::find_root_manifest_for_wd(&path)?;

        let ctx = GlobalContext::default()?;
        let cargo_ws = cargo::core::Workspace::new(&manifest_path, &ctx)?;

        let mut ws = Self::new(cargo_ws.root_manifest().into());

        for package in cargo_ws.members() {
            let name = package.name().to_string();
            let path = package.root();
            ws.crates
                .insert(name.clone(), Krate::from_path(path.to_str().unwrap())?);
        }

        Ok(ws)
    }
}
