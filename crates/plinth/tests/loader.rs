//! Headless integration tests: the arena example scene loads, simulates,
//! and responds to injected input — the seed of the agent playtest loop.

use std::time::Duration;

use bevy::time::TimeUpdateStrategy;
use plinth::prelude::*;

const ARENA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/scenes/arena.scene.json"
);

/// A deterministic headless arena: every `update()` is one 60 Hz tick.
fn arena_game() -> Game {
    let mut game = Game::headless().level(ARENA);
    game.bevy()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_micros(
            16_667,
        )));
    game.bevy().insert_resource(Time::<Fixed>::from_hz(60.0));
    game
}

fn scene_pos(game: &mut Game, id: &str) -> Vec3 {
    let world = game.bevy().world_mut();
    let mut q = world.query::<(&Transform, &SceneEntity)>();
    q.iter(world)
        .find(|(_, s)| s.id == id)
        .unwrap_or_else(|| panic!("scene entity `{id}` exists"))
        .0
        .translation
}

#[test]
fn arena_loads_and_physics_settles() {
    let mut game = arena_game();
    for _ in 0..120 {
        game.update();
    }
    let world = game.bevy().world_mut();

    let mut q = world.query::<&SceneEntity>();
    assert_eq!(q.iter(world).count(), 7, "all arena entities spawned");

    let mut q = world.query::<&Camera3d>();
    assert_eq!(q.iter(world).count(), 1, "camera spawned");
    let mut q = world.query::<&DirectionalLight>();
    assert_eq!(q.iter(world).count(), 1, "sun spawned");
    let mut q = world.query::<&PointLight>();
    assert_eq!(q.iter(world).count(), 1, "lamp spawned");

    // The dynamic crate starts at y=1 and must fall to rest on the ground
    // (1m cube => center at y=0.5).
    let crate_y = scene_pos(&mut game, "crate-1").y;
    assert!(
        (crate_y - 0.5).abs() < 0.1,
        "crate should rest on the ground, y = {crate_y}"
    );

    // The player character floats at its configured float height (default
    // 1.25) above the ground plane at y=0.
    let player_y = scene_pos(&mut game, "player").y;
    assert!(
        (player_y - 1.25).abs() < 0.35,
        "player should settle at float height, y = {player_y}"
    );
}

#[test]
fn injected_input_walks_the_player_forward() {
    let mut game = arena_game();
    for _ in 0..90 {
        game.update();
    }
    let start = scene_pos(&mut game, "player");

    // Device-level injection, the same path the playtest MCP uses.
    game.bevy()
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    for _ in 0..120 {
        game.update();
    }
    let end = scene_pos(&mut game, "player");

    // W is forward = -Z; 2 s at 6 m/s minus acceleration ramp.
    assert!(
        start.z - end.z > 3.0,
        "player should walk forward: start {start}, end {end}"
    );
    assert!(
        (end.x - start.x).abs() < 0.5,
        "no sideways drift: start {start}, end {end}"
    );
}

#[test]
fn orbit_camera_follows_and_look_rotates_movement() {
    let mut game = arena_game();
    for _ in 0..90 {
        game.update();
    }

    // The follow camera holds its configured distance, behind the player
    // (+Z side at yaw 0).
    let player = scene_pos(&mut game, "player");
    let camera = scene_pos(&mut game, "camera");
    let distance = camera.distance(player);
    assert!(
        (distance - 9.0).abs() < 0.2,
        "camera should orbit at distance 9, got {distance}"
    );
    assert!(
        camera.z > player.z + 4.0,
        "camera should start behind the player: camera {camera}, player {player}"
    );

    // Inject mouse look to orbit ~180 degrees, the same way the playtest
    // MCP will: device-level MouseMotion messages.
    let per_frame = std::f32::consts::PI / 0.004 / 30.0;
    for _ in 0..30 {
        game.bevy()
            .world_mut()
            .write_message(bevy::input::mouse::MouseMotion {
                delta: Vec2::new(per_frame, 0.0),
            });
        game.update();
    }

    // Forward is camera-relative now, so W must walk the player toward +Z.
    let start = scene_pos(&mut game, "player");
    game.bevy()
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    for _ in 0..120 {
        game.update();
    }
    let end = scene_pos(&mut game, "player");
    assert!(
        end.z - start.z > 3.0,
        "after a 180-degree orbit, W should walk +Z: start {start}, end {end}"
    );
}

#[test]
fn model_component_spawns_a_gltf_scene_root() {
    let path = std::env::temp_dir().join("plinth-model-test.scene.json");
    std::fs::write(
        &path,
        r#"{ "version": 1, "entities": [
            { "id": "knight", "components": {
                "transform": { "position": [1, 0, 2] },
                "model": { "path": "models/knight.glb" } } }
        ] }"#,
    )
    .unwrap();
    let mut game = Game::headless().level(&path);
    game.update();

    let world = game.bevy().world_mut();
    let mut q = world.query::<(&SceneEntity, &SceneRoot, &Transform)>();
    let (scene, _root, transform) = q.single(world).expect("knight spawned with a SceneRoot");
    assert_eq!(scene.id, "knight");
    assert_eq!(transform.translation, Vec3::new(1.0, 0.0, 2.0));
}

#[test]
fn broken_scene_fails_loudly_with_diagnostics() {
    let path = std::env::temp_dir().join("plinth-broken-test.scene.json");
    std::fs::write(&path, r#"{ "version": 99, "entities": [] }"#).unwrap();

    let result = std::panic::catch_unwind(move || {
        let mut game = Game::headless().level(&path);
        game.update();
    });
    let payload = result.expect_err("loading a broken scene must panic");
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_default();
    assert!(
        message.contains("failed validation"),
        "panic should carry diagnostics, got: {message}"
    );
    assert!(
        message.contains("unsupported scene format version 99"),
        "panic should carry the specific finding, got: {message}"
    );
}
