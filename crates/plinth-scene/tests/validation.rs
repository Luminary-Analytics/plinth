//! Contract tests for the scene format: error-message quality is part of the
//! public API — agents repair files based on these strings.

use plinth_scene::{DiagKind, validate_str};

fn diags_for(src: &str) -> Vec<String> {
    let (_, diags) = validate_str(src);
    diags.iter().map(|d| d.to_string()).collect()
}

#[test]
fn minimal_scene_is_valid() {
    let (doc, diags) = validate_str(
        r#"{ "version": 1, "entities": [
            { "id": "thing", "components": { "transform": {} } }
        ] }"#,
    );
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    assert_eq!(doc.unwrap().entities[0].id, "thing");
}

#[test]
fn defaults_are_applied() {
    let (doc, diags) = validate_str(
        r#"{ "version": 1, "entities": [
            { "id": "hero", "components": { "character": { "player": true } } }
        ] }"#,
    );
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    let doc = doc.unwrap();
    let character = doc.entities[0].components.character.as_ref().unwrap();
    assert!(character.player);
    assert_eq!(character.speed, 6.0);
    assert_eq!(character.float_height, 1.25);
}

#[test]
fn syntax_error_reports_line() {
    let (doc, diags) = validate_str("{ \"version\": 1,\n  \"entities\": [ }");
    assert!(doc.is_none());
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].kind, DiagKind::Parse);
    assert_eq!(diags[0].line, Some(2));
}

#[test]
fn unknown_component_lists_vocabulary() {
    let rendered = diags_for(
        r#"{ "version": 1, "entities": [
            { "id": "x", "components": { "transfrom": {} } }
        ] }"#,
    );
    assert_eq!(rendered.len(), 1);
    assert!(
        rendered[0].contains("unknown field `transfrom`"),
        "{rendered:?}"
    );
    assert!(
        rendered[0].contains("`transform`"),
        "should list valid names: {rendered:?}"
    );
}

#[test]
fn unknown_shape_variant_lists_options() {
    let rendered = diags_for(
        r#"{ "version": 1, "entities": [
            { "id": "x", "components": { "shape": { "cubed": { "size": [1, 2, 3] } } } }
        ] }"#,
    );
    assert_eq!(rendered.len(), 1);
    assert!(
        rendered[0].contains("entities[0].components.shape"),
        "{rendered:?}"
    );
    assert!(
        rendered[0].contains("unknown variant `cubed`"),
        "{rendered:?}"
    );
    assert!(rendered[0].contains("cuboid"), "{rendered:?}");
}

#[test]
fn unknown_field_inside_component_is_rejected() {
    let rendered = diags_for(
        r#"{ "version": 1, "entities": [
            { "id": "x", "components": { "transform": { "postion": [0, 0, 0] } } }
        ] }"#,
    );
    assert_eq!(rendered.len(), 1);
    assert!(
        rendered[0].contains("unknown field `postion`"),
        "{rendered:?}"
    );
}

#[test]
fn duplicate_ids_point_at_both_entities() {
    let rendered = diags_for(
        r#"{ "version": 1, "entities": [
            { "id": "rock", "components": { "transform": {} } },
            { "id": "rock", "components": { "transform": {} } }
        ] }"#,
    );
    assert_eq!(rendered.len(), 1);
    assert!(rendered[0].contains("entities[1].id"), "{rendered:?}");
    assert!(rendered[0].contains("entities[0]"), "{rendered:?}");
}

#[test]
fn bad_id_pattern_is_rejected() {
    let rendered = diags_for(
        r#"{ "version": 1, "entities": [
            { "id": "Rock One", "components": { "transform": {} } }
        ] }"#,
    );
    assert_eq!(rendered.len(), 1);
    assert!(
        rendered[0].contains("invalid id `Rock One`"),
        "{rendered:?}"
    );
}

#[test]
fn unsupported_version_is_rejected() {
    let rendered = diags_for(r#"{ "version": 2, "entities": [] }"#);
    assert_eq!(rendered.len(), 1);
    assert!(
        rendered[0].contains("unsupported scene format version 2"),
        "{rendered:?}"
    );
}

#[test]
fn empty_components_is_rejected() {
    let rendered = diags_for(
        r#"{ "version": 1, "entities": [
            { "id": "ghost", "components": {} }
        ] }"#,
    );
    assert_eq!(rendered.len(), 1);
    assert!(rendered[0].contains("no components"), "{rendered:?}");
}

#[test]
fn from_shape_requires_shape() {
    let rendered = diags_for(
        r#"{ "version": 1, "entities": [
            { "id": "wall", "components": { "rigid_body": "static", "collider": "from_shape" } }
        ] }"#,
    );
    assert_eq!(rendered.len(), 1);
    assert!(rendered[0].contains("requires a `shape`"), "{rendered:?}");
}

#[test]
fn dynamic_body_requires_collider() {
    let rendered = diags_for(
        r#"{ "version": 1, "entities": [
            { "id": "crate-1", "components": { "shape": { "cuboid": { "size": [1, 1, 1] } }, "rigid_body": "dynamic" } }
        ] }"#,
    );
    assert_eq!(rendered.len(), 1);
    assert!(rendered[0].contains("require a `collider`"), "{rendered:?}");
}

#[test]
fn character_conflicts_with_explicit_physics() {
    let rendered = diags_for(
        r#"{ "version": 1, "entities": [
            { "id": "hero", "components": { "character": {}, "rigid_body": "dynamic", "collider": "from_shape" } }
        ] }"#,
    );
    assert!(
        rendered
            .iter()
            .any(|d| d.contains("brings its own physics body")),
        "{rendered:?}"
    );
}

#[test]
fn numeric_ranges_are_enforced() {
    let rendered = diags_for(
        r##"{ "version": 1, "entities": [
            { "id": "cam", "components": { "camera3d": { "fov_degrees": 200 } } },
            { "id": "ball", "components": { "shape": { "sphere": { "radius": -1 } } } },
            { "id": "floor", "components": { "shape": { "plane": { "size": [10, 10] } }, "material": { "color": "#zzz" } } }
        ] }"##,
    );
    assert_eq!(rendered.len(), 3, "{rendered:?}");
    assert!(
        rendered[0].contains("fov_degrees must be within (0, 180)"),
        "{rendered:?}"
    );
    assert!(
        rendered[1].contains("sphere.radius must be positive"),
        "{rendered:?}"
    );
    assert!(rendered[2].contains("invalid color `#zzz`"), "{rendered:?}");
}

#[test]
fn float_height_must_clear_the_capsule() {
    let rendered = diags_for(
        r#"{ "version": 1, "entities": [
            { "id": "hero", "components": { "character": { "float_height": 0.5 } } }
        ] }"#,
    );
    assert_eq!(rendered.len(), 1);
    assert!(rendered[0].contains("float_height"), "{rendered:?}");
    assert!(rendered[0].contains("capsule"), "{rendered:?}");
}

#[test]
fn example_scenes_are_valid() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/scenes");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).expect("examples/scenes exists") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            let src = std::fs::read_to_string(&path).unwrap();
            let (_, diags) = validate_str(&src);
            assert!(diags.is_empty(), "{path:?} has diagnostics: {diags:?}");
            checked += 1;
        }
    }
    assert!(checked > 0, "no example scenes found in {dir}");
}

#[test]
fn committed_schema_matches_generated() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/scene.schema.json"
    );
    let committed: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect(
        "schemas/scene.schema.json exists — regenerate with `cargo run -p plinth-cli -- schema`",
    ))
    .unwrap();
    assert_eq!(
        committed,
        plinth_scene::schema_json(),
        "schemas/scene.schema.json is stale — regenerate with `cargo run -p plinth-cli -- schema > schemas/scene.schema.json`"
    );
}
