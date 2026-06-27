//! AFS2 archive parser

use crate::reader::{align, Reader};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AfsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid AFS2 magic")]
    BadMagic,
    #[error("Cue ID {0} not found in archive")]
    CueNotFound(i32),
}

/// A file entry in an AFS2 archive
#[derive(Debug, Clone, Copy)]
pub struct AfsFileEntry {
    pub cue_id: i32,
    pub offset: u32,
    pub size: u32,
}

/// AFS2 archive
pub struct AfsArchive<R: Read + Seek> {
    pub alignment: u32,
    pub subkey: u16,
    pub files: Vec<AfsFileEntry>,
    cue_index: HashMap<i32, usize>,
    reader: Reader<R>,
}

impl<R: Read + Seek> AfsArchive<R> {
    /// Create a new AFS archive from a reader
    pub fn new(r: R) -> Result<Self, AfsError> {
        let mut buf = Reader::new(r);

        let magic = buf.read_u32()?;
        if magic != 0x41465332 {
            // "AFS2"
            return Err(AfsError::BadMagic);
        }

        let version = buf.read_bytes(4)?;
        let file_count = buf.read_u32_le()?;
        let alignment = buf.read_u16_le()? as u32;
        let subkey = buf.read_u16_le()?;

        let cue_id_size = version[2] as usize;
        let offset_size = version[1] as usize;
        // Handle the case when offset_size * 8 == 32 to avoid overflow
        let offset_mask = if offset_size >= 4 {
            0xFFFF_FFFFu32
        } else {
            (1u32 << (offset_size * 8)) - 1
        };

        // Read file entries starting at 0x10
        buf.seek(SeekFrom::Start(0x10))?;

        // Read cue IDs
        let mut cue_ids = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            let cue_id = if cue_id_size == 2 {
                buf.read_u16_le()? as i32
            } else {
                buf.read_u32_le()? as i32
            };
            cue_ids.push(cue_id);
        }

        // Read offsets
        let mut offsets = Vec::with_capacity(file_count as usize + 1);
        for _ in 0..=file_count {
            let offset = if offset_size == 2 {
                (buf.read_u16_le()? as u32) & offset_mask
            } else {
                buf.read_u32_le()? & offset_mask
            };
            offsets.push(offset);
        }

        // Calculate sizes
        let mut files = Vec::with_capacity(file_count as usize);
        for i in 0..file_count as usize {
            let aligned_offset = align(alignment, offsets[i]);
            // Align the next boundary too (except the final archive end), matching
            // vgmstream awb.c so a subfile's size spans up to the next aligned start.
            let next_offset = if i + 1 < file_count as usize {
                align(alignment, offsets[i + 1])
            } else {
                offsets[i + 1]
            };
            let size = next_offset - aligned_offset;

            files.push(AfsFileEntry {
                cue_id: cue_ids[i],
                offset: aligned_offset,
                size,
            });
        }

        let cue_index = files
            .iter()
            .enumerate()
            .map(|(idx, file)| (file.cue_id, idx))
            .collect();

        Ok(Self {
            alignment,
            subkey,
            files,
            cue_index,
            reader: buf,
        })
    }

    /// Get file data for a specific cue ID
    pub fn file_data_for_cue_id(&mut self, cue_id: i32) -> Result<Vec<u8>, AfsError> {
        if let Some(&idx) = self.cue_index.get(&cue_id) {
            return self.file_data_at_index(idx);
        }

        // Fallback to first file if cue IDs start at 0
        if !self.files.is_empty() && self.files[0].cue_id == 0 {
            return self.file_data_at_index(0);
        }

        Err(AfsError::CueNotFound(cue_id))
    }

    /// Get file data for an entry
    pub fn file_data(&mut self, entry: AfsFileEntry) -> Result<Vec<u8>, AfsError> {
        self.file_data_by_entry(&entry)
    }

    fn file_data_at_index(&mut self, index: usize) -> Result<Vec<u8>, AfsError> {
        let entry = self.files[index];
        self.file_data_by_entry(&entry)
    }

    fn file_data_by_entry(&mut self, entry: &AfsFileEntry) -> Result<Vec<u8>, AfsError> {
        Ok(self
            .reader
            .read_bytes_at(entry.size as usize, entry.offset as u64)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_afs_file_entry() {
        let entry = AfsFileEntry {
            cue_id: 0,
            offset: 0x100,
            size: 0x200,
        };
        assert_eq!(entry.cue_id, 0);
        assert_eq!(entry.offset, 0x100);
        assert_eq!(entry.size, 0x200);
    }
}
