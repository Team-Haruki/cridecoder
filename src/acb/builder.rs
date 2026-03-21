//! ACB/AWB encoder - builds CRI audio containers from audio files

use crate::acb::consts::*;
use crate::acb::utf::Value;
use encoding_rs::SHIFT_JIS;
use std::collections::HashMap;
use std::io::{self, Seek, SeekFrom, Write};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuilderError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("No audio tracks provided")]
    NoTracks,
    #[error("Track name too long: {0}")]
    NameTooLong(String),
    #[error("Invalid audio data")]
    InvalidAudioData,
    #[error("Unsupported audio format")]
    UnsupportedFormat,
}

/// Audio track input for ACB building
#[derive(Debug, Clone)]
pub struct TrackInput {
    pub name: String,
    pub cue_id: u32,
    pub data: Vec<u8>,
    pub encode_type: i32,
}

impl TrackInput {
    pub fn new(name: impl Into<String>, cue_id: u32, data: Vec<u8>) -> Self {
        let encode_type = Self::detect_encode_type(&data);
        Self {
            name: name.into(),
            cue_id,
            data,
            encode_type,
        }
    }

    fn detect_encode_type(data: &[u8]) -> i32 {
        if data.len() < 4 {
            return WAVEFORM_ENCODE_TYPE_HCA;
        }
        match &data[0..4] {
            [0x80, 0x00, ..] | [0x00, 0x00, 0x00, 0x80] => WAVEFORM_ENCODE_TYPE_ADX,
            b"HCA\x00" | [0xc8, 0xc3, 0xc1, 0x00] => WAVEFORM_ENCODE_TYPE_HCA, // HCA or masked HCA
            _ if data.len() > 6 && data[0] == 0xC8 => WAVEFORM_ENCODE_TYPE_HCA, // masked HCA header
            _ => WAVEFORM_ENCODE_TYPE_HCA,
        }
    }
}

// ============================================================================
// UTF Table Builder
// ============================================================================

/// Column definition for UTF table building
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub typ: u8,
    pub flag: u8,
    pub constant_value: Option<Value>,
}

impl ColumnDef {
    pub fn constant(name: impl Into<String>, value: Value) -> Self {
        let typ = match &value {
            Value::U8(_) => COLUMN_TYPE_1BYTE,
            Value::I8(_) => COLUMN_TYPE_1BYTE2,
            Value::U16(_) => COLUMN_TYPE_2BYTE,
            Value::I16(_) => COLUMN_TYPE_2BYTE2,
            Value::U32(_) => COLUMN_TYPE_4BYTE,
            Value::I32(_) => COLUMN_TYPE_4BYTE2,
            Value::U64(_) => COLUMN_TYPE_8BYTE,
            Value::F32(_) => COLUMN_TYPE_FLOAT,
            Value::String(_) => COLUMN_TYPE_STRING,
            Value::Data(_) => COLUMN_TYPE_DATA,
        };
        Self {
            name: name.into(),
            typ,
            flag: COLUMN_FLAG_NAME | COLUMN_FLAG_DEFAULT,
            constant_value: Some(value),
        }
    }

    pub fn per_row(name: impl Into<String>, typ: u8) -> Self {
        Self {
            name: name.into(),
            typ,
            flag: COLUMN_FLAG_NAME | COLUMN_FLAG_ROW,
            constant_value: None,
        }
    }
}

/// Builder for UTF tables
pub struct UtfTableBuilder {
    pub table_name: String,
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<HashMap<String, Value>>,
}

impl UtfTableBuilder {
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }

    pub fn add_column(&mut self, col: ColumnDef) -> &mut Self {
        self.columns.push(col);
        self
    }

    pub fn add_row(&mut self, row: HashMap<String, Value>) -> &mut Self {
        self.rows.push(row);
        self
    }

    /// Get the size of a column value in bytes
    fn value_size(typ: u8) -> u32 {
        match typ {
            COLUMN_TYPE_1BYTE | COLUMN_TYPE_1BYTE2 => 1,
            COLUMN_TYPE_2BYTE | COLUMN_TYPE_2BYTE2 => 2,
            COLUMN_TYPE_4BYTE | COLUMN_TYPE_4BYTE2 | COLUMN_TYPE_FLOAT | COLUMN_TYPE_STRING => 4,
            COLUMN_TYPE_8BYTE | COLUMN_TYPE_DATA => 8,
            _ => 0,
        }
    }

    /// Encode string to Shift-JIS with null terminator
    fn encode_string(s: &str) -> Vec<u8> {
        let (encoded, _, _) = SHIFT_JIS.encode(s);
        let mut result = encoded.into_owned();
        result.push(0);
        result
    }

    /// Build the UTF table and write to output
    pub fn build<W: Write + Seek>(&self, writer: &mut W) -> Result<(), BuilderError> {
        // Phase 1: Collect all strings and data blobs
        let mut string_table = StringTable::new();
        let mut data_table = DataTable::new();

        // Add table name
        let table_name_offset = string_table.add(&self.table_name);

        // Add column names and constant strings/data
        let mut column_name_offsets = Vec::new();
        let mut constant_offsets: Vec<Option<(u32, u32)>> = Vec::new();

        for col in &self.columns {
            column_name_offsets.push(string_table.add(&col.name));

            if let Some(ref val) = col.constant_value {
                match val {
                    Value::String(s) => {
                        let off = string_table.add(s);
                        constant_offsets.push(Some((off, 0)));
                    }
                    Value::Data(d) => {
                        let off = data_table.add(d);
                        constant_offsets.push(Some((off, d.len() as u32)));
                    }
                    _ => constant_offsets.push(None),
                }
            } else {
                constant_offsets.push(None);
            }
        }

        // Add row strings/data
        let mut row_string_data_offsets: Vec<HashMap<String, (u32, u32)>> = Vec::new();
        for row in &self.rows {
            let mut offsets = HashMap::new();
            for col in &self.columns {
                if col.constant_value.is_some() {
                    continue;
                }
                if let Some(val) = row.get(&col.name) {
                    match val {
                        Value::String(s) => {
                            let off = string_table.add(s);
                            offsets.insert(col.name.clone(), (off, 0));
                        }
                        Value::Data(d) => {
                            let off = data_table.add(d);
                            offsets.insert(col.name.clone(), (off, d.len() as u32));
                        }
                        _ => {}
                    }
                }
            }
            row_string_data_offsets.push(offsets);
        }

        // Phase 2: Calculate sizes and offsets
        // Schema size: for each column, 1 byte (flag|type) + 4 bytes (name offset) + optional constant value
        let mut schema_size: u32 = 0;
        for col in &self.columns {
            schema_size += 5; // flag|type (1) + name offset (4)
            if col.constant_value.is_some() {
                schema_size += Self::value_size(col.typ);
            }
        }

        // Row size: sum of per-row column sizes
        let row_width: u32 = self
            .columns
            .iter()
            .filter(|c| c.constant_value.is_none())
            .map(|c| Self::value_size(c.typ))
            .sum();

        let rows_size = row_width * self.rows.len() as u32;

        // Offsets (relative to byte 8, after @UTF + table_size)
        let schema_offset: u32 = 0x18; // After the 0x20 header - 8
        let rows_offset = schema_offset + schema_size;
        let strings_offset = rows_offset + rows_size;
        let data_offset = strings_offset + string_table.len() as u32;
        let table_size = data_offset + data_table.len() as u32;

        // Phase 3: Write header
        writer.write_all(b"@UTF")?;
        write_u32_be(writer, table_size)?; // table_size (excluding magic and this field)
        write_u16_be(writer, 0x0001)?; // version
        write_u16_be(writer, (rows_offset - 8) as u16)?; // rows_offset relative to +8
        write_u32_be(writer, strings_offset - 8)?; // strings_offset relative to +8
        write_u32_be(writer, data_offset - 8)?; // data_offset relative to +8
        write_u32_be(writer, table_name_offset)?; // table_name offset in string table
        write_u16_be(writer, self.columns.len() as u16)?; // number of columns
        write_u16_be(writer, row_width as u16)?; // row width
        write_u32_be(writer, self.rows.len() as u32)?; // number of rows

        // Phase 4: Write schema
        for (i, col) in self.columns.iter().enumerate() {
            let info = col.flag | col.typ;
            writer.write_all(&[info])?;
            write_u32_be(writer, column_name_offsets[i])?;

            // Write constant value if present
            if let Some(ref val) = col.constant_value {
                self.write_value(writer, val, &constant_offsets[i])?;
            }
        }

        // Phase 5: Write rows
        for (row_idx, row) in self.rows.iter().enumerate() {
            for col in &self.columns {
                if col.constant_value.is_some() {
                    continue;
                }
                if let Some(val) = row.get(&col.name) {
                    let offset_pair = row_string_data_offsets[row_idx].get(&col.name);
                    self.write_value(writer, val, &offset_pair.cloned())?;
                } else {
                    // Write zero/default for missing values
                    self.write_default(writer, col.typ)?;
                }
            }
        }

        // Phase 6: Write string table
        writer.write_all(string_table.data())?;

        // Phase 7: Write data table
        writer.write_all(data_table.data())?;

        Ok(())
    }

    fn write_value<W: Write>(
        &self,
        writer: &mut W,
        val: &Value,
        offset_pair: &Option<(u32, u32)>,
    ) -> Result<(), BuilderError> {
        match val {
            Value::U8(v) => writer.write_all(&[*v])?,
            Value::I8(v) => writer.write_all(&[*v as u8])?,
            Value::U16(v) => write_u16_be(writer, *v)?,
            Value::I16(v) => write_i16_be(writer, *v)?,
            Value::U32(v) => write_u32_be(writer, *v)?,
            Value::I32(v) => write_i32_be(writer, *v)?,
            Value::U64(v) => write_u64_be(writer, *v)?,
            Value::F32(v) => write_f32_be(writer, *v)?,
            Value::String(_) => {
                if let Some((offset, _)) = offset_pair {
                    write_u32_be(writer, *offset)?;
                } else {
                    write_u32_be(writer, 0)?;
                }
            }
            Value::Data(_) => {
                if let Some((offset, size)) = offset_pair {
                    write_u32_be(writer, *offset)?;
                    write_u32_be(writer, *size)?;
                } else {
                    write_u32_be(writer, 0)?;
                    write_u32_be(writer, 0)?;
                }
            }
        }
        Ok(())
    }

    fn write_default<W: Write>(&self, writer: &mut W, typ: u8) -> Result<(), BuilderError> {
        let size = Self::value_size(typ) as usize;
        writer.write_all(&vec![0u8; size])?;
        Ok(())
    }
}

// ============================================================================
// String Table
// ============================================================================

struct StringTable {
    data: Vec<u8>,
    offsets: HashMap<String, u32>,
}

impl StringTable {
    fn new() -> Self {
        Self {
            data: Vec::new(),
            offsets: HashMap::new(),
        }
    }

    fn add(&mut self, s: &str) -> u32 {
        if let Some(&offset) = self.offsets.get(s) {
            return offset;
        }
        let offset = self.data.len() as u32;
        let encoded = UtfTableBuilder::encode_string(s);
        self.data.extend(encoded);
        self.offsets.insert(s.to_string(), offset);
        offset
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}

// ============================================================================
// Data Table
// ============================================================================

struct DataTable {
    data: Vec<u8>,
}

impl DataTable {
    fn new() -> Self {
        Self { data: Vec::new() }
    }

    fn add(&mut self, d: &[u8]) -> u32 {
        let offset = self.data.len() as u32;
        self.data.extend(d);
        offset
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}

// ============================================================================
// AFS2 Archive Builder
// ============================================================================

/// Builder for AFS2 (AWB) archives
pub struct AfsArchiveBuilder {
    alignment: u32,
    files: Vec<(u32, Vec<u8>)>, // (cue_id, data)
}

impl AfsArchiveBuilder {
    pub fn new() -> Self {
        Self {
            alignment: 32,
            files: Vec::new(),
        }
    }

    pub fn with_alignment(mut self, alignment: u32) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn add_file(&mut self, cue_id: u32, data: Vec<u8>) -> &mut Self {
        self.files.push((cue_id, data));
        self
    }

    fn align_offset(offset: u32, alignment: u32) -> u32 {
        (offset + alignment - 1) & !(alignment - 1)
    }

    /// Build the AFS2 archive and write to output
    pub fn build<W: Write + Seek>(&self, writer: &mut W) -> Result<(), BuilderError> {
        if self.files.is_empty() {
            return Err(BuilderError::NoTracks);
        }

        let file_count = self.files.len() as u32;

        // AFS2 header structure:
        // 0x00: "AFS2" magic (4 bytes)
        // 0x04: version/type (4 bytes) - typically 0x01 0x04 0x02 0x00
        // 0x08: file count (4 bytes LE)
        // 0x0C: alignment (4 bytes LE)
        // 0x10: file IDs (2 bytes each * file_count)
        // Then: file offset table (4 bytes each * (file_count + 1))
        // Then: file data (aligned)

        let id_table_size = file_count * 2;
        let offset_table_size = (file_count + 1) * 4;
        let header_size = 0x10 + id_table_size + offset_table_size;
        let data_start = Self::align_offset(header_size, self.alignment);

        // Calculate file offsets
        let mut file_offsets = Vec::new();
        let mut current_offset = data_start;
        for (_, data) in &self.files {
            file_offsets.push(current_offset);
            current_offset = Self::align_offset(current_offset + data.len() as u32, self.alignment);
        }
        file_offsets.push(current_offset); // End offset

        // Write header
        writer.write_all(b"AFS2")?;
        writer.write_all(&[0x01, 0x04, 0x02, 0x00])?; // Version
        write_u32_le(writer, file_count)?;
        write_u32_le(writer, self.alignment)?;

        // Write file IDs
        for (cue_id, _) in &self.files {
            write_u16_le(writer, *cue_id as u16)?;
        }

        // Write offset table
        for offset in &file_offsets {
            write_u32_le(writer, *offset)?;
        }

        // Pad to data start
        let current_pos = writer.stream_position()? as u32;
        let padding = data_start - current_pos;
        writer.write_all(&vec![0u8; padding as usize])?;

        // Write file data with alignment padding
        for (i, (_, data)) in self.files.iter().enumerate() {
            writer.write_all(data)?;
            if i < self.files.len() - 1 {
                let current_pos = writer.stream_position()? as u32;
                let next_offset = file_offsets[i + 1];
                let padding = next_offset - current_pos;
                writer.write_all(&vec![0u8; padding as usize])?;
            }
        }

        Ok(())
    }
}

impl Default for AfsArchiveBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ACB Builder
// ============================================================================

/// Builder for complete ACB files
pub struct AcbBuilder {
    tracks: Vec<TrackInput>,
    acb_version: u32,
    streaming_awb: bool,
}

impl AcbBuilder {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            acb_version: 0x01300500, // Common ACB version
            streaming_awb: false,
        }
    }

    pub fn add_track(&mut self, track: TrackInput) -> &mut Self {
        self.tracks.push(track);
        self
    }

    pub fn streaming_awb(mut self, streaming: bool) -> Self {
        self.streaming_awb = streaming;
        self
    }

    /// Build the ACB file and optionally AWB file
    pub fn build<W: Write + Seek>(
        &self,
        acb_writer: &mut W,
        awb_writer: Option<&mut W>,
    ) -> Result<(), BuilderError> {
        if self.tracks.is_empty() {
            return Err(BuilderError::NoTracks);
        }

        // Build AWB archive (embedded or external)
        let mut awb_data = Vec::new();
        {
            let mut awb_cursor = std::io::Cursor::new(&mut awb_data);
            let mut awb_builder = AfsArchiveBuilder::new();
            for track in &self.tracks {
                awb_builder.add_file(track.cue_id, track.data.clone());
            }
            awb_builder.build(&mut awb_cursor)?;
        }

        if self.streaming_awb {
            // External AWB: write to separate file
            if let Some(awb_out) = awb_writer {
                awb_out.write_all(&awb_data)?;
            }
        }

        // Build ACB UTF table
        let mut acb_table = UtfTableBuilder::new("Header");

        // Add ACB header columns
        acb_table.add_column(ColumnDef::constant("FileIdentifier", Value::U32(0)));
        acb_table.add_column(ColumnDef::constant("Size", Value::U32(0))); // Will be updated
        acb_table.add_column(ColumnDef::constant("Version", Value::U32(self.acb_version)));
        acb_table.add_column(ColumnDef::constant("Type", Value::U8(0)));
        acb_table.add_column(ColumnDef::constant("Target", Value::U8(0)));
        acb_table.add_column(ColumnDef::constant(
            "AcbVolume",
            Value::F32(1.0),
        ));
        acb_table.add_column(ColumnDef::constant(
            "NumCueLimit",
            Value::U16(self.tracks.len() as u16),
        ));

        // Add Cue table
        let cue_table = self.build_cue_table()?;
        acb_table.add_column(ColumnDef::constant("CueTable", Value::Data(cue_table)));

        // Add CueName table
        let cue_name_table = self.build_cue_name_table()?;
        acb_table.add_column(ColumnDef::constant(
            "CueNameTable",
            Value::Data(cue_name_table),
        ));

        // Add Waveform table
        let waveform_table = self.build_waveform_table()?;
        acb_table.add_column(ColumnDef::constant(
            "WaveformTable",
            Value::Data(waveform_table),
        ));

        // Add Synth table
        let synth_table = self.build_synth_table()?;
        acb_table.add_column(ColumnDef::constant("SynthTable", Value::Data(synth_table)));

        // Add AWB (embedded or reference)
        if !self.streaming_awb {
            acb_table.add_column(ColumnDef::constant(
                "AwbFile",
                Value::Data(awb_data),
            ));
        } else {
            acb_table.add_column(ColumnDef::constant(
                "StreamAwbHash",
                Value::Data(vec![0u8; 16]),
            ));
        }

        // Add one row (ACB has single-row header table)
        acb_table.add_row(HashMap::new());

        // Build ACB
        acb_table.build(acb_writer)?;

        Ok(())
    }

    fn build_cue_table(&self) -> Result<Vec<u8>, BuilderError> {
        let mut table = UtfTableBuilder::new("CueTable");

        table.add_column(ColumnDef::per_row("CueId", COLUMN_TYPE_4BYTE));
        table.add_column(ColumnDef::per_row("ReferenceType", COLUMN_TYPE_1BYTE));
        table.add_column(ColumnDef::per_row("ReferenceIndex", COLUMN_TYPE_2BYTE));

        for (i, track) in self.tracks.iter().enumerate() {
            let mut row = HashMap::new();
            row.insert("CueId".to_string(), Value::U32(track.cue_id));
            row.insert("ReferenceType".to_string(), Value::U8(3)); // Synth reference
            row.insert("ReferenceIndex".to_string(), Value::U16(i as u16));
            table.add_row(row);
        }

        let mut buf = std::io::Cursor::new(Vec::new());
        table.build(&mut buf)?;
        Ok(buf.into_inner())
    }

    fn build_cue_name_table(&self) -> Result<Vec<u8>, BuilderError> {
        let mut table = UtfTableBuilder::new("CueNameTable");

        table.add_column(ColumnDef::per_row("CueIndex", COLUMN_TYPE_2BYTE));
        table.add_column(ColumnDef::per_row("CueName", COLUMN_TYPE_STRING));

        for (i, track) in self.tracks.iter().enumerate() {
            let mut row = HashMap::new();
            row.insert("CueIndex".to_string(), Value::U16(i as u16));
            row.insert("CueName".to_string(), Value::String(track.name.clone()));
            table.add_row(row);
        }

        let mut buf = std::io::Cursor::new(Vec::new());
        table.build(&mut buf)?;
        Ok(buf.into_inner())
    }

    fn build_waveform_table(&self) -> Result<Vec<u8>, BuilderError> {
        let mut table = UtfTableBuilder::new("WaveformTable");

        table.add_column(ColumnDef::per_row("Id", COLUMN_TYPE_2BYTE));
        table.add_column(ColumnDef::per_row("EncodeType", COLUMN_TYPE_1BYTE));
        table.add_column(ColumnDef::per_row("Streaming", COLUMN_TYPE_1BYTE));
        table.add_column(ColumnDef::per_row("MemoryAwbId", COLUMN_TYPE_2BYTE));
        table.add_column(ColumnDef::per_row("StreamAwbId", COLUMN_TYPE_2BYTE));

        for (i, track) in self.tracks.iter().enumerate() {
            let mut row = HashMap::new();
            row.insert("Id".to_string(), Value::U16(i as u16));
            row.insert("EncodeType".to_string(), Value::U8(track.encode_type as u8));
            row.insert(
                "Streaming".to_string(),
                Value::U8(if self.streaming_awb { 1 } else { 0 }),
            );
            row.insert(
                "MemoryAwbId".to_string(),
                Value::U16(if self.streaming_awb {
                    0xFFFF
                } else {
                    i as u16
                }),
            );
            row.insert(
                "StreamAwbId".to_string(),
                Value::U16(if self.streaming_awb {
                    i as u16
                } else {
                    0xFFFF
                }),
            );
            table.add_row(row);
        }

        let mut buf = std::io::Cursor::new(Vec::new());
        table.build(&mut buf)?;
        Ok(buf.into_inner())
    }

    fn build_synth_table(&self) -> Result<Vec<u8>, BuilderError> {
        let mut table = UtfTableBuilder::new("SynthTable");

        table.add_column(ColumnDef::per_row("Type", COLUMN_TYPE_1BYTE));
        table.add_column(ColumnDef::per_row("VoiceLimitGroupName", COLUMN_TYPE_STRING));
        table.add_column(ColumnDef::per_row("ReferenceItems", COLUMN_TYPE_DATA));

        for i in 0..self.tracks.len() {
            let mut row = HashMap::new();
            row.insert("Type".to_string(), Value::U8(0)); // Single waveform
            row.insert("VoiceLimitGroupName".to_string(), Value::String(String::new()));

            // ReferenceItems: 2-byte count + 2-byte index pairs
            let ref_items = {
                let mut data = Vec::new();
                data.extend(&1u16.to_be_bytes()); // count
                data.extend(&(i as u16).to_be_bytes()); // waveform index
                data
            };
            row.insert("ReferenceItems".to_string(), Value::Data(ref_items));
            table.add_row(row);
        }

        let mut buf = std::io::Cursor::new(Vec::new());
        table.build(&mut buf)?;
        Ok(buf.into_inner())
    }
}

impl Default for AcbBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Binary writing helpers (Big Endian)
// ============================================================================

fn write_u16_be<W: Write>(w: &mut W, v: u16) -> io::Result<()> {
    w.write_all(&v.to_be_bytes())
}

fn write_i16_be<W: Write>(w: &mut W, v: i16) -> io::Result<()> {
    w.write_all(&v.to_be_bytes())
}

fn write_u32_be<W: Write>(w: &mut W, v: u32) -> io::Result<()> {
    w.write_all(&v.to_be_bytes())
}

fn write_i32_be<W: Write>(w: &mut W, v: i32) -> io::Result<()> {
    w.write_all(&v.to_be_bytes())
}

fn write_u64_be<W: Write>(w: &mut W, v: u64) -> io::Result<()> {
    w.write_all(&v.to_be_bytes())
}

fn write_f32_be<W: Write>(w: &mut W, v: f32) -> io::Result<()> {
    w.write_all(&v.to_be_bytes())
}

// Little Endian helpers for AFS2
fn write_u16_le<W: Write>(w: &mut W, v: u16) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_u32_le<W: Write>(w: &mut W, v: u32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_table() {
        let mut st = StringTable::new();
        let off1 = st.add("hello");
        let off2 = st.add("world");
        let off3 = st.add("hello"); // Duplicate

        assert_eq!(off1, 0);
        assert_eq!(off3, off1); // Should reuse
        assert!(off2 > off1);
    }

    #[test]
    fn test_afs_builder() {
        let mut builder = AfsArchiveBuilder::new();
        builder.add_file(0, vec![1, 2, 3, 4]);
        builder.add_file(1, vec![5, 6, 7, 8, 9]);

        let mut output = std::io::Cursor::new(Vec::new());
        builder.build(&mut output).unwrap();

        let data = output.into_inner();
        assert_eq!(&data[0..4], b"AFS2");
    }

    #[test]
    fn test_utf_table_builder() {
        let mut builder = UtfTableBuilder::new("TestTable");
        builder.add_column(ColumnDef::constant("Version", Value::U32(1)));
        builder.add_column(ColumnDef::per_row("Name", COLUMN_TYPE_STRING));

        let mut row = HashMap::new();
        row.insert("Name".to_string(), Value::String("test".to_string()));
        builder.add_row(row);

        let mut output = std::io::Cursor::new(Vec::new());
        builder.build(&mut output).unwrap();

        let data = output.into_inner();
        assert_eq!(&data[0..4], b"@UTF");
    }
}
