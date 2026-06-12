# Plinth

> **The stable base your game stands on.**

Plinth is an open-source, code-first, **AI-first** 3D game engine for solo developers and small teams, built on [Bevy](https://bevyengine.org).

The thesis: the missing piece in game development isn't another renderer — it's an engine designed from the ground up to be driven by you *and your coding agent together*. Plinth is two things that make each other work:

1. **A deliberately stable, LLM-legible framework API.** An opinionated golden path over Bevy that covers ~90% of game code and doesn't shift underneath you — each Plinth release pins one Bevy version, and Plinth absorbs the migration churn. Raw Bevy ECS remains a first-class, documented escape hatch.
2. **First-class agent tooling.** Plinth ships with an MCP server, machine-readable docs, and project scaffolding — so your agent doesn't just write game code, it connects to the **running game**: screenshots, entity queries, input injection, simulation stepping. **Your agent can playtest your game.**

## The north star

This is where the API is headed (aspirational — see status below):

```rust
use plinth::prelude::*;

fn main() {
    Game::new("Ember Hollow")
        // Scenes are schema-validated data files. Edit them while the game runs.
        .level("scenes/arena.scene.json")
        // Golden-path third-person character: controller, camera, animation.
        .player(Character::third_person())
        .enemies(Spawn::wave("goblin", 12))
        .run();
}
```

Then tell your agent: *"make the goblins flee when their health drops below 20%."* It writes the system, rebuilds, reconnects to the running game over MCP, injects inputs, screenshots the fight, and verifies the behavior itself — before you ever pick up the controller.

## Pillars

- **Code-first.** Behavior is Rust; content is declarative, schema-published data files that hot-reload and validate in milliseconds.
- **AI-first, not AI-flavored.** Stable APIs agents can't hallucinate against, docs whose every example compiles in CI, and a runtime your agent can observe and drive.
- **Open assets, safely.** Importers plus a license/provenance manifest: search open asset libraries from your agent, place results in a scene, and get a correct credits file for free.
- **Honest scope.** v1 proves a single-player third-person ARPG. Multiplayer ships in v2 on a v1 architecture built ready for it (fixed-timestep simulation, replayable input). We will never claim "MMO-ready."
- **Free forever.** MIT/Apache-2.0 dual license. No royalties, no runtime fees, ever.

## Status

**Pre-0.1 — quietly public.** Working today: the schema-published scene format with `plinth validate` (millisecond feedback, no engine compile), and scene loading on the ratified stack — `Game::new("…").level("….scene.json")` spawns a playable world with physics, lights, camera, and a walking/jumping player character (`cargo run -p plinth --example arena`). Headless mode steps deterministically for tests and agents. The full design rationale and roadmap live in [DESIGN.md](DESIGN.md). The loud launch happens when the loop above is real.

## Contributing

Early contributors are very welcome — see [CONTRIBUTING.md](CONTRIBUTING.md). Contributions are accepted under the project's dual license with DCO sign-off (no CLA).

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
