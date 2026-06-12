//! The asset manifest: provenance and license metadata for every asset a
//! project ships (`assets/manifest.json`). This is what makes it safe — for
//! humans and agents — to pull open-licensed content into a game: licenses
//! are tracked, attribution is generated, and unknowns get flagged.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diag::Diagnostic;

/// The manifest format version this build of Plinth understands.
pub const SUPPORTED_MANIFEST_VERSION: u32 = 1;

/// SPDX license identifiers Plinth recognizes. Anything else produces a
/// warning (not an error): double-check the asset's terms and use the
/// closest SPDX id.
pub const KNOWN_LICENSES: &[&str] = &[
    "CC0-1.0",
    "CC-BY-3.0",
    "CC-BY-4.0",
    "CC-BY-SA-3.0",
    "CC-BY-SA-4.0",
    "MIT",
    "Apache-2.0",
    "OFL-1.1",
    "Zlib",
    "BSD-3-Clause",
    "Unlicense",
];

/// `assets/manifest.json`: every external asset in the project, with its
/// license and provenance. `plinth credits` renders this into CREDITS.md.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetManifest {
    /// Manifest format version. This version of Plinth understands `1`.
    pub version: u32,

    /// Every tracked asset.
    pub assets: Vec<AssetEntry>,
}

/// One asset's provenance record.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetEntry {
    /// Path relative to the project's `assets/` directory, with forward
    /// slashes — the same path scene files reference.
    pub path: String,

    /// SPDX license identifier, e.g. `CC0-1.0`, `CC-BY-4.0`, `MIT`.
    pub license: String,

    /// Where this asset came from (URL). Strongly recommended — provenance
    /// is the point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Creator name for attribution. Required in practice by `CC-BY`
    /// licenses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Human-readable asset name for the credits file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Parse a manifest from JSON source.
pub fn parse_manifest_str(src: &str) -> Result<AssetManifest, Diagnostic> {
    let mut de = serde_json::Deserializer::from_str(src);
    serde_path_to_error::deserialize::<_, AssetManifest>(&mut de).map_err(|err| {
        let location = match err.path().to_string() {
            path if path == "." => String::new(),
            path => path,
        };
        let inner = err.into_inner();
        Diagnostic {
            kind: crate::DiagKind::Parse,
            severity: crate::Severity::Error,
            location,
            line: Some(inner.line()).filter(|&l| l > 0),
            column: Some(inner.column()).filter(|&c| c > 0),
            message: crate::strip_position_suffix(&inner.to_string()),
        }
    })
}

/// Parse and validate a manifest, returning the document (when it parsed)
/// and every finding.
pub fn validate_manifest_str(src: &str) -> (Option<AssetManifest>, Vec<Diagnostic>) {
    match parse_manifest_str(src) {
        Ok(doc) => {
            let diags = validate_manifest(&doc);
            (Some(doc), diags)
        }
        Err(diag) => (None, vec![diag]),
    }
}

/// Semantic rules for a parsed manifest.
pub fn validate_manifest(doc: &AssetManifest) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    if doc.version != SUPPORTED_MANIFEST_VERSION {
        diags.push(Diagnostic::semantic(
            "version",
            format!(
                "unsupported manifest version {}; this Plinth supports {SUPPORTED_MANIFEST_VERSION}",
                doc.version
            ),
        ));
    }

    let mut seen: Vec<(&str, usize)> = Vec::new();
    for (i, entry) in doc.assets.iter().enumerate() {
        let at = |suffix: &str| format!("assets[{i}]{suffix}");

        if let Some(problem) = invalid_asset_path(&entry.path) {
            diags.push(Diagnostic::semantic(at(".path"), problem));
        }
        if let Some((_, first)) = seen.iter().find(|(p, _)| *p == entry.path) {
            diags.push(Diagnostic::semantic(
                at(".path"),
                format!(
                    "duplicate path `{}` (already listed at assets[{first}])",
                    entry.path
                ),
            ));
        } else {
            seen.push((&entry.path, i));
        }

        if entry.license.trim().is_empty() {
            diags.push(Diagnostic::semantic(
                at(".license"),
                "license is required; use an SPDX id like CC0-1.0".to_owned(),
            ));
        } else if !KNOWN_LICENSES.contains(&entry.license.as_str()) {
            diags.push(Diagnostic::warning(
                at(".license"),
                format!(
                    "unknown license `{}`; known ids: {}. Double-check the asset's terms",
                    entry.license,
                    KNOWN_LICENSES.join(", ")
                ),
            ));
        }

        if entry.license.starts_with("CC-BY") && entry.author.is_none() {
            diags.push(Diagnostic::warning(
                at(""),
                format!(
                    "`{}` requires attribution but `author` is missing",
                    entry.license
                ),
            ));
        }

        if let Some(source) = &entry.source
            && !(source.starts_with("https://") || source.starts_with("http://"))
        {
            diags.push(Diagnostic::warning(
                at(".source"),
                format!("source `{source}` is not a URL"),
            ));
        }
    }

    diags
}

/// Why an asset path is unacceptable, if it is.
pub(crate) fn invalid_asset_path(path: &str) -> Option<String> {
    if path.is_empty() {
        return Some("path must not be empty".to_owned());
    }
    if path.contains('\\') {
        return Some(format!("path `{path}` must use forward slashes"));
    }
    if path.starts_with('/') || path.contains(':') {
        return Some(format!(
            "path `{path}` must be relative to the assets/ directory"
        ));
    }
    if path.split('/').any(|segment| segment == "..") {
        return Some(format!("path `{path}` must not contain `..`"));
    }
    None
}

/// Render the manifest as a Markdown credits file, grouped by license.
/// Deterministic: same manifest, same output.
pub fn render_credits(manifest: &AssetManifest) -> String {
    let mut by_license: Vec<(&str, Vec<&AssetEntry>)> = Vec::new();
    for entry in &manifest.assets {
        match by_license.iter_mut().find(|(l, _)| *l == entry.license) {
            Some((_, list)) => list.push(entry),
            None => by_license.push((&entry.license, vec![entry])),
        }
    }
    by_license.sort_by_key(|(license, _)| *license);

    let mut out = String::from(
        "# Credits\n\nThis project uses the following third-party assets. \
         Generated by `plinth credits` from `assets/manifest.json` — edit the \
         manifest, not this file.\n",
    );
    for (license, mut entries) in by_license {
        entries.sort_by_key(|e| e.path.as_str());
        out.push_str(&format!("\n## {license}\n\n"));
        for entry in entries {
            let title = entry.title.as_deref().unwrap_or(&entry.path);
            out.push_str(&format!("- **{title}** (`{}`)", entry.path));
            if let Some(author) = &entry.author {
                out.push_str(&format!(" by {author}"));
            }
            if let Some(source) = &entry.source {
                out.push_str(&format!(" — <{source}>"));
            }
            out.push('\n');
        }
    }
    out
}

/// The JSON Schema for [`AssetManifest`], published as
/// `schemas/manifest.schema.json`.
pub fn manifest_schema_json() -> serde_json::Value {
    let schema = schemars::schema_for!(AssetManifest);
    serde_json::to_value(schema).expect("schema serializes")
}
