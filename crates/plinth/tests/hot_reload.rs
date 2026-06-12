//! Hot-reload integration tests: edit the scene file while the game runs.

use std::path::PathBuf;
use std::time::Duration;

use bevy::time::TimeUpdateStrategy;
use plinth::prelude::*;

const SCENE_V1: &str = r#"{ "version": 1, "entities": [
    { "id": "ground", "components": {
        "shape": { "cuboid": { "size": [40, 1, 40] } },
        "transform": { "position": [0, -0.5, 0] },
        "rigid_body": "static", "collider": "from_shape" } },
    { "id": "hero", "components": {
        "transform": { "position": [0, 2, 0] },
        "character": { "player": true } } }
] }"#;

const SCENE_V2: &str = r#"{ "version": 1, "entities": [
    { "id": "ground", "components": {
        "shape": { "cuboid": { "size": [40, 1, 40] } },
        "transform": { "position": [0, -0.5, 0] },
        "rigid_body": "static", "collider": "from_shape" } },
    { "id": "hero", "components": {
        "transform": { "position": [0, 2, 0] },
        "character": { "player": true } } },
    { "id": "obelisk", "components": {
        "transform": { "position": [5, 1.5, 0] },
        "shape": { "cuboid": { "size": [1, 3, 1] } },
        "rigid_body": "static", "collider": "from_shape" } }
] }"#;

const SCENE_BROKEN: &str = r#"{ "version": 1, "entities": [
    { "id": "ground", "components": { "shape": { "cubed": {} } } }
] }"#;

fn temp_scene(name: &str, content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

fn game_with(path: &PathBuf) -> Game {
    let mut game = Game::headless().level(path);
    game.bevy()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_micros(
            16_667,
        )));
    game.bevy().insert_resource(Time::<Fixed>::from_hz(60.0));
    game
}

fn scene_ids(game: &mut Game) -> Vec<String> {
    let world = game.bevy().world_mut();
    let mut q = world.query::<&SceneEntity>();
    let mut ids: Vec<String> = q.iter(world).map(|s| s.id.clone()).collect();
    ids.sort();
    ids
}

#[test]
fn valid_edit_respawns_the_scene() {
    let path = temp_scene("plinth-hot-reload-valid.scene.json", SCENE_V1);
    let mut game = game_with(&path);
    for _ in 0..30 {
        game.update();
    }
    assert_eq!(scene_ids(&mut game), ["ground", "hero"]);

    // Save a new version of the scene while the game runs.
    std::fs::write(&path, SCENE_V2).unwrap();
    for _ in 0..5 {
        game.update();
    }
    assert_eq!(
        scene_ids(&mut game),
        ["ground", "hero", "obelisk"],
        "the obelisk should appear after a live edit"
    );

    // The respawned character still works: it falls and settles again.
    for _ in 0..90 {
        game.update();
    }
    let world = game.bevy().world_mut();
    let mut q = world.query_filtered::<&Transform, With<PlayerControlled>>();
    let y = q.single(world).unwrap().translation.y;
    assert!(
        (y - 1.25).abs() < 0.35,
        "respawned player should settle at float height, y = {y}"
    );
}

#[test]
fn invalid_edit_keeps_the_world_and_recovers() {
    let path = temp_scene("plinth-hot-reload-invalid.scene.json", SCENE_V1);
    let mut game = game_with(&path);
    for _ in 0..30 {
        game.update();
    }
    assert_eq!(scene_ids(&mut game), ["ground", "hero"]);

    // A broken save must not nuke the running world.
    std::fs::write(&path, SCENE_BROKEN).unwrap();
    for _ in 0..5 {
        game.update();
    }
    assert_eq!(
        scene_ids(&mut game),
        ["ground", "hero"],
        "invalid edits keep the previous world"
    );

    // Fixing the file picks the change up.
    std::fs::write(&path, SCENE_V2).unwrap();
    for _ in 0..5 {
        game.update();
    }
    assert_eq!(
        scene_ids(&mut game),
        ["ground", "hero", "obelisk"],
        "the next valid save reloads"
    );
}
