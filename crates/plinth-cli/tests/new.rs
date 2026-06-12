//! Integration tests for `plinth new`, run against the real binary.

use std::path::Path;
use std::process::Command;

fn plinth() -> Command {
    Command::new(env!("CARGO_BIN_EXE_plinth"))
}

fn temp_workdir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("plinth-new-test-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn new_scaffolds_a_valid_project() {
    let dir = temp_workdir("happy");
    let output = plinth()
        .current_dir(&dir)
        .args(["new", "mygame"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let root = dir.join("mygame");
    for file in [
        "Cargo.toml",
        "src/main.rs",
        "scenes/arena.scene.json",
        "README.md",
        "CLAUDE.md",
        ".mcp.json",
        "assets/manifest.json",
        ".gitignore",
    ] {
        assert!(root.join(file).is_file(), "missing {file}");
    }

    // The project name is substituted, not left as a placeholder.
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(manifest.contains("name = \"mygame\""), "{manifest}");
    assert!(!manifest.contains("{{name}}"), "{manifest}");

    // The generated content passes the generated project's own validation
    // workflow: `plinth validate` from the project root.
    let validate = plinth()
        .current_dir(&root)
        .arg("validate")
        .output()
        .unwrap();
    assert!(
        validate.status.success(),
        "generated scene should validate, stdout: {}",
        String::from_utf8_lossy(&validate.stdout)
    );
}

#[test]
fn new_rejects_bad_names_and_existing_dirs() {
    let dir = temp_workdir("reject");

    let bad = plinth()
        .current_dir(&dir)
        .args(["new", "My Game"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("invalid project name"),
        "stderr: {}",
        String::from_utf8_lossy(&bad.stderr)
    );

    std::fs::create_dir_all(dir.join("taken")).unwrap();
    std::fs::write(dir.join("taken/file.txt"), "occupied").unwrap();
    let taken = plinth()
        .current_dir(&dir)
        .args(["new", "taken"])
        .output()
        .unwrap();
    assert!(!taken.status.success());
    assert!(
        String::from_utf8_lossy(&taken.stderr).contains("not empty"),
        "stderr: {}",
        String::from_utf8_lossy(&taken.stderr)
    );
}

#[test]
fn generated_main_matches_engine_api() {
    // The template's API usage must mirror the arena example that compiles
    // in CI — if the example moves, the template moves with it.
    let example = include_str!("../../plinth/examples/arena.rs");
    let template = include_str!("../templates/main.rs.tmpl");
    for call in ["Game::new(", ".level(", ".run()"] {
        assert!(example.contains(call), "example lost `{call}`");
        assert!(template.contains(call), "template lost `{call}`");
    }
    assert!(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plinth/examples/arena.rs"
        ))
        .exists()
    );
}
