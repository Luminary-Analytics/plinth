//! # Plinth
//!
//! The stable base your game stands on.
//!
//! Plinth is an AI-first, code-first 3D game framework built on [Bevy].
//! It provides a deliberately stable, LLM-legible golden-path API for the
//! ~90% of game code that should be easy, and treats dropping into raw Bevy
//! ECS as a first-class, documented escape hatch for the rest.
//!
//! Each Plinth release pins one Bevy minor version; Plinth absorbs Bevy
//! migration churn so your game code and your coding agent's knowledge stay
//! valid.
//!
//! ```no_run
//! use plinth::prelude::*;
//!
//! fn main() {
//!     Game::new("My Game")
//!         // Scenes are schema-validated data files (`plinth validate`).
//!         .level("scenes/arena.scene.json")
//!         .run();
//! }
//! ```
//!
//! A scene's `character` component with `"player": true` walks and jumps out
//! of the box (WASD/space, gamepad), powered by the ratified stack: avian3d
//! physics and a tnua floating-capsule controller behind leafwing actions.
//!
//! [Bevy]: https://bevyengine.org

mod camera;
mod character;
mod loader;
mod plugins;

pub use camera::OrbitCamera;
pub use character::{
    PlayerAction, PlayerControlled, PlinthScheme, PlinthSchemeConfig, default_input_map,
};
pub use loader::SceneEntity;

// Re-export the stack so games depend on exactly the versions Plinth pins.
pub use avian3d;
pub use bevy;
pub use bevy_tnua;
pub use leafwing_input_manager;

/// The Plinth golden-path API, plus Bevy's prelude as the supported escape
/// hatch. Importing one prelude gives you both layers.
pub mod prelude {
    pub use crate::{Game, OrbitCamera, PlayerAction, PlayerControlled, SceneEntity};
    pub use bevy::prelude::*;
}

use std::path::PathBuf;

use bevy::prelude::*;

/// The entry point for a Plinth game.
///
/// `Game` owns a Bevy [`App`] configured with Plinth's defaults. Use the
/// builder methods for the golden path, and [`Game::bevy`] when you need the
/// full power of the underlying engine.
pub struct Game {
    app: App,
    plugins_finished: bool,
}

impl Game {
    /// Create a windowed game with Plinth's full stack: rendering, physics,
    /// character control, input, and the scene loader.
    pub fn new(title: impl Into<String>) -> Self {
        let mut app = App::new();
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: title.into(),
                ..Default::default()
            }),
            ..Default::default()
        }));
        app.add_plugins(plugins::PlinthCorePlugin);
        Self {
            app,
            plugins_finished: false,
        }
    }

    /// Create a windowless, renderless game with the same simulation stack.
    ///
    /// Headless games run anywhere (CI, agents, servers) and step
    /// deterministically with [`Game::update`] — scenes load, physics
    /// simulates, characters respond to injected input; nothing draws.
    pub fn headless() -> Self {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::input::InputPlugin,
            // avian reads scene/mesh state unconditionally (see
            // spikes/README.md); register what rendering would have.
            bevy::scene::ScenePlugin,
        ));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(plugins::PlinthCorePlugin);
        Self {
            app,
            plugins_finished: false,
        }
    }

    /// Queue a scene file (`*.scene.json`) to load at startup. May be called
    /// multiple times; scenes spawn in order.
    ///
    /// Invalid scenes fail loudly at startup with the same diagnostics
    /// `plinth validate` prints. After startup the file is watched: saving a
    /// valid edit respawns that scene's entities live, while an invalid edit
    /// logs its diagnostics and leaves the running world untouched.
    pub fn level(mut self, path: impl Into<PathBuf>) -> Self {
        self.app
            .world_mut()
            .resource_mut::<loader::LoadedScenes>()
            .0
            .push(loader::SceneRecord {
                path: path.into(),
                fingerprint: None,
            });
        self
    }

    /// Escape hatch: mutable access to the underlying Bevy [`App`].
    ///
    /// Everything Bevy can do remains available here. Plinth's stability
    /// promise covers the `Game` API; code using the escape hatch tracks the
    /// Bevy version pinned by this Plinth release.
    pub fn bevy(&mut self) -> &mut App {
        &mut self.app
    }

    /// Advance the game by exactly one frame, finishing plugin setup on the
    /// first call. The stepping primitive for tests and agent tooling.
    pub fn update(&mut self) {
        if !self.plugins_finished {
            self.app.finish();
            self.app.cleanup();
            self.plugins_finished = true;
        }
        self.app.update();
    }

    /// Run the game until exit.
    pub fn run(mut self) {
        let _ = self.app.run();
    }
}
