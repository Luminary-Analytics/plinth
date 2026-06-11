use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "plinth",
    version,
    about = "Plinth — the stable base your game stands on"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold a new Plinth game project
    New {
        /// Name of the project to create
        name: String,
    },
    /// Validate scene and prefab data files against their schemas
    Validate,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::New { name } => {
            eprintln!("`plinth new {name}` lands in milestone M1 (scaffolding + first template).");
            std::process::exit(1);
        }
        Command::Validate => {
            eprintln!("`plinth validate` lands in milestone M1 (scene schema + validation).");
            std::process::exit(1);
        }
    }
}
