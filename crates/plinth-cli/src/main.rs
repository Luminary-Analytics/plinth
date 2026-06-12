use std::path::{Path, PathBuf};
use std::process::ExitCode;

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
    /// Validate Plinth data files (*.scene.json): schema + semantic rules
    Validate {
        /// Files or directories (searched recursively for *.scene.json).
        /// Defaults to ./scenes and ./examples when present.
        paths: Vec<PathBuf>,
        /// Emit machine-readable JSON diagnostics on stdout
        #[arg(long)]
        json: bool,
    },
    /// Print the scene JSON Schema (the contract agents and editors validate against)
    Schema,
    /// Scaffold a new Plinth game project
    New {
        /// Name of the project to create
        name: String,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Validate { paths, json } => validate(paths, json),
        Command::Schema => {
            println!(
                "{}",
                serde_json::to_string_pretty(&plinth_scene::schema_json())
                    .expect("schema serializes")
            );
            ExitCode::SUCCESS
        }
        Command::New { name } => {
            eprintln!("`plinth new {name}` lands later in M1 (scaffolding + first template).");
            ExitCode::FAILURE
        }
    }
}

fn validate(paths: Vec<PathBuf>, json: bool) -> ExitCode {
    let roots = if paths.is_empty() {
        let defaults: Vec<PathBuf> = ["scenes", "examples"]
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .collect();
        if defaults.is_empty() {
            eprintln!(
                "error: no paths given and no ./scenes or ./examples directory found\n\
                 usage: plinth validate <files-or-directories>..."
            );
            return ExitCode::from(2);
        }
        defaults
    } else {
        paths
    };

    let mut files = Vec::new();
    for root in &roots {
        if root.is_dir() {
            collect_scene_files(root, &mut files);
        } else if root.is_file() {
            files.push(root.clone());
        } else {
            eprintln!("error: path not found: {}", root.display());
            return ExitCode::from(2);
        }
    }
    files.sort();
    files.dedup();

    if files.is_empty() {
        eprintln!("error: no *.scene.json files found under the given paths");
        return ExitCode::from(2);
    }

    let mut all: Vec<(PathBuf, Vec<plinth_scene::Diagnostic>, Option<usize>)> = Vec::new();
    for file in files {
        match std::fs::read_to_string(&file) {
            Ok(src) => {
                let (doc, diags) = plinth_scene::validate_str(&src);
                all.push((file, diags, doc.map(|d| d.entities.len())));
            }
            Err(err) => {
                eprintln!("error: cannot read {}: {err}", file.display());
                return ExitCode::from(2);
            }
        }
    }

    let error_count: usize = all.iter().map(|(_, d, _)| d.len()).sum();

    if json {
        let diagnostics: Vec<serde_json::Value> = all
            .iter()
            .flat_map(|(file, diags, _)| {
                diags.iter().map(|d| {
                    serde_json::json!({
                        "file": file.display().to_string(),
                        "location": d.location,
                        "line": d.line,
                        "column": d.column,
                        "kind": d.kind.to_string(),
                        "message": d.message,
                    })
                })
            })
            .collect();
        let report = serde_json::json!({
            "ok": error_count == 0,
            "files_checked": all.len(),
            "diagnostics": diagnostics,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("report serializes")
        );
    } else {
        for (file, diags, entities) in &all {
            if diags.is_empty() {
                let n = entities.unwrap_or(0);
                println!("{}: OK ({n} entities)", file.display());
            } else {
                println!("{}:", file.display());
                for d in diags {
                    println!("  {d}");
                }
            }
        }
        let files_with_errors = all.iter().filter(|(_, d, _)| !d.is_empty()).count();
        println!(
            "{} file(s) checked: {} OK, {} with errors ({} error(s))",
            all.len(),
            all.len() - files_with_errors,
            files_with_errors,
            error_count
        );
    }

    if error_count == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn collect_scene_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_scene_files(&path, out);
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".scene.json"))
        {
            out.push(path);
        }
    }
}
