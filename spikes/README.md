# M0 Spikes

Throwaway-but-kept experiments that ratify the provisional defaults in
[DESIGN.md §7](../DESIGN.md). Each spike is a headless, deterministic binary
with PASS/FAIL assertions — run them with `cargo run -p <spike>`.

## Physics: `avian3d` vs `bevy_rapier3d` (via `bevy_tnua`)

Per the design doc, the tiebreaker is the **character-controller test**: both
spikes run the identical scenario — settle onto ground, walk +X for 2 s, jump
while moving, land — at a manually-stepped 60 Hz fixed timestep with
assertions on grounding, displacement, jump apex, drift, and NaN-freedom.

Results (2026-06-11, M-series macOS, dev profile with opt-level 3 deps):

| | `plinth-spike-avian` | `plinth-spike-rapier` |
|---|---|---|
| Versions | avian3d 0.6.1 + bevy-tnua 0.31 + tnua-avian3d 0.11.1 | bevy_rapier3d 0.33 + bevy-tnua 0.31 + tnua-rapier3d 0.16 |
| Settle (float_height 1.0) | y=1.066, grounded | y=1.050, grounded |
| Walk 2 s @ 4 m/s (ideal 8.0 m) | **7.897 m** | **7.900 m** |
| Jump apex | 2.197 | 2.190 |
| Land + drift | grounded, z≈0 | grounded, z=0 |
| Verdict | **PASS** | **PASS** |
| Perf (300 updates, headless) | 85 ms (~3.5k updates/s) | 59 ms (~5.1k updates/s) |

Behavior is a dead heat — tnua abstracts both backends faithfully. The perf
gap is fixed overhead in a 3-collider scene (avian runs extra scene/asset
bookkeeping systems), not real-scene scaling evidence.

Qualitative notes gathered while writing the spikes (the LLM-legibility axis
the façade optimizes for):

- **avian is the more Bevy-idiomatic API.** Components are the whole story
  (`RigidBody::Static`, full-extent `Collider::cuboid(40, 1, 40)`, plugin
  takes the schedule directly). rapier carries physics-engine conventions:
  half-extent cuboids, `RigidBody::Fixed`, an explicit `Velocity` component
  required for tnua, `.in_fixed_schedule()` builder.
- **tnua treats both as first-class backends**, so the character-controller
  layer is portable either way; switching backends later is contained.
- Both offer an `enhanced-determinism` feature and wasm support.
- Ecosystem pull for the v2 networking phase: the replication crates we're
  most likely to wrap (lightyear, bevy_replicon examples) lean avian.

**Verdict: avian3d.** With the character-controller test tied, the AI-first
criteria break it: avian's components-ARE-the-API design (full-extent
colliders, no `Velocity` bookkeeping, physics state queryable as plain ECS
components) is the more LLM-legible and Bevy-idiomatic surface, the escape
hatch stays pure ECS, and the replication crates targeted for v2 (lightyear,
bevy_replicon examples) lean avian. Rapier remains reachable via tnua's
backend abstraction if a game outgrows avian.

Headless findings Plinth's runtime preset must absorb (and worth filing
upstream): avian 0.6 unconditionally reads `AssetEvent<Mesh>` and
`SceneSpawner`, so headless apps need `AssetPlugin` + `ScenePlugin` +
`init_asset::<Mesh>()` even with primitive-only colliders.

## Input: `leafwing-input-manager`

Exercised inside the avian spike: movement and jump are driven **from code**
— no window, no devices — by mutating `ButtonInput<KeyCode>` and letting
bevy_input → leafwing `InputMap` → `ActionState` → tnua run the full real
pipeline. PASS; this is the injection path the playtest MCP will use.

Design finding: inject at the **device level** (`ButtonInput`), not by
calling `ActionState::press` manually — leafwing recomputes `ActionState`
from mappings every frame, so manual presses get overwritten unless you win
a fragile ordering race (and leafwing 0.20 removed its `MockInput` helper).
