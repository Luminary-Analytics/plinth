//! The third-person orbit camera: follows a target entity, orbited by the
//! player's look input (mouse / right stick).

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::character::{PlayerAction, PlayerControlled};

/// Orbit-follow state for a camera. Spawned by the loader for
/// `camera3d: { "follow": "<id>" }`; insert it yourself to make any camera
/// follow any entity.
#[derive(Component, Debug)]
pub struct OrbitCamera {
    /// The entity being orbited.
    pub target: Entity,
    /// Orbit distance in meters.
    pub distance: f32,
    /// Orbit yaw in radians. `0.0` places the camera on the target's +Z
    /// side, looking toward -Z — directly behind a freshly spawned character.
    pub yaw: f32,
    /// Downward pitch in radians; positive looks down on the target.
    pub pitch: f32,
}

/// Radians of orbit per unit of look input (mouse pixels, stick deflection).
const LOOK_SENSITIVITY: f32 = 0.004;
/// Keep the camera off the poles (~83 degrees).
const PITCH_LIMIT: f32 = 1.45;

/// Apply look input to every orbit camera and recompute its transform from
/// the target's current position.
pub(crate) fn orbit_camera(
    players: Query<&ActionState<PlayerAction>, With<PlayerControlled>>,
    mut cameras: Query<(&mut OrbitCamera, &mut Transform)>,
    targets: Query<&Transform, Without<OrbitCamera>>,
) {
    let look = players
        .single()
        .map(|actions| actions.axis_pair(&PlayerAction::Look))
        .unwrap_or_default();

    for (mut orbit, mut camera_transform) in &mut cameras {
        orbit.yaw -= look.x * LOOK_SENSITIVITY;
        orbit.pitch = (orbit.pitch + look.y * LOOK_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);

        let Ok(target) = targets.get(orbit.target) else {
            continue;
        };
        let focus = target.translation;
        let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0);
        *camera_transform =
            Transform::from_translation(focus + rotation * (Vec3::Z * orbit.distance))
                .looking_at(focus, Vec3::Y);
    }
}
