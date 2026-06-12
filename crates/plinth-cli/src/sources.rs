//! Open asset sources: search and fetch from CC0/CC-BY libraries, recording
//! every download in the asset manifest and regenerating CREDITS.md.
//!
//! v1 sources:
//! - **PolyHaven** (no key): CC0 photoreal environment props; multi-file
//!   glTF + textures, downloaded into `assets/models/<id>/`.
//! - **Poly Pizza** (free API key via [`POLYPIZZA_KEY_ENV`]): CC0/CC-BY
//!   low-poly game models (characters, weapons, creatures); single GLB.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use plinth_scene::{AssetEntry, AssetManifest};
use serde_json::{Value, json};

/// Environment variable holding the Poly Pizza API key (free; create one at
/// <https://poly.pizza/settings/api>).
pub const POLYPIZZA_KEY_ENV: &str = "PLINTH_POLYPIZZA_KEY";

const POLYPIZZA_KEY_HINT: &str = "Poly Pizza needs a free API key: create one at https://poly.pizza/settings/api and set PLINTH_POLYPIZZA_KEY";

/// One search result, source-qualified so `add` can fetch it.
#[derive(Debug, Clone)]
pub struct AssetHit {
    pub source: &'static str,
    pub id: String,
    pub title: String,
    pub author: String,
    pub license: String,
    pub polycount: Option<u64>,
    pub page: String,
}

impl AssetHit {
    pub fn to_json(&self) -> Value {
        json!({
            "source": self.source,
            "id": self.id,
            "title": self.title,
            "author": self.author,
            "license": self.license,
            "polycount": self.polycount,
            "page": self.page,
        })
    }
}

/// Everything `add` did, for reporting back to humans and agents.
#[derive(Debug)]
pub struct AddOutcome {
    /// Manifest-relative path (what scenes reference).
    pub asset_path: String,
    pub license: String,
    pub files_downloaded: usize,
    /// Ready-to-paste scene component.
    pub scene_snippet: String,
}

/// Search every available source. Returns hits plus notes about sources
/// that were skipped (e.g. missing Poly Pizza key).
pub fn search_all(query: &str, limit: usize) -> (Vec<AssetHit>, Vec<String>) {
    let mut hits = Vec::new();
    let mut notes = Vec::new();

    match polyhaven_catalog() {
        Ok(catalog) => hits.extend(filter_polyhaven(&catalog, query)),
        Err(err) => notes.push(format!("polyhaven search failed: {err}")),
    }

    match std::env::var(POLYPIZZA_KEY_ENV) {
        Ok(key) if !key.trim().is_empty() => match polypizza_search(query, &key) {
            Ok(pp) => hits.extend(pp),
            Err(err) => notes.push(format!("polypizza search failed: {err}")),
        },
        _ => notes.push(format!("polypizza skipped: {POLYPIZZA_KEY_HINT}")),
    }

    hits.truncate(limit);
    (hits, notes)
}

/// Download an asset into `<project_root>/assets/`, record it in the
/// manifest, and regenerate CREDITS.md.
pub fn add_asset(source: &str, id: &str, project_root: &Path) -> Result<AddOutcome, String> {
    let (relative_path, entry, files_downloaded) = match source {
        "polyhaven" => polyhaven_add(id, project_root)?,
        "polypizza" => polypizza_add(id, project_root)?,
        other => {
            return Err(format!(
                "unknown source `{other}`; available: polyhaven, polypizza"
            ));
        }
    };

    let license = entry.license.clone();
    record_in_manifest(project_root, entry)?;

    Ok(AddOutcome {
        scene_snippet: format!("\"model\": {{ \"path\": \"{relative_path}\" }}"),
        asset_path: relative_path,
        license,
        files_downloaded,
    })
}

// ---------------------------------------------------------------------------
// PolyHaven

fn polyhaven_catalog() -> Result<Value, String> {
    http_json("https://api.polyhaven.com/assets?type=models")
}

/// Pure: filter the catalog by a case-insensitive query over id, name,
/// tags, and categories.
fn filter_polyhaven(catalog: &Value, query: &str) -> Vec<AssetHit> {
    let needle = query.to_lowercase();
    let Some(map) = catalog.as_object() else {
        return Vec::new();
    };
    let mut hits: Vec<AssetHit> = map
        .iter()
        .filter(|(id, meta)| {
            let mut haystack = id.to_lowercase();
            if let Some(name) = meta["name"].as_str() {
                haystack.push_str(&name.to_lowercase());
            }
            for list in ["tags", "categories"] {
                if let Some(values) = meta[list].as_array() {
                    for v in values {
                        if let Some(s) = v.as_str() {
                            haystack.push_str(&s.to_lowercase());
                        }
                    }
                }
            }
            haystack.contains(&needle)
        })
        .map(|(id, meta)| AssetHit {
            source: "polyhaven",
            id: id.clone(),
            title: meta["name"].as_str().unwrap_or(id).to_owned(),
            author: meta["authors"]
                .as_object()
                .and_then(|a| a.keys().next().cloned())
                .unwrap_or_else(|| "Poly Haven".to_owned()),
            // Everything on PolyHaven is CC0: https://polyhaven.com/license
            license: "CC0-1.0".to_owned(),
            polycount: meta["polycount"].as_u64(),
            page: format!("https://polyhaven.com/a/{id}"),
        })
        .collect();
    hits.sort_by(|a, b| a.id.cmp(&b.id));
    hits
}

/// Pure: extract the 1k glTF file set (main file + texture includes) from a
/// PolyHaven `/files/{id}` response as (relative path, url) pairs.
fn polyhaven_gltf_files(files: &Value) -> Result<Vec<(String, String)>, String> {
    let gltf = files["gltf"]["1k"]["gltf"]
        .as_object()
        .ok_or("this asset has no 1k glTF variant")?;
    let main_url = gltf["url"]
        .as_str()
        .ok_or("malformed PolyHaven response: missing gltf url")?;
    let main_name = main_url
        .rsplit('/')
        .next()
        .unwrap_or("model.gltf")
        .to_owned();

    let mut out = vec![(main_name, main_url.to_owned())];
    if let Some(include) = gltf.get("include").and_then(Value::as_object) {
        for (relative, meta) in include {
            let url = meta["url"]
                .as_str()
                .ok_or("malformed PolyHaven response: include without url")?;
            if relative.split('/').any(|s| s == "..") {
                return Err(format!("refusing include path with traversal: {relative}"));
            }
            out.push((relative.clone(), url.to_owned()));
        }
    }
    Ok(out)
}

fn polyhaven_add(id: &str, project_root: &Path) -> Result<(String, AssetEntry, usize), String> {
    let catalog = polyhaven_catalog()?;
    let meta = catalog
        .get(id)
        .ok_or_else(|| format!("polyhaven has no model `{id}`; try `assets search` first"))?
        .clone();
    let files = http_json(&format!("https://api.polyhaven.com/files/{id}"))?;
    let downloads = polyhaven_gltf_files(&files)?;

    let asset_dir = project_root.join("assets").join("models").join(id);
    let mut main_file = String::new();
    for (relative, url) in &downloads {
        let target = asset_dir.join(relative);
        download_to(url, &target)?;
        if relative.ends_with(".gltf") || relative.ends_with(".glb") {
            main_file = relative.clone();
        }
    }
    if main_file.is_empty() {
        return Err("download succeeded but no glTF file was present".into());
    }

    let relative_path = format!("models/{id}/{main_file}");
    let entry = AssetEntry {
        path: relative_path.clone(),
        license: "CC0-1.0".to_owned(),
        source: Some(format!("https://polyhaven.com/a/{id}")),
        author: meta["authors"]
            .as_object()
            .and_then(|a| a.keys().next().cloned()),
        title: meta["name"].as_str().map(str::to_owned),
    };
    Ok((relative_path, entry, downloads.len()))
}

// ---------------------------------------------------------------------------
// Poly Pizza

fn polypizza_key() -> Result<String, String> {
    match std::env::var(POLYPIZZA_KEY_ENV) {
        Ok(key) if !key.trim().is_empty() => Ok(key),
        _ => Err(POLYPIZZA_KEY_HINT.to_owned()),
    }
}

fn polypizza_search(query: &str, key: &str) -> Result<Vec<AssetHit>, String> {
    let encoded: String = query
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect();
    let response = http_json_with_key(
        &format!("https://api.poly.pizza/v1.1/search/{encoded}"),
        key,
    )?;
    Ok(map_polypizza_results(&response))
}

/// Pure: map a Poly Pizza search response, tolerating both documented field
/// casings.
fn map_polypizza_results(response: &Value) -> Vec<AssetHit> {
    let results = response
        .get("results")
        .or_else(|| response.get("Results"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    results
        .iter()
        .filter_map(|m| {
            let field =
                |names: &[&str]| -> Option<Value> { names.iter().find_map(|n| m.get(*n)).cloned() };
            let id = field(&["Id", "ID", "id"])?;
            let id = match id {
                Value::String(s) => s,
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            let title = field(&["Title", "title"])?.as_str()?.to_owned();
            let author = field(&["Creator", "creator"])
                .and_then(|c| {
                    c.get("Username")
                        .or_else(|| c.get("username"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| "unknown".to_owned());
            let license = map_polypizza_license(
                field(&["Licence", "License", "licence", "license"]).as_ref(),
            );
            Some(AssetHit {
                source: "polypizza",
                page: format!("https://poly.pizza/m/{id}"),
                polycount: field(&["TriCount", "Tris", "triCount"]).and_then(|v| v.as_u64()),
                id,
                title,
                author,
                license,
            })
        })
        .collect()
}

/// Pure: Poly Pizza hosts CC0 and CC-BY 3.0 (Google Poly heritage) models.
fn map_polypizza_license(raw: Option<&Value>) -> String {
    let text = raw
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
        .to_uppercase();
    if text.contains("CC0") || text.contains('0') && text.contains("CC") {
        "CC0-1.0".to_owned()
    } else if text.contains("BY") {
        "CC-BY-3.0".to_owned()
    } else {
        // Surface whatever the source said; the manifest validator will warn.
        if text.is_empty() {
            "CC0-1.0".to_owned()
        } else {
            text
        }
    }
}

fn polypizza_add(id: &str, project_root: &Path) -> Result<(String, AssetEntry, usize), String> {
    let key = polypizza_key()?;
    let model = http_json_with_key(&format!("https://api.poly.pizza/v1.1/model/{id}"), &key)?;
    let hits = map_polypizza_results(&json!({ "results": [model] }));
    let hit = hits
        .first()
        .ok_or("unexpected Poly Pizza model response shape")?;
    let download = model
        .get("Download")
        .or_else(|| model.get("download"))
        .and_then(Value::as_str)
        .ok_or("model has no download URL")?;

    let file_name = format!("{}.glb", slug(&hit.title));
    let target = project_root.join("assets").join("models").join(&file_name);
    download_to(download, &target)?;

    let relative_path = format!("models/{file_name}");
    let entry = AssetEntry {
        path: relative_path.clone(),
        license: hit.license.clone(),
        source: Some(hit.page.clone()),
        author: Some(hit.author.clone()),
        title: Some(hit.title.clone()),
    };
    Ok((relative_path, entry, 1))
}

/// Pure: filesystem-friendly name from a title.
fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_end_matches('-').to_owned();
    if trimmed.is_empty() {
        "asset".to_owned()
    } else {
        trimmed
    }
}

// ---------------------------------------------------------------------------
// Manifest + credits bookkeeping

fn record_in_manifest(project_root: &Path, entry: AssetEntry) -> Result<(), String> {
    let manifest_path = project_root.join("assets").join("manifest.json");
    let mut manifest = if manifest_path.exists() {
        let src = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("cannot read {}: {e}", manifest_path.display()))?;
        plinth_scene::parse_manifest_str(&src)
            .map_err(|d| format!("{} is invalid: {d}", manifest_path.display()))?
    } else {
        AssetManifest {
            version: 1,
            assets: Vec::new(),
        }
    };

    manifest.assets.retain(|a| a.path != entry.path);
    manifest.assets.push(entry);
    manifest.assets.sort_by(|a, b| a.path.cmp(&b.path));

    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("manifest serializes") + "\n",
    )
    .map_err(|e| format!("cannot write {}: {e}", manifest_path.display()))?;

    std::fs::write(
        project_root.join("CREDITS.md"),
        plinth_scene::render_credits(&manifest),
    )
    .map_err(|e| format!("cannot write CREDITS.md: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP helpers

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("plinth/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn http_json(url: &str) -> Result<Value, String> {
    agent()
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| format!("invalid JSON from {url}: {e}"))
}

fn http_json_with_key(url: &str, key: &str) -> Result<Value, String> {
    let response = agent().get(url).set("X-Auth-Token", key).call();
    match response {
        Ok(r) => r
            .into_json()
            .map_err(|e| format!("invalid JSON from {url}: {e}")),
        Err(ureq::Error::Status(401 | 403, _)) => Err(format!(
            "Poly Pizza rejected the API key. {POLYPIZZA_KEY_HINT}"
        )),
        Err(e) => Err(e.to_string()),
    }
}

fn download_to(url: &str, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let response = agent().get(url).call().map_err(|e| e.to_string())?;
    let reader = response.into_reader();
    let mut bytes = Vec::new();
    reader
        .take(512 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("download failed for {url}: {e}"))?;
    std::fs::write(target, bytes).map_err(|e| format!("cannot write {}: {e}", target.display()))
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn polyhaven_fixture() -> Value {
        json!({
            "Barrel_01": {
                "name": "Barrel 01",
                "authors": { "Rico Cilliers": "All" },
                "categories": ["props"],
                "tags": ["wood", "container"],
                "polycount": 8000
            },
            "treasure_chest": {
                "name": "Treasure Chest",
                "authors": { "James Ray Cock": "All" },
                "categories": ["props"],
                "tags": ["wood", "gold"],
                "polycount": 12000
            },
            "rock_moss_set_01": {
                "name": "Rock Moss Set 01",
                "authors": { "Rob Tuytel": "All" },
                "categories": ["nature"],
                "tags": ["stone"],
                "polycount": 30000
            }
        })
    }

    #[test]
    fn polyhaven_filter_matches_name_tags_and_categories() {
        let catalog = polyhaven_fixture();
        let wood: Vec<String> = filter_polyhaven(&catalog, "wood")
            .into_iter()
            .map(|h| h.id)
            .collect();
        assert_eq!(wood, ["Barrel_01", "treasure_chest"]);

        let nature = filter_polyhaven(&catalog, "NATURE");
        assert_eq!(nature.len(), 1);
        assert_eq!(nature[0].id, "rock_moss_set_01");
        assert_eq!(nature[0].license, "CC0-1.0");
        assert_eq!(nature[0].author, "Rob Tuytel");
        assert_eq!(nature[0].polycount, Some(30000));
    }

    #[test]
    fn polyhaven_gltf_files_extracts_main_and_includes() {
        let files = json!({
            "gltf": { "1k": { "gltf": {
                "url": "https://dl.polyhaven.org/x/Barrel_01.gltf",
                "size": 100,
                "include": {
                    "textures/barrel_diff_1k.jpg": { "url": "https://dl.polyhaven.org/x/t.jpg" }
                }
            } } }
        });
        let list = polyhaven_gltf_files(&files).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].0, "Barrel_01.gltf");
        assert_eq!(list[1].0, "textures/barrel_diff_1k.jpg");
    }

    #[test]
    fn polyhaven_gltf_files_requires_1k_variant() {
        let err = polyhaven_gltf_files(&json!({ "gltf": {} })).unwrap_err();
        assert!(err.contains("no 1k glTF"), "{err}");
    }

    #[test]
    fn polypizza_mapping_tolerates_documented_shape() {
        let response = json!({ "results": [{
            "Id": "ABC123",
            "Title": "Knight",
            "Creator": { "Username": "Quaternius" },
            "Licence": "CC0",
            "Download": "https://static.poly.pizza/ABC123.glb",
            "TriCount": 1500
        }] });
        let hits = map_polypizza_results(&response);
        assert_eq!(hits.len(), 1);
        let h = &hits[0];
        assert_eq!(h.id, "ABC123");
        assert_eq!(h.title, "Knight");
        assert_eq!(h.author, "Quaternius");
        assert_eq!(h.license, "CC0-1.0");
        assert_eq!(h.polycount, Some(1500));
        assert_eq!(h.page, "https://poly.pizza/m/ABC123");
    }

    #[test]
    fn polypizza_license_mapping() {
        assert_eq!(map_polypizza_license(Some(&json!("CC0"))), "CC0-1.0");
        assert_eq!(map_polypizza_license(Some(&json!("CC-BY"))), "CC-BY-3.0");
        assert_eq!(map_polypizza_license(None), "CC0-1.0");
    }

    #[test]
    fn slug_is_filesystem_friendly() {
        assert_eq!(slug("Low Poly Knight (Blue)"), "low-poly-knight-blue");
        assert_eq!(slug("***"), "asset");
    }

    #[test]
    fn manifest_recording_is_idempotent_and_writes_credits() {
        let dir = std::env::temp_dir().join("plinth-sources-manifest-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let entry = AssetEntry {
            path: "models/knight.glb".into(),
            license: "CC0-1.0".into(),
            source: Some("https://poly.pizza/m/X".into()),
            author: Some("Quaternius".into()),
            title: Some("Knight".into()),
        };
        record_in_manifest(&dir, entry.clone()).unwrap();
        record_in_manifest(&dir, entry).unwrap();

        let manifest: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("assets/manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            manifest["assets"].as_array().unwrap().len(),
            1,
            "{manifest}"
        );
        let credits = std::fs::read_to_string(dir.join("CREDITS.md")).unwrap();
        assert!(
            credits.contains("**Knight** (`models/knight.glb`) by Quaternius"),
            "{credits}"
        );
    }
}
