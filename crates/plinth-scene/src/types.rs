//! The Plinth scene component vocabulary.
//!
//! These types are the stable on-disk contract. Doc comments on every field
//! flow into the generated JSON Schema as `description`s, so they are written
//! for the humans *and* agents reading `schemas/scene.schema.json`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A Plinth scene document: the on-disk format for levels and prefabs
/// (`*.scene.json`). Behavior lives in Rust; scenes declare *what exists*.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneDoc {
    /// Optional JSON Schema reference so editors and agents can validate
    /// this file in place.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    /// Scene format version. This version of Plinth understands `1`.
    pub version: u32,

    /// Human-readable scene name. Defaults to the file stem when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The entities this scene spawns, in order.
    pub entities: Vec<EntityDef>,
}

/// One entity in a scene: a stable string id plus the components attached
/// to it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDef {
    /// Stable identifier, unique within the scene. Lowercase letters,
    /// digits, `-` and `_` (pattern: `^[a-z0-9][a-z0-9_-]*$`). Used for
    /// diffs, cross-references, and runtime lookups.
    pub id: String,

    /// The components attached to this entity. At least one is required.
    pub components: ComponentsDef,
}

/// The set of Plinth components an entity may have. Unknown keys are
/// rejected — if a component name does not match this vocabulary,
/// validation fails with the list of valid names.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentsDef {
    /// Position, rotation, and scale in world space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<TransformDef>,

    /// A primitive 3D shape, rendered with this entity's `material` and
    /// usable for physics via `"collider": "from_shape"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<ShapeDef>,

    /// Surface appearance for `shape`. Requires `shape` on the same entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<MaterialDef>,

    /// A 3D camera. The scene's main view if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera3d: Option<Camera3dDef>,

    /// A light source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<LightDef>,

    /// How this entity participates in physics. `dynamic` and `kinematic`
    /// bodies require a `collider`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rigid_body: Option<RigidBodyDef>,

    /// Physics collision geometry. Use `"from_shape"` to reuse `shape`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collider: Option<ColliderDef>,

    /// A floating-capsule character controller (walk, jump, slopes). Brings
    /// its own physics body and capsule — do not combine with `rigid_body`
    /// or `collider`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<CharacterDef>,
}

/// Position, rotation, and scale in world space. Y is up; -Z is forward;
/// units are meters.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransformDef {
    /// World-space position `[x, y, z]` in meters.
    #[serde(default)]
    pub position: [f32; 3],

    /// Rotation as Euler angles `[x, y, z]` in degrees, applied as yaw (Y),
    /// then pitch (X), then roll (Z).
    #[serde(default)]
    pub rotation_degrees: [f32; 3],

    /// Per-axis scale `[x, y, z]`. Each component must be positive.
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
}

fn default_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

/// A primitive 3D shape. Exactly one variant key, e.g.
/// `{ "cuboid": { "size": [40, 1, 40] } }`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ShapeDef {
    /// Axis-aligned box; `size` is the full extent on each axis in meters.
    Cuboid {
        /// Full extents `[x, y, z]` in meters. All components positive.
        size: [f32; 3],
    },
    /// Sphere of the given radius.
    Sphere {
        /// Radius in meters. Positive.
        radius: f32,
    },
    /// Capsule aligned to the Y axis: a cylinder of `length` capped by two
    /// hemispheres of `radius`. Total height is `length + 2 * radius`.
    Capsule {
        /// Hemisphere (and cylinder) radius in meters. Positive.
        radius: f32,
        /// Cylindrical section length in meters. Positive.
        length: f32,
    },
    /// Cylinder aligned to the Y axis.
    Cylinder {
        /// Radius in meters. Positive.
        radius: f32,
        /// Full height in meters. Positive.
        height: f32,
    },
    /// Flat rectangle on the XZ plane, facing +Y.
    Plane {
        /// Extents `[width, depth]` in meters. Both positive.
        size: [f32; 2],
    },
}

/// Surface appearance for an entity's `shape` (physically-based).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialDef {
    /// Base color as a hex string: `#rgb` or `#rrggbb`.
    #[serde(default = "default_color")]
    pub color: String,

    /// Metalness, `0.0` (dielectric) to `1.0` (metal).
    #[serde(default)]
    pub metallic: f32,

    /// Perceptual roughness, `0.0` (mirror) to `1.0` (fully diffuse).
    #[serde(default = "default_roughness")]
    pub roughness: f32,
}

fn default_color() -> String {
    "#ffffff".to_owned()
}

fn default_roughness() -> f32 {
    0.5
}

/// A 3D perspective camera. Static by default; set `follow` to turn it into
/// a third-person orbit camera.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Camera3dDef {
    /// World-space point the camera looks at. When omitted, the camera uses
    /// its transform's rotation as-is. Mutually exclusive with `follow`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_at: Option<[f32; 3]>,

    /// The scene id of an entity to orbit-follow (third-person camera). The
    /// player's look input (mouse / right stick) orbits around the target,
    /// and player movement becomes camera-relative. In follow mode the
    /// camera's own `transform` is ignored — its position is computed every
    /// frame. Mutually exclusive with `look_at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow: Option<String>,

    /// Orbit distance from the followed entity, in meters. Positive. Only
    /// meaningful with `follow`.
    #[serde(default = "default_distance")]
    pub distance: f32,

    /// Initial downward orbit angle in degrees, exclusive range (-89, 89).
    /// Positive looks down on the target. Only meaningful with `follow`.
    #[serde(default = "default_pitch")]
    pub pitch_degrees: f32,

    /// Vertical field of view in degrees, exclusive range (0, 180).
    #[serde(default = "default_fov")]
    pub fov_degrees: f32,
}

fn default_distance() -> f32 {
    8.0
}

fn default_pitch() -> f32 {
    20.0
}

fn default_fov() -> f32 {
    45.0
}

/// A light source. Exactly one variant key, e.g.
/// `{ "directional": { "illuminance": 10000 } }`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LightDef {
    /// Sun-like light: parallel rays, position-independent. Aim it with the
    /// entity's `transform.rotation_degrees`.
    Directional {
        /// Illuminance in lux. Non-negative. ~10,000 is overcast daylight.
        #[serde(default = "default_illuminance")]
        illuminance: f32,
        /// Whether this light casts shadows.
        #[serde(default = "default_true")]
        shadows: bool,
    },
    /// Omnidirectional light radiating from the entity's position.
    Point {
        /// Luminous power in lumens. Non-negative.
        #[serde(default = "default_intensity")]
        intensity: f32,
        /// Falloff distance in meters. Positive.
        #[serde(default = "default_range")]
        range: f32,
        /// Whether this light casts shadows.
        #[serde(default)]
        shadows: bool,
    },
}

fn default_illuminance() -> f32 {
    10_000.0
}

fn default_intensity() -> f32 {
    1_000_000.0
}

fn default_range() -> f32 {
    20.0
}

fn default_true() -> bool {
    true
}

/// How an entity participates in physics simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RigidBodyDef {
    /// Immovable world geometry (floors, walls).
    Static,
    /// Fully simulated: affected by gravity, forces, and collisions.
    Dynamic,
    /// Moved by code, pushes dynamic bodies, unaffected by forces.
    Kinematic,
}

/// Physics collision geometry. Either `"from_shape"` (reuse the entity's
/// `shape`) or an explicit primitive, e.g. `{ "cuboid": { "size": [1, 1, 1] } }`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ColliderDef {
    /// Reuse this entity's `shape` as the collision geometry.
    FromShape,
    /// Axis-aligned box; `size` is the full extent on each axis in meters.
    Cuboid {
        /// Full extents `[x, y, z]` in meters. All components positive.
        size: [f32; 3],
    },
    /// Sphere of the given radius.
    Sphere {
        /// Radius in meters. Positive.
        radius: f32,
    },
    /// Capsule aligned to the Y axis (see `shape.capsule`).
    Capsule {
        /// Hemisphere (and cylinder) radius in meters. Positive.
        radius: f32,
        /// Cylindrical section length in meters. Positive.
        length: f32,
    },
    /// Cylinder aligned to the Y axis.
    Cylinder {
        /// Radius in meters. Positive.
        radius: f32,
        /// Full height in meters. Positive.
        height: f32,
    },
}

/// A floating-capsule character controller: walking, jumping, slopes, and
/// ground snapping. Spawns with its own dynamic body and capsule collider.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CharacterDef {
    /// This character is controlled by the local player's input.
    #[serde(default)]
    pub player: bool,

    /// Height the capsule's center floats above the ground, in meters.
    /// Must exceed half the capsule's total height (`length / 2 + radius`),
    /// or the character would rest inside its own collider. Positive.
    #[serde(default = "default_float_height")]
    pub float_height: f32,

    /// Maximum walk speed in meters per second. Positive.
    #[serde(default = "default_speed")]
    pub speed: f32,

    /// Apex height of a full jump in meters. `0` disables jumping.
    #[serde(default = "default_jump_height")]
    pub jump_height: f32,

    /// Collision capsule radius in meters. Positive.
    #[serde(default = "default_radius")]
    pub radius: f32,

    /// Collision capsule cylindrical length in meters. Positive.
    #[serde(default = "default_length")]
    pub length: f32,
}

fn default_float_height() -> f32 {
    1.25
}

fn default_speed() -> f32 {
    6.0
}

fn default_jump_height() -> f32 {
    1.5
}

fn default_radius() -> f32 {
    0.5
}

fn default_length() -> f32 {
    1.0
}
