//! Mirrors R/crates/airsl-cli/src/cli.rs: Cli -> Command::Lua(LuaCommand::Setup { dir }) plus the
//! proposed `--native` and `--lua <LUA>` (ValueEnum "5.4"/"luajit", requires = "native").
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Runs Lua scripts on the embedded `airsl` runtime.
#[derive(Debug, Parser)]
#[command(name = "airsl", version, about, long_about = None)]
struct Cli {
    /// What to do.
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Prepare a project for the Lua language server.
    #[command(subcommand)]
    Lua(LuaCommand),
}

/// Which Lua ABI the native module is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LuaAbi {
    /// Lua 5.4 (PUC-Rio).
    #[value(name = "5.4")]
    Lua54,
    /// LuaJIT 2.1 (the Lua 5.1 ABI).
    #[value(name = "luajit")]
    LuaJit,
}

#[derive(Debug, Subcommand)]
enum LuaCommand {
    /// Write a description of the built-in modules and add it to the language server's settings.
    Setup {
        /// Project directory to set up. Defaults to the working directory.
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Also install the native module with LuaRocks.
        #[arg(long)]
        native: bool,
        /// The Lua to install the native module for, instead of the one luarocks targets.
        #[arg(long, value_enum, requires = "native")]
        lua: Option<LuaAbi>,
    },
}

fn main() {
    let cli = Cli::parse();
    println!("{cli:?}");
}
