# cridecoder

A pure Rust library for CRI Middleware codec encoding and decoding. Supports ACB/AWB audio containers, HCA (High Compression Audio) encoding/decoding, and USM video container extraction/building.

## Credits

This project's CRI format implementation is based on and inspired by [vgmstream](https://github.com/vgmstream/vgmstream), a library for playing streamed audio from video games. Many thanks to the vgmstream contributors for their reverse-engineering work on CRI Middleware formats.

## Features

- **ACB/AWB Extraction & Building** — Parse and create CRI ACB audio containers
- **HCA Encoding & Decoding** — Encode PCM to HCA, decode HCA to PCM/WAV
- **USM Extraction & Building** — Extract or create USM video containers
- **USM Metadata** — Read and export USM metadata as structured JSON
- **Key Testing** — Test decryption keys for encrypted HCA files
- **Encryption Support** — Encode HCA with encryption keys
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

### ACB Building

```rust
use std::io::Cursor;
use cridecoder::{AcbBuilder, TrackInput};

let hca_data = std::fs::read("track.hca").unwrap();
let track = TrackInput::new("my_track", 0, hca_data);

let mut builder = AcbBuilder::new();
builder.add_track(track);

let mut output = Cursor::new(Vec::new());
builder.build(&mut output, None).unwrap();
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

### PCM to HCA

```rust
use std::io::Cursor;
use cridecoder::{HcaEncoder, HcaEncoderConfig};

// Generate or load PCM samples (interleaved stereo f32)
let samples: Vec<f32> = vec![0.0; 44100 * 2]; // 1 second of silence

let config = HcaEncoderConfig::new(44100, 2)  // 44.1kHz stereo
    .with_bitrate(256_000);  // 256 kbps

let mut encoder = HcaEncoder::new(config).unwrap();
let mut output = Cursor::new(Vec::new());
encoder.encode(&samples, &mut output).unwrap();
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

### USM Building

```rust
use std::io::Cursor;
use cridecoder::UsmBuilder;

let video_data = std::fs::read("video.m2v").unwrap();

let builder = UsmBuilder::new("my_video".to_string())
    .video(video_data);

let mut output = Cursor::new(Vec::new());
builder.build(&mut output).unwrap();
```

## Supported Formats

| Format | Description | Operations |
|--------|-------------|------------|
| ACB | CRI Audio Container | Extract / Build |
| AWB | CRI Audio Waveform Bank | Extract / Build |
| HCA | High Compression Audio | Encode / Decode |
| USM | CRI Video Container | Extract / Build |

## License

MIT
