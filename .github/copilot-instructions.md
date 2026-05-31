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
