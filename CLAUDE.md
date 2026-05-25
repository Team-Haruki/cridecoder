# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**cridecoder** is a pure Rust library for CRI Middleware codec encoding and decoding. Supports ACB/AWB audio containers, HCA audio encoding/decoding, and USM video container extraction/building. Optional Python bindings via pyo3/maturin. CRI format implementation is based on [vgmstream](https://github.com/vgmstream/vgmstream).

## Build & Test Commands

```bash
# Build
cargo build
cargo build --release

# Test (unit tests only — no fixtures needed)
cargo test

# Full test suite (integration tests need se_0126_01.acb and 0703.usm in project root)
RUST_MIN_STACK=16777216 cargo test

# Run a single test
RUST_MIN_STACK=16777216 cargo test test_hca_encoder_roundtrip

# Lint & format
cargo clippy -- -D warnings
cargo fmt --all --check

# Python extension
maturin build --release           # build wheel
maturin develop                   # install in active venv for testing
pytest tests/test_python.py -v    # Python tests (requires maturin develop first)

# Publish dry run
cargo publish --dry-run --allow-dirty
```

## Architecture

The crate has three main modules, each with a module root file (`src/acb.rs`, `src/hca.rs`, `src/usm.rs`) that re-exports the public API, plus a `src/<module>/` directory for internals:

- **ACB** (`src/acb/`): CRI Audio Container extraction and building. Parses UTF tables (`utf.rs`), AFS2 archives (`afs.rs`), extracts tracks (`track.rs`, `extractor.rs`), and builds containers (`builder.rs`).
- **HCA** (`src/hca/`): High Compression Audio codec. Core decoder state machine (`decoder.rs`), high-level decode/WAV API (`hca_file.rs`), encoder (`encoder.rs`), cipher for encryption/decryption (`cipher.rs`), bit-level I/O (`bitreader.rs`), IMDCT transform (`imdct.rs`).
- **USM** (`src/usm/`): CRI Video Container. Demuxes interleaved video (M2V) and audio (ADX) chunks (`extractor.rs`), metadata reading (`metadata.rs`), container building (`builder.rs`).

Supporting files:
- `src/reader.rs` — Binary reader utilities wrapping `byteorder` (endianness, alignment)
- `src/python.rs` — Python bindings (behind `python` feature flag)
- `src/lib.rs` — Crate root, re-exports public API, `#[pymodule]` registration

## Key Conventions

- **Module style**: Rust 2018+ flat module style (`src/acb.rs` + `src/acb/` directory), not `mod.rs`
- **Error handling**: `thiserror` derive macros for error enums
- **Binary I/O**: Use `reader.rs` wrapper for all binary reading, not raw `byteorder` calls
- **Text encoding**: CRI formats use Shift-JIS; use `encoding_rs::SHIFT_JIS`
- **Python feature**: Gated behind `#[cfg(feature = "python")]` — pure Rust builds must not depend on pyo3
- **Dual crate types**: `cdylib` (for Python) + `rlib` (for Rust consumers)

## Important Technical Notes

- The `ClHca` struct (HCA decoder state) is ~200KB on stack. Integration tests **require** `RUST_MIN_STACK=16777216` or they will stack overflow.
- ACB files may contain embedded AWB data or reference external `.awb` files.
- USM files contain interleaved video/audio chunks with XOR masking for encryption.
- HCA encryption masks the file magic bytes (`HCA\0` becomes `0xC8C3C100`).
- Integration tests skip gracefully if fixture files (`se_0126_01.acb`, `0703.usm`) are not present.

## Git commits

All commit subjects must follow:

```text
[Type] Short description starting with capital letter
```

Allowed types:

| Type      | Usage                                                 |
|-----------|-------------------------------------------------------|
| `[Feat]`  | New feature or capability                             |
| `[Fix]`   | Bug fix                                               |
| `[Chore]` | Maintenance, refactoring, dependency or build changes |
| `[Docs]`  | Documentation-only changes                            |

Rules:

- Description starts with a capital letter.
- Use imperative mood: `Add ...`, not `Added ...`.
- No trailing period.
- Keep the subject at or below roughly 70 characters.
- **Agent attribution uses the standard Git `Co-authored-by:` trailer in the commit body, not a free-form `Agent:` line.** This makes GitHub render the co-author avatar on the commit page. The trailer must be on its own line, separated from the subject by a blank line, in the form `Co-authored-by: <Display Name> <email>`. Suggested values per agent:
  - Claude (any 4.x): `Co-authored-by: Claude Opus 4.7 <noreply@anthropic.com>` (substitute the actual model, e.g. `Claude Sonnet 4.6`, `Claude Haiku 4.5`)
  - Codex: `Co-authored-by: Codex <noreply@openai.com>`
  - Copilot: `Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>`

Examples from this repo's history:

```text
[Feat] Add encoding Python bindings
[Fix] Resolve check and clippy warnings
[Chore] Configure Dependabot updates
[Feat] Add encoding support but not tested in game
```

## GitHub Actions workflows

Use the standardized workflow layout in `.github/workflows`:

- `ci.yml` runs on `main` pushes, pull requests targeting `main`, and manual dispatch.
- Rust CI order: `cargo fmt --all -- --check`, `cargo check --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings`, then `cargo test --locked`.
- `release-crate.yml` publishes the Rust crate and keeps its package-specific release flow.
- `release-python.yml` builds and publishes Python artifacts and keeps its package-specific release flow.

Workflow maintenance rules:

- Keep workflow filenames and top-level names aligned: `CI`, `Release`, `Docker`, and optional package-specific names.
- Use `actions/checkout@v6`, `actions/setup-go@v6`, `actions/upload-artifact@v7`, `actions/download-artifact@v8`, `softprops/action-gh-release@v3`, and current Docker actions (`setup-buildx@v4`, `login@v4`, `metadata@v6`, `build-push@v7`).
- Keep `permissions` minimal: `contents: read` for CI/Docker build-only work, `contents: write` for release publishing, and `packages: write` only when pushing container images.
- Use workflow `concurrency` keyed by workflow name and ref, with release jobs using `release-${{ github.ref_name }}` and `cancel-in-progress: false`.
- Do not reintroduce legacy workflow names such as `rust-ci.yml`, `build.yml`, `release-build.yml`, `docker-build.yml`, or `docker-release.yml` unless a package-specific workflow already exists and is intentionally preserved.
