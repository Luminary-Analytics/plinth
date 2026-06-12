//! Scene loading: turn a validated `SceneDoc` into live ECS entities.
//!
//! The mapping from the scene vocabulary to engine components lives here and
//! only here — the loader is the single source of truth for what each scene
//! component *means* at runtime.

use std::path::PathBuf;
use std::time::SystemTime;

use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use plinth_scene::{
    Camera3dDef, ColliderDef, ComponentsDef, LightDef, MaterialDef, RigidBodyDef, SceneDoc,
    ShapeDef, TransformDef, parse_hex_color,
};

use crate::camera::OrbitCamera;
use crate::character::{self, PlayerControlled, PlinthSchemeConfig};

/// One scene file registered via [`crate::Game::level`], plus the on-disk
/// fingerprint it was last spawned from (None before the first load).
pub(crate) struct SceneRecord {
    pub path: PathBuf,
    pub fingerprint: Option<(SystemTime, u64)>,
}

/// Every scene file this game runs. Index order is spawn order, and each
/// spawned entity is tagged with its record's index for hot-reload.
#[derive(Resource, Default)]
pub(crate) struct LoadedScenes(pub Vec<SceneRecord>);

/// Attached to every entity spawned from a scene file, carrying its stable
/// scene id. The id is also mirrored into [`Name`] for inspectors.
#[derive(Component, Debug, Clone)]
pub struct SceneEntity {
    pub id: String,
}

/// Which [`LoadedScenes`] record an entity came from; hot-reload despawns
/// and respawns by this tag.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SceneIndex(pub usize);

/// Thickness given to colliders generated from a `plane` shape, which has no
/// volume of its own.
const PLANE_COLLIDER_THICKNESS: f32 = 0.1;

fn fingerprint(path: &PathBuf) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// Startup loading is synchronous and fail-fast: a broken scene stops the
/// game with the same diagnostics `plinth validate` prints.
pub(crate) fn startup_load_scenes(
    mut commands: Commands,
    mut scenes: ResMut<LoadedScenes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut char_configs: ResMut<Assets<PlinthSchemeConfig>>,
) {
    for (index, record) in scenes.0.iter_mut().enumerate() {
        let src = match std::fs::read_to_string(&record.path) {
            Ok(src) => src,
            Err(err) => panic!(
                "plinth: cannot read scene file {}: {err}",
                record.path.display()
            ),
        };
        let (doc, diags) = plinth_scene::validate_str(&src);
        if !diags.is_empty() {
            let rendered: Vec<String> = diags.iter().map(|d| format!("  {d}")).collect();
            panic!(
                "plinth: scene {} failed validation:\n{}\n(fix the file or run `plinth validate` for details)",
                record.path.display(),
                rendered.join("\n")
            );
        }
        let doc = doc.expect("no diagnostics implies a parsed document");
        record.fingerprint = fingerprint(&record.path);
        spawn_scene_doc(
            &mut commands,
            &doc,
            index,
            &mut meshes,
            &mut materials,
            &mut char_configs,
        );
    }
}

/// Hot-reload: when a scene file's fingerprint changes, despawn what it
/// spawned and respawn from the new content. An invalid edit logs the
/// diagnostics and leaves the running world untouched — fix the file and it
/// reloads on the next change.
pub(crate) fn watch_scenes(
    mut commands: Commands,
    mut scenes: ResMut<LoadedScenes>,
    spawned: Query<(Entity, &SceneIndex)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut char_configs: ResMut<Assets<PlinthSchemeConfig>>,
) {
    for (index, record) in scenes.0.iter_mut().enumerate() {
        // A vanished file is usually an editor mid-save (remove + rename);
        // wait for it to come back.
        let Some(current) = fingerprint(&record.path) else {
            continue;
        };
        if record.fingerprint == Some(current) {
            continue;
        }
        record.fingerprint = Some(current);

        let Ok(src) = std::fs::read_to_string(&record.path) else {
            continue;
        };
        let (doc, diags) = plinth_scene::validate_str(&src);
        if !diags.is_empty() {
            let rendered: Vec<String> = diags.iter().map(|d| format!("  {d}")).collect();
            error!(
                "plinth: scene {} changed but failed validation — keeping the previous world:\n{}",
                record.path.display(),
                rendered.join("\n")
            );
            continue;
        }
        let doc = doc.expect("no diagnostics implies a parsed document");

        for (entity, scene_index) in &spawned {
            if scene_index.0 == index {
                commands.entity(entity).despawn();
            }
        }
        info!("plinth: reloaded scene {}", record.path.display());
        spawn_scene_doc(
            &mut commands,
            &doc,
            index,
            &mut meshes,
            &mut materials,
            &mut char_configs,
        );
    }
}

pub(crate) fn spawn_scene_doc(
    commands: &mut Commands,
    doc: &SceneDoc,
    index: usize,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    char_configs: &mut Assets<PlinthSchemeConfig>,
) {
    // Pass 1 spawns everything; pass 2 resolves cross-entity references
    // (camera follow targets) once every id has an Entity.
    let mut by_id: std::collections::HashMap<&str, Entity> = std::collections::HashMap::new();
    let mut follow_cameras: Vec<(Entity, &Camera3dDef)> = Vec::new();

    for def in &doc.entities {
        let c = &def.components;
        let mut entity = commands.spawn((
            SceneEntity { id: def.id.clone() },
            SceneIndex(index),
            Name::new(def.id.clone()),
            to_transform(c),
        ));

        if let Some(shape) = &c.shape {
            entity.insert((
                Mesh3d(meshes.add(shape_mesh(shape))),
                MeshMaterial3d(materials.add(to_material(c.material.as_ref()))),
            ));
        }

        if let Some(light) = &c.light {
            match *light {
                LightDef::Directional {
                    illuminance,
                    shadows,
                } => {
                    entity.insert(DirectionalLight {
                        illuminance,
                        shadows_enabled: shadows,
                        ..Default::default()
                    });
                }
                LightDef::Point {
                    intensity,
                    range,
                    shadows,
                } => {
                    entity.insert(PointLight {
                        intensity,
                        range,
                        shadows_enabled: shadows,
                        ..Default::default()
                    });
                }
            }
        }

        if let Some(cam) = &c.camera3d {
            entity.insert((
                Camera3d::default(),
                Projection::from(PerspectiveProjection {
                    fov: cam.fov_degrees.to_radians(),
                    ..Default::default()
                }),
            ));
        }

        if let Some(rb) = &c.rigid_body {
            entity.insert(match rb {
                RigidBodyDef::Static => RigidBody::Static,
                RigidBodyDef::Dynamic => RigidBody::Dynamic,
                RigidBodyDef::Kinematic => RigidBody::Kinematic,
            });
        }

        if let Some(col) = &c.collider {
            entity.insert(to_collider(col, c.shape.as_ref()));
        }

        if let Some(ch) = &c.character {
            entity.insert(character::character_bundle(ch, char_configs));
            if ch.player {
                entity.insert((
                    PlayerControlled,
                    character::default_input_map(),
                    leafwing_input_manager::action_state::ActionState::<
                        character::PlayerAction,
                    >::default(),
                ));
            }
        }

        let entity_id = entity.id();
        by_id.insert(def.id.as_str(), entity_id);
        if let Some(cam) = &c.camera3d
            && cam.follow.is_some()
        {
            follow_cameras.push((entity_id, cam));
        }
    }

    for (camera_entity, cam) in follow_cameras {
        let target_id = cam.follow.as_deref().expect("queued only with follow");
        let target = by_id[target_id]; // reference validated before load
        commands.entity(camera_entity).insert(OrbitCamera {
            target,
            distance: cam.distance,
            yaw: 0.0,
            pitch: cam.pitch_degrees.to_radians(),
        });
    }
}

/// Build the entity's `Transform`, letting `camera3d.look_at` override the
/// authored rotation when present.
fn to_transform(c: &ComponentsDef) -> Transform {
    let mut transform = c.transform.as_ref().map_or_else(Transform::default, |t| {
        let TransformDef {
            position,
            rotation_degrees: [rx, ry, rz],
            scale,
        } = *t;
        Transform {
            translation: Vec3::from(position),
            rotation: Quat::from_euler(
                EulerRot::YXZ,
                ry.to_radians(),
                rx.to_radians(),
                rz.to_radians(),
            ),
            scale: Vec3::from(scale),
        }
    });
    if let Some(Camera3dDef {
        look_at: Some(target),
        ..
    }) = &c.camera3d
    {
        transform = transform.looking_at(Vec3::from(*target), Vec3::Y);
    }
    transform
}

fn shape_mesh(shape: &ShapeDef) -> Mesh {
    match *shape {
        ShapeDef::Cuboid { size: [x, y, z] } => Cuboid::new(x, y, z).into(),
        ShapeDef::Sphere { radius } => Sphere::new(radius).into(),
        ShapeDef::Capsule { radius, length } => Capsule3d::new(radius, length).into(),
        ShapeDef::Cylinder { radius, height } => Cylinder::new(radius, height).into(),
        ShapeDef::Plane { size: [w, d] } => Plane3d::default().mesh().size(w, d).into(),
    }
}

fn to_material(def: Option<&MaterialDef>) -> StandardMaterial {
    let Some(def) = def else {
        return StandardMaterial::default();
    };
    let [r, g, b] = parse_hex_color(&def.color).expect("color validated before load");
    StandardMaterial {
        base_color: Color::srgb_u8(r, g, b),
        metallic: def.metallic,
        perceptual_roughness: def.roughness,
        ..Default::default()
    }
}

fn to_collider(def: &ColliderDef, shape: Option<&ShapeDef>) -> Collider {
    match *def {
        ColliderDef::FromShape => {
            let shape = shape.expect("from_shape validated before load");
            match *shape {
                ShapeDef::Cuboid { size: [x, y, z] } => Collider::cuboid(x, y, z),
                ShapeDef::Sphere { radius } => Collider::sphere(radius),
                ShapeDef::Capsule { radius, length } => Collider::capsule(radius, length),
                ShapeDef::Cylinder { radius, height } => Collider::cylinder(radius, height),
                // A plane has no volume; give it a thin slab so things rest on it.
                ShapeDef::Plane { size: [w, d] } => {
                    Collider::cuboid(w, PLANE_COLLIDER_THICKNESS, d)
                }
            }
        }
        ColliderDef::Cuboid { size: [x, y, z] } => Collider::cuboid(x, y, z),
        ColliderDef::Sphere { radius } => Collider::sphere(radius),
        ColliderDef::Capsule { radius, length } => Collider::capsule(radius, length),
        ColliderDef::Cylinder { radius, height } => Collider::cylinder(radius, height),
    }
}
