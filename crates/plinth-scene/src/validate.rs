//! Semantic validation: rules that hold after the document parses.

use crate::diag::Diagnostic;
use crate::types::*;

/// The scene format version this build of Plinth understands.
pub const SUPPORTED_VERSION: u32 = 1;

/// Check every semantic rule and return all findings (never short-circuits,
/// so an agent sees the full fix list in one pass).
pub fn validate_doc(doc: &SceneDoc) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    if doc.version != SUPPORTED_VERSION {
        diags.push(Diagnostic::semantic(
            "version",
            format!(
                "unsupported scene format version {}; this Plinth supports {SUPPORTED_VERSION}",
                doc.version
            ),
        ));
    }

    let mut seen_ids: Vec<(&str, usize)> = Vec::new();
    for (i, entity) in doc.entities.iter().enumerate() {
        let at = |suffix: &str| format!("entities[{i}]{suffix}");

        if !valid_id(&entity.id) {
            diags.push(Diagnostic::semantic(
                at(".id"),
                format!(
                    "invalid id `{}`: use lowercase letters, digits, `-` and `_`, starting with a letter or digit",
                    entity.id
                ),
            ));
        }
        if let Some((_, first)) = seen_ids.iter().find(|(id, _)| *id == entity.id) {
            diags.push(Diagnostic::semantic(
                at(".id"),
                format!(
                    "duplicate id `{}` (already used by entities[{first}])",
                    entity.id
                ),
            ));
        } else {
            seen_ids.push((&entity.id, i));
        }

        validate_components(&entity.components, &at(".components"), &mut diags);
    }

    diags
}

fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn push(diags: &mut Vec<Diagnostic>, base: &str, suffix: &str, message: String) {
    diags.push(Diagnostic::semantic(format!("{base}{suffix}"), message));
}

fn validate_components(c: &ComponentsDef, base: &str, diags: &mut Vec<Diagnostic>) {
    let at = |suffix: &str| format!("{base}{suffix}");

    if c.transform.is_none()
        && c.shape.is_none()
        && c.material.is_none()
        && c.camera3d.is_none()
        && c.light.is_none()
        && c.rigid_body.is_none()
        && c.collider.is_none()
        && c.character.is_none()
    {
        push(diags, base, "", "entity has no components".to_owned());
        return;
    }

    if let Some(t) = &c.transform {
        for (axis, value) in ["x", "y", "z"].iter().zip(t.scale) {
            if value <= 0.0 {
                push(
                    diags,
                    base,
                    ".transform.scale",
                    format!("scale.{axis} must be positive, got {value}"),
                );
            }
        }
    }

    if let Some(shape) = &c.shape {
        validate_shape_dims(shape_dims(shape), &at(".shape"), diags);
    }

    if let Some(m) = &c.material {
        if c.shape.is_none() {
            push(
                diags,
                base,
                ".material",
                "material requires a `shape` component on the same entity".into(),
            );
        }
        if parse_hex_color(&m.color).is_none() {
            push(
                diags,
                base,
                ".material.color",
                format!("invalid color `{}`: expected `#rgb` or `#rrggbb`", m.color),
            );
        }
        for (field, value) in [("metallic", m.metallic), ("roughness", m.roughness)] {
            if !(0.0..=1.0).contains(&value) {
                push(
                    diags,
                    base,
                    ".material",
                    format!("{field} must be within 0.0..=1.0, got {value}"),
                );
            }
        }
    }

    if let Some(cam) = &c.camera3d
        && !(cam.fov_degrees > 0.0 && cam.fov_degrees < 180.0)
    {
        push(
            diags,
            base,
            ".camera3d.fov_degrees",
            format!(
                "fov_degrees must be within (0, 180), got {}",
                cam.fov_degrees
            ),
        );
    }

    if let Some(light) = &c.light {
        match light {
            LightDef::Directional { illuminance, .. } => {
                if *illuminance < 0.0 {
                    push(
                        diags,
                        base,
                        ".light.directional.illuminance",
                        format!("illuminance must be non-negative, got {illuminance}"),
                    );
                }
            }
            LightDef::Point {
                intensity, range, ..
            } => {
                if *intensity < 0.0 {
                    push(
                        diags,
                        base,
                        ".light.point.intensity",
                        format!("intensity must be non-negative, got {intensity}"),
                    );
                }
                if *range <= 0.0 {
                    push(
                        diags,
                        base,
                        ".light.point.range",
                        format!("range must be positive, got {range}"),
                    );
                }
            }
        }
    }

    if let Some(collider) = &c.collider {
        match collider {
            ColliderDef::FromShape => {
                if c.shape.is_none() {
                    push(
                        diags,
                        base,
                        ".collider",
                        "`from_shape` requires a `shape` component on the same entity".into(),
                    );
                }
            }
            ColliderDef::Cuboid { size } => {
                validate_shape_dims(
                    vec![
                        ("size.x", size[0]),
                        ("size.y", size[1]),
                        ("size.z", size[2]),
                    ],
                    &at(".collider.cuboid"),
                    diags,
                );
            }
            ColliderDef::Sphere { radius } => {
                validate_shape_dims(vec![("radius", *radius)], &at(".collider.sphere"), diags);
            }
            ColliderDef::Capsule { radius, length } => {
                validate_shape_dims(
                    vec![("radius", *radius), ("length", *length)],
                    &at(".collider.capsule"),
                    diags,
                );
            }
            ColliderDef::Cylinder { radius, height } => {
                validate_shape_dims(
                    vec![("radius", *radius), ("height", *height)],
                    &at(".collider.cylinder"),
                    diags,
                );
            }
        }
    }

    if matches!(
        c.rigid_body,
        Some(RigidBodyDef::Dynamic) | Some(RigidBodyDef::Kinematic)
    ) && c.collider.is_none()
    {
        push(
            diags,
            base,
            ".rigid_body",
            "dynamic and kinematic bodies require a `collider` component".into(),
        );
    }

    if let Some(ch) = &c.character {
        if c.rigid_body.is_some() || c.collider.is_some() {
            push(
                diags,
                base,
                ".character",
                "character brings its own physics body; remove `rigid_body`/`collider` from this entity"
                    .into(),
            );
        }
        for (field, value, positive) in [
            ("float_height", ch.float_height, true),
            ("speed", ch.speed, true),
            ("jump_height", ch.jump_height, false),
            ("radius", ch.radius, true),
            ("length", ch.length, true),
        ] {
            let bad = if positive { value <= 0.0 } else { value < 0.0 };
            if bad {
                let requirement = if positive { "positive" } else { "non-negative" };
                push(
                    diags,
                    base,
                    ".character",
                    format!("{field} must be {requirement}, got {value}"),
                );
            }
        }
        if ch.float_height <= ch.length / 2.0 + ch.radius {
            push(
                diags,
                base,
                ".character.float_height",
                format!(
                    "float_height ({}) must exceed half the capsule's total height ({}); the character cannot float inside its own collider",
                    ch.float_height,
                    ch.length / 2.0 + ch.radius
                ),
            );
        }
    }
}

fn shape_dims(shape: &ShapeDef) -> Vec<(&'static str, f32)> {
    match shape {
        ShapeDef::Cuboid { size } => {
            vec![
                ("cuboid.size.x", size[0]),
                ("cuboid.size.y", size[1]),
                ("cuboid.size.z", size[2]),
            ]
        }
        ShapeDef::Sphere { radius } => vec![("sphere.radius", *radius)],
        ShapeDef::Capsule { radius, length } => {
            vec![("capsule.radius", *radius), ("capsule.length", *length)]
        }
        ShapeDef::Cylinder { radius, height } => {
            vec![("cylinder.radius", *radius), ("cylinder.height", *height)]
        }
        ShapeDef::Plane { size } => {
            vec![("plane.size.width", size[0]), ("plane.size.depth", size[1])]
        }
    }
}

fn validate_shape_dims(dims: Vec<(&'static str, f32)>, base: &str, diags: &mut Vec<Diagnostic>) {
    for (field, value) in dims {
        if value <= 0.0 {
            diags.push(Diagnostic::semantic(
                base,
                format!("{field} must be positive, got {value}"),
            ));
        }
    }
}

/// Parse `#rgb` or `#rrggbb` into linear-ish RGB bytes. Public so the engine
/// loader uses the exact same interpretation as the validator.
pub fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let hex = s.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let mut out = [0u8; 3];
            for (i, c) in hex.chars().enumerate() {
                let v = c.to_digit(16)? as u8;
                out[i] = v * 17;
            }
            Some(out)
        }
        6 => {
            let mut out = [0u8; 3];
            for i in 0..3 {
                out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
            }
            Some(out)
        }
        _ => None,
    }
}
