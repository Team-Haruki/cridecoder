//! USM video/audio extraction

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::reader::Reader;
use encoding_rs::SHIFT_JIS;
use thiserror::Error;

/// Column storage masks
const COLUMN_STORAGE_MASK: u8 = 0xF0;
const _COLUMN_STORAGE_PERROW: u8 = 0x50;
const COLUMN_STORAGE_CONSTANT: u8 = 0x30;
const COLUMN_STORAGE_CONSTANT2: u8 = 0x70;
const _COLUMN_STORAGE_ZERO: u8 = 0x10;

/// Column type masks
const COLUMN_TYPE_MASK: u8 = 0x0F;
const COLUMN_TYPE_DATA: u8 = 0x0B;
const COLUMN_TYPE_STRING: u8 = 0x0A;
const COLUMN_TYPE_FLOAT: u8 = 0x08;
const COLUMN_TYPE_8BYTE: u8 = 0x06;
const COLUMN_TYPE_4BYTE2: u8 = 0x05;
const COLUMN_TYPE_4BYTE: u8 = 0x04;
const COLUMN_TYPE_2BYTE2: u8 = 0x03;
const COLUMN_TYPE_2BYTE: u8 = 0x02;
const COLUMN_TYPE_1BYTE2: u8 = 0x01;
const COLUMN_TYPE_1BYTE: u8 = 0x00;

/// USM extraction errors
#[derive(Debug, Error)]
pub enum UsmError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid CRID signature")]
    InvalidCridSignature,
    #[error("invalid UTF signature")]
    InvalidUtfSignature,
    #[error("expected {0} signature")]
    ExpectedSignature(String),
    #[error("expected {0}")]
    ExpectedMarker(String),
    #[error("unknown column type: {0}")]
    UnknownColumnType(u8),
}

/// A value from a UTF table row
#[derive(Debug, Clone)]
pub enum UtfValue {
    Byte(u8),
    SByte(i8),
    UShort(u16),
    Short(i16),
    UInt(u32),
    Int(i32),
    ULong(u64),
    Float(f32),
    String(Vec<u8>),
    Data(Vec<u8>),
}

/// A row in a UTF table
pub type UtfRow = std::collections::HashMap<String, UtfValue>;

/// A UTF table
pub type UtfTable = Vec<UtfRow>;

/// Read column data from a UTF table
fn read_column_data<R: Read + Seek>(
    reader: &mut Reader<R>,
    column_type: u8,
    string_table_offset: i64,
    data_offset: i64,
) -> Result<UtfValue, UsmError> {
    match column_type {
        COLUMN_TYPE_DATA => {
            let offset = reader.read_u32()?;
            let size = reader.read_u32()?;
            let current_pos = reader.stream_position()?;
            reader.seek(SeekFrom::Start((data_offset + offset as i64 - 24) as u64))?;
            let data = reader.read_bytes(size as usize)?;
            reader.seek(SeekFrom::Start(current_pos))?;
            Ok(UtfValue::Data(data))
        }
        COLUMN_TYPE_STRING => {
            let offset = reader.read_u32()?;
            let current_pos = reader.stream_position()?;
            reader.seek(SeekFrom::Start(
                (string_table_offset + offset as i64 - 24) as u64,
            ))?;
            let s = read_cstring(reader)?;
            reader.seek(SeekFrom::Start(current_pos))?;
            Ok(UtfValue::String(s))
        }
        COLUMN_TYPE_FLOAT => Ok(UtfValue::Float(reader.read_f32()?)),
        COLUMN_TYPE_8BYTE => Ok(UtfValue::ULong(reader.read_u64()?)),
        COLUMN_TYPE_4BYTE2 => Ok(UtfValue::Int(reader.read_i32()?)),
        COLUMN_TYPE_4BYTE => Ok(UtfValue::UInt(reader.read_u32()?)),
        COLUMN_TYPE_2BYTE2 => Ok(UtfValue::Short(reader.read_i16()?)),
        COLUMN_TYPE_2BYTE => Ok(UtfValue::UShort(reader.read_u16()?)),
        COLUMN_TYPE_1BYTE2 => Ok(UtfValue::SByte(reader.read_i8()?)),
        COLUMN_TYPE_1BYTE => Ok(UtfValue::Byte(reader.read_u8()?)),
        _ => Err(UsmError::UnknownColumnType(column_type)),
    }
}

/// Read null-terminated C string as bytes
fn read_cstring<R: Read + Seek>(reader: &mut Reader<R>) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    loop {
        let b = reader.read_u8()?;
        if b == 0 {
            break;
        }
        buf.push(b);
    }
    Ok(buf)
}

/// Align stream to boundary
fn align_stream<R: Read + Seek>(reader: &mut Reader<R>, alignment: u64) -> io::Result<u64> {
    let pos = reader.stream_position()?;
    let remainder = pos % alignment;
    if remainder != 0 {
        reader.seek(SeekFrom::Current((alignment - remainder) as i64))
    } else {
        Ok(pos)
    }
}

/// Field info for UTF table parsing
struct FieldInfo {
    name: String,
    column_type: u8,
    constant: Option<UtfValue>,
}

/// Parse a UTF table from the reader
fn get_utf_table<R: Read + Seek>(reader: &mut Reader<R>) -> Result<UtfTable, UsmError> {
    let sig = reader.read_bytes(4)?;
    if &sig != b"@UTF" {
        return Err(UsmError::InvalidUtfSignature);
    }

    let table_size = reader.read_u32()?;
    let _version = reader.read_u16()?;
    let row_offset = reader.read_u16()?;
    let string_table_offset = reader.read_u32()?;
    let data_offset = reader.read_u32()?;
    let _table_name_offset = reader.read_u32()?;
    let number_of_fields = reader.read_u16()?;
    let _row_size = reader.read_u16()?;
    let number_of_rows = reader.read_u32()?;

    let table_data = reader.read_bytes((table_size - 24) as usize)?;
    let mut utf_reader = Reader::new(io::Cursor::new(table_data));

    let mut fields = Vec::with_capacity(number_of_fields as usize);

    for _ in 0..number_of_fields {
        let field_type = utf_reader.read_u8()?;
        let name_offset = utf_reader.read_u32()?;

        let occurrence = field_type & COLUMN_STORAGE_MASK;
        let type_key = field_type & COLUMN_TYPE_MASK;

        // Read field name
        let current_pos = utf_reader.stream_position()?;
        utf_reader.seek(SeekFrom::Start(
            (string_table_offset as i64 + name_offset as i64 - 24) as u64,
        ))?;
        let field_name_bytes = read_cstring(&mut utf_reader)?;
        let field_name = String::from_utf8_lossy(&field_name_bytes).to_string();
        utf_reader.seek(SeekFrom::Start(current_pos))?;

        if occurrence == COLUMN_STORAGE_CONSTANT || occurrence == COLUMN_STORAGE_CONSTANT2 {
            let field_val = read_column_data(
                &mut utf_reader,
                type_key,
                string_table_offset as i64,
                data_offset as i64,
            )?;
            fields.push(FieldInfo {
                name: field_name,
                column_type: type_key,
                constant: Some(field_val),
            });
        } else {
            fields.push(FieldInfo {
                name: field_name,
                column_type: type_key,
                constant: None,
            });
        }
    }

    utf_reader.seek(SeekFrom::Start((row_offset as i64 - 24) as u64))?;

    let mut rows = Vec::with_capacity(number_of_rows as usize);
    for _ in 0..number_of_rows {
        let mut row = UtfRow::new();
        for field in &fields {
            if let Some(ref constant) = field.constant {
                row.insert(field.name.clone(), constant.clone());
            } else {
                let val = read_column_data(
                    &mut utf_reader,
                    field.column_type,
                    string_table_offset as i64,
                    data_offset as i64,
                )?;
                row.insert(field.name.clone(), val);
            }
        }
        rows.push(row);
    }

    Ok(rows)
}

/// Generate mask from key
fn get_mask(key: u64) -> (Vec<Vec<u8>>, Vec<u8>) {
    let key1 = (key & 0xFFFFFFFF) as u32;
    let key2 = ((key >> 32) & 0xFFFFFFFF) as u32;

    let mut t = [0u8; 0x20];
    t[0x00] = (key1 & 0xFF) as u8;
    t[0x01] = ((key1 >> 8) & 0xFF) as u8;
    t[0x02] = ((key1 >> 16) & 0xFF) as u8;
    t[0x03] = (((key1 >> 24) & 0xFF) as u8).wrapping_sub(0x34);
    t[0x04] = ((key2 & 0xF) as u8).wrapping_add(0xF9);
    t[0x05] = ((key2 >> 8) & 0xFF) as u8 ^ 0x13;
    t[0x06] = (((key2 >> 16) & 0xFF) as u8).wrapping_add(0x61);
    t[0x07] = t[0x00] ^ 0xFF;
    t[0x08] = (t[0x02] as u16 + t[0x01] as u16) as u8;
    t[0x09] = (t[0x01] as i16 - t[0x07] as i16) as u8;
    t[0x0A] = t[0x02] ^ 0xFF;
    t[0x0B] = t[0x01] ^ 0xFF;
    t[0x0C] = (t[0x0B] as u16 + t[0x09] as u16) as u8;
    t[0x0D] = (t[0x08] as i16 - t[0x03] as i16) as u8;
    t[0x0E] = t[0x0D] ^ 0xFF;
    t[0x0F] = (t[0x0A] as i16 - t[0x0B] as i16) as u8;
    t[0x10] = (t[0x08] as i16 - t[0x0F] as i16) as u8;
    t[0x11] = t[0x10] ^ t[0x07];
    t[0x12] = t[0x0F] ^ 0xFF;
    t[0x13] = t[0x03] ^ 0x10;
    t[0x14] = (t[0x04] as i16 - 0x32) as u8;
    t[0x15] = (t[0x05] as u16 + 0xED) as u8;
    t[0x16] = t[0x06] ^ 0xF3;
    t[0x17] = (t[0x13] as i16 - t[0x0F] as i16) as u8;
    t[0x18] = (t[0x15] as u16 + t[0x07] as u16) as u8;
    t[0x19] = (0x21i16 - t[0x13] as i16) as u8;
    t[0x1A] = t[0x14] ^ t[0x17];
    t[0x1B] = (t[0x16] as u16 + t[0x16] as u16) as u8;
    t[0x1C] = (t[0x17] as u16 + 0x44) as u8;
    t[0x1D] = (t[0x03] as u16 + t[0x04] as u16) as u8;
    t[0x1E] = (t[0x05] as i16 - t[0x16] as i16) as u8;
    t[0x1F] = t[0x1D] ^ t[0x13];

    let t2 = b"URUC";
    let mut vmask1 = vec![0u8; 0x20];
    let mut vmask2 = vec![0u8; 0x20];
    let mut amask = vec![0u8; 0x20];

    for (i, &ti) in t.iter().enumerate() {
        vmask1[i] = ti;
        vmask2[i] = ti ^ 0xFF;
        if i & 1 != 0 {
            amask[i] = t2[(i >> 1) & 3];
        } else {
            amask[i] = ti ^ 0xFF;
        }
    }

    (vec![vmask1, vmask2], amask)
}

/// Mask video content
fn mask_video(content: &[u8], vmask: &[Vec<u8>]) -> Vec<u8> {
    let mut result = content.to_vec();
    let size = result.len().saturating_sub(0x40);
    let base = 0x40;

    if size >= 0x200 {
        let mut mask = vmask[1].clone();

        // Second pass
        for i in 0x100..size {
            result[base + i] ^= mask[i & 0x1F];
            mask[i & 0x1F] = result[base + i] ^ vmask[1][i & 0x1F];
        }

        // First pass
        mask.copy_from_slice(&vmask[0]);
        for i in 0..0x100 {
            mask[i & 0x1F] ^= result[0x100 + base + i];
            result[base + i] ^= mask[i & 0x1F];
        }
    }

    result
}

/// Mask audio content
fn mask_audio(content: &[u8], amask: &[u8]) -> Vec<u8> {
    let mut result = content.to_vec();
    let size = result.len().saturating_sub(0x140);
    let base = 0x140;

    for i in 0..size {
        result[base + i] ^= amask[i & 0x1F];
    }

    result
}

/// Decode Shift-JIS bytes to String
fn decode_shift_jis(data: &[u8]) -> String {
    let (decoded, _, _) = SHIFT_JIS.decode(data);
    decoded.to_string()
}

/// Extract USM from a reader
pub fn extract_usm<R: Read + Seek>(
    usm: R,
    target_dir: &Path,
    fallback_name: &[u8],
    key: Option<u64>,
    export_audio: bool,
) -> Result<Vec<PathBuf>, UsmError> {
    let mut reader = Reader::new(usm);

    let (vmask, amask) = key.map(get_mask).unzip();

    let (filename, has_audio) = parse_usm_header(&mut reader, fallback_name)?;
    let decoded_filename = decode_shift_jis(&filename);

    let (mut video_file, audio_file, output_files) =
        create_output_files(target_dir, &decoded_filename, has_audio, export_audio)?;

    let mut audio_file = audio_file;

    extract_usm_chunks(
        &mut reader,
        &mut video_file,
        audio_file.as_mut(),
        vmask.as_ref(),
        amask.as_ref(),
    )?;

    Ok(output_files)
}

/// Parse USM header
fn parse_usm_header<R: Read + Seek>(
    reader: &mut Reader<R>,
    fallback_name: &[u8],
) -> Result<(Vec<u8>, bool), UsmError> {
    let sig = reader.read_bytes(4)?;
    if &sig != b"CRID" {
        return Err(UsmError::InvalidCridSignature);
    }

    let block_size = reader.read_u32()?;
    reader.seek(SeekFrom::Start(0x20))?;
    let entry_table = get_utf_table(reader)?;

    let filename = extract_filename(&entry_table, fallback_name);
    let offset = 8 + block_size as i64;

    let (has_audio, offset) = parse_usm_header_chunks(reader, offset)?;
    skip_metadata_section(reader, offset)?;

    Ok((filename, has_audio))
}

/// Extract filename from entry table
fn extract_filename(entry_table: &UtfTable, fallback_name: &[u8]) -> Vec<u8> {
    if let Some(row) = entry_table.last() {
        if let Some(UtfValue::String(filename)) = row.get("filename") {
            return filename.clone();
        }
        if let Some(UtfValue::Data(filename)) = row.get("filename") {
            return filename.clone();
        }
    }
    fallback_name.to_vec()
}

/// Parse USM header chunks
fn parse_usm_header_chunks<R: Read + Seek>(
    reader: &mut Reader<R>,
    mut offset: i64,
) -> Result<(bool, i64), UsmError> {
    // First @SFV chunk
    seek_and_check_signature(reader, offset, "@SFV")?;
    let block_size = reader.read_u32()?;
    reader.seek(SeekFrom::Start((offset + 0x20) as u64))?;
    let _ = get_utf_table(reader)?;
    offset += 8 + block_size as i64;

    // Check for optional @SFA chunk
    reader.seek(SeekFrom::Start(offset as u64))?;
    let next_sig = reader.read_bytes(4)?;
    let mut has_audio = false;

    let mut next_sig = next_sig;
    if &next_sig == b"@SFA" {
        let block_size = reader.read_u32()?;
        reader.seek(SeekFrom::Start((offset + 0x20) as u64))?;
        let _ = get_utf_table(reader)?;
        offset += 8 + block_size as i64;
        has_audio = true;
        reader.seek(SeekFrom::Start(offset as u64))?;
        next_sig = reader.read_bytes(4)?;
    }

    // Second @SFV with HEADER END
    if &next_sig != b"@SFV" {
        return Err(UsmError::ExpectedSignature("@SFV".to_string()));
    }
    let block_size = reader.read_u32()?;
    reader.seek(SeekFrom::Start((offset + 0x20) as u64))?;
    let header_end = reader.read_bytes(11)?;
    if &header_end != b"#HEADER END" {
        return Err(UsmError::ExpectedMarker("#HEADER END".to_string()));
    }
    offset += 8 + block_size as i64;

    // Optional @SFA with HEADER END
    if has_audio {
        seek_and_check_signature(reader, offset, "@SFA")?;
        let block_size = reader.read_u32()?;
        reader.seek(SeekFrom::Start((offset + 0x20) as u64))?;
        let header_end = reader.read_bytes(11)?;
        if &header_end != b"#HEADER END" {
            return Err(UsmError::ExpectedMarker("#HEADER END".to_string()));
        }
        offset += 8 + block_size as i64;
    }

    Ok((has_audio, offset))
}

/// Seek to offset and check signature
fn seek_and_check_signature<R: Read + Seek>(
    reader: &mut Reader<R>,
    offset: i64,
    expected: &str,
) -> Result<(), UsmError> {
    reader.seek(SeekFrom::Start(offset as u64))?;
    let sig = reader.read_bytes(4)?;
    if sig != expected.as_bytes() {
        return Err(UsmError::ExpectedSignature(expected.to_string()));
    }
    Ok(())
}

/// Skip metadata section
fn skip_metadata_section<R: Read + Seek>(
    reader: &mut Reader<R>,
    mut offset: i64,
) -> Result<(), UsmError> {
    // First metadata @SFV
    seek_and_check_signature(reader, offset, "@SFV")?;
    let block_size = reader.read_u32()?;
    reader.seek(SeekFrom::Start((offset + 0x20) as u64))?;
    let _ = get_utf_table(reader)?;
    offset += 8 + block_size as i64;

    // Second metadata @SFV with METADATA END
    seek_and_check_signature(reader, offset, "@SFV")?;
    reader.seek(SeekFrom::Current(28))?;
    let metadata_end = reader.read_bytes(13)?;
    if &metadata_end != b"#METADATA END" {
        return Err(UsmError::ExpectedMarker("#METADATA END".to_string()));
    }
    align_stream(reader, 4)?;
    reader.seek(SeekFrom::Current(16))?;

    Ok(())
}

/// Create output files
fn create_output_files(
    target_dir: &Path,
    decoded_filename: &str,
    has_audio: bool,
    export_audio: bool,
) -> Result<(File, Option<File>, Vec<PathBuf>), UsmError> {
    let base_name = Path::new(decoded_filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(decoded_filename);

    let video_path = target_dir.join(format!("{}.m2v", base_name));
    let video_file = File::create(&video_path)?;

    let mut output_files = vec![video_path];
    let audio_file = if has_audio && export_audio {
        let audio_path = target_dir.join(format!("{}.adx", base_name));
        let file = File::create(&audio_path)?;
        output_files.push(audio_path);
        Some(file)
    } else {
        None
    };

    Ok((video_file, audio_file, output_files))
}

/// Extract USM chunks
fn extract_usm_chunks<R: Read + Seek>(
    reader: &mut Reader<R>,
    video_file: &mut File,
    mut audio_file: Option<&mut File>,
    vmask: Option<&Vec<Vec<u8>>>,
    amask: Option<&Vec<u8>>,
) -> Result<(), UsmError> {
    while let Ok(next_sig) = reader.read_bytes(4) {
        let block_size = reader.read_u32()?;
        let current_pos = reader.stream_position()?;
        let next_offset = current_pos + block_size as u64;

        let chunk_header_size = reader.read_u16()?;
        let chunk_footer_size = reader.read_u16()?;
        let _ = reader.read_bytes(3)?;
        let data_type_byte = reader.read_i8()?;
        let data_type = (data_type_byte & 0b11) as u8;
        reader.seek(SeekFrom::Current(16))?;

        let contents_end = reader.read_bytes(13)?;
        if &contents_end == b"#CONTENTS END" {
            break;
        }

        reader.seek(SeekFrom::Current(-13))?;
        let read_data_len =
            block_size as usize - chunk_header_size as usize - chunk_footer_size as usize;

        process_chunk(
            reader,
            &next_sig,
            read_data_len,
            data_type,
            video_file,
            &mut audio_file,
            vmask,
            amask,
        )?;

        reader.seek(SeekFrom::Start(next_offset))?;
    }

    Ok(())
}

/// Process a chunk
#[allow(clippy::too_many_arguments)]
fn process_chunk<R: Read + Seek>(
    reader: &mut Reader<R>,
    sig: &[u8],
    read_data_len: usize,
    data_type: u8,
    video_file: &mut File,
    audio_file: &mut Option<&mut File>,
    vmask: Option<&Vec<Vec<u8>>>,
    amask: Option<&Vec<u8>>,
) -> Result<(), UsmError> {
    if sig == b"@SFV" {
        let content = reader.read_bytes(read_data_len)?;
        let content = if data_type == 0 {
            if let Some(vmask) = vmask {
                mask_video(&content, vmask)
            } else {
                content
            }
        } else {
            content
        };
        video_file.write_all(&content)?;
    } else if sig == b"@SFA" {
        if let Some(audio_file) = audio_file {
            let content = reader.read_bytes(read_data_len)?;
            let content = if data_type == 0 {
                if let Some(amask) = amask {
                    mask_audio(&content, amask)
                } else {
                    content
                }
            } else {
                content
            };
            audio_file.write_all(&content)?;
        }
    }

    Ok(())
}

/// Extract USM from a file path
pub fn extract_usm_file(
    usm_path: &Path,
    target_dir: &Path,
    key: Option<u64>,
    export_audio: bool,
) -> Result<Vec<PathBuf>, UsmError> {
    let file = File::open(usm_path)?;
    let fallback_name = usm_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.as_bytes().to_vec())
        .unwrap_or_default();
    extract_usm(file, target_dir, &fallback_name, key, export_audio)
}
