//! The golden-path character: tnua-based floating capsule controller, driven
//! by leafwing actions so input is injectable and replayable by design.

use avian3d::prelude::{Collider, LockedAxes, RigidBody};
use bevy::prelude::*;
use bevy_tnua::builtins::{TnuaBuiltinJumpConfig, TnuaBuiltinWalkConfig};
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::TnuaAvian3dSensorShape;
use leafwing_input_manager::prelude::*;

/// Plinth's built-in control scheme: walking basis plus a jump action.
#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
pub enum PlinthScheme {
    Jump(TnuaBuiltinJump),
}

/// The actions a player character responds to. Map them from any device via
/// leafwing's `InputMap`; Plinth installs [`default_input_map`] out of the box.
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum PlayerAction {
    /// Planar movement. X is right, Y is forward (camera-relative when an
    /// orbit camera is active, world-relative otherwise).
    #[actionlike(DualAxis)]
    Move,
    /// Camera look: orbits the follow camera around its target.
    #[actionlike(DualAxis)]
    Look,
    Jump,
}

/// Marks the entity driven by the local player's input.
#[derive(Component, Debug, Default)]
pub struct PlayerControlled;

/// WASD + mouse + space; left stick + right stick + south button.
pub fn default_input_map() -> InputMap<PlayerAction> {
    InputMap::default()
        .with_dual_axis(PlayerAction::Move, VirtualDPad::wasd())
        .with_dual_axis(PlayerAction::Move, GamepadStick::LEFT)
        .with_dual_axis(PlayerAction::Look, MouseMove::default())
        .with_dual_axis(PlayerAction::Look, GamepadStick::RIGHT)
        .with(PlayerAction::Jump, KeyCode::Space)
        .with(PlayerAction::Jump, GamepadButton::South)
}

/// Everything a `character` scene component expands to. The legible one-line
/// scene entry becomes a full physics body + controller + config asset.
pub(crate) fn character_bundle(
    def: &plinth_scene::CharacterDef,
    configs: &mut Assets<PlinthSchemeConfig>,
) -> impl Bundle {
    (
        RigidBody::Dynamic,
        Collider::capsule(def.radius, def.length),
        TnuaController::<PlinthScheme>::default(),
        TnuaConfig::<PlinthScheme>(configs.add(PlinthSchemeConfig {
            basis: TnuaBuiltinWalkConfig {
                float_height: def.float_height,
                speed: def.speed,
                ..Default::default()
            },
            jump: TnuaBuiltinJumpConfig {
                height: def.jump_height,
                ..Default::default()
            },
        })),
        // Cast a shape slightly thinner than the capsule so ledges register
        // before the collider slips off them.
        TnuaAvian3dSensorShape(Collider::cylinder((def.radius * 0.9).max(0.05), 0.0)),
        // Tnua turns the character toward its motion around Y; lock the rest.
        LockedAxes::new().lock_rotation_x().lock_rotation_z(),
    )
}

/// Feed player input into the tnua controller. Runs in `FixedUpdate` inside
/// [`TnuaUserControlsSystems`].
pub(crate) fn player_controls(
    mut query: Query<
        (
            &ActionState<PlayerAction>,
            &mut TnuaController<PlinthScheme>,
        ),
        With<PlayerControlled>,
    >,
    cameras: Query<&crate::camera::OrbitCamera>,
) {
    // With an orbit camera active, "forward" means away from the camera.
    let camera_yaw = cameras.iter().next().map(|orbit| orbit.yaw);

    for (actions, mut controller) in &mut query {
        controller.initiate_action_feeding();

        let axis = actions.axis_pair(&PlayerAction::Move);
        // Input Y is "forward"; world forward is -Z.
        let mut motion = Vec3::new(axis.x, 0.0, -axis.y);
        if let Some(yaw) = camera_yaw {
            motion = Quat::from_rotation_y(yaw) * motion;
        }
        if motion.length_squared() > 1.0 {
            motion = motion.normalize();
        }
        controller.basis = TnuaBuiltinWalk {
            desired_motion: motion,
            desired_forward: Dir3::new(motion).ok(),
        };

        if actions.pressed(&PlayerAction::Jump) {
            controller.action(PlinthScheme::Jump(TnuaBuiltinJump::default()));
        }
    }
}
