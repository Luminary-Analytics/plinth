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
//!     Game::new("My Game").run();
//! }
//! ```
//!
//! [Bevy]: https://bevyengine.org

use bevy::prelude::*;

/// The Plinth golden-path API, plus Bevy's prelude as the supported escape
/// hatch. Importing one prelude gives you both layers.
pub mod prelude {
    pub use crate::Game;
    pub use bevy::prelude::*;
}

/// The entry point for a Plinth game.
///
/// `Game` owns a Bevy [`App`] configured with Plinth's defaults. Use the
/// builder methods for the golden path, and [`Game::bevy`] when you need the
/// full power of the underlying engine.
pub struct Game {
    app: App,
}

impl Game {
    /// Create a game with Plinth's default plugins and a titled window.
    pub fn new(title: impl Into<String>) -> Self {
        let mut app = App::new();
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: title.into(),
                ..Default::default()
            }),
            ..Default::default()
        }));
        Self { app }
    }

    /// Escape hatch: mutable access to the underlying Bevy [`App`].
    ///
    /// Everything Bevy can do remains available here. Plinth's stability
    /// promise covers the `Game` API; code using the escape hatch tracks the
    /// Bevy version pinned by this Plinth release.
    pub fn bevy(&mut self) -> &mut App {
        &mut self.app
    }

    /// Run the game until exit.
    pub fn run(mut self) {
        let _ = self.app.run();
    }
}
