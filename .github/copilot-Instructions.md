# Copilot Instructions for cridecoder

## Project Context

This is a pure Rust library (`cridecoder`) for decoding CRI Middleware audio/video formats (ACB, HCA, USM). The CRI format implementation is based on [vgmstream](https://github.com/vgmstream/vgmstream). It also provides optional Python bindings via pyo3/maturin.

## Code Style

- **Rust edition**: 2021
- **Error handling**: Use `thiserror` derive macros for error enums
- **Binary I/O**: Use the `reader.rs` wrapper around `byteorder` for all binary reading
- **Text encoding**: CRI formats use Shift-JIS; use `encoding_rs::SHIFT_JIS` for decoding
- **Module structure**: Use Rust 2018+ flat module style (`src/acb.rs` + `src/acb/` directory), not `mod.rs`
- **Feature gates**: Python bindings use `#[cfg(feature = "python")]` — pure Rust builds should not depend on pyo3
- **Tests**: Unit tests are inline `#[cfg(test)] mod tests`, integration tests are in `tests/`

## Important Notes

- The `ClHca` struct (HCA decoder state) is very large (~200KB on stack). Use `RUST_MIN_STACK=16777216` when running integration tests
- ACB files may contain embedded AWB data or reference external `.awb` files
- HCA files support encryption — use `HcaDecoder::set_key()` or `KeyTest` for key testing
- USM files contain interleaved video (M2V) and audio (ADX) chunks with XOR masking

## Public API

```rust
// ACB
pub fn extract_acb_from_file(path, output_dir) -> Result<Option<Vec<String>>>
pub fn extract_acb(reader, output_dir, awb_reader) -> Result<Vec<String>>

// HCA
pub struct HcaDecoder { ... }
impl HcaDecoder {
    pub fn from_file(path) -> Result<Self>
    pub fn from_reader(reader) -> Result<Self>
    pub fn info(&self) -> &HcaInfo
    pub fn decode_to_wav(&mut self, writer) -> Result<()>
    pub fn decode_all(&mut self) -> Result<Vec<f32>>
}

// USM
pub fn extract_usm_file(path, output_dir, key, export_audio) -> Result<Vec<PathBuf>>
pub fn extract_usm(reader, output_dir, name, key, export_audio) -> Result<Vec<PathBuf>>
```

## Testing

```bash
cargo test                              # Unit tests only
RUST_MIN_STACK=16777216 cargo test       # Full test suite (needs test fixture files)
```
