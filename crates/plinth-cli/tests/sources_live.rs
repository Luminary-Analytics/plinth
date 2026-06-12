//! Live-network tests for asset sources. Ignored in CI (network, external
//! services); run locally with: cargo test -p plinth-cli -- --ignored

use std::process::Command;

fn plinth() -> Command {
    Command::new(env!("CARGO_BIN_EXE_plinth"))
}

#[test]
#[ignore = "network: hits the real PolyHaven API"]
fn polyhaven_search_finds_barrels() {
    let output = plinth()
        .args(["assets", "search", "barrel"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("polyhaven:Barrel"), "{stdout}");
    assert!(stdout.contains("CC0-1.0"), "{stdout}");
}

#[test]
#[ignore = "network: downloads a real model from PolyHaven"]
fn polyhaven_add_downloads_and_manifests() {
    let dir = std::env::temp_dir().join("plinth-sources-live-add");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let output = plinth()
        .current_dir(&dir)
        .args(["assets", "add", "polyhaven", "Barrel_01"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Files on disk, tracked in the manifest, credited.
    let manifest_src = std::fs::read_to_string(dir.join("assets/manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_src).unwrap();
    let model_path = manifest["assets"][0]["path"]
        .as_str()
        .expect("manifest records the model path");
    assert!(
        model_path.starts_with("models/Barrel_01/") && model_path.ends_with(".gltf"),
        "{manifest_src}"
    );
    assert!(
        dir.join("assets").join(model_path).is_file(),
        "downloaded glTF missing at {model_path}: {stdout}"
    );
    assert_eq!(
        manifest["assets"][0]["license"], "CC0-1.0",
        "{manifest_src}"
    );
    let credits = std::fs::read_to_string(dir.join("CREDITS.md")).unwrap();
    assert!(credits.contains("Barrel_01"), "{credits}");

    // And the whole project validates clean (no untracked-model warnings).
    std::fs::create_dir_all(dir.join("scenes")).unwrap();
    std::fs::write(
        dir.join("scenes/test.scene.json"),
        format!(
            r#"{{ "version": 1, "entities": [
            {{ "id": "barrel", "components": {{ "model": {{ "path": "{model_path}" }} }} }}
        ] }}"#
        ),
    )
    .unwrap();
    let validate = plinth().current_dir(&dir).arg("validate").output().unwrap();
    let stdout = String::from_utf8_lossy(&validate.stdout);
    assert!(validate.status.success(), "{stdout}");
    assert!(stdout.contains("0 error(s), 0 warning(s)"), "{stdout}");
}

#[test]
#[ignore = "network + PLINTH_POLYPIZZA_KEY: hits the real Poly Pizza API"]
fn polypizza_search_finds_knights_when_keyed() {
    if std::env::var("PLINTH_POLYPIZZA_KEY").is_err() {
        eprintln!("skipping: PLINTH_POLYPIZZA_KEY not set");
        return;
    }
    let output = plinth()
        .args(["assets", "search", "knight"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("polypizza:"), "{stdout}");
}
