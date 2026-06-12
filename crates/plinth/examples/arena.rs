//! The canonical arena scene, windowed.
//!
//! Run from the repository root: `cargo run -p plinth --example arena`
//! WASD to move, space to jump.

use plinth::prelude::*;

fn main() {
    Game::new("Plinth Arena")
        .level("examples/scenes/arena.scene.json")
        .run();
}
