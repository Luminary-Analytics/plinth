//! End-to-end agent-loop test: a fake agent speaks MCP over stdio to the
//! real `plinth mcp` binary, which drives a real running (headless) game.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

const PORT: u16 = 15761;

const SCENE: &str = r#"{ "version": 1, "entities": [
    { "id": "ground", "components": {
        "shape": { "cuboid": { "size": [60, 1, 60] } },
        "transform": { "position": [0, -0.5, 0] },
        "rigid_body": "static", "collider": "from_shape" } },
    { "id": "hero", "components": {
        "transform": { "position": [0, 2, 0] },
        "character": { "player": true } } }
] }"#;

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpClient {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_plinth"))
            .args(["mcp", "--game", &format!("http://127.0.0.1:{PORT}")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let message = json!({
            "jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params
        });
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], json!(self.next_id), "{response}");
        assert!(
            response.get("error").is_none(),
            "unexpected MCP error: {response}"
        );
        response["result"].clone()
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
    }
}

fn tool_text(result: &Value) -> Value {
    assert_ne!(result["isError"], json!(true), "tool errored: {result}");
    let text = result["content"][0]["text"].as_str().expect("text content");
    serde_json::from_str(text).expect("tool text is JSON")
}

fn hero_z(client: &mut McpClient) -> f64 {
    let scene = tool_text(&client.call_tool("scene_entities", json!({})));
    scene["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "hero")
        .expect("hero exists")["position"][2]
        .as_f64()
        .unwrap()
}

#[test]
fn agent_loop_end_to_end() {
    // A real game, running headless on the playtest port.
    let path = std::env::temp_dir().join("plinth-mcp-test.scene.json");
    std::fs::write(&path, SCENE).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_in_game = stop.clone();
    let game_thread = std::thread::spawn(move || {
        let mut game = plinth::Game::headless().level(&path).playtest_on_port(PORT);
        while !stop_in_game.load(Ordering::Relaxed) {
            game.update();
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    std::thread::sleep(Duration::from_millis(500));

    let mut client = McpClient::start();

    // MCP handshake.
    let init = client.request(
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "fake-agent", "version": "0" }
        }),
    );
    assert_eq!(init["serverInfo"]["name"], "plinth-playtest", "{init}");
    assert_eq!(init["protocolVersion"], "2024-11-05", "{init}");

    // Tool discovery.
    let tools = client.request("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for expected in [
        "scene_entities",
        "inject_input",
        "game_time",
        "screenshot",
        "query",
        "search_assets",
        "add_asset",
    ] {
        assert!(
            names.contains(&expected),
            "missing tool {expected}: {names:?}"
        );
    }

    // Observe, act, observe — the agent playtest loop.
    let before = hero_z(&mut client);
    let injected = tool_text(&client.call_tool("inject_input", json!({ "press": ["W"] })));
    assert_eq!(injected["pressed"][0], "KeyW", "{injected}");
    std::thread::sleep(Duration::from_millis(600));
    let after = hero_z(&mut client);
    assert!(
        before - after > 0.5,
        "the agent's W press should walk the hero -Z: before {before}, after {after}"
    );

    // Raw BRP passthrough works for arbitrary ECS access.
    let listed = tool_text(&client.call_tool("query", json!({ "method": "rpc.discover" })));
    assert!(
        listed["methods"]
            .as_array()
            .is_some_and(|m| m.iter().any(|x| x["name"] == "plinth/scene")),
        "rpc.discover should list plinth methods: {listed}"
    );

    // A tool error (game unreachable) is reported as isError, not a crash.
    drop(client.stdin);
    let _ = client.child.wait();
    stop.store(true, Ordering::Relaxed);
    game_thread.join().unwrap();
}
