# c4-rs

Rust port of [Avalanche-io/c4](https://github.com/Avalanche-io/c4), the original Go implementation of C4 content identification and c4m tooling.

[![CI](https://github.com/bgyss/c4-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/bgyss/c4-rs/actions/workflows/ci.yml)
[![Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](./LICENSE)

## What is C4?

C4 is a content identification system for files and filesystem-like trees. A C4 ID is derived from the content itself, so the same bytes produce the same identifier on any machine and in any implementation. The format is based on SHA-512 and uses a `c4`-prefixed, filename-safe base58 string.

```bash
$ echo -n "hello" | c4-rs id
c447Fm3BJZQ62765jMZJH4m28hrDM7Szbj9CUmj4F4gnvyDYXYz4WfnK2nYRhFvRgYEectEXYBYWLDpLo6XGNAfKdt
```

C4 also represents directory snapshots as c4m manifests: plain-text listings that combine familiar filesystem metadata with C4 IDs.

```bash
$ c4-rs id ./project
-rw-r--r-- - 13 README.md c44iCq6un9W47x7ydjJSWp4arMJ...
drwxr-xr-x - 66 src/ c44nbgL6nkBWsEBDCUCr4LufsjVhJt...
```

The manifest is intended to be readable, diffable, pipeable, and easy to process with normal Unix tools.

## Relationship to the Go Original

This project is a derivative Rust port of [Avalanche-io/c4](https://github.com/Avalanche-io/c4), which is the original Go implementation. The goal is to preserve compatible C4 IDs, c4m text, and common CLI workflows while providing a small Rust crate and a fast standalone `c4-rs` binary.

The Go project remains the upstream reference for the broader C4 toolkit, documentation, and ecosystem. This Rust port currently implements the core identifier, manifest, scan, store, tree, and reconciliation primitives used by the CLI, but it is not yet a full replacement for every feature in the Go toolkit.

## Install

Download prebuilt archives from the [GitHub releases page](https://github.com/bgyss/c4-rs/releases):

- `c4-rs-macos.tar.gz`
- `c4-rs-linux.tar.gz`
- `c4-rs-windows.zip`

Or build from source:

```bash
git clone https://github.com/bgyss/c4-rs.git
cd c4-rs
cargo build --release
```

The release binary will be at `target/release/c4-rs`.

## Common Workflows

Generate an ID for stdin:

```bash
echo -n "hello" | c4-rs id
```

Snapshot a file or directory:

```bash
c4-rs id ./deliverables > deliverables.c4m
```

Compare two states:

```bash
c4-rs diff old.c4m ./deliverables > changes.c4m
```

Merge manifests or directories:

```bash
c4-rs merge branch-a.c4m branch-b.c4m > merged.c4m
```

List paths from a manifest:

```bash
c4-rs paths deliverables.c4m
```

Store content in a local folder store and retrieve it by ID:

```bash
C4_STORE=.c4 c4-rs id -s ./file.bin
C4_STORE=.c4 c4-rs cat c4...
```

## Commands

| Command | What it does |
|---------|-------------|
| `c4-rs id` | Identify stdin, files, directories, or c4m files |
| `c4-rs cat` | Print file content, canonicalize c4m files, or retrieve content by C4 ID |
| `c4-rs paths` | Convert between c4m manifests and plain path lists |
| `c4-rs merge` | Combine entries from multiple manifests or directories |
| `c4-rs diff` | Produce a manifest-style difference between two states |
| `c4-rs patch` | Resolve a manifest chain to its final manifest |
| `c4-rs log` | Print manifest-chain history IDs |
| `c4-rs split` | Split a manifest chain into before/after files |
| `c4-rs intersect` | Print entries shared by two states |
| `c4-rs explain` | Print a short human-readable summary for supported commands |
| `c4-rs version` | Print the CLI version |

## Scan Modes

`c4-rs id` supports scan modes compatible with the common C4 workflow:

```bash
c4-rs id -m s ./project   # structure only
c4-rs id -m m ./project   # metadata
c4-rs id -m f ./project   # full content IDs
```

Full mode is the default. You can exclude simple filename patterns:

```bash
c4-rs id --exclude "*.log" --exclude target ./project
```

## Rust Library

The crate exposes a small `c4` library as well as the `c4-rs` binary:

```rust
use c4::identify;

fn main() -> std::io::Result<()> {
    let id = identify("hello".as_bytes())?;
    println!("{id}");
    Ok(())
}
```

Core modules:

- `id` for C4 IDs and parsing
- `sha512` for hashing backends
- `c4m` for manifest parsing and formatting
- `scan` for filesystem snapshots
- `store` for local content storage
- `tree` for C4 tree reading
- `reconcile` for manifest reconciliation planning

## Development

Use Nix to enter a shell with `mise` and `rustup`, then let `mise.toml` select the pinned Rust toolchain:

```bash
nix develop
mise run check
```

Useful commands:

```bash
mise run build
mise run test
cargo fmt --all --check
cargo build --release --locked
```

On macOS, the `mise` tasks intentionally avoid Nix Darwin linker wrappers so the rustup-managed toolchain links against the Apple Command Line Tools SDK.

## Compatibility Notes

This port is designed to produce the same C4 IDs as the Go original for the same content. c4m formatting and the implemented CLI commands aim to follow the same model, but the Go implementation currently covers more of the full C4 ecosystem, including richer store and patch behavior.

When behavior matters for interoperability, compare against [Avalanche-io/c4](https://github.com/Avalanche-io/c4), the upstream Go reference.

## Upstream C4 Ecosystem

The original project README points to the wider C4 toolkit and language ecosystem. Useful upstream links:

- [Avalanche-io/c4](https://github.com/Avalanche-io/c4) - original Go CLI and library
- [C4 toolkit](https://github.com/Avalanche-io/c4toolkit) - binary releases and toolkit matrix
- [c4m documentation](https://github.com/Avalanche-io/c4/tree/main/c4m) - manifest format documentation in the Go project
- [C4 ID whitepaper](https://cccc.io/c4id-whitepaper-u2.pdf) - background on the identifier format

## License

Apache 2.0. See [LICENSE](./LICENSE).
