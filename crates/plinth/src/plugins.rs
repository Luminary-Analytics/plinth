//! Plugin assembly: the ratified stack (avian3d + tnua + leafwing), wired
//! into the fixed-timestep schedules proven by the M0 spikes.

use avian3d::prelude::PhysicsPlugins;
use bevy::prelude::*;
use bevy_tnua::TnuaUserControlsSystems;
use bevy_tnua::prelude::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;
use leafwing_input_manager::prelude::InputManagerPlugin;

use crate::character::{self, PlayerAction, PlinthScheme};
use crate::{camera, loader};

/// Everything Plinth adds on top of Bevy, shared by windowed and headless
/// games: physics, character control, input, and the scene loader.
pub(crate) struct PlinthCorePlugin;

impl Plugin for PlinthCorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PhysicsPlugins::new(FixedPostUpdate),
            TnuaControllerPlugin::<PlinthScheme>::new(FixedUpdate),
            TnuaAvian3dPlugin::new(FixedUpdate),
            InputManagerPlugin::<PlayerAction>::default(),
        ));
        app.init_resource::<loader::LoadedScenes>();
        app.add_systems(Startup, loader::startup_load_scenes);
        app.add_systems(
            FixedUpdate,
            character::player_controls.in_set(TnuaUserControlsSystems),
        );
        app.add_systems(Update, (loader::watch_scenes, camera::orbit_camera));
    }
}
