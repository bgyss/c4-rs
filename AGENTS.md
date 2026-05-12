# Repository Guidelines

## Project Structure & Module Organization

This is a small Rust 2021 crate for the `c4-rs` CLI and `c4` library. Library entry points live in `src/lib.rs`, while the command-line interface lives in `src/main.rs`. Core modules are split by responsibility: `id.rs` for C4 identifiers, `sha512.rs` for hashing, `c4m.rs` for manifest parsing and formatting, `scan.rs` for filesystem scans, `store.rs` for content storage, `tree.rs` for tree reading, and `reconcile.rs` for manifest operations. Integration tests live in `tests/cli.rs`; module-level unit tests live beside the implementation in `src/*.rs`. Build outputs are under `target/` and should stay untracked.

## Build, Test, and Development Commands

Use the repository shell before normal development:

```bash
nix develop
```

The shell installs `mise` and `rustup`, then lets `mise.toml` select Rust `1.94.0`. Common tasks:

```bash
mise run build   # cargo build with macOS/Nix linker environment cleaned up
mise run test    # cargo test for unit and integration tests
mise run check   # runs build and test
cargo run -- version
```

Plain `cargo build` and `cargo test` are fine outside Nix when the pinned Rust toolchain is already active.

## Coding Style & Naming Conventions

Follow idiomatic Rust formatting with `rustfmt` defaults: four-space indentation, `snake_case` functions and modules, `PascalCase` types, and `SCREAMING_SNAKE_CASE` constants. Keep public library APIs in `src/lib.rs` intentional and minimal. Prefer `io::Result` or domain-specific errors for runtime paths; `unwrap()` is acceptable in tests where failures should be immediate.

## Testing Guidelines

Use Rust’s built-in test framework. Put focused unit tests in the relevant module’s `mod tests` block, and use `tests/cli.rs` for end-to-end CLI behavior through `CARGO_BIN_EXE_c4-rs`. Name tests after the behavior under test, for example `version_and_stdin_id_work` or `merge_diff_patch_log_split_and_intersect_work`. Run `mise run test` before submitting changes.

## Commit & Pull Request Guidelines

This repository currently has no commit history, so use a simple imperative style such as `Add manifest chain parser` or `Fix C4 store lookup error`. Keep commits scoped to one logical change. Pull requests should include a short summary, the commands run for verification, and any CLI behavior changes with example input/output when relevant. Link issues when available and call out compatibility risks for manifest formatting, identifier generation, or storage layout changes.

## Agent-Specific Instructions

Avoid rewriting generated or cached files under `target/`. Preserve the Nix plus `mise` workflow unless the user explicitly asks to change toolchain management.
