# cridecoder

A pure Rust library for CRI Middleware codec decoding. Supports ACB/AWB audio containers, HCA (High Compression Audio) decoding, and USM video container extraction.

## Credits

This project's CRI format implementation is based on and inspired by [vgmstream](https://github.com/vgmstream/vgmstream), a library for playing streamed audio from video games. Many thanks to the vgmstream contributors for their reverse-engineering work on CRI Middleware formats.

## Features

- **ACB/AWB Extraction** — Parse CRI ACB audio containers and extract embedded/external audio tracks
- **HCA Decoding** — Decode HCA audio to PCM samples (f32 or i16) or WAV files
- **USM Extraction** — Extract MPEG2 video and ADX audio streams from USM video containers
- **USM Metadata** — Read and export USM metadata as structured JSON
- **Key Testing** — Test decryption keys for encrypted HCA files
- **Pure Rust** — No C dependencies, works on any platform Rust supports

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
cridecoder = "0.1"
```

### ACB Extraction

```rust
use std::path::Path;
use cridecoder::extract_acb_from_file;

let tracks = extract_acb_from_file(
    Path::new("audio.acb"),
    Path::new("output/"),
).unwrap();

if let Some(tracks) = tracks {
    for track in &tracks {
        println!("Extracted: {}", track);
    }
}
```

### HCA to WAV

```rust
use std::fs::File;
use cridecoder::HcaDecoder;

let mut decoder = HcaDecoder::from_file("audio.hca").unwrap();
let info = decoder.info();
println!("Sample rate: {}, Channels: {}", info.sampling_rate, info.channel_count);

let mut output = File::create("output.wav").unwrap();
decoder.decode_to_wav(&mut output).unwrap();
```

### USM Extraction

```rust
use std::path::Path;
use cridecoder::extract_usm_file;

let files = extract_usm_file(
    Path::new("video.usm"),
    Path::new("output/"),
    None,   // optional video decryption key
    false,  // export audio
).unwrap();

for file in &files {
    println!("Extracted: {:?}", file);
}
```

## Supported Formats

| Format | Description | Operation |
|--------|-------------|-----------|
| ACB | CRI Audio Container | Extract embedded HCA/ADX tracks |
| AWB | CRI Audio Waveform Bank | External track storage for ACB |
| HCA | High Compression Audio | Decode to WAV/PCM |
| USM | CRI Video Container | Extract M2V video + ADX audio |

## License

MIT
