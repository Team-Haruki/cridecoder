//! High-level ACB → WAV decoding.
//!
//! Composes ACB extraction with the HCA decoder so callers can go straight from
//! an ACB to decoded audio without managing the intermediate HCA bytes
//! themselves. The per-AWB AFS2 subkey is applied automatically.

use std::collections::VecDeque;
use std::fs;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use std::sync::Mutex;

use thiserror::Error;

use crate::acb::extractor::{extract_acb_to_memory, read_validated_acb, ExtractedAcbTrack, ExtractError};
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
    decode_acb_to_wav_to_memory_parallel(acb_file, acb_file_path, key, 1)
}

/// Multithreaded [`decode_acb_to_wav_to_memory`]: tracks are decoded
/// concurrently by up to `threads` workers, and when there are fewer HCA
/// tracks than threads each track additionally uses block-parallel HCA
/// decoding ([`HcaDecoder::decode_to_wav_parallel`]) with the remaining
/// budget. Output is identical to the serial version. `threads <= 1` decodes
/// serially.
pub fn decode_acb_to_wav_to_memory_parallel<R: Read + Seek>(
    acb_file: R,
    acb_file_path: Option<&Path>,
    key: Option<u64>,
    threads: usize,
) -> Result<Vec<DecodedAcbTrack>, DecodeAcbError> {
    let tracks = extract_acb_to_memory(acb_file, acb_file_path)?;

    let hca_count = tracks.iter().filter(|t| t.extension == "hca").count();
    if threads <= 1 || hca_count == 0 {
        let mut outputs = Vec::with_capacity(tracks.len());
        for track in tracks {
            outputs.push(decode_one_track(track, key, 1)?);
        }
        return Ok(outputs);
    }

    // Split the thread budget: one worker per HCA track (capped at `threads`),
    // and whatever is left over goes into block-parallel decoding inside each
    // track, so a single-track ACB still uses the full budget.
    let workers = threads.min(hca_count);
    // Round up so e.g. 10 threads / 6 tracks gives each track 2 block-threads
    // instead of leaving cores idle; the mild oversubscription is cheaper.
    let per_track_threads = threads.div_ceil(workers);

    let count = tracks.len();
    let queue: Mutex<VecDeque<(usize, ExtractedAcbTrack)>> =
        Mutex::new(tracks.into_iter().enumerate().collect());
    let mut slots: Vec<Option<Result<DecodedAcbTrack, DecodeAcbError>>> =
        (0..count).map(|_| None).collect();
    let slot_results: Vec<Mutex<&mut Option<_>>> = slots.iter_mut().map(Mutex::new).collect();

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let queue = &queue;
            let slot_results = &slot_results;
            scope.spawn(move || loop {
                let Some((idx, track)) = queue.lock().unwrap().pop_front() else {
                    break;
                };
                let result = decode_one_track(track, key, per_track_threads);
                **slot_results[idx].lock().unwrap() = Some(result);
            });
        }
    });
    drop(slot_results);

    // Propagate the first failure in track order (matching serial behavior).
    let mut outputs = Vec::with_capacity(count);
    for slot in slots {
        outputs.push(slot.expect("worker filled every slot")?);
    }
    Ok(outputs)
}

/// Decode a single extracted track: HCA -> WAV (block-parallel when
/// `hca_threads > 1`), anything else passed through verbatim.
fn decode_one_track(
    track: ExtractedAcbTrack,
    key: Option<u64>,
    hca_threads: usize,
) -> Result<DecodedAcbTrack, DecodeAcbError> {
    if track.extension == "hca" {
        let mut decoder = HcaDecoder::from_reader(Cursor::new(track.data))?;
        if let Some(k) = key {
            decoder.set_encryption_key(k, track.subkey as u64);
        }
        let mut wav = Vec::new();
        decoder.decode_to_wav_parallel(&mut wav, hca_threads)?;
        Ok(DecodedAcbTrack {
            name: track.name,
            cue_id: track.cue_id,
            extension: "wav".to_string(),
            data: wav,
        })
    } else {
        Ok(DecodedAcbTrack {
            name: track.name,
            cue_id: track.cue_id,
            extension: track.extension,
            data: track.data,
        })
    }
}

/// Like [`decode_acb_to_wav_to_memory`], but writes each decoded track to
/// `target_dir` and returns the written paths.
pub fn decode_acb_to_wav<R: Read + Seek>(
    acb_file: R,
    target_dir: &Path,
    acb_file_path: Option<&Path>,
    key: Option<u64>,
) -> Result<Vec<String>, DecodeAcbError> {
    decode_acb_to_wav_parallel(acb_file, target_dir, acb_file_path, key, 1)
}

/// Multithreaded [`decode_acb_to_wav`]; see
/// [`decode_acb_to_wav_to_memory_parallel`] for the threading model.
pub fn decode_acb_to_wav_parallel<R: Read + Seek>(
    acb_file: R,
    target_dir: &Path,
    acb_file_path: Option<&Path>,
    key: Option<u64>,
    threads: usize,
) -> Result<Vec<String>, DecodeAcbError> {
    let tracks = decode_acb_to_wav_to_memory_parallel(acb_file, acb_file_path, key, threads)?;

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
    decode_acb_to_wav_from_file_parallel(acb_path, target_dir, key, 1)
}

/// Multithreaded [`decode_acb_to_wav_from_file`]; see
/// [`decode_acb_to_wav_to_memory_parallel`] for the threading model.
pub fn decode_acb_to_wav_from_file_parallel(
    acb_path: &Path,
    target_dir: &Path,
    key: Option<u64>,
    threads: usize,
) -> Result<Vec<String>, DecodeAcbError> {
    // Slurp once so the parser reads from memory instead of issuing many small
    // syscalls against the file handle.
    let data = match read_validated_acb(acb_path)? {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };
    decode_acb_to_wav_parallel(Cursor::new(data), target_dir, Some(acb_path), key, threads)
}
