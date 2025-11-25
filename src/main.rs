use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use neki_lang::cmd;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  /// Generate language template
  Gen {
    /// Input directory (Mod folder)
    #[arg(short, long)]
    input: PathBuf,
    /// Output directory
    #[arg(short, long)]
    output: PathBuf,
    /// To generate test operation for every replace patch operation
    #[arg(short, long)]
    test: bool,
  },
  /// Initialize configuration files (in executable's directory)
  Init {
    /// Overwrite existing config files
    #[arg(short, long)]
    force: bool,
  },
}

fn main() -> Result<()> {
  let cli = Cli::parse();

  match cli.command {
    Commands::Gen {
      input,
      output,
      test,
    } => cmd::generate::run(input, output, test),
    Commands::Init { force } => cmd::init::run(force),
  }
}
