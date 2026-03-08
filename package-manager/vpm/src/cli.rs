use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};

pub const DEFAULT_REGISTRY: &str = "https://vpm.voidium.uk/";

#[repr(u8)]
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorMode {
    Auto = 0,
    Always = 1,
    Never = 2,
}

#[derive(Parser)]
#[command(
    name = "vpm",
    version = env!("CARGO_PKG_VERSION"),
    about = "Void Package Manager"
)]
pub struct Cli {
    #[arg(short, long, global = true, action = ArgAction::SetTrue)]
    pub verbose: bool,
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Init {
        name: Option<String>,
    },
    Publish {
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        github: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
    },
    Login {
        username: String,
        password: String,
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
    },
    Logout {
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
    },
    Whoami {
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
    },
    Search {
        query: String,
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
    },
    Info {
        name: String,
        #[arg(long)]
        version: Option<String>,
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
        #[arg(long)]
        readme: bool,
    },
    List,
    #[command(visible_aliases = ["remove", "delete", "rm"])]
    Uninstall {
        name: String,
    },
    Clean {
        #[arg(long)]
        lock: bool,
        #[arg(long)]
        cache: bool,
        #[arg(long)]
        imports: bool,
        #[arg(long)]
        all: bool,
    },
    Doctor {
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
    },
    Install {
        name: Option<String>,
        #[arg(long)]
        version: Option<String>,
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
    },
}
