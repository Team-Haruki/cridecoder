# AGENTS.md

## Project Overview

**cridecoder** is a pure Rust library for decoding CRI Middleware formats, with optional Python bindings via pyo3/maturin.

### Credits

CRI format implementation is based on [vgmstream](https://github.com/vgmstream/vgmstream).

## Architecture

```
src/
├── lib.rs          # Crate root, re-exports public API, #[pymodule] (behind "python" feature)
├── reader.rs       # Binary reader utilities (endianness, primitive types, alignment)
├── acb.rs          # ACB module root (re-exports submodules)
├── acb/
│   ├── consts.rs   # ACB/UTF constants and helpers
│   ├── utf.rs      # CRI UTF table parser
│   ├── afs.rs      # AFS2 archive parser
│   ├── track.rs    # Track list extraction from ACB
│   └── extractor.rs # ACB extraction logic
├── hca.rs          # HCA module root (re-exports submodules)
├── hca/
│   ├── decoder.rs  # Core HCA decoder (ClHca, header parsing, block decoding)
│   ├── hca_file.rs # High-level HcaDecoder with streaming, WAV output, key testing
│   ├── tables.rs   # Lookup tables for HCA decoding
│   ├── cipher.rs   # HCA encryption/decryption cipher
│   ├── ath.rs      # ATH (Absolute Threshold of Hearing) tables
│   ├── bitreader.rs # Bit-level reader/writer
│   └── imdct.rs    # Inverse MDCT transform
├── usm.rs          # USM module root (re-exports submodules)
├── usm/
│   ├── extractor.rs # USM extraction (video/audio stream demuxing)
│   └── metadata.rs  # USM metadata reading and JSON export
└── python.rs       # Python bindings (behind "python" feature)
```

## Key Types

- `extract_acb_from_file()` / `extract_acb()` — ACB extraction entry points
- `HcaDecoder` — High-level HCA to WAV/PCM decoder
- `extract_usm_file()` / `extract_usm()` — USM extraction entry points
- `ClHca` — Low-level HCA decoder state machine

## Building

```bash
# Pure Rust
cargo build
cargo test                              # Unit tests only
RUST_MIN_STACK=16777216 cargo test       # Unit + integration tests (HCA needs larger stack)

# Python extension
python3 -m maturin build --release

# crates.io dry run
cargo publish --dry-run --allow-dirty
```

## Testing

- **Unit tests**: Inline `#[cfg(test)] mod tests` in most modules
- **Integration tests**: `tests/integration_tests.rs` — requires `se_0126_01.acb` and `0703.usm` test fixtures in project root
- Integration tests need `RUST_MIN_STACK=16777216` due to large `ClHca` struct

## Conventions

- Use `thiserror` for error types
- Use `byteorder` for binary reading via the `reader.rs` wrapper
- Use `encoding_rs` for Shift-JIS text decoding (CRI uses Shift-JIS strings)
- Public API lives in module root files (`acb.rs`, `hca.rs`, `usm.rs`); internals are `mod` (private)
- Python bindings are behind `#[cfg(feature = "python")]` so they're opt-in
