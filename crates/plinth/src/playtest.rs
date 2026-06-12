//! The playtest server: lets a coding agent observe and drive a *running*
//! game. Built on the Bevy Remote Protocol (JSON-RPC over HTTP), extended
//! with `plinth/*` methods for the golden path. `plinth mcp` bridges this to
//! MCP for agents.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy::remote::{BrpError, BrpResult, RemotePlugin};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use serde_json::{Value, json};

use crate::character::PlayerControlled;
use crate::loader::SceneEntity;

/// The default playtest port — BRP's well-known port.
pub const DEFAULT_PLAYTEST_PORT: u16 = 15702;

/// Fixed-timestep ticks queued by `plinth/time {"action": "step"}`, applied
/// while the virtual clock is paused.
#[derive(Resource, Default)]
struct PendingSteps(u32);

pub(crate) struct PlaytestPlugin {
    pub port: u16,
}

impl Plugin for PlaytestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingSteps>();
        app.add_plugins((
            RemotePlugin::default()
                .with_method("plinth/scene", scene_method)
                .with_method("plinth/input", input_method)
                .with_method("plinth/time", time_method)
                .with_method("plinth/screenshot", screenshot_method),
            RemoteHttpPlugin::default().with_port(self.port),
        ));
        // Runs after First (where the clock updates) and before
        // RunFixedMainLoop (which consumes the virtual delta).
        app.add_systems(PreUpdate, apply_pending_steps);
    }
}

fn invalid(message: impl Into<String>) -> BrpError {
    BrpError {
        code: bevy::remote::error_codes::INVALID_PARAMS,
        message: message.into(),
        data: None,
    }
}

/// `plinth/scene`: every scene-spawned entity with its id, position, and
/// whether it's the player. The golden-path world snapshot; use raw
/// `bevy/query` for anything deeper.
fn scene_method(
    In(_params): In<Option<Value>>,
    query: Query<(Entity, &SceneEntity, &Transform, Option<&PlayerControlled>)>,
    time: Res<Time<Virtual>>,
) -> BrpResult {
    let entities: Vec<Value> = query
        .iter()
        .map(|(entity, scene, transform, player)| {
            let p = transform.translation;
            json!({
                "id": scene.id,
                "entity": entity.to_bits(),
                "position": [p.x, p.y, p.z],
                "player": player.is_some(),
            })
        })
        .collect();
    Ok(json!({ "paused": time.is_paused(), "entities": entities }))
}

/// `plinth/input`: device-level injection. Pressed keys stay held until
/// released, exactly like a physical keyboard.
/// `{"press": ["KeyW"], "release": ["Space"], "mouse_look": [dx, dy]}`
fn input_method(
    In(params): In<Option<Value>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: MessageWriter<MouseMotion>,
) -> BrpResult {
    let params = params.unwrap_or_else(|| json!({}));
    let mut applied = json!({ "pressed": [], "released": [] });

    for (field, press) in [("press", true), ("release", false)] {
        if let Some(list) = params.get(field) {
            let names = list
                .as_array()
                .ok_or_else(|| invalid(format!("`{field}` must be an array of key names")))?;
            for name in names {
                let name = name
                    .as_str()
                    .ok_or_else(|| invalid("key names are strings"))?;
                let key = parse_key(name).ok_or_else(|| {
                    invalid(format!(
                        "unknown key `{name}`; use names like KeyW, KeyA, Space, ShiftLeft, ArrowUp, Digit1"
                    ))
                })?;
                if press {
                    keys.press(key);
                } else {
                    keys.release(key);
                }
                applied[if press { "pressed" } else { "released" }]
                    .as_array_mut()
                    .expect("initialized above")
                    .push(json!(format!("{key:?}")));
            }
        }
    }

    if let Some(look) = params.get("mouse_look") {
        let pair = look
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or_else(|| invalid("`mouse_look` must be [dx, dy]"))?;
        let dx = pair[0].as_f64().unwrap_or(0.0) as f32;
        let dy = pair[1].as_f64().unwrap_or(0.0) as f32;
        mouse.write(MouseMotion {
            delta: Vec2::new(dx, dy),
        });
        applied["mouse_look"] = json!([dx, dy]);
    }

    Ok(applied)
}

/// `plinth/time`: `{"action": "pause" | "resume" | "step", "ticks": n}`.
/// Stepping requires the clock to be paused and advances exactly `ticks`
/// fixed-timestep ticks (default 1).
fn time_method(
    In(params): In<Option<Value>>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut pending: ResMut<PendingSteps>,
) -> BrpResult {
    let params = params.unwrap_or_else(|| json!({}));
    let action = params
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("`action` must be \"pause\", \"resume\", or \"step\""))?;
    match action {
        "pause" => virtual_time.pause(),
        "resume" => virtual_time.unpause(),
        "step" => {
            if !virtual_time.is_paused() {
                return Err(invalid("`step` requires the clock to be paused first"));
            }
            let ticks = params.get("ticks").and_then(Value::as_u64).unwrap_or(1);
            if ticks == 0 || ticks > 600 {
                return Err(invalid("`ticks` must be within 1..=600"));
            }
            pending.0 += ticks as u32;
        }
        other => {
            return Err(invalid(format!(
                "unknown action `{other}`; use \"pause\", \"resume\", or \"step\""
            )));
        }
    }
    Ok(json!({ "paused": virtual_time.is_paused(), "queued_ticks": pending.0 }))
}

/// While paused, convert queued steps into virtual-time advancement so the
/// fixed-timestep schedules (physics, controls) run exactly that many ticks.
fn apply_pending_steps(
    mut pending: ResMut<PendingSteps>,
    mut virtual_time: ResMut<Time<Virtual>>,
    fixed: Res<Time<Fixed>>,
) {
    if pending.0 > 0 && virtual_time.is_paused() {
        let step = fixed.timestep() * pending.0;
        virtual_time.advance_by(step);
        pending.0 = 0;
    }
}

/// `plinth/screenshot`: `{"path": "..."}` (default: a temp file). Capture is
/// asynchronous — the file appears once the next frame renders. Requires a
/// window; headless games refuse.
fn screenshot_method(
    In(params): In<Option<Value>>,
    mut commands: Commands,
    windows: Query<Entity, With<bevy::window::PrimaryWindow>>,
) -> BrpResult {
    if windows.single().is_err() {
        return Err(invalid(
            "screenshots need a window; this game is running headless",
        ));
    }
    let path = params
        .as_ref()
        .and_then(|p| p.get("path"))
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("plinth-screenshot.png"));
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()));
    Ok(json!({ "path": path.display().to_string(), "status": "capturing" }))
}

fn parse_key(name: &str) -> Option<KeyCode> {
    use KeyCode::*;
    // Single letters and digits are accepted with or without the bevy
    // prefix: "W" == "KeyW", "1" == "Digit1".
    let canonical = name.strip_prefix("Key").unwrap_or(name);
    if canonical.len() == 1 {
        let c = canonical.chars().next().expect("len checked");
        let upper = c.to_ascii_uppercase();
        if upper.is_ascii_uppercase() {
            return letter_key(upper);
        }
        if c.is_ascii_digit() {
            return digit_key(c);
        }
    }
    if let Some(d) = name.strip_prefix("Digit")
        && d.len() == 1
    {
        return digit_key(d.chars().next().expect("len checked"));
    }
    Some(match name {
        "Space" => Space,
        "Enter" => Enter,
        "Escape" => Escape,
        "Tab" => Tab,
        "Backspace" => Backspace,
        "ShiftLeft" | "Shift" => ShiftLeft,
        "ShiftRight" => ShiftRight,
        "ControlLeft" | "Control" => ControlLeft,
        "ControlRight" => ControlRight,
        "AltLeft" | "Alt" => AltLeft,
        "AltRight" => AltRight,
        "ArrowUp" => ArrowUp,
        "ArrowDown" => ArrowDown,
        "ArrowLeft" => ArrowLeft,
        "ArrowRight" => ArrowRight,
        _ => return None,
    })
}

fn letter_key(c: char) -> Option<KeyCode> {
    use KeyCode::*;
    Some(match c {
        'A' => KeyA,
        'B' => KeyB,
        'C' => KeyC,
        'D' => KeyD,
        'E' => KeyE,
        'F' => KeyF,
        'G' => KeyG,
        'H' => KeyH,
        'I' => KeyI,
        'J' => KeyJ,
        'K' => KeyK,
        'L' => KeyL,
        'M' => KeyM,
        'N' => KeyN,
        'O' => KeyO,
        'P' => KeyP,
        'Q' => KeyQ,
        'R' => KeyR,
        'S' => KeyS,
        'T' => KeyT,
        'U' => KeyU,
        'V' => KeyV,
        'W' => KeyW,
        'X' => KeyX,
        'Y' => KeyY,
        'Z' => KeyZ,
        _ => return None,
    })
}

fn digit_key(c: char) -> Option<KeyCode> {
    use KeyCode::*;
    Some(match c {
        '0' => Digit0,
        '1' => Digit1,
        '2' => Digit2,
        '3' => Digit3,
        '4' => Digit4,
        '5' => Digit5,
        '6' => Digit6,
        '7' => Digit7,
        '8' => Digit8,
        '9' => Digit9,
        _ => return None,
    })
}
