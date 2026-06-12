//! # plinth-scene
//!
//! Plinth's scene data format: the stable, schema-published contract for
//! `*.scene.json` files. This crate is deliberately **Bevy-free** so that
//! tooling built on it (the `plinth` CLI, editors, agents) compiles and runs
//! in milliseconds — validating a scene never costs an engine build.
//!
//! The JSON Schema (`schemas/scene.schema.json` in the repository) is
//! generated from these exact types via [`schema_json`], so the schema and
//! the loader cannot drift apart.
//!
//! ```
//! let scene = r#"{
//!     "version": 1,
//!     "entities": [
//!         { "id": "sun", "components": { "light": { "directional": {} } } }
//!     ]
//! }"#;
//! let (doc, diagnostics) = plinth_scene::validate_str(scene);
//! assert!(diagnostics.is_empty());
//! assert_eq!(doc.unwrap().entities.len(), 1);
//! ```
//!
//! Validation reports *all* findings at once, with paths and line numbers an
//! agent can act on without guessing:
//!
//! ```
//! let broken = r#"{ "version": 1, "entities": [
//!     { "id": "crate", "components": { "shape": { "cubed": { "size": [1, 1, 1] } } } }
//! ] }"#;
//! let (_, diagnostics) = plinth_scene::validate_str(broken);
//! let rendered = diagnostics[0].to_string();
//! assert!(rendered.contains("entities[0].components.shape"));
//! assert!(rendered.contains("unknown variant `cubed`"));
//! ```

mod diag;
mod manifest;
mod types;
mod validate;

pub use diag::{DiagKind, Diagnostic, Severity};
pub use manifest::{
    AssetEntry, AssetManifest, KNOWN_LICENSES, SUPPORTED_MANIFEST_VERSION, manifest_schema_json,
    parse_manifest_str, render_credits, validate_manifest, validate_manifest_str,
};
pub use types::{
    Camera3dDef, CharacterDef, ColliderDef, ComponentsDef, EntityDef, LightDef, MaterialDef,
    ModelDef, RigidBodyDef, SceneDoc, ShapeDef, TransformDef,
};
pub use validate::{SUPPORTED_VERSION, parse_hex_color, validate_doc};

/// Parse a scene document from JSON source, reporting the failure with a
/// document path and line/column when it does not match the format.
pub fn parse_str(src: &str) -> Result<SceneDoc, Diagnostic> {
    let mut de = serde_json::Deserializer::from_str(src);
    serde_path_to_error::deserialize::<_, SceneDoc>(&mut de).map_err(|err| {
        let location = match err.path().to_string() {
            // serde_path_to_error renders an unlocatable path as ".".
            path if path == "." => String::new(),
            path => path,
        };
        let inner = err.into_inner();
        Diagnostic {
            kind: DiagKind::Parse,
            severity: Severity::Error,
            location,
            line: Some(inner.line()).filter(|&l| l > 0),
            column: Some(inner.column()).filter(|&c| c > 0),
            message: strip_position_suffix(&inner.to_string()),
        }
    })
}

/// Parse and semantically validate a scene document. Returns the document
/// (when it parsed) and every diagnostic found.
pub fn validate_str(src: &str) -> (Option<SceneDoc>, Vec<Diagnostic>) {
    match parse_str(src) {
        Ok(doc) => {
            let diags = validate_doc(&doc);
            (Some(doc), diags)
        }
        Err(diag) => (None, vec![diag]),
    }
}

/// The JSON Schema for [`SceneDoc`], generated from the Rust types. This is
/// the artifact published as `schemas/scene.schema.json`.
pub fn schema_json() -> serde_json::Value {
    let schema = schemars::schema_for!(SceneDoc);
    serde_json::to_value(schema).expect("schema serializes")
}

/// serde_json appends " at line N column M" to messages; we carry position
/// structurally instead, so drop the suffix rather than say it twice.
pub(crate) fn strip_position_suffix(message: &str) -> String {
    match message.rfind(" at line ") {
        Some(idx) => message[..idx].to_owned(),
        None => message.to_owned(),
    }
}
