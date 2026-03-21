//! Python bindings for cridecoder
//!
//! Provides Python functions for CRI codec operations:
//! - ACB extraction and building
//! - HCA decoding and encoding
//! - USM extraction and building

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use std::fs;
use std::io::Cursor;
use std::path::Path;

use crate::acb;
use crate::acb::{AcbBuilder, TrackInput};
use crate::hca::{HcaDecoder, HcaEncoder, HcaEncoderConfig};
use crate::usm;
use crate::usm::UsmBuilder;

/// Extract audio tracks from an ACB file.
///
/// Args:
///     acb_path: Path to the ACB file
///     output_dir: Directory to write extracted files to
///
/// Returns:
///     List of extracted file paths, or None if the file is invalid
#[pyfunction]
fn extract_acb(acb_path: &str, output_dir: &str) -> PyResult<Option<Vec<String>>> {
    let acb_path = Path::new(acb_path);
    let output_dir = Path::new(output_dir);
    fs::create_dir_all(output_dir)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create output dir: {}", e)))?;

    acb::extract_acb_from_file(acb_path, output_dir)
        .map_err(|e| PyRuntimeError::new_err(format!("ACB extraction failed: {}", e)))
}

/// Build an ACB file from track data.
///
/// Args:
///     tracks: List of tuples (name, cue_id, hca_data)
///     output_path: Path to write the ACB file
///
/// Returns:
///     None on success
#[pyfunction]
fn build_acb(tracks: Vec<(String, u32, Vec<u8>)>, output_path: &str) -> PyResult<()> {
    let mut builder = AcbBuilder::new();
    
    for (name, cue_id, data) in tracks {
        let track = TrackInput::new(name, cue_id, data);
        builder.add_track(track);
    }
    
    let mut output = fs::File::create(output_path)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create output file: {}", e)))?;
    
    builder.build(&mut output, None)
        .map_err(|e| PyRuntimeError::new_err(format!("ACB build failed: {}", e)))?;
    
    Ok(())
}

/// Build an ACB file from track data (returns bytes).
///
/// Args:
///     tracks: List of tuples (name, cue_id, hca_data)
///
/// Returns:
///     ACB file data as bytes
#[pyfunction]
fn build_acb_bytes(tracks: Vec<(String, u32, Vec<u8>)>) -> PyResult<Vec<u8>> {
    let mut builder = AcbBuilder::new();
    
    for (name, cue_id, data) in tracks {
        let track = TrackInput::new(name, cue_id, data);
        builder.add_track(track);
    }
    
    let mut output = Cursor::new(Vec::new());
    builder.build(&mut output, None)
        .map_err(|e| PyRuntimeError::new_err(format!("ACB build failed: {}", e)))?;
    
    Ok(output.into_inner())
}

/// Decode an HCA file to WAV format.
///
/// Args:
///     hca_path: Path to the HCA file
///     wav_path: Path to write the output WAV file
///
/// Returns:
///     dict with HCA info (sample_rate, channels, block_count, etc.)
#[pyfunction]
fn decode_hca<'py>(
    py: Python<'py>,
    hca_path: &str,
    wav_path: &str,
) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
    let mut decoder = HcaDecoder::from_file(hca_path)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to open HCA: {}", e)))?;

    let info = decoder.info().clone();

    let mut output = fs::File::create(wav_path)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create WAV: {}", e)))?;

    decoder
        .decode_to_wav(&mut output)
        .map_err(|e| PyRuntimeError::new_err(format!("HCA decode failed: {}", e)))?;

    let dict = pyo3::types::PyDict::new(py);
    dict.set_item("sample_rate", info.sampling_rate)?;
    dict.set_item("channels", info.channel_count)?;
    dict.set_item("block_count", info.block_count)?;
    dict.set_item("block_size", info.block_size)?;
    dict.set_item("encoder_delay", info.encoder_delay)?;
    dict.set_item("samples_per_block", info.samples_per_block)?;

    Ok(dict)
}

/// Decode HCA data (bytes) to WAV bytes in memory.
///
/// Args:
///     hca_data: Raw HCA file data as bytes
///
/// Returns:
///     WAV file data as bytes
#[pyfunction]
fn decode_hca_bytes(hca_data: &[u8]) -> PyResult<Vec<u8>> {
    let mut decoder = HcaDecoder::from_reader(Cursor::new(hca_data.to_vec()))
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to parse HCA: {}", e)))?;

    let mut wav_buf = Vec::new();
    decoder
        .decode_to_wav(&mut wav_buf)
        .map_err(|e| PyRuntimeError::new_err(format!("HCA decode failed: {}", e)))?;

    Ok(wav_buf)
}

/// Encode WAV data to HCA format.
///
/// Args:
///     wav_data: WAV file data as bytes
///     sample_rate: Sample rate (optional, auto-detect from WAV if None)
///     channels: Number of channels (optional, auto-detect from WAV if None)
///     bitrate: Target bitrate in bps (default: 256000)
///     encryption_key: Optional encryption key (u64)
///
/// Returns:
///     HCA file data as bytes
#[pyfunction]
#[pyo3(signature = (wav_data, sample_rate=None, channels=None, bitrate=256000, encryption_key=None))]
fn encode_hca_bytes(
    wav_data: &[u8],
    sample_rate: Option<u32>,
    channels: Option<u32>,
    bitrate: u32,
    encryption_key: Option<u64>,
) -> PyResult<Vec<u8>> {
    // Parse WAV header
    if wav_data.len() < 44 || &wav_data[0..4] != b"RIFF" || &wav_data[8..12] != b"WAVE" {
        return Err(PyRuntimeError::new_err("Invalid WAV data"));
    }

    // Find fmt and data chunks
    let mut pos = 12;
    let mut wav_channels = 2u32;
    let mut wav_sample_rate = 44100u32;
    let mut bits_per_sample = 16u32;
    let mut data_start = 0;
    let mut data_len = 0;

    while pos + 8 <= wav_data.len() {
        let chunk_id = &wav_data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([
            wav_data[pos + 4],
            wav_data[pos + 5],
            wav_data[pos + 6],
            wav_data[pos + 7],
        ]) as usize;

        if chunk_id == b"fmt " && chunk_size >= 16 {
            wav_channels = u16::from_le_bytes([wav_data[pos + 10], wav_data[pos + 11]]) as u32;
            wav_sample_rate = u32::from_le_bytes([
                wav_data[pos + 12],
                wav_data[pos + 13],
                wav_data[pos + 14],
                wav_data[pos + 15],
            ]);
            bits_per_sample = u16::from_le_bytes([wav_data[pos + 22], wav_data[pos + 23]]) as u32;
        } else if chunk_id == b"data" {
            data_start = pos + 8;
            data_len = chunk_size;
            break;
        }

        pos += 8 + chunk_size;
        if chunk_size % 2 != 0 {
            pos += 1; // padding
        }
    }

    if data_start == 0 || data_len == 0 {
        return Err(PyRuntimeError::new_err("No data chunk in WAV"));
    }

    let final_sample_rate = sample_rate.unwrap_or(wav_sample_rate);
    let final_channels = channels.unwrap_or(wav_channels);

    // Convert PCM to f32
    let samples: Vec<f32> = match bits_per_sample {
        16 => {
            let sample_count = data_len / 2;
            (0..sample_count)
                .map(|i| {
                    let idx = data_start + i * 2;
                    let sample = i16::from_le_bytes([wav_data[idx], wav_data[idx + 1]]);
                    sample as f32 / 32768.0
                })
                .collect()
        }
        24 => {
            let sample_count = data_len / 3;
            (0..sample_count)
                .map(|i| {
                    let idx = data_start + i * 3;
                    let sample = ((wav_data[idx] as i32)
                        | ((wav_data[idx + 1] as i32) << 8)
                        | ((wav_data[idx + 2] as i32) << 16))
                        << 8
                        >> 8; // sign extend
                    sample as f32 / 8388608.0
                })
                .collect()
        }
        32 => {
            let sample_count = data_len / 4;
            (0..sample_count)
                .map(|i| {
                    let idx = data_start + i * 4;
                    f32::from_le_bytes([
                        wav_data[idx],
                        wav_data[idx + 1],
                        wav_data[idx + 2],
                        wav_data[idx + 3],
                    ])
                })
                .collect()
        }
        _ => return Err(PyRuntimeError::new_err(format!("Unsupported bit depth: {}", bits_per_sample))),
    };

    // Create encoder config
    let mut config = HcaEncoderConfig::new(final_sample_rate, final_channels)
        .with_bitrate(bitrate);
    
    if let Some(key) = encryption_key {
        config = config.with_encryption(key);
    }

    // Encode
    let mut encoder = HcaEncoder::new(config)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create encoder: {}", e)))?;

    let mut output = Cursor::new(Vec::new());
    encoder.encode(&samples, &mut output)
        .map_err(|e| PyRuntimeError::new_err(format!("HCA encode failed: {}", e)))?;

    Ok(output.into_inner())
}

/// Encode a WAV file to HCA file.
///
/// Args:
///     wav_path: Path to the input WAV file
///     hca_path: Path to write the output HCA file
///     bitrate: Target bitrate in bps (default: 256000)
///     encryption_key: Optional encryption key (u64)
///
/// Returns:
///     dict with encoding info
#[pyfunction]
#[pyo3(signature = (wav_path, hca_path, bitrate=256000, encryption_key=None))]
fn encode_hca<'py>(
    py: Python<'py>,
    wav_path: &str,
    hca_path: &str,
    bitrate: u32,
    encryption_key: Option<u64>,
) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
    let wav_data = fs::read(wav_path)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to read WAV: {}", e)))?;
    
    let hca_data = encode_hca_bytes(&wav_data, None, None, bitrate, encryption_key)?;
    
    fs::write(hca_path, &hca_data)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to write HCA: {}", e)))?;
    
    let dict = pyo3::types::PyDict::new(py);
    dict.set_item("size", hca_data.len())?;
    dict.set_item("bitrate", bitrate)?;
    
    Ok(dict)
}

/// Extract video/audio from a USM file.
///
/// Args:
///     usm_path: Path to the USM file
///     output_dir: Directory to write extracted files to
///     key: Optional decryption key (u64)
///     export_audio: Whether to export audio tracks (default: false)
///
/// Returns:
///     List of extracted file paths
#[pyfunction]
#[pyo3(signature = (usm_path, output_dir, key=None, export_audio=false))]
fn extract_usm(
    usm_path: &str,
    output_dir: &str,
    key: Option<u64>,
    export_audio: bool,
) -> PyResult<Vec<String>> {
    let usm_path = Path::new(usm_path);
    let output_dir = Path::new(output_dir);
    fs::create_dir_all(output_dir)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create output dir: {}", e)))?;

    let files = usm::extract_usm_file(usm_path, output_dir, key, export_audio)
        .map_err(|e| PyRuntimeError::new_err(format!("USM extraction failed: {}", e)))?;

    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

/// Build a USM file from video data.
///
/// Args:
///     name: Name for the USM file (used in metadata)
///     video_data: M2V video data as bytes
///     output_path: Path to write the USM file
///     encryption_key: Optional encryption key (u64)
///
/// Returns:
///     None on success
#[pyfunction]
#[pyo3(signature = (name, video_data, output_path, encryption_key=None))]
fn build_usm(
    name: &str,
    video_data: Vec<u8>,
    output_path: &str,
    encryption_key: Option<u64>,
) -> PyResult<()> {
    let mut builder = UsmBuilder::new(name.to_string())
        .video(video_data);
    
    if let Some(key) = encryption_key {
        builder = builder.encryption_key(key);
    }
    
    let mut output = fs::File::create(output_path)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create output file: {}", e)))?;
    
    builder.build(&mut output)
        .map_err(|e| PyRuntimeError::new_err(format!("USM build failed: {}", e)))?;
    
    Ok(())
}

/// Build a USM file from video data (returns bytes).
///
/// Args:
///     name: Name for the USM file (used in metadata)
///     video_data: M2V video data as bytes
///     encryption_key: Optional encryption key (u64)
///
/// Returns:
///     USM file data as bytes
#[pyfunction]
#[pyo3(signature = (name, video_data, encryption_key=None))]
fn build_usm_bytes(
    name: &str,
    video_data: Vec<u8>,
    encryption_key: Option<u64>,
) -> PyResult<Vec<u8>> {
    let mut builder = UsmBuilder::new(name.to_string())
        .video(video_data);
    
    if let Some(key) = encryption_key {
        builder = builder.encryption_key(key);
    }
    
    let mut output = Cursor::new(Vec::new());
    builder.build(&mut output)
        .map_err(|e| PyRuntimeError::new_err(format!("USM build failed: {}", e)))?;
    
    Ok(output.into_inner())
}

/// Read metadata from a USM file.
///
/// Args:
///     usm_path: Path to the USM file
///
/// Returns:
///     Metadata as a JSON string
#[pyfunction]
fn read_usm_metadata(usm_path: &str) -> PyResult<String> {
    let usm_path = Path::new(usm_path);
    let metadata = usm::read_metadata_file(usm_path)
        .map_err(|e| PyRuntimeError::new_err(format!("Metadata read failed: {}", e)))?;

    serde_json::to_string_pretty(&metadata)
        .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization failed: {}", e)))
}

/// Register all Python functions to the module
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // ACB functions
    m.add_function(wrap_pyfunction!(extract_acb, m)?)?;
    m.add_function(wrap_pyfunction!(build_acb, m)?)?;
    m.add_function(wrap_pyfunction!(build_acb_bytes, m)?)?;
    
    // HCA functions
    m.add_function(wrap_pyfunction!(decode_hca, m)?)?;
    m.add_function(wrap_pyfunction!(decode_hca_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(encode_hca, m)?)?;
    m.add_function(wrap_pyfunction!(encode_hca_bytes, m)?)?;
    
    // USM functions
    m.add_function(wrap_pyfunction!(extract_usm, m)?)?;
    m.add_function(wrap_pyfunction!(build_usm, m)?)?;
    m.add_function(wrap_pyfunction!(build_usm_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(read_usm_metadata, m)?)?;
    
    Ok(())
}
