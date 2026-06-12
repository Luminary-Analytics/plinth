//! Playtest API integration test: a real HTTP round-trip against a running
//! headless game — the transport `plinth mcp` builds on.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use plinth::prelude::*;
use serde_json::{Value, json};

const PORT: u16 = 15760;

const SCENE: &str = r#"{ "version": 1, "entities": [
    { "id": "ground", "components": {
        "shape": { "cuboid": { "size": [60, 1, 60] } },
        "transform": { "position": [0, -0.5, 0] },
        "rigid_body": "static", "collider": "from_shape" } },
    { "id": "hero", "components": {
        "transform": { "position": [0, 2, 0] },
        "character": { "player": true } } }
] }"#;

fn brp(method: &str, params: Value) -> Result<Value, String> {
    let response: Value = ureq::post(&format!("http://127.0.0.1:{PORT}"))
        .send_json(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    if let Some(err) = response.get("error") {
        return Err(err.to_string());
    }
    Ok(response["result"].clone())
}

fn brp_with_retry(method: &str, params: Value) -> Value {
    // The HTTP listener comes up on the game's first update; retry briefly.
    for _ in 0..100 {
        if let Ok(result) = brp(method, params.clone()) {
            return result;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("playtest server never answered `{method}`");
}

fn hero_position(scene: &Value) -> Vec3 {
    let entity = scene["entities"]
        .as_array()
        .expect("entities array")
        .iter()
        .find(|e| e["id"] == "hero")
        .expect("hero exists");
    let p = entity["position"].as_array().expect("position triple");
    Vec3::new(
        p[0].as_f64().unwrap() as f32,
        p[1].as_f64().unwrap() as f32,
        p[2].as_f64().unwrap() as f32,
    )
}

#[test]
fn playtest_api_observes_and_drives_the_game() {
    let path = std::env::temp_dir().join("plinth-playtest-test.scene.json");
    std::fs::write(&path, SCENE).unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let stop_in_game = stop.clone();
    let game_thread = std::thread::spawn(move || {
        let mut game = Game::headless().level(&path).playtest_on_port(PORT);
        while !stop_in_game.load(Ordering::Relaxed) {
            game.update();
            std::thread::sleep(Duration::from_millis(2));
        }
    });

    // Observe: the scene snapshot sees both entities and the player flag.
    let scene = brp_with_retry("plinth/scene", json!({}));
    assert_eq!(scene["entities"].as_array().unwrap().len(), 2);
    assert!(
        scene["entities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == "hero" && e["player"] == true),
        "{scene}"
    );

    // Let the hero settle, then drive it: press W over the wire.
    std::thread::sleep(Duration::from_millis(400));
    let before = hero_position(&brp_with_retry("plinth/scene", json!({})));
    let applied = brp_with_retry("plinth/input", json!({ "press": ["KeyW"] }));
    assert_eq!(applied["pressed"][0], "KeyW", "{applied}");
    std::thread::sleep(Duration::from_millis(600));
    let after = hero_position(&brp_with_retry("plinth/scene", json!({})));
    assert!(
        before.z - after.z > 0.5,
        "hero should walk -Z while W is held: before {before}, after {after}"
    );

    // Pause freezes the world even with W still held...
    let paused = brp_with_retry("plinth/time", json!({ "action": "pause" }));
    assert_eq!(paused["paused"], true);
    let frozen_a = hero_position(&brp_with_retry("plinth/scene", json!({})));
    std::thread::sleep(Duration::from_millis(300));
    let frozen_b = hero_position(&brp_with_retry("plinth/scene", json!({})));
    assert!(
        frozen_a.distance(frozen_b) < 0.01,
        "paused world must not move: {frozen_a} vs {frozen_b}"
    );

    // ...and stepping advances exactly a bounded slice of time.
    brp_with_retry("plinth/time", json!({ "action": "step", "ticks": 30 }));
    std::thread::sleep(Duration::from_millis(300));
    let stepped = hero_position(&brp_with_retry("plinth/scene", json!({})));
    let moved = frozen_b.distance(stepped);
    assert!(
        moved > 0.2 && moved < 6.0,
        "30 stepped ticks at walk speed should move ~3m, got {moved}"
    );

    // Bad input is an agent-actionable error, not a crash.
    let err = brp("plinth/input", json!({ "press": ["KeyQQ"] })).unwrap_err();
    assert!(err.contains("unknown key"), "{err}");

    // Headless games refuse screenshots with a clear message.
    let err = brp("plinth/screenshot", json!({})).unwrap_err();
    assert!(err.contains("headless"), "{err}");

    stop.store(true, Ordering::Relaxed);
    game_thread.join().unwrap();
}
