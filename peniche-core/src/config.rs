use anyhow::Context;
use colored::Color;
use colored::ColoredString;
use colored::Colorize as _;
use serde::Deserialize;
use std::collections::HashMap;
use std::env::current_dir;
use std::hash::DefaultHasher;
use std::hash::Hash as _;
use std::hash::Hasher as _;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as AsyncCommand;
use tokio::task::JoinSet;

pub fn parse_command(command: &str) -> (&str, Vec<&str>) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let program = parts.first().unwrap_or(&""); // Safely get the first part or empty string if none
    let args = parts.get(1..).unwrap_or(&[]).to_vec(); // Get remaining parts as args or empty if none

    (program, args)
}

/// Generates a hash value for a given string.
fn hash_key(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

/// Converts a hash value to an RGB color.
fn hash_to_rgb(hash: u64) -> Color {
    let r = ((hash & 0xFF0000) >> 16) as u8;
    let g = ((hash & 0x00FF00) >> 8) as u8;
    let b = (hash & 0x0000FF) as u8;
    Color::TrueColor { r, g, b }
}

/// Colorizes the text with a consistent RGB color based on its hash.
fn colorize_key(key: &str) -> ColoredString {
    let hash = hash_key(key);
    let color = hash_to_rgb(hash);
    key.color(color)
}

#[derive(Deserialize, Debug)]
struct RawConfig {
    cmd: HashMap<String, serde_json::Value>, // Use serde_json::Value for flexible parsing
}

#[derive(Debug, Clone)]
pub struct Config {
    pub cmd: HashMap<String, Command>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Command {
    Simple {
        key: String,
        #[serde(skip)]
        colored_key: ColoredString,
        command: String,
        working_dir: Option<String>,
        env: Option<HashMap<String, String>>, // Optional environment variables
    },
    PlatformSpecific {
        key: String,
        #[serde(skip)]
        colored_key: ColoredString,
        commands: PlatformCommands,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformCommands {
    pub windows: Option<String>,
    pub linux: Option<String>,
    pub darwin: Option<String>,
    pub command: Option<String>,
    pub working_dir: Option<String>,
    pub env: Option<HashMap<String, String>>, // Optional environment variables
}

impl Config {
    pub async fn from_file(path: Option<&str>) -> anyhow::Result<Self> {
        let path = path.unwrap_or("Peniche.toml");
        let mut file = File::open(path)
            .await
            .context("Failed to open configuration file")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .await
            .context("Failed to read configuration file")?;

        let raw_config: RawConfig = toml::from_str(&contents)?;
        let mut commands = HashMap::new();

        for (key, value) in raw_config.cmd {
            let colored_key = colorize_key(&key);

            match value {
                serde_json::Value::String(cmd) => {
                    // Assume no working_dir is specified if only a string is provided
                    commands.insert(
                        key.clone(),
                        Command::Simple {
                            key,
                            colored_key,
                            command: cmd,
                            working_dir: None,
                            env: None,
                        },
                    );
                }
                serde_json::Value::Object(map) => {
                    let platform_commands = PlatformCommands {
                        windows: map
                            .get("windows")
                            .and_then(|v| v.as_str())
                            .map(String::from),

                        linux: map.get("linux").and_then(|v| v.as_str()).map(String::from),

                        darwin: map.get("darwin").and_then(|v| v.as_str()).map(String::from),

                        command: map
                            .get("command")
                            .and_then(|v| v.as_str())
                            .map(String::from),

                        working_dir: map
                            .get("working_dir")
                            .and_then(|v| v.as_str())
                            .map(String::from),

                        env: map.get("env").and_then(|v| v.as_object()).map(|obj| {
                            obj.iter()
                                .filter_map(|(k, v)| {
                                    if let Some(value) = v.as_str() {
                                        Some((k.clone(), value.to_string()))
                                    } else {
                                        None
                                    }
                                })
                                .collect::<HashMap<String, String>>()
                        }),
                    };
                    commands.insert(
                        key.clone(),
                        Command::PlatformSpecific {
                            key,
                            colored_key,
                            commands: platform_commands,
                        },
                    );
                }
                _ => return Err(anyhow::anyhow!("Unexpected format in command definition")),
            }
        }

        Ok(Config { cmd: commands })
    }

    pub async fn execute_commands_in_parallel(&self, cmd_names: Vec<String>) {
        let mut join_set = JoinSet::new();

        for name in cmd_names {
            if let Some(command) = self.cmd.get(&name).cloned() {
                join_set.spawn(tokio::spawn(async move {
                    let _ = command.stream_command().await.unwrap();
                }));
            } else {
                eprintln!("Command '{}' not found in configuration", name);
            }
        }

        while let Some(_) = join_set.join_next().await {}
    }
}

impl Command {
    pub async fn stream_command(&self) -> anyhow::Result<()> {
        let (key, command, working_dir, env_vars) = match self {
            Command::Simple {
                key: _,
                colored_key,
                command,
                working_dir,
                env,
            } => (
                colored_key.clone().bold(),
                command,
                working_dir.clone(),
                env,
            ),

            Command::PlatformSpecific {
                key: _,
                colored_key,
                commands,
            } => {
                let os_type = std::env::consts::OS;
                let command = match os_type {
                    "windows" => &commands.windows,
                    "linux" => &commands.linux,
                    "darwin" => &commands.darwin,
                    _ => &commands.command,
                }
                .as_deref()
                .or(commands.command.as_deref())
                .unwrap_or_default()
                .to_string();

                let wd = commands
                    .working_dir
                    .clone()
                    .unwrap_or(current_dir()?.to_string_lossy().to_string());

                (
                    colored_key.clone().bold(),
                    &command.clone(),
                    Some(wd),
                    &commands.env.clone(),
                )
            }
        };

        let (program, args) = parse_command(command);

        let mut cmd = AsyncCommand::new(program);
        if let Some(working_dir) = working_dir {
            cmd.current_dir(working_dir);
        }

        if let Some(env_vars) = env_vars {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        if args.len() > 0 {
            cmd.args(args);
        }

        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdout = BufReader::new(child.stdout.take().unwrap());
        let stderr = BufReader::new(child.stderr.take().unwrap());

        let mut stdout_lines = stdout.lines();
        let mut stderr_lines = stderr.lines();

        let tag_key = format!("{}{}{}", "[".dimmed(), key, "]".dimmed());

        tokio::select! {
            _ = async {
                while let Some(line) = stdout_lines.next_line().await? {
                    println!("{} {}", tag_key, line);
                }
                Ok::<(), anyhow::Error>(())
            } => {},
            _ = async {
                while let Some(line) = stderr_lines.next_line().await? {
                    eprintln!("{} {}", tag_key, line);
                }
                Ok::<(), anyhow::Error>(())
            } => {},
        }

        child.wait().await?; // Ensure process has finished
        Ok(())
    }
}
