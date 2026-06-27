//! High-level ACB → WAV decoding.
//!
//! Composes ACB extraction with the HCA decoder so callers can go straight from
//! an ACB to decoded audio without managing the intermediate HCA bytes
//! themselves. The per-AWB AFS2 subkey is applied automatically.

use std::fs;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

use thiserror::Error;

use crate::acb::extractor::{extract_acb_to_memory, read_validated_acb, ExtractError};
use crate::hca::{HcaDecoder, HcaDecoderError};

/// A decoded ACB track held in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAcbTrack {
    /// Cue name of the track (also the output file stem).
    pub name: String,
    /// Cue id of the track (cue-table index).
    pub cue_id: i32,
    /// Output extension: `"wav"` for a decoded HCA track, otherwise the
    /// original waveform extension (non-HCA tracks are passed through raw).
    pub extension: String,
    /// Decoded WAV bytes (HCA), or the raw waveform bytes (non-HCA).
    pub data: Vec<u8>,
}

#[derive(Error, Debug)]
pub enum DecodeAcbError {
    #[error("ACB extraction failed: {0}")]
    Extract(#[from] ExtractError),
    #[error("HCA decode failed: {0}")]
    Hca(#[from] HcaDecoderError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Extract an ACB and decode its HCA tracks to WAV, returning everything in
/// memory. The per-AWB AFS2 subkey is applied automatically, so encrypted
/// (type-56) ACBs only need the global `key`. Non-HCA tracks are returned
/// verbatim with their original extension.
pub fn decode_acb_to_wav_to_memory<R: Read + Seek>(
    acb_file: R,
    acb_file_path: Option<&Path>,
    key: Option<u64>,
) -> Result<Vec<DecodedAcbTrack>, DecodeAcbError> {
    let tracks = extract_acb_to_memory(acb_file, acb_file_path)?;

    let mut outputs = Vec::with_capacity(tracks.len());
    for track in tracks {
        if track.extension == "hca" {
            let mut decoder = HcaDecoder::from_reader(Cursor::new(track.data))?;
            if let Some(k) = key {
                decoder.set_encryption_key(k, track.subkey as u64);
            }
            let mut wav = Vec::new();
            decoder.decode_to_wav(&mut wav)?;
            outputs.push(DecodedAcbTrack {
                name: track.name,
                cue_id: track.cue_id,
                extension: "wav".to_string(),
                data: wav,
            });
        } else {
            outputs.push(DecodedAcbTrack {
                name: track.name,
                cue_id: track.cue_id,
                extension: track.extension,
                data: track.data,
            });
        }
    }
    Ok(outputs)
}

/// Like [`decode_acb_to_wav_to_memory`], but writes each decoded track to
/// `target_dir` and returns the written paths.
pub fn decode_acb_to_wav<R: Read + Seek>(
    acb_file: R,
    target_dir: &Path,
    acb_file_path: Option<&Path>,
    key: Option<u64>,
) -> Result<Vec<String>, DecodeAcbError> {
    let tracks = decode_acb_to_wav_to_memory(acb_file, acb_file_path, key)?;

    fs::create_dir_all(target_dir)?;
    let mut outputs = Vec::with_capacity(tracks.len());
    for track in tracks {
        let path = target_dir.join(format!("{}.{}", track.name, track.extension));
        fs::write(&path, &track.data)?;
        outputs.push(path.to_string_lossy().into_owned());
    }
    Ok(outputs)
}

/// Convenience wrapper over [`decode_acb_to_wav`] that reads from a file path
/// (also used to resolve sibling external streaming `.awb` archives).
pub fn decode_acb_to_wav_from_file(
    acb_path: &Path,
    target_dir: &Path,
    key: Option<u64>,
) -> Result<Vec<String>, DecodeAcbError> {
    // Slurp once so the parser reads from memory instead of issuing many small
    // syscalls against the file handle.
    let data = match read_validated_acb(acb_path)? {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };
    decode_acb_to_wav(Cursor::new(data), target_dir, Some(acb_path), key)
}
