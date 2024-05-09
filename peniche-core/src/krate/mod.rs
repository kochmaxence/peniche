use anyhow::anyhow;
use cargo::{
    core::{
        compiler::{CompileMode, MessageFormat},
        Dependency, EitherManifest, SourceId,
    },
    ops::{self, CompileOptions, NewOptions},
    util::{
        toml::read_manifest,
        toml_mut::{
            dependency::{GitSource, PathSource, RegistrySource, Source, WorkspaceSource},
            manifest::LocalManifest,
        },
    },
    GlobalContext,
};
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, str::FromStr, vec};

use crate::resolve_manifest_path;

#[derive(Debug, Default, Serialize, Deserialize)]
pub enum KrateSource {
    #[default]
    Registry,
    Path(PathBuf),
    Git(String),
    Workspace,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub enum KrateKind {
    #[default]
    Bin,
    Lib,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Krate {
    pub name: String,
    pub version: String,
    pub path: KrateSource,
    pub manifest_path: Option<PathBuf>,
    pub dependencies: HashMap<String, Krate>,
}

impl Krate {
    pub fn new(name: String, version: String, source: KrateSource) -> Self {
        let manifest_path = match &source {
            KrateSource::Path(path) => Some(resolve_manifest_path(path).1),
            _ => None,
        };

        Krate {
            name,
            version,
            path: source,
            manifest_path,
            dependencies: HashMap::new(),
        }
    }

    pub fn install_krate_globally(&self) -> anyhow::Result<&Self> {
        let root: Option<&str> = None;

        let crate_install_list: Vec<(String, Option<VersionReq>)> = vec![];

        let (source_id, gctx) = match &self.path {
            KrateSource::Path(path) => {
                let mut gctx = GlobalContext::default()?;
                gctx.reload_rooted_at(path)?;
                gctx.shell().set_verbosity(cargo::core::Verbosity::Normal);

                Ok((SourceId::for_path(&path)?, gctx))
            }
            _ => Err(anyhow!("Only workspace members can be installed globally")),
        }?;

        let mut compile_opts = CompileOptions::new(&gctx, CompileMode::Build)?;
        compile_opts.build_config.requested_profile = "release".into();
        compile_opts.build_config.message_format = MessageFormat::Human;
        compile_opts.rustdoc_document_private_items = false;

        ops::install(
            &gctx,
            root,
            crate_install_list,
            source_id,
            false,
            &compile_opts,
            true,
            false,
        )?;

        Ok(self)
    }

    pub fn uninstall_krate_globally(&self) -> anyhow::Result<&Self> {
        let bin = vec![self.name.clone()];
        let spec = vec![self.name.as_ref()];

        let gctx = match &self.path {
            KrateSource::Path(path) => {
                let mut gctx = GlobalContext::default()?;
                gctx.reload_rooted_at(path)?;

                Ok(gctx)
            }
            _ => Err(anyhow!("Only workspace members can be installed globally")),
        }?;

        ops::uninstall(None, spec, &bin, &gctx)?;

        Ok(self)
    }

    pub fn create_in_workspace(
        kind: KrateKind,
        name: String,
        path: PathBuf,
    ) -> anyhow::Result<Self> {
        let ctx = GlobalContext::default()?;
        let is_bin = kind == KrateKind::Bin;
        let is_lib = kind == KrateKind::Lib;

        let opts = NewOptions::new(
            Some(ops::VersionControl::NoVcs),
            is_bin,
            is_lib,
            path.clone(),
            Some(name),
            None,
            None,
        )?;

        let _ = ops::new(&opts, &ctx)?;

        Self::from_path(&path.to_string_lossy().to_string())
    }

    pub fn as_cargo_dependency(
        &self,
    ) -> anyhow::Result<cargo::util::toml_mut::dependency::Dependency> {
        let source = match &self.path {
            KrateSource::Path(path) => Source::Path(PathSource::new(path)),
            KrateSource::Workspace => Source::Workspace(WorkspaceSource::new()),
            KrateSource::Registry => Source::Registry(RegistrySource::new(&self.version)),
            KrateSource::Git(url) => Source::Git(GitSource::new(url)),
        };

        let dep = cargo::util::toml_mut::dependency::Dependency::new(&self.name).set_source(source);

        Ok(dep)
    }

    pub fn add_dependency(&self, _dep: String) -> anyhow::Result<Self> {
        todo!()
    }

    pub fn link_to(&self, dep: &Krate) -> anyhow::Result<()> {
        if let Some(manifest_path) = &self.manifest_path {
            let mut local_manifest = LocalManifest::try_new(manifest_path)?;

            let table_name = vec!["dependencies".to_string()];
            let cargo_dep = dep.as_cargo_dependency()?;
            println!("{:#?}", cargo_dep);
            local_manifest.insert_into_table(&table_name, &cargo_dep)?;
            local_manifest.write()?;
        }

        Ok(())
    }

    pub fn from_cargo_dependency(dep: &Dependency, source: KrateSource) -> anyhow::Result<Self> {
        let name = dep.name_in_toml().to_string();
        let version = dep.version_req().to_string();
        Ok(Self::new(name, version, source))
    }

    pub fn from_path(path: &str) -> anyhow::Result<Self> {
        let (_, manifest_path) = resolve_manifest_path(&PathBuf::from_str(path)?);

        let source_id = SourceId::for_path(&manifest_path)?;
        let ctx = GlobalContext::default()?;

        let manifest = match read_manifest(&manifest_path, source_id, &ctx)? {
            EitherManifest::Real(manifest) => manifest,
            _ => return Err(anyhow!("Failed to read manifest")),
        };

        let name = manifest.name().to_string();
        let version = manifest.version().to_string();
        let source = KrateSource::Path(path.to_owned().into());

        let mut krate = Krate::new(name, version, source);

        for dep in manifest.dependencies() {
            let dep_source = if dep.source_id().is_path() {
                KrateSource::Path(dep.source_id().url().to_string().into())
            } else {
                KrateSource::Registry
            };

            let dep_krate = Krate::from_cargo_dependency(dep, dep_source)?;
            krate.dependencies.insert(dep_krate.name.clone(), dep_krate);
        }

        Ok(krate)
    }
}
