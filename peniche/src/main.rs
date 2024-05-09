use std::{env::current_dir, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use peniche_core::{
    config::Config,
    info_msg,
    krate::{Krate, KrateKind},
    log::handle_error,
    success_msg,
    workspace::Workspace,
};

/// Manage your rust monorepository
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(short, long, default_value = "Peniche.toml")]
    config: String,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show informations about the workspace
    Info,
    /// Initialize a new cargo workspace
    Init {
        #[clap(
            help = "Name of the workspace (will be the name of the directory if path is not set)"
        )]
        name: String,
        #[clap(help = "Directory to create the workspace")]
        path: Option<PathBuf>,
    },
    /// Create a new member crate in the workspace
    #[clap(group(
        clap::ArgGroup::new("newtype")
            .required(false)
            .args(&["bin", "lib"]),
    ))]
    #[clap(alias = "n")]
    New {
        #[clap(
            long,
            help = "Creates a binary project",
            group = "newtype",
            default_value_t = true
        )]
        bin: bool,

        #[clap(long, help = "Creates a library project", group = "newtype")]
        lib: bool,

        #[clap(help = "One (or more) names for the crate(s) to create")]
        names: Vec<String>,
    },
    #[clap(alias = "i")]
    Install {
        #[clap(help = "One (or more) names for the crate(s) to install globally")]
        names: Vec<String>,
    },
    #[clap(alias = "u")]
    Uninstall {
        #[clap(help = "One (or more) names for the crate(s) to uninstall globally")]
        names: Vec<String>,
    },
    #[clap(alias = "r")]
    Run {
        #[clap(help = "One (or more) scripts to run")]
        names: Vec<String>,
        #[clap(long, help = "List all available commands", action = clap::ArgAction::SetTrue)]
        list: bool,
    },
    /// Remove a crate from the workspace, optionally DELETING THE CRATE DIRECTORY!!!!
    #[clap(alias = "rm")]
    Delete {
        #[clap(help = "One (or more) names of crates to remove from the workspace")]
        names: Vec<String>,

        #[clap(
            long,
            help = "DELETE THE WHOLE CRATE DIRECTORY!!!",
            default_value_t = false
        )]
        rmdir: bool,
    },
    /// List all crates in the workspace
    #[clap(alias = "ls")]
    ListCrates,
    /// Add a workspace crate as a dependency of another workspace crate
    #[clap(alias = "ln")]
    Link { from: String, to: String },
    /// Perform a release
    Release {
        /// Release version type (major, minor, patch)
        #[clap(short, long)]
        version: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::from_file(None).await?;

    match cli.command {
        Commands::Info => {
            // let current_dir = handle_error(get_current_dir(), "Could not get current directory")?;
            // let ws = handle_error(Workspace::from_path(&current_dir.to_string_lossy()), "Failed to load workspace")?;
            info_msg!("Workspace info:");
            todo!()
        }
        Commands::Init { name, path } => {
            let current_dir = get_current_dir()?;
            let path = path.unwrap_or(current_dir.join(&name));
            handle_error(
                Workspace::initialize(&path.to_string_lossy(), &name),
                "Failed to initialize workspace",
            )?;
            success_msg!("Initialized workspace at {}", path.display());
        }
        Commands::New { bin: _, lib, names } => {
            let current_dir = get_current_dir()?;
            let ws = Workspace::from_path(&current_dir.to_string_lossy())?;

            for name in names {
                let kind = if lib { KrateKind::Lib } else { KrateKind::Bin };
                let path = ws.path.clone().join(&name);
                info_msg!("Creating {} ({:?}) at {:?}", name, kind, path);
                handle_error(
                    Krate::create_in_workspace(kind, name.clone(), path),
                    &format!("Failed to create crate '{}'", name),
                )?;
                success_msg!("Created new crate '{}'", name.bold().underline());
            }
        }
        Commands::Install { names } => {
            let current_dir = get_current_dir()?;
            let ws = Workspace::from_path(&current_dir.to_string_lossy())?;

            for name in names {
                let krate = ws.crates.get(&name).unwrap();
                handle_error(
                    krate.install_krate_globally(),
                    &format!("Failed to install crate {} globally", name),
                )?;
            }
        }
        Commands::Uninstall { names } => {
            let current_dir = get_current_dir()?;
            let ws = Workspace::from_path(&current_dir.to_string_lossy())?;

            for name in names {
                let krate = ws.crates.get(&name).unwrap();
                handle_error(
                    krate.uninstall_krate_globally(),
                    &format!("Failed to uninstall crate {} globally", name),
                )?;
            }
        }
        Commands::Run { names, list } => {
            if list || names.is_empty() {
                // If the list flag is set, display all available commands
                info_msg!("Available commands:");
                for (key, _) in &config.cmd {
                    println!("{}", key);
                }
            } else {
                // Otherwise, execute specified commands
                if !names.is_empty() {
                    config.execute_commands_in_parallel(names).await;
                } else {
                    println!("No command specified to run.");
                }
            }
        }
        Commands::Delete { names, rmdir } => {
            let mut ws = Workspace::from_path(&current_dir().unwrap().to_string_lossy())?;
            for name in names {
                handle_error(
                    ws.remove_member_crate(&name, rmdir),
                    &format!("Failed to remove crate '{}'", name),
                )?;
                success_msg!("Removed crate '{}'", name.bold().underline());
            }
        }
        Commands::ListCrates => {
            let ws = Workspace::from_path(&current_dir().unwrap().to_string_lossy())?;
            for (_, krate) in ws.crates {
                let path = match krate.path {
                    peniche_core::krate::KrateSource::Registry => "cargo",
                    peniche_core::krate::KrateSource::Path(path) => {
                        &path.to_string_lossy().to_string()
                    }
                    peniche_core::krate::KrateSource::Git(repo) => &repo.to_string(),
                    peniche_core::krate::KrateSource::Workspace => "workspace",
                };
                info_msg!("{} ({}) - {}", krate.name.bold(), krate.version, path);
            }
        }
        Commands::Link { from, to } => {
            let ws = Workspace::from_path(&current_dir().unwrap().to_string_lossy())?;
            let from_krate = ws
                .crates
                .get(&from)
                .ok_or_else(|| anyhow::anyhow!("Crate '{}' not found", from))?;
            let to_krate = ws
                .crates
                .get(&to)
                .ok_or_else(|| anyhow::anyhow!("Crate '{}' not found", to))?;

            handle_error(
                from_krate.link_to(to_krate),
                &format!("Failed to link '{}' to '{}'", from, to),
            )?;
            success_msg!("Linked '{}' to '{}'", from.bold(), to.bold());
        }
        Commands::Release { version } => {
            // Implement release logic
            success_msg!("Released version {}", version);
            todo!()
        }
    }
    Ok(())
}

/// Simplified function to get the current directory with error handling
fn get_current_dir() -> Result<PathBuf> {
    std::env::current_dir().context("Failed to determine the current directory")
}
