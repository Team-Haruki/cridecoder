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

## Git Commit Format

All commits must follow: `[Type] Short description starting with capital letter`

Types: `[Feat]`, `[Fix]`, `[Chore]`, `[Docs]`. Imperative mood, no trailing period, ≤ ~70 chars. Agent commits must include a sign-off line: `Signed-off-by: <agent-name>`.

## Important Technical Notes

- The `ClHca` struct (HCA decoder state) is ~200KB on stack. Integration tests **require** `RUST_MIN_STACK=16777216` or they will stack overflow.
- ACB files may contain embedded AWB data or reference external `.awb` files.
- USM files contain interleaved video/audio chunks with XOR masking for encryption.
- HCA encryption masks the file magic bytes (`HCA\0` becomes `0xC8C3C100`).
- Integration tests skip gracefully if fixture files (`se_0126_01.acb`, `0703.usm`) are not present.
