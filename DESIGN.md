# Plinth — Engine Design Document

> **Plinth**: the stable base your game stands on. An open-source, code-first, AI-first 3D game engine that lets solo devs and small teams use AI agents to build high-performance 3D games — built on Bevy.
>
> Name verified available 2026-06-11: crates.io `plinth` free; `plinth.games` unregistered. Home: [github.com/Luminary-Analytics/plinth](https://github.com/Luminary-Analytics/plinth). crates.io `plinth` + `plinth-cli` reserved (0.0.1 stubs published 2026-06-11). `plinth.games` registered 2026-06-11 (GoDaddy, renews 2027-06-12) and 301-forwards to this repo until the docs site exists.
>
> Status: decisions ratified 2026-06-11 via design interview. Owner: Rich Bellantoni.

## One-paragraph thesis

We are not building a renderer — we are building the AI-native layer the ecosystem is missing. The engine is an opinionated, batteries-included framework on top of Bevy whose two pillars reinforce each other: a **deliberately stable, LLM-legible façade API** (attacking Bevy's churn problem, the #1 cause of AI-hallucinated game code) and **first-class agent tooling** — an MCP server that lets a coding agent scaffold a project, build against the façade, and *playtest the running game itself* (screenshots, state queries, input injection). The headline no other engine can print: **your agent can playtest your game.**

## Decision log

| # | Decision | Choice |
|---|----------|--------|
| 1 | Foundation | Build on **Bevy** (Rust); this project is the AI-native layer, not a new renderer |
| 2 | Product identity | **Stable façade API + agent tooling** are the core; runtime AI / asset generation are later modules |
| 3 | Façade depth | **Additive** (Next.js-over-React model); raw Bevy is a supported, documented escape hatch |
| 4 | Flagship genre | **Single-player third-person ARPG** vertical slice; hero shooter is the v2/v3 flagship |
| 5 | Platforms | Desktop (Win/Mac/Linux) tier 1; **web (wasm/WebGPU) tier 2**; mobile post-v1; consoles out of scope |
| 6 | Authoring model | **Behavior in Rust; scenes/prefabs/content in declarative, schema-published data files** with CLI validation |
| 7 | Iteration loop | **Agent playtest loop**: data hot-reload + MCP runtime inspection/control + compile-time budgets. No code-hot-reload promise; no scripting language in v1 |
| 8 | Asset story | **Importers + curated CC0 packs + license/provenance manifest + MCP search-and-place** over indexed open libraries. No hosted registry in v1 |
| 9 | Networking | v1 ships zero netcode but **enforces multiplayer-ready architecture**; v2 wraps an ecosystem replication crate; MMORPG documented as out-of-engine-scope with an architecture guide |
| 10 | License | **MIT OR Apache-2.0 dual**, DCO sign-off, no CLA, public **no-royalties-ever** pledge |
| 11 | Capacity & launch | ~15–25 hrs/week. Repo quietly public from day 0; **loud launch on the "magic moment" demo** (~4–6 months) |

## 1. Foundation & façade

- Each engine release **pins one Bevy version**; we absorb Bevy migration churn on users' behalf. Migration guides are published in machine-readable form so agents can apply them.
- The façade covers the golden path (~90% of game code): app bootstrap, scenes, transforms, camera, character control, combat primitives, UI, audio, save/load.
- Dropping to raw Bevy ECS is a **first-class, documented** pattern, not a leak. Context packs document both layers and when to cross the boundary.
- Stability policy: semver on the façade; deprecations live at least one minor release; breaking changes require an RFC.
- We upstream fixes to Bevy rather than forking. Bevy plugin ecosystem (physics, UI, etc.) remains fully usable.

## 2. Authoring model

- **Rust for behavior.** Systems, components, game logic.
- **Declarative data for content.** Scenes, prefabs, item tables, tuning values live in data files.
  - The schema is a **published, versioned artifact** — it doubles as AI context.
  - `engine validate` checks any data file in milliseconds, with agent-actionable error messages. Agents get schema feedback without paying Rust compile times.
  - Stay aligned with Bevy's evolving scene-format direction where practical; **our schema + validators are the stable contract**, whatever the underlying serialization.
- Content files must diff cleanly in PRs; future visual tooling reads/writes the same files.

## 3. The agent playtest loop (crown jewel)

The MCP server ships with the engine and connects to a **running game** (windowed or headless):

- **Observe**: capture screenshots; query ECS state ("list enemies within 10m of player"); subscribe to structured frame events.
- **Act**: inject input through the input abstraction; pause/step the simulation; teleport/modify entities for test setup.
- **Iterate**: data hot-reload applies scene/asset edits live; compile-time budgets keep code changes fast (prebuilt engine artifacts, dynamic-linking dev profile, published "<N seconds to running game" targets enforced in CI).

Explicit non-promises for v1: Rust code hot-reload (may exist behind an experimental flag; too fragile to promise) and an embedded scripting language (revisit post-v1 for *modding*, not core authoring — it would split the API surface and the AI context in two).

## 4. Open-asset pipeline

- First-class importers: glTF/GLB, PBR textures, common audio formats (largely inherited from Bevy, wrapped in the golden path).
- Templates ship pre-wired with **curated CC0 starter packs** (e.g., Kenney, Quaternius, PolyHaven, ambientCG).
- **Asset manifest** per project: provenance + license metadata for every asset, auto-generated CREDITS file, warnings on unknown licenses. This is what makes it safe for an *agent* to choose assets autonomously.
- MCP **search-and-place**: agent searches indexed open libraries, license-checks, downloads, registers in the manifest, and places the asset in a scene in one turn. Index only libraries whose ToS permits it; cache metadata, not assets.
- Future (not v1): hosted registry ("npm for game assets") can grow on top of the manifest format. AI asset *generation* services are community-plugin territory.

## 5. Multiplayer roadmap

- **v1 (now):** zero netcode, but enforced architecture: fixed-timestep simulation core separated from rendering; all input flows through a replayable abstraction (the same one the playtest MCP uses); determinism discipline where cheap; no hidden global mutable state in the façade.
- **v2:** façade wrapper over an ecosystem replication crate (evaluate `lightyear` vs `bevy_replicon`) — server-authoritative co-op / tavern-scale multiplayer. Hero shooter (rollback, lag compensation, dedicated servers) becomes the v2/v3 flagship the way the ARPG is v1's.
- **MMORPG:** honestly documented as out of engine scope. The engine is a great MMO *client* and provides clean server-simulation building blocks plus an architecture guide; the persistent-world backend is a bespoke project. No "MMO-ready" marketing, ever.

## 6. License, governance, sustainability

- **MIT OR Apache-2.0 dual** (Bevy's convention — frictionless code exchange with upstream).
- **DCO** sign-off; **no CLA**.
- Public pledge: **no royalties, no runtime fees, ever** — stated in the README, structurally credible because of the permissive license.
- Governance: BDFL (Rich) initially; RFC process for façade API changes (the stability promise needs gatekeeping); promote maintainers from consistent contributors.
- Sustainability: GitHub Sponsors at launch; later options are foundation membership, support, services — never license leverage.

## 7. Recommended defaults (provisional — not interviewed; confirm via spikes)

| Area | Default | Notes |
|------|---------|-------|
| Physics | `avian` vs `rapier` | 1-week spike; pick whichever survives the character-controller test best |
| Character controller | `bevy_tnua` candidate | Depends on physics pick |
| Input | `leafwing-input-manager` | Replayable abstraction fits the playtest loop and netcode-readiness |
| UI | `bevy_ui` golden path; `bevy_egui` escape hatch | ARPG needs inventory/HUD early |
| Audio | `bevy_audio` default; evaluate `kira` | |
| Navigation/AI | navmesh crate spike (`oxidized_navigation` / `vleue_navigator`) | Needed for enemy AI in flagship |
| Docs site | Astro Starlight or mdBook | Plus `llms.txt` + versioned context packs |
| Repo | Monorepo | `crates/` (façade, cli, mcp, assets), `schemas/`, `templates/`, `examples/`, `showcase/`, `docs/`, `context-packs/` |
| Distribution | crates.io + single CLI binary | `engine new` scaffolds project incl. agent config (MCP setup, skills) |
| CI | GitHub Actions | clippy, fmt, doctests (every doc example compiles — hallucination control), schema validation, headless render smoke tests on 3 OSes + wasm build, compile-time budget check |

## 8. Milestones (~4–6 months at 15–25 hrs/week)

- **M0 — Foundations (weeks 1–2):** real name; repo scaffold + CI + licenses; Bevy version pin; physics/input spikes; write the north-star README code sample ("the whole pitch in 30 lines").
- **M1 — Façade core (weeks 3–8):** app bootstrap; scene data format + schema + `validate` CLI; transforms/camera/lighting golden path; character controller + third-person camera; data hot-reload. First template runs.
- **M2 — Agent loop (weeks 9–14):** MCP v1: launch/attach, screenshot, ECS query, input injection, pause/step; structured event channel; context pack v1; scaffold wires up agent config.
- **M3 — Asset layer (weeks 15–20):** importer golden path; manifest + credits + license warnings; MCP search-and-place across 2–3 indexed libraries.
- **M4 — Magic moment & launch (weeks 21–24):** combat-arena template (enemies, health, pickups); 90-second demo video of the full loop; docs site; launch (Show HN, Bevy community, r/rust_gamedev).
- **Post-launch:** showcase ARPG slice deepens in public (dogfooding); ~8–10 week release cadence; v2 = networking phase.

## 9. Success metric

> A developer with no Rust experience, using a coding agent, goes from `engine new` to a playable third-person combat arena **that they modified themselves** in one weekend — with the agent verifying its own changes through the playtest MCP.

Secondary: p50 time from "agent receives a change request" to "verified in running game."

## 10. Top risks & mitigations

1. **Bevy churn tax** — pinning + absorbing migrations is recurring work. *Mitigate:* façade test suite as canary; upstream relationships; machine-readable migration guides.
2. **Rust compile times undermine the loop.** *Mitigate:* compile-time budgets enforced in CI; dynamic-linking dev profile; prebuilt engine artifacts.
3. **Runtime-control MCP is novel** — unknown unknowns. *Mitigate:* build M2 early; smallest useful surface first (screenshot + state query + input).
4. **Solo-maintainer capacity.** *Mitigate:* scope discipline (this document); lean on ecosystem crates; contributor onboarding docs from day one; quietly-public repo invites early collaborators.
5. **"AI-first" skepticism/noise.** *Mitigate:* lead with the demo, not the adjective; honest scoping (especially around MMORPGs) as a trust asset.
6. **Asset indexing legality.** *Mitigate:* index only permissively-licensed libraries with permissive ToS; metadata-only caching; manifest makes provenance auditable.
