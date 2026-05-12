# c4-rs

Standalone Rust port of the C4 content identification and c4m tooling.

## Development

Use Nix to enter a shell with `mise` and `rustup`, then let `mise` select the Rust toolchain:

```bash
nix develop
mise run test
```

The Rust toolchain is pinned in `mise.toml` and is managed through rustup, not by Nix. On macOS the build and test tasks also avoid Nix's Darwin linker wrappers so the rustup toolchain links against the Apple Command Line Tools SDK.
