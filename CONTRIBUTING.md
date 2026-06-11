# Contributing to Plinth

Thanks for your interest — early contributors shape everything here.

## Ground rules

- **License:** all contributions are dual-licensed MIT OR Apache-2.0, the same as the project. By contributing you agree your work is released under both.
- **DCO, no CLA:** every commit must carry a `Signed-off-by` line certifying the [Developer Certificate of Origin](https://developercertificate.org/). Use `git commit -s`. There is no contributor license agreement and never will be.
- **API stability is the product.** Changes to the public `plinth` façade API require an RFC issue describing the motivation, the API surface, and the migration story *before* a PR. Internal code, docs, tooling, and examples don't need RFCs — just open a PR.
- **Docs are load-bearing.** Every public API example must compile (doctests run in CI). An agent will read what you write; broken examples teach broken code.

## Development setup

1. Install stable Rust via [rustup](https://rustup.rs) (the pinned toolchain in `rust-toolchain.toml` applies automatically).
2. Linux only: install Bevy's system dependencies, e.g. on Debian/Ubuntu:
   `sudo apt-get install libasound2-dev libudev-dev pkg-config`
3. `cargo test --workspace` should pass before you open a PR; CI also runs `cargo fmt --check` and `cargo clippy -- -D warnings`.

## Where help is wanted right now

See [DESIGN.md](DESIGN.md) for the roadmap (M0–M4). Issues tagged `good first issue` are kept genuinely small and well-scoped.
