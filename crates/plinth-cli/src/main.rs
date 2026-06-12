mod mcp;
mod new;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use plinth_scene::Diagnostic;

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
    /// Validate Plinth data files (*.scene.json + asset manifests):
    /// schema, semantic rules, and scene-to-manifest cross-checks
    Validate {
        /// Files or directories (searched recursively for *.scene.json and
        /// manifest.json). Defaults to ./scenes, ./examples, and ./assets.
        paths: Vec<PathBuf>,
        /// Emit machine-readable JSON diagnostics on stdout
        #[arg(long)]
        json: bool,
    },
    /// Print a JSON Schema (the contracts agents and editors validate against)
    Schema {
        /// Which schema to print
        #[arg(value_enum, default_value_t = SchemaKind::Scene)]
        kind: SchemaKind,
    },
    /// Generate CREDITS.md from the asset manifest's license/provenance data
    Credits {
        /// Path to the asset manifest
        #[arg(long, default_value = "assets/manifest.json")]
        manifest: PathBuf,
        /// Where to write the credits file
        #[arg(long, default_value = "CREDITS.md")]
        out: PathBuf,
    },
    /// Serve the playtest MCP server: bridges a coding agent (stdio) to a
    /// running game's playtest API so it can observe, drive, and screenshot it
    Mcp {
        /// BRP URL of the running game
        #[arg(long, default_value = "http://127.0.0.1:15702")]
        game: String,
    },
    /// Scaffold a new Plinth game project
    New {
        /// Name of the project to create
        name: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum SchemaKind {
    Scene,
    Manifest,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Validate { paths, json } => validate(paths, json),
        Command::Schema { kind } => {
            let schema = match kind {
                SchemaKind::Scene => plinth_scene::schema_json(),
                SchemaKind::Manifest => plinth_scene::manifest_schema_json(),
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&schema).expect("schema serializes")
            );
            ExitCode::SUCCESS
        }
        Command::Credits { manifest, out } => credits(&manifest, &out),
        Command::Mcp { game } => mcp::serve(game),
        Command::New { name } => match new::create_project(&name) {
            Ok(root) => {
                println!(
                    "Created `{name}`.\n\n  cd {}\n  cargo run\n\nWASD + mouse + space. Edit scenes/*.scene.json while it runs — they hot-reload.\nValidate content anytime with `plinth validate`.",
                    root.display()
                );
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("error: {err}");
                ExitCode::from(2)
            }
        },
    }
}

fn credits(manifest_path: &Path, out: &Path) -> ExitCode {
    let src = match std::fs::read_to_string(manifest_path) {
        Ok(src) => src,
        Err(err) => {
            eprintln!("error: cannot read {}: {err}", manifest_path.display());
            return ExitCode::from(2);
        }
    };
    let (doc, diags) = plinth_scene::validate_manifest_str(&src);
    if diags.iter().any(Diagnostic::is_error) {
        eprintln!("{}:", manifest_path.display());
        for d in &diags {
            eprintln!("  {d}");
        }
        eprintln!("fix the manifest before generating credits");
        return ExitCode::FAILURE;
    }
    let doc = doc.expect("no errors implies a parsed manifest");
    let rendered = plinth_scene::render_credits(&doc);
    if let Err(err) = std::fs::write(out, rendered) {
        eprintln!("error: cannot write {}: {err}", out.display());
        return ExitCode::from(2);
    }
    println!(
        "Wrote {} ({} asset(s) credited).",
        out.display(),
        doc.assets.len()
    );
    ExitCode::SUCCESS
}

enum FileKind {
    Scene,
    Manifest,
}

fn classify(path: &Path) -> Option<FileKind> {
    let name = path.file_name()?.to_str()?;
    if name.ends_with(".scene.json") {
        Some(FileKind::Scene)
    } else if name == "manifest.json" || name.ends_with(".manifest.json") {
        Some(FileKind::Manifest)
    } else {
        None
    }
}

struct CheckedFile {
    path: PathBuf,
    diags: Vec<Diagnostic>,
    /// Entity count for scenes, asset count for manifests.
    items: Option<(FileKind, usize)>,
}

fn validate(paths: Vec<PathBuf>, json: bool) -> ExitCode {
    let roots = if paths.is_empty() {
        let defaults: Vec<PathBuf> = ["scenes", "examples", "assets"]
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .collect();
        if defaults.is_empty() {
            eprintln!(
                "error: no paths given and no ./scenes, ./examples, or ./assets directory found\n\
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
            collect_data_files(root, &mut files);
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
        eprintln!("error: no *.scene.json or manifest.json files found under the given paths");
        return ExitCode::from(2);
    }

    // Validate each file, remembering scene model references and manifest
    // contents for the cross-check.
    let mut checked: Vec<CheckedFile> = Vec::new();
    let mut model_refs: Vec<(usize, String, String)> = Vec::new(); // (file idx, location, path)
    let mut manifest_paths: Vec<String> = Vec::new();
    let mut manifests_seen = false;

    for file in files {
        let src = match std::fs::read_to_string(&file) {
            Ok(src) => src,
            Err(err) => {
                eprintln!("error: cannot read {}: {err}", file.display());
                return ExitCode::from(2);
            }
        };
        let file_index = checked.len();
        match classify(&file) {
            Some(FileKind::Scene) | None => {
                let (doc, diags) = plinth_scene::validate_str(&src);
                if let Some(doc) = &doc {
                    for (i, entity) in doc.entities.iter().enumerate() {
                        if let Some(model) = &entity.components.model {
                            model_refs.push((
                                file_index,
                                format!("entities[{i}].components.model.path"),
                                model.path.clone(),
                            ));
                        }
                    }
                }
                checked.push(CheckedFile {
                    path: file,
                    diags,
                    items: doc.map(|d| (FileKind::Scene, d.entities.len())),
                });
            }
            Some(FileKind::Manifest) => {
                manifests_seen = true;
                let (doc, diags) = plinth_scene::validate_manifest_str(&src);
                if let Some(doc) = &doc {
                    manifest_paths.extend(doc.assets.iter().map(|a| a.path.clone()));
                }
                checked.push(CheckedFile {
                    path: file,
                    diags,
                    items: doc.map(|d| (FileKind::Manifest, d.assets.len())),
                });
            }
        }
    }

    // Cross-check: every model a scene references should be tracked in a
    // manifest (license + provenance).
    if manifests_seen {
        for (file_index, location, path) in model_refs {
            if !manifest_paths.contains(&path) {
                checked[file_index].diags.push(Diagnostic::warning(
                    location,
                    format!(
                        "model `{path}` is not tracked in the asset manifest; add it with its license and source"
                    ),
                ));
            }
        }
    }

    let error_count: usize = checked
        .iter()
        .map(|f| f.diags.iter().filter(|d| d.is_error()).count())
        .sum();
    let warning_count: usize = checked
        .iter()
        .map(|f| f.diags.iter().filter(|d| !d.is_error()).count())
        .sum();

    if json {
        let diagnostics: Vec<serde_json::Value> = checked
            .iter()
            .flat_map(|f| {
                f.diags.iter().map(|d| {
                    serde_json::json!({
                        "file": f.path.display().to_string(),
                        "location": d.location,
                        "line": d.line,
                        "column": d.column,
                        "kind": d.kind.to_string(),
                        "severity": if d.is_error() { "error" } else { "warning" },
                        "message": d.message,
                    })
                })
            })
            .collect();
        let report = serde_json::json!({
            "ok": error_count == 0,
            "files_checked": checked.len(),
            "errors": error_count,
            "warnings": warning_count,
            "diagnostics": diagnostics,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("report serializes")
        );
    } else {
        for f in &checked {
            if f.diags.is_empty() {
                match &f.items {
                    Some((FileKind::Scene, n)) => {
                        println!("{}: OK ({n} entities)", f.path.display());
                    }
                    Some((FileKind::Manifest, n)) => {
                        println!("{}: OK ({n} assets)", f.path.display());
                    }
                    None => println!("{}: OK", f.path.display()),
                }
            } else {
                println!("{}:", f.path.display());
                for d in &f.diags {
                    println!("  {d}");
                }
            }
        }
        println!(
            "{} file(s) checked: {} error(s), {} warning(s)",
            checked.len(),
            error_count,
            warning_count
        );
    }

    if error_count == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn collect_data_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_data_files(&path, out);
        } else if classify(&path).is_some() {
            out.push(path);
        }
    }
}
