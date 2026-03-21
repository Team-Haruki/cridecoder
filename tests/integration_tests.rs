//! Integration tests for cridecoder
//!
//! These tests require the test fixture files:
//! - se_0126_01.acb (ACB audio container)
//! - 0703.usm (USM video container)
//!
//! Tests are gated behind `#[ignore]` if the files are not present.

use std::fs;
use std::io::Cursor;
use std::path::Path;

/// Test that ACB extraction works with a real .acb file
#[test]
fn test_acb_extraction() {
    let acb_path = Path::new("se_0126_01.acb");
    if !acb_path.exists() {
        eprintln!("Skipping test: se_0126_01.acb not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let result = cridecoder::extract_acb_from_file(acb_path, dir.path());

    let tracks = result.expect("ACB extraction should not error");
    let tracks = tracks.expect("Should find tracks in ACB");
    assert!(tracks.len() > 0, "Should extract at least one track");

    // Verify extracted files exist and are HCA
    for track_path in &tracks {
        let p = Path::new(track_path);
        assert!(p.exists(), "Extracted file should exist: {}", track_path);
        let meta = fs::metadata(p).unwrap();
        assert!(
            meta.len() > 0,
            "Extracted file should not be empty: {}",
            track_path
        );

        // Verify HCA signature (first 4 bytes should match HCA magic masked)
        let data = fs::read(p).unwrap();
        assert!(data.len() >= 8, "HCA file too small: {}", track_path);
        // HCA files start with 'HCA\0' (0x48434100) masked by 0x7F7F7F7F
        let sig = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) & 0x7F7F7F7F;
        assert_eq!(sig, 0x48434100, "Should be HCA file: {}", track_path);
    }
}

/// Test that ACB extraction reports the correct number of tracks
#[test]
fn test_acb_track_count() {
    let acb_path = Path::new("se_0126_01.acb");
    if !acb_path.exists() {
        eprintln!("Skipping test: se_0126_01.acb not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let tracks = cridecoder::extract_acb_from_file(acb_path, dir.path())
        .unwrap()
        .unwrap();

    // se_0126_01.acb is known to have 4 tracks
    assert_eq!(tracks.len(), 4, "Expected 4 tracks from se_0126_01.acb");
}

/// Test that HCA files extracted from ACB can be decoded to WAV
#[test]
fn test_hca_decode_to_wav() {
    let acb_path = Path::new("se_0126_01.acb");
    if !acb_path.exists() {
        eprintln!("Skipping test: se_0126_01.acb not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let tracks = cridecoder::extract_acb_from_file(acb_path, dir.path())
        .unwrap()
        .unwrap();

    // Decode the first HCA track
    let hca_path = &tracks[0];
    let mut decoder = cridecoder::HcaDecoder::from_file(hca_path).expect("Should open HCA file");

    let info = decoder.info();
    assert!(info.sampling_rate > 0, "Sample rate should be > 0");
    assert!(info.channel_count > 0, "Channel count should be > 0");
    assert!(info.block_count > 0, "Block count should be > 0");
    assert!(info.block_size > 0, "Block size should be > 0");
    assert_eq!(
        info.samples_per_block, 1024,
        "Samples per block should be 1024"
    );

    // Decode to WAV
    let mut wav_buf = Vec::new();
    decoder
        .decode_to_wav(&mut wav_buf)
        .expect("Should decode HCA to WAV");

    // Verify WAV header
    assert!(
        wav_buf.len() > 44,
        "WAV output should be > 44 bytes (header)"
    );
    assert_eq!(&wav_buf[0..4], b"RIFF", "Should start with RIFF magic");
    assert_eq!(&wav_buf[8..12], b"WAVE", "Should have WAVE marker");
    assert_eq!(&wav_buf[12..16], b"fmt ", "Should have fmt chunk");
    assert_eq!(&wav_buf[36..40], b"data", "Should have data chunk");
}

/// Test that ALL HCA tracks extracted from ACB can be decoded to valid WAV
#[test]
fn test_all_hca_export_to_wav() {
    let acb_path = Path::new("se_0126_01.acb");
    if !acb_path.exists() {
        eprintln!("Skipping test: se_0126_01.acb not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let tracks = cridecoder::extract_acb_from_file(acb_path, dir.path())
        .unwrap()
        .unwrap();

    assert_eq!(tracks.len(), 4, "Should have 4 tracks");

    for (i, track_path) in tracks.iter().enumerate() {
        let p = Path::new(track_path);
        assert!(p.exists(), "Track {} should exist: {}", i, track_path);
        assert!(
            track_path.ends_with(".hca"),
            "Track {} should be .hca: {}",
            i,
            track_path
        );

        // Decode each HCA to WAV
        let mut decoder = cridecoder::HcaDecoder::from_file(track_path)
            .unwrap_or_else(|e| panic!("Track {} HCA open failed: {:?}", i, e));

        let info = decoder.info().clone();
        assert!(info.sampling_rate > 0, "Track {} sample rate > 0", i);
        assert!(info.channel_count > 0, "Track {} channels > 0", i);
        assert!(info.block_count > 0, "Track {} block count > 0", i);

        let mut wav_buf = Vec::new();
        decoder
            .decode_to_wav(&mut wav_buf)
            .unwrap_or_else(|e| panic!("Track {} WAV decode failed: {:?}", i, e));

        // Verify WAV output
        assert!(
            wav_buf.len() > 44,
            "Track {} WAV too small: {}",
            i,
            wav_buf.len()
        );
        assert_eq!(&wav_buf[0..4], b"RIFF", "Track {} RIFF magic", i);
        assert_eq!(&wav_buf[8..12], b"WAVE", "Track {} WAVE marker", i);

        // Verify WAV data size matches header
        let data_size = u32::from_le_bytes([wav_buf[40], wav_buf[41], wav_buf[42], wav_buf[43]]);
        assert_eq!(
            wav_buf.len() - 44,
            data_size as usize,
            "Track {} WAV data size mismatch",
            i
        );

        eprintln!(
            "Track {}: {} -> WAV {} bytes (rate={}, ch={}, blocks={})",
            i,
            p.file_name().unwrap().to_string_lossy(),
            wav_buf.len(),
            info.sampling_rate,
            info.channel_count,
            info.block_count
        );
    }
}

/// Test HCA decoder info metadata
#[test]
fn test_hca_decoder_info() {
    let acb_path = Path::new("se_0126_01.acb");
    if !acb_path.exists() {
        eprintln!("Skipping test: se_0126_01.acb not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let tracks = cridecoder::extract_acb_from_file(acb_path, dir.path())
        .unwrap()
        .unwrap();

    let decoder = cridecoder::HcaDecoder::from_file(&tracks[0]).unwrap();
    let info = decoder.info();

    // Known values for se_0126_01.hca
    assert_eq!(info.sampling_rate, 44100);
    assert_eq!(info.channel_count, 2);
    assert_eq!(info.block_count, 4931);
    assert_eq!(info.block_size, 341);
    assert_eq!(info.encoder_delay, 128);
    assert_eq!(info.samples_per_block, 1024);
}

/// Test HCA decode_all returns expected sample count
#[test]
fn test_hca_decode_all_samples() {
    let acb_path = Path::new("se_0126_01.acb");
    if !acb_path.exists() {
        eprintln!("Skipping test: se_0126_01.acb not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let tracks = cridecoder::extract_acb_from_file(acb_path, dir.path())
        .unwrap()
        .unwrap();

    let mut decoder = cridecoder::HcaDecoder::from_file(&tracks[0]).unwrap();
    let info = decoder.info().clone();
    let samples = decoder.decode_all().expect("Should decode all samples");

    // Total samples should be approximately (block_count * samples_per_block - encoder_delay) * channels
    let expected_total = ((info.block_count * info.samples_per_block as u32) - info.encoder_delay)
        as usize
        * info.channel_count as usize;
    assert_eq!(samples.len(), expected_total, "Sample count mismatch");

    // Verify that samples are in a reasonable range
    let max_val = samples.iter().copied().fold(0.0f32, f32::max);
    let min_val = samples.iter().copied().fold(0.0f32, f32::min);
    assert!(
        max_val <= 1.5,
        "Max sample value should be reasonable: {}",
        max_val
    );
    assert!(
        min_val >= -1.5,
        "Min sample value should be reasonable: {}",
        min_val
    );
}

/// Test USM extraction works with a real .usm file
#[test]
fn test_usm_extraction() {
    let usm_path = Path::new("0703.usm");
    if !usm_path.exists() {
        eprintln!("Skipping test: 0703.usm not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let result = cridecoder::extract_usm_file(usm_path, dir.path(), None, false);

    let files = result.expect("USM extraction should not error");
    assert!(files.len() > 0, "Should extract at least one file");

    // Verify extracted video file
    for file_path in &files {
        assert!(
            file_path.exists(),
            "Extracted file should exist: {:?}",
            file_path
        );
        let meta = fs::metadata(file_path).unwrap();
        assert!(
            meta.len() > 0,
            "Extracted file should not be empty: {:?}",
            file_path
        );
    }

    // The first file should be an .m2v video
    let video_path = &files[0];
    let ext = video_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    assert_eq!(ext, "m2v", "First extracted file should be .m2v video");
}

/// Test USM metadata extraction
#[test]
fn test_usm_metadata() {
    let usm_path = Path::new("0703.usm");
    if !usm_path.exists() {
        eprintln!("Skipping test: 0703.usm not found");
        return;
    }

    let result = cridecoder::usm::read_metadata_file(usm_path);
    let metadata = result.expect("USM metadata reading should not error");
    assert!(
        !metadata.sections.is_empty(),
        "Should have metadata sections"
    );
}

/// Test ACB extraction with an invalid file returns None
#[test]
fn test_acb_invalid_file() {
    let dir = tempfile::tempdir().unwrap();

    // Non-existent file
    let result = cridecoder::extract_acb_from_file(Path::new("nonexistent.acb"), dir.path());
    match result {
        Ok(None) => {} // expected
        Ok(Some(_)) => panic!("Should not find tracks in non-existent file"),
        Err(_) => {} // also acceptable
    }

    // File too small
    let tiny_file = dir.path().join("tiny.acb");
    fs::write(&tiny_file, b"too small").unwrap();
    let result = cridecoder::extract_acb_from_file(&tiny_file, dir.path());
    match result {
        Ok(None) => {} // expected
        Ok(Some(_)) => panic!("Should not find tracks in tiny file"),
        Err(_) => {} // also acceptable
    }
}

/// Test ACB extraction with a file that has wrong magic
#[test]
fn test_acb_wrong_magic() {
    let dir = tempfile::tempdir().unwrap();
    let bad_file = dir.path().join("bad_magic.acb");
    fs::write(&bad_file, vec![0u8; 64]).unwrap();
    let result = cridecoder::extract_acb_from_file(&bad_file, dir.path());
    match result {
        Ok(None) => {} // expected - wrong magic returns None
        Ok(Some(_)) => panic!("Should not extract from file with wrong magic"),
        Err(_) => {} // also acceptable
    }
}

/// Test HCA decoder rejects invalid data
#[test]
fn test_hca_invalid_data() {
    let bad_data = vec![0u8; 256];
    let result = cridecoder::HcaDecoder::from_reader(Cursor::new(bad_data));
    assert!(result.is_err(), "Should reject invalid HCA data");
}

/// Test HCA decoder from_reader with in-memory HCA data
#[test]
fn test_hca_from_reader() {
    let acb_path = Path::new("se_0126_01.acb");
    if !acb_path.exists() {
        eprintln!("Skipping test: se_0126_01.acb not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let tracks = cridecoder::extract_acb_from_file(acb_path, dir.path())
        .unwrap()
        .unwrap();

    // Read HCA data into memory
    let hca_data = fs::read(&tracks[0]).unwrap();
    let decoder = cridecoder::HcaDecoder::from_reader(Cursor::new(hca_data))
        .expect("Should create decoder from reader");

    let info = decoder.info();
    assert_eq!(info.sampling_rate, 44100);
    assert_eq!(info.channel_count, 2);
}

/// Test extract_acb with in-memory data
#[test]
fn test_acb_from_memory() {
    let acb_path = Path::new("se_0126_01.acb");
    if !acb_path.exists() {
        eprintln!("Skipping test: se_0126_01.acb not found");
        return;
    }

    let data = fs::read(acb_path).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let result = cridecoder::extract_acb(Cursor::new(data), dir.path(), None);
    let tracks = result.expect("Should extract from in-memory ACB data");
    assert_eq!(tracks.len(), 4, "Should extract 4 tracks from memory");
}

/// Test extract_usm with in-memory data
#[test]
fn test_usm_from_memory() {
    let usm_path = Path::new("0703.usm");
    if !usm_path.exists() {
        eprintln!("Skipping test: 0703.usm not found");
        return;
    }

    let data = fs::read(usm_path).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let result =
        cridecoder::usm::extract_usm(Cursor::new(data), dir.path(), b"0703.usm", None, false);
    let files = result.expect("Should extract from in-memory USM data");
    assert!(files.len() > 0, "Should extract at least one file");
}

// =============================================================================
// Encoder Tests
// =============================================================================

/// Test HCA encoder with synthetic sine wave
#[test]
fn test_hca_encoder_roundtrip() {
    use cridecoder::{HcaDecoder, HcaEncoder, HcaEncoderConfig};

    // Generate a 1-second stereo sine wave at 44.1kHz
    let sample_rate = 44100;
    let channels = 2u32;
    let duration_secs = 1.0;
    let sample_count = (sample_rate as f32 * duration_secs) as usize;
    let freq = 440.0; // A4 note

    let mut samples = Vec::with_capacity(sample_count * channels as usize);
    for i in 0..sample_count {
        let t = i as f32 / sample_rate as f32;
        let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
        for _ in 0..channels {
            samples.push(sample);
        }
    }

    // Create encoder config
    let config = HcaEncoderConfig {
        channels,
        sample_rate,
        bitrate: 256_000,
        ..Default::default()
    };

    // Encode to HCA
    let mut encoder = HcaEncoder::new(config).expect("Should create encoder");
    let mut hca_data = Vec::new();
    encoder
        .encode(&samples, &mut Cursor::new(&mut hca_data))
        .expect("Should encode samples");

    // Verify HCA magic
    assert_eq!(&hca_data[0..4], b"HCA\x00", "Should have HCA magic");

    // Decode back
    let mut decoder = HcaDecoder::from_reader(Cursor::new(&hca_data))
        .expect("Should create decoder from encoded HCA");

    let info = decoder.info();
    assert_eq!(info.sampling_rate, sample_rate, "Sample rate should match");
    assert_eq!(info.channel_count as u32, channels, "Channel count should match");

    let decoded = decoder.decode_all().expect("Should decode encoded HCA");
    assert!(decoded.len() > 0, "Should decode some samples");

    // The decoded length won't exactly match due to HCA frame boundaries and encoder delay
    // but it should be reasonably close
    let expected_min = sample_count * channels as usize - 2048 * channels as usize;
    let expected_max = sample_count * channels as usize + 4096 * channels as usize;
    assert!(
        decoded.len() >= expected_min && decoded.len() <= expected_max,
        "Decoded sample count {} should be close to original {} (range {}-{})",
        decoded.len(),
        sample_count * channels as usize,
        expected_min,
        expected_max
    );
}

/// Test ACB builder creates valid container
#[test]
fn test_acb_builder_basic() {
    use cridecoder::{AcbBuilder, TrackInput};

    // Create a minimal HCA file (just header for testing structure)
    let dummy_hca = create_minimal_hca_header();

    let input = TrackInput::new("test_track", 0, dummy_hca);

    let mut builder = AcbBuilder::new();
    builder.add_track(input);

    let mut output = Vec::new();
    let result = builder.build(&mut Cursor::new(&mut output), None);
    assert!(result.is_ok(), "ACB build should succeed: {:?}", result.err());
    assert!(output.len() > 64, "ACB should have content");

    // Verify UTF magic
    assert_eq!(&output[0..4], b"@UTF", "Should have UTF magic");
}

/// Test AFS2 archive builder
#[test]
fn test_afs_archive_builder() {
    use cridecoder::AfsArchiveBuilder;

    let file1 = vec![0x48, 0x43, 0x41, 0x00]; // HCA magic
    let file2 = vec![0x48, 0x43, 0x41, 0x00, 0x01, 0x02];

    let mut builder = AfsArchiveBuilder::new();
    builder.add_file(0, file1);
    builder.add_file(1, file2);

    let mut output = Cursor::new(Vec::new());
    builder.build(&mut output).expect("AFS build should succeed");

    let data = output.into_inner();
    // Verify AFS2 magic
    assert_eq!(&data[0..4], b"AFS2", "Should have AFS2 magic");
}

/// Test USM builder creates valid container
#[test]
fn test_usm_builder_structure() {
    use cridecoder::UsmBuilder;

    // Create minimal M2V header (MPEG-2 sequence header)
    let video_data = vec![
        0x00, 0x00, 0x01, 0xB3, // sequence_header_code
        0x14, 0x00, 0xF0, 0x24, // picture size, aspect, frame rate
        0xFF, 0xFF, 0xE0, 0x00, // bit rate, vbv buffer
    ];

    let builder = UsmBuilder::new("test".to_string())
        .video(video_data);

    let mut output = Cursor::new(Vec::new());
    let result = builder.build(&mut output);
    assert!(result.is_ok(), "USM build should succeed: {:?}", result.err());

    let data = output.into_inner();
    // Verify CRID magic
    assert_eq!(&data[0..4], b"CRID", "Should have CRID magic");
}

/// Helper to create a minimal HCA header for testing
fn create_minimal_hca_header() -> Vec<u8> {
    let mut data = Vec::new();

    // HCA signature
    data.extend_from_slice(b"HCA\x00");
    data.extend_from_slice(&[0x02, 0x00]); // version 2.0
    data.extend_from_slice(&[0x00, 0x60]); // header size 96

    // fmt chunk
    data.extend_from_slice(b"fmt\x00");
    data.extend_from_slice(&[2]); // 2 channels
    data.extend_from_slice(&[0x00, 0xAC, 0x44]); // 44100 Hz (BE)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x10]); // 16 blocks
    data.extend_from_slice(&[0x00, 0x00]); // encoder delay
    data.extend_from_slice(&[0x00, 0x00]); // insert samples

    // comp chunk
    data.extend_from_slice(b"comp");
    data.extend_from_slice(&[0x01, 0x55]); // frame size 341
    data.extend_from_slice(&[0x00]); // min resolution
    data.extend_from_slice(&[0x0F]); // max resolution
    data.extend_from_slice(&[0x01]); // track count
    data.extend_from_slice(&[0x01]); // channel config
    data.extend_from_slice(&[0x80]); // total band count
    data.extend_from_slice(&[0x60]); // base band count
    data.extend_from_slice(&[0x00]); // stereo band count
    data.extend_from_slice(&[0x00]); // bands per hfr group
    data.extend_from_slice(&[0x00, 0x00]); // reserved

    // Pad to header size
    while data.len() < 96 {
        data.push(0);
    }

    data
}
