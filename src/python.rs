//! Python bindings for cridecoder
//!
//! Provides Python functions for CRI codec operations:
//! - ACB extraction
//! - HCA decoding to WAV
//! - USM extraction

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use std::fs;
use std::io::Cursor;
use std::path::Path;

use crate::acb;
use crate::hca::HcaDecoder;
use crate::usm;

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

/// Decode an HCA file to WAV format.
///
/// Args:
///     hca_path: Path to the HCA file
///     wav_path: Path to write the output WAV file
///
/// Returns:
///     dict with HCA info (sample_rate, channels, block_count, etc.)
#[pyfunction]
fn decode_hca(hca_path: &str, wav_path: &str) -> PyResult<PyObject> {
    Python::with_gil(|py| {
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

        Ok(dict.into())
    })
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
    m.add_function(wrap_pyfunction!(extract_acb, m)?)?;
    m.add_function(wrap_pyfunction!(decode_hca, m)?)?;
    m.add_function(wrap_pyfunction!(decode_hca_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(extract_usm, m)?)?;
    m.add_function(wrap_pyfunction!(read_usm_metadata, m)?)?;
    Ok(())
}
