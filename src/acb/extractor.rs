//! ACB file extractor

use crate::acb::afs::AfsArchive;
use crate::acb::consts::wave_type_extension;
use crate::acb::track::{Track, TrackList};
use crate::acb::utf::{get_bytes_field, get_string_field, UtfTable};
use std::fs::{self, File};
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("UTF error: {0}")]
    Utf(#[from] crate::acb::utf::UtfError),
    #[error("Track error: {0}")]
    Track(#[from] crate::acb::track::TrackError),
    #[error("AFS error: {0}")]
    Afs(#[from] crate::acb::afs::AfsError),
    #[error("Invalid ACB file")]
    InvalidAcb,
}

/// Extract all audio files from an ACB file
pub fn extract_acb<R: Read + Seek>(
    acb_file: R,
    target_dir: &Path,
    acb_file_path: Option<&Path>,
) -> Result<Vec<String>, ExtractError> {
    let utf = UtfTable::new(acb_file)?;

    let track_list = TrackList::new(&utf)?;

    let mut embedded_awb = load_embedded_awb(&utf.rows[0]);
    let mut external_awbs = load_external_awbs(&utf.rows[0], acb_file_path);

    extract_all_tracks(&track_list, target_dir, &mut embedded_awb, &mut external_awbs)
}

fn load_embedded_awb(row: &std::collections::HashMap<String, crate::acb::utf::Value>) -> Option<AfsArchive<Cursor<Vec<u8>>>> {
    let awb_data = get_bytes_field(row, "AwbFile")?;
    if awb_data.is_empty() {
        return None;
    }
    AfsArchive::new(Cursor::new(awb_data.to_vec())).ok()
}

fn load_external_awbs(
    row: &std::collections::HashMap<String, crate::acb::utf::Value>,
    acb_file_path: Option<&Path>,
) -> Vec<AfsArchive<Cursor<Vec<u8>>>> {
    let mut external_awbs = Vec::new();

    let stream_awb_hash = match get_bytes_field(row, "StreamAwbHash") {
        Some(data) if !data.is_empty() => data,
        _ => return external_awbs,
    };

    let hash_table = match UtfTable::new(Cursor::new(stream_awb_hash)) {
        Ok(t) => t,
        Err(_) => return external_awbs,
    };

    let acb_dir = acb_file_path.and_then(|p| p.parent());

    for awb_row in &hash_table.rows {
        let awb_name = match get_string_field(awb_row, "Name") {
            Some(n) => n,
            None => continue,
        };

        let awb_path = match acb_dir {
            Some(dir) => dir.join(format!("{}.awb", awb_name)),
            None => continue,
        };

        if let Some(awb) = load_external_awb_file(&awb_path) {
            external_awbs.push(awb);
        }
    }

    external_awbs
}

fn load_external_awb_file(awb_path: &Path) -> Option<AfsArchive<Cursor<Vec<u8>>>> {
    if !awb_path.exists() {
        return None;
    }

    let awb_data = fs::read(awb_path).ok()?;
    AfsArchive::new(Cursor::new(awb_data)).ok()
}

fn extract_all_tracks(
    track_list: &TrackList,
    target_dir: &Path,
    embedded_awb: &mut Option<AfsArchive<Cursor<Vec<u8>>>>,
    external_awbs: &mut [AfsArchive<Cursor<Vec<u8>>>],
) -> Result<Vec<String>, ExtractError> {
    let mut outputs = Vec::new();
    fs::create_dir_all(target_dir)?;

    for track in &track_list.tracks {
        if let Some(output_path) = extract_single_track(track, target_dir, embedded_awb, external_awbs)? {
            outputs.push(output_path);
        }
    }

    Ok(outputs)
}

fn extract_single_track(
    track: &Track,
    target_dir: &Path,
    embedded_awb: &mut Option<AfsArchive<Cursor<Vec<u8>>>>,
    external_awbs: &mut [AfsArchive<Cursor<Vec<u8>>>],
) -> Result<Option<String>, ExtractError> {
    let ext = wave_type_extension(track.enc_type);
    let ext = if ext.is_empty() {
        format!(".{}", track.enc_type)
    } else {
        ext.to_string()
    };

    let filename = format!("{}{}", track.name, ext);
    let output_path = target_dir.join(&filename);

    let data = get_track_data(track, embedded_awb, external_awbs)?;
    let data = match data {
        Some(d) => d,
        None => return Ok(None),
    };

    fs::write(&output_path, data)?;
    Ok(Some(output_path.to_string_lossy().into_owned()))
}

fn get_track_data(
    track: &Track,
    embedded_awb: &mut Option<AfsArchive<Cursor<Vec<u8>>>>,
    external_awbs: &mut [AfsArchive<Cursor<Vec<u8>>>],
) -> Result<Option<Vec<u8>>, ExtractError> {
    if track.is_stream {
        if track.stream_awb_id >= 0 && (track.stream_awb_id as usize) < external_awbs.len() {
            let awb = &mut external_awbs[track.stream_awb_id as usize];
            let data = awb.file_data_for_cue_id(track.wav_id)?;
            return Ok(Some(data));
        }
    } else if let Some(awb) = embedded_awb.as_mut() {
        let data = awb.file_data_for_cue_id(track.wav_id)?;
        return Ok(Some(data));
    }

    Ok(None)
}

/// Convenience function to extract from a file path
pub fn extract_acb_from_file(acb_path: &Path, target_dir: &Path) -> Result<Option<Vec<String>>, ExtractError> {
    let info = match fs::metadata(acb_path) {
        Ok(i) => i,
        Err(_) => return Ok(None),
    };

    // A valid ACB file must have at least @UTF magic (4 bytes) + header (28 bytes) = 32 bytes
    if info.len() < 32 {
        return Ok(None);
    }

    let mut file = File::open(acb_path)?;
    
    // Read and validate the first 4 bytes
    let mut header = [0u8; 4];
    use std::io::Read;
    file.read_exact(&mut header)?;

    // Check for @UTF magic (0x40 0x55 0x54 0x46)
    if header != [0x40, 0x55, 0x54, 0x46] {
        return Ok(None); // Not a valid ACB file
    }

    // Seek back to start
    use std::io::Seek;
    file.seek(std::io::SeekFrom::Start(0))?;

    let outputs = extract_acb(file, target_dir, Some(acb_path))?;
    Ok(Some(outputs))
}
