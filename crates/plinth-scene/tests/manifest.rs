//! Contract tests for the asset manifest: licenses tracked, attribution
//! enforced, credits deterministic.

use plinth_scene::{Severity, render_credits, validate_manifest_str};

fn diags_for(src: &str) -> Vec<(Severity, String)> {
    let (_, diags) = validate_manifest_str(src);
    diags.iter().map(|d| (d.severity, d.to_string())).collect()
}

#[test]
fn empty_manifest_is_valid() {
    let (doc, diags) = validate_manifest_str(r#"{ "version": 1, "assets": [] }"#);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(doc.unwrap().assets.len(), 0);
}

#[test]
fn full_entry_is_valid() {
    let (_, diags) = validate_manifest_str(
        r#"{ "version": 1, "assets": [
            { "path": "models/knight.glb", "license": "CC0-1.0",
              "source": "https://kenney.nl/assets/example", "author": "Kenney",
              "title": "Knight" }
        ] }"#,
    );
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn unknown_license_is_a_warning_not_an_error() {
    let diags = diags_for(
        r#"{ "version": 1, "assets": [
            { "path": "models/x.glb", "license": "Custom-EULA" }
        ] }"#,
    );
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].0, Severity::Warning, "{diags:?}");
    assert!(
        diags[0].1.contains("unknown license `Custom-EULA`"),
        "{diags:?}"
    );
    assert!(
        diags[0].1.contains("CC0-1.0"),
        "should list known ids: {diags:?}"
    );
}

#[test]
fn cc_by_without_author_warns_about_attribution() {
    let diags = diags_for(
        r#"{ "version": 1, "assets": [
            { "path": "models/tree.glb", "license": "CC-BY-4.0" }
        ] }"#,
    );
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].0, Severity::Warning);
    assert!(diags[0].1.contains("requires attribution"), "{diags:?}");
}

#[test]
fn bad_paths_are_errors() {
    let diags = diags_for(
        r#"{ "version": 1, "assets": [
            { "path": "../outside.glb", "license": "CC0-1.0" },
            { "path": "/abs/path.glb", "license": "CC0-1.0" },
            { "path": "models/dup.glb", "license": "CC0-1.0" },
            { "path": "models/dup.glb", "license": "CC0-1.0" }
        ] }"#,
    );
    let errors: Vec<&String> = diags
        .iter()
        .filter(|(s, _)| *s == Severity::Error)
        .map(|(_, m)| m)
        .collect();
    assert_eq!(errors.len(), 3, "{diags:?}");
    assert!(errors[0].contains("must not contain `..`"), "{errors:?}");
    assert!(errors[1].contains("must be relative"), "{errors:?}");
    assert!(errors[2].contains("duplicate path"), "{errors:?}");
}

#[test]
fn credits_render_grouped_and_deterministic() {
    let (doc, diags) = validate_manifest_str(
        r#"{ "version": 1, "assets": [
            { "path": "models/tree.glb", "license": "CC-BY-4.0",
              "author": "Quaternius", "title": "Low Poly Tree",
              "source": "https://quaternius.com/" },
            { "path": "models/knight.glb", "license": "CC0-1.0", "author": "Kenney" },
            { "path": "audio/hit.ogg", "license": "CC0-1.0" }
        ] }"#,
    );
    assert!(diags.is_empty(), "{diags:?}");
    let credits = render_credits(&doc.unwrap());

    let cc0_pos = credits.find("## CC0-1.0").expect("CC0 section");
    let ccby_pos = credits.find("## CC-BY-4.0").expect("CC-BY section");
    assert!(
        ccby_pos < cc0_pos,
        "licenses sorted alphabetically:\n{credits}"
    );
    assert!(
        credits.contains(
            "**Low Poly Tree** (`models/tree.glb`) by Quaternius — <https://quaternius.com/>"
        ),
        "{credits}"
    );
    // Entries without a title fall back to the path.
    assert!(credits.contains("**audio/hit.ogg**"), "{credits}");
    assert!(
        credits.contains("edit the\nmanifest, not this file")
            || credits.contains("edit the manifest"),
        "{credits}"
    );
}

#[test]
fn committed_manifest_schema_matches_generated() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/manifest.schema.json"
    );
    let committed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect(
            "schemas/manifest.schema.json exists — regenerate with `plinth schema manifest`",
        ))
        .unwrap();
    assert_eq!(
        committed,
        plinth_scene::manifest_schema_json(),
        "schemas/manifest.schema.json is stale — regenerate with `cargo run -p plinth-cli -- schema manifest > schemas/manifest.schema.json`"
    );
}
