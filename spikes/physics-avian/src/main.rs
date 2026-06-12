//! M0 spike: bevy_tnua character controller on avian3d, fully headless.
//!
//! Scenario (60 Hz fixed timestep, manually stepped, deterministic):
//!   frames   0..60   settle onto the ground (no input)
//!   frames  60..180  walk +X at 4 m/s, driven through leafwing-input-manager
//!   frames 180..186  jump held (while still walking)
//!   frames    ..300  land and keep walking
//!
//! Also proves the input-injection primitive: movement is driven by pressing
//! leafwing actions from code, exactly as the playtest MCP will.

use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_tnua::builtins::{TnuaBuiltinJumpConfig, TnuaBuiltinWalkConfig};
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::*;
use leafwing_input_manager::prelude::*;

const FLOAT_HEIGHT: f32 = 1.0;
const WALK_SPEED: f32 = 4.0;

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
enum Action {
    Forward,
    Jump,
}

#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
enum ControlScheme {
    Jump(TnuaBuiltinJump),
}

#[derive(Component)]
struct Player;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        bevy::input::InputPlugin,
        // Headless shim: avian's collider backend reads SceneSpawner
        // unconditionally (scene-based collider hierarchies).
        bevy::scene::ScenePlugin,
        PhysicsPlugins::new(FixedPostUpdate),
        TnuaControllerPlugin::<ControlScheme>::new(FixedUpdate),
        TnuaAvian3dPlugin::new(FixedUpdate),
        InputManagerPlugin::<Action>::default(),
    ));
    // Every app.update() advances time by exactly one 60 Hz tick.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_micros(
        16_667,
    )));
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
    // Headless shim: avian's collider cache reads AssetEvent<Mesh> even when
    // no mesh colliders exist, and nothing else registers Mesh without
    // bevy_render's plugins.
    app.init_asset::<Mesh>();
    app.add_systems(Startup, setup);
    app.add_systems(FixedUpdate, apply_controls.in_set(TnuaUserControlsSystems));

    app.finish();
    app.cleanup();

    run_scenario(&mut app);
}

fn setup(mut commands: Commands, mut configs: ResMut<Assets<ControlSchemeConfig>>) {
    // Ground: 40x1x40 slab (avian takes full extents), top surface at y = 0.
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 1.0, 40.0),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    // Character: capsule, rotation locked, tnua-controlled.
    commands.spawn((
        Player,
        RigidBody::Dynamic,
        Collider::capsule(0.5, 1.0),
        TnuaController::<ControlScheme>::default(),
        TnuaConfig::<ControlScheme>(configs.add(ControlSchemeConfig {
            basis: TnuaBuiltinWalkConfig {
                float_height: FLOAT_HEIGHT,
                speed: WALK_SPEED,
                ..Default::default()
            },
            jump: TnuaBuiltinJumpConfig {
                height: 1.5,
                ..Default::default()
            },
        })),
        TnuaAvian3dSensorShape(Collider::cylinder(0.45, 0.0)),
        LockedAxes::ROTATION_LOCKED,
        Transform::from_xyz(0.0, 2.0, 0.0),
        InputMap::new([
            (Action::Forward, KeyCode::KeyW),
            (Action::Jump, KeyCode::Space),
        ]),
        ActionState::<Action>::default(),
    ));
}

fn apply_controls(
    mut q: Query<(&ActionState<Action>, &mut TnuaController<ControlScheme>), With<Player>>,
) {
    let Ok((actions, mut controller)) = q.single_mut() else {
        return;
    };
    controller.initiate_action_feeding();
    // desired_motion is a unit-direction throttle; the speed itself comes
    // from TnuaBuiltinWalkConfig::speed.
    let desired_motion = if actions.pressed(&Action::Forward) {
        Vec3::X
    } else {
        Vec3::ZERO
    };
    controller.basis = TnuaBuiltinWalk {
        desired_motion,
        desired_forward: None,
    };
    if actions.pressed(&Action::Jump) {
        controller.action(ControlScheme::Jump(TnuaBuiltinJump::default()));
    }
}

fn sample(app: &mut App) -> (Vec3, bool) {
    let world = app.world_mut();
    let mut q =
        world.query_filtered::<(&Transform, &TnuaController<ControlScheme>), With<Player>>();
    let (transform, controller) = q.single(world).expect("player exists");
    (
        transform.translation,
        controller.is_airborne().unwrap_or(true),
    )
}

fn run_scenario(app: &mut App) {
    let mut max_jump_y = f32::MIN;
    let mut airborne_seen = false;
    let mut pos_settle = Vec3::ZERO;
    let mut grounded_settle = false;
    let mut pos_walk_end = Vec3::ZERO;

    // Inject input at the device level, exactly as the playtest MCP will:
    // mutate ButtonInput<KeyCode> and let bevy_input + leafwing do the rest.
    fn set_key(app: &mut App, key: KeyCode, down: bool) {
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        if down {
            input.press(key);
        } else {
            input.release(key);
        }
    }

    let start = Instant::now();
    for f in 0..300u32 {
        match f {
            60 => set_key(app, KeyCode::KeyW, true),
            180 => set_key(app, KeyCode::Space, true),
            186 => set_key(app, KeyCode::Space, false),
            _ => {}
        }
        app.update();
        let (pos, airborne) = sample(app);
        assert!(pos.is_finite(), "NaN/inf position at frame {f}: {pos:?}");
        if f == 59 {
            pos_settle = pos;
            grounded_settle = !airborne;
        }
        if f == 179 {
            pos_walk_end = pos;
        }
        if (180..260).contains(&f) {
            max_jump_y = max_jump_y.max(pos.y);
            airborne_seen |= airborne;
        }
    }
    let elapsed = start.elapsed();
    let (pos_final, airborne_final) = sample(app);

    println!("--- plinth physics spike: avian3d + tnua (headless) ---");
    println!("settle  : pos={pos_settle:.3?} grounded={grounded_settle}");
    println!(
        "walk-end: pos={pos_walk_end:.3?} (dx={:.3} over 2s)",
        pos_walk_end.x - pos_settle.x
    );
    println!("jump    : max_y={max_jump_y:.3} airborne_seen={airborne_seen}");
    println!("final   : pos={pos_final:.3?} grounded={}", !airborne_final);
    println!(
        "perf    : 300 updates in {elapsed:.2?} ({:.0} updates/sec)",
        300.0 / elapsed.as_secs_f64()
    );

    let dx = pos_walk_end.x - pos_settle.x;
    let mut failures = Vec::new();
    if !grounded_settle || (pos_settle.y - FLOAT_HEIGHT).abs() > 0.35 {
        failures.push(format!(
            "settle: grounded={grounded_settle} y={:.3}",
            pos_settle.y
        ));
    }
    if !(5.0..=9.0).contains(&dx) {
        failures.push(format!("walk: dx={dx:.3}, expected ~8"));
    }
    if !airborne_seen || max_jump_y < FLOAT_HEIGHT + 0.8 {
        failures.push(format!(
            "jump: airborne={airborne_seen} max_y={max_jump_y:.3}"
        ));
    }
    if airborne_final {
        failures.push("final: still airborne".into());
    }
    if pos_final.z.abs() > 0.2 {
        failures.push(format!("drift: z={:.3}", pos_final.z));
    }

    if failures.is_empty() {
        println!("SPIKE PASS");
    } else {
        println!("SPIKE FAIL: {failures:?}");
        std::process::exit(1);
    }
}
