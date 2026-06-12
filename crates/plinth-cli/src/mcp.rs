//! `plinth mcp`: an MCP (Model Context Protocol) stdio server that bridges a
//! coding agent to a *running* Plinth game's playtest API.
//!
//! Transport: newline-delimited JSON-RPC 2.0 on stdin/stdout (the MCP stdio
//! transport). Game side: the BRP HTTP server every debug-build Plinth game
//! serves (see `Game::playtest_on_port`).

use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{Value, json};

pub fn serve(game_url: String) -> std::process::ExitCode {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        let method = message["method"].as_str().unwrap_or_default().to_owned();
        let id = message.get("id").cloned();

        // Notifications (no id) never get responses.
        let Some(id) = id else { continue };

        let result = match method.as_str() {
            "initialize" => Ok(initialize_result(&message)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => Ok(handle_tool_call(&game_url, &message["params"])),
            _ => Err(json!({ "code": -32601, "message": format!("method not found: {method}") })),
        };

        let response = match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        };
        let Ok(serialized) = serde_json::to_string(&response) else {
            continue;
        };
        if writeln!(stdout, "{serialized}")
            .and_then(|()| stdout.flush())
            .is_err()
        {
            break;
        }
    }
    std::process::ExitCode::SUCCESS
}

fn initialize_result(request: &Value) -> Value {
    let requested = request["params"]["protocolVersion"]
        .as_str()
        .unwrap_or("2024-11-05");
    json!({
        "protocolVersion": requested,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "plinth-playtest",
            "version": env!("CARGO_PKG_VERSION"),
        },
    })
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "scene_entities",
            "description": "Snapshot of the running game: every scene-spawned entity with its stable id, world position, and whether it is the player character, plus whether the clock is paused. Start here to see what exists and where things are.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "inject_input",
            "description": "Drive the game at the device level, exactly like a physical keyboard/mouse. Pressed keys stay held until released. Keys use names like KeyW, Space, ShiftLeft, ArrowUp, Digit1 (single letters like \"W\" also work). mouse_look orbits the camera by [dx, dy] pixels.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "press": { "type": "array", "items": { "type": "string" }, "description": "Keys to press and hold" },
                    "release": { "type": "array", "items": { "type": "string" }, "description": "Keys to release" },
                    "mouse_look": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": "Camera look delta [dx, dy]" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "game_time",
            "description": "Pause, resume, or single-step the simulation. step requires pause first and advances exactly `ticks` fixed-timestep ticks (60 per second), letting you inspect the world between precise slices of time.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["pause", "resume", "step"] },
                    "ticks": { "type": "integer", "minimum": 1, "maximum": 600, "description": "Fixed ticks to advance (step only, default 1)" }
                },
                "required": ["action"],
                "additionalProperties": false
            }
        },
        {
            "name": "screenshot",
            "description": "Capture what the game window currently shows and return it as an image. Requires the windowed game (cargo run); headless games refuse.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "query",
            "description": "Raw Bevy Remote Protocol passthrough for anything the golden-path tools don't cover: call any BRP method, e.g. method=\"bevy/query\" with params={\"data\":{\"components\":[\"bevy_transform::components::transform::Transform\"]}} to query arbitrary ECS state.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "method": { "type": "string", "description": "BRP method name, e.g. bevy/query, bevy/list" },
                    "params": { "type": "object", "description": "Method parameters" }
                },
                "required": ["method"],
                "additionalProperties": false
            }
        },
        {
            "name": "search_assets",
            "description": "Search open-licensed 3D model libraries (PolyHaven: CC0 photoreal props; Poly Pizza: CC0/CC-BY low-poly game models, characters, weapons). Works without the game running. Returns source-qualified ids for add_asset.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What to look for, e.g. \"barrel\", \"knight\"" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50, "description": "Max results (default 20)" }
                },
                "required": ["query"],
                "additionalProperties": false
            }
        },
        {
            "name": "add_asset",
            "description": "Download a searched asset into assets/, record its license and provenance in assets/manifest.json, and regenerate CREDITS.md. Returns the model path and a ready-to-paste scene component — paste it into a *.scene.json and the running game hot-reloads it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": { "type": "string", "enum": ["polyhaven", "polypizza"], "description": "Source from search results" },
                    "id": { "type": "string", "description": "Asset id from search results" }
                },
                "required": ["source", "id"],
                "additionalProperties": false
            }
        }
    ])
}

fn handle_tool_call(game_url: &str, params: &Value) -> Value {
    let name = params["name"].as_str().unwrap_or_default();
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let outcome = match name {
        "scene_entities" => brp(game_url, "plinth/scene", json!({})).map(text_content),
        "inject_input" => brp(game_url, "plinth/input", args).map(text_content),
        "game_time" => brp(game_url, "plinth/time", args).map(text_content),
        "screenshot" => capture_screenshot(game_url),
        "query" => {
            let method = args["method"].as_str().unwrap_or_default().to_owned();
            let params = args.get("params").cloned().unwrap_or_else(|| json!({}));
            brp(game_url, &method, params).map(text_content)
        }
        "search_assets" => {
            let query = args["query"].as_str().unwrap_or_default();
            let limit = args["limit"].as_u64().unwrap_or(20) as usize;
            let (hits, notes) = crate::sources::search_all(query, limit);
            let hits: Vec<Value> = hits.iter().map(crate::sources::AssetHit::to_json).collect();
            Ok(text_content(json!({
                "hits": hits,
                "notes": notes,
                "next": "fetch one with the add_asset tool",
            })))
        }
        "add_asset" => {
            let source = args["source"].as_str().unwrap_or_default();
            let id = args["id"].as_str().unwrap_or_default();
            crate::sources::add_asset(source, id, std::path::Path::new(".")).map(|outcome| {
                text_content(json!({
                    "path": outcome.asset_path,
                    "license": outcome.license,
                    "files_downloaded": outcome.files_downloaded,
                    "manifest": "assets/manifest.json updated; CREDITS.md regenerated",
                    "scene_snippet": outcome.scene_snippet,
                }))
            })
        }
        other => Err(format!("unknown tool `{other}`")),
    };

    // Game-facing tools fail when the game isn't running; asset tools fail on
    // network/key problems. Both come back as agent-actionable text.
    let hint = matches!(name, "search_assets" | "add_asset");
    match outcome {
        Ok(content) => json!({ "content": [content] }),
        Err(message) if hint => json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true,
        }),
        Err(message) => json!({
            "content": [{ "type": "text", "text": format!(
                "{message}\n\nIs the game running? Start it with `cargo run` (debug builds serve the playtest API on port 15702)."
            ) }],
            "isError": true,
        }),
    }
}

fn text_content(value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    json!({ "type": "text", "text": text })
}

fn capture_screenshot(game_url: &str) -> Result<Value, String> {
    let path = std::env::temp_dir().join(format!("plinth-screenshot-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&path);

    brp(
        game_url,
        "plinth/screenshot",
        json!({ "path": path.display().to_string() }),
    )?;

    // Capture is asynchronous; the file appears once the next frame renders.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(bytes) = std::fs::read(&path)
            && !bytes.is_empty()
        {
            let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Ok(json!({ "type": "image", "data": data, "mimeType": "image/png" }));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("screenshot was triggered but no image appeared within 5s".into())
}

/// One JSON-RPC call to the game's BRP server.
fn brp(game_url: &str, method: &str, params: Value) -> Result<Value, String> {
    let response: Value = ureq::post(game_url)
        .timeout(Duration::from_secs(10))
        .send_json(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .map_err(|err| format!("cannot reach the game at {game_url}: {err}"))?
        .into_json()
        .map_err(|err| format!("invalid response from the game: {err}"))?;
    if let Some(error) = response.get("error") {
        let message = error["message"].as_str().unwrap_or("unknown error");
        return Err(format!("game refused `{method}`: {message}"));
    }
    Ok(response["result"].clone())
}
