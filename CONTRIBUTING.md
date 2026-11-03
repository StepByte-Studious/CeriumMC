# Contributing to CeriumMC

Thank you for helping. Please read this whole page before opening a pull request. The clean-room rules are not optional.

## Clean-room rules

CeriumMC must be our own work. Breaking these rules can put the whole project at legal risk, so any PR that breaks them is closed.

1. **Study behaviour, never transcribe code.** You may study vanilla's data files, the reports from its data generator, network traffic, and what happens in game. You may use class and package *names* to find where a system lives. You must not copy decompiled code, or translate it line by line into Rust.
2. **Write research notes first.** Findings go in `docs/research/<topic>.md`, in your own words: what the system does, its edge cases, and how you measured them. Code is written from those notes.
3. **Vanilla is the oracle.** When behaviour is unclear, run the vanilla server and measure it. Tests compare CeriumMC against vanilla.
4. **Never commit Mojang files.** That means no jars, no extracted classes, no data or asset files, and no generated reports. `.cached/` and `*.jar` are git-ignored for this reason. CI downloads the vanilla jar itself and checks its hash.

## Workflow

- **CI is the source of truth.** Every PR must pass the full CI run: format, clippy, docs, the minimum supported Rust version (MSRV), the dependency policy, and tests on every platform. You don't need a local Rust toolchain. Pushing a branch and reading CI results is a supported workflow.
- **Follow the roadmap.** Work maps to a step in [Roadmap.md](Roadmap.md). Mention the step number in your PR description.
- **Keep changes small.** One focused change per PR.

## Code standards

- Rust edition 2024. MSRV is set by `rust-version` in the root `Cargo.toml`.
- `cargo fmt` formatting. Clippy must pass with warnings treated as errors.
- `unsafe` is denied workspace-wide. Any exception needs a documented reason and a review.
- All code must be portable: x86_64, AArch64 and 32-bit ARM. No architecture-specific code without a portable fallback.
- Game logic must be deterministic. Results must not depend on thread count, scheduling or CPU architecture.

## Dependencies

Each new dependency needs a reason in the PR description. It must:

- use a license allowed by `deny.toml`;
- build on every CI target;
- avoid C or system libraries unless explicitly approved.

## License

By contributing, you agree that your contributions are dual licensed under MIT OR Apache-2.0, as described in the [README](README.md#license).
