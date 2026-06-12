//! Integration tests for the asset-layer CLI: credits generation and the
//! scene-to-manifest cross-check.

use std::process::Command;

fn plinth() -> Command {
    Command::new(env!("CARGO_BIN_EXE_plinth"))
}

fn temp_project(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("plinth-assets-test-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("scenes")).unwrap();
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    dir
}

#[test]
fn credits_generates_from_manifest() {
    let dir = temp_project("credits");
    std::fs::write(
        dir.join("assets/manifest.json"),
        r#"{ "version": 1, "assets": [
            { "path": "models/knight.glb", "license": "CC0-1.0",
              "author": "Kenney", "title": "Knight",
              "source": "https://kenney.nl" }
        ] }"#,
    )
    .unwrap();

    let output = plinth().current_dir(&dir).arg("credits").output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let credits = std::fs::read_to_string(dir.join("CREDITS.md")).unwrap();
    assert!(credits.contains("## CC0-1.0"), "{credits}");
    assert!(
        credits.contains("**Knight** (`models/knight.glb`) by Kenney"),
        "{credits}"
    );
}

#[test]
fn validate_warns_on_untracked_models_until_manifested() {
    let dir = temp_project("crosscheck");
    std::fs::write(
        dir.join("scenes/level.scene.json"),
        r#"{ "version": 1, "entities": [
            { "id": "knight", "components": { "model": { "path": "models/knight.glb" } } }
        ] }"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("assets/manifest.json"),
        r#"{ "version": 1, "assets": [] }"#,
    )
    .unwrap();

    // Untracked model: exit 0 (warning, not error) but the finding is loud.
    let output = plinth().current_dir(&dir).arg("validate").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "warnings must not fail: {stdout}");
    assert!(
        stdout.contains("not tracked in the asset manifest"),
        "{stdout}"
    );
    assert!(stdout.contains("1 warning(s)"), "{stdout}");

    // Tracking it clears the warning.
    std::fs::write(
        dir.join("assets/manifest.json"),
        r#"{ "version": 1, "assets": [
            { "path": "models/knight.glb", "license": "CC0-1.0" }
        ] }"#,
    )
    .unwrap();
    let output = plinth().current_dir(&dir).arg("validate").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("0 error(s), 0 warning(s)"), "{stdout}");
}

#[test]
fn manifest_schema_command_prints_schema() {
    let output = plinth().args(["schema", "manifest"]).output().unwrap();
    assert!(output.status.success());
    let schema: serde_json::Value = serde_json::from_slice(&output.stdout).expect("schema is JSON");
    assert!(
        schema["title"]
            .as_str()
            .unwrap_or_default()
            .contains("AssetManifest"),
        "{schema}"
    );
}
