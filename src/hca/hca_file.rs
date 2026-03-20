//! High-level HCA decoder with streaming capabilities

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};

use super::decoder::{ClHca, HcaError};
pub use super::decoder::HcaInfo;

/// Key test parameters for testing HCA decryption keys
#[derive(Debug, Clone)]
pub struct KeyTest {
    pub key: u64,
    pub subkey: u64,
    pub start_offset: u32,
    pub best_score: i32,
    pub best_key: u64,
}

impl Default for KeyTest {
    fn default() -> Self {
        Self {
            key: 0,
            subkey: 0,
            start_offset: 0,
            best_score: 0,
            best_key: 0,
        }
    }
}

// Key testing constants
const HCA_KEY_SCORE_SCALE: i32 = 10;
const HCA_KEY_MAX_SKIP_BLANKS: i32 = 1200;
const HCA_KEY_MIN_TEST_FRAMES: i32 = 3;
const HCA_KEY_MAX_TEST_FRAMES: i32 = 7;
const HCA_KEY_MAX_FRAME_SCORE: i32 = 600;
const HCA_KEY_MAX_TOTAL_SCORE: i32 = HCA_KEY_MAX_TEST_FRAMES * 50 * HCA_KEY_SCORE_SCALE;

/// High-level HCA decoder wrapping the low-level ClHCA decoder with streaming capabilities
pub struct HcaDecoder<R: Read + Seek> {
    reader: R,
    info: HcaInfo,
    handle: ClHca,
    buf: Vec<u8>,
    fbuf: Vec<f32>,
    current_delay: i32,
    current_block: u32,
    owns_file: bool,
}

impl HcaDecoder<File> {
    /// Create a new HCA decoder from a file path
    pub fn from_file(filename: &str) -> Result<Self, HcaDecoderError> {
        let file = File::open(filename)?;
        let mut decoder = HcaDecoder::from_reader(file)?;
        decoder.owns_file = true;
        Ok(decoder)
    }
}

impl<R: Read + Seek> HcaDecoder<R> {
    /// Create a new HCA decoder from a reader
    pub fn from_reader(mut reader: R) -> Result<Self, HcaDecoderError> {
        // Test header
        let mut header_buf = [0u8; 8];
        reader.read_exact(&mut header_buf)?;

        let header_size = ClHca::is_hca_file(&header_buf)
            .ok_or(HcaDecoderError::InvalidHeader)?;

        if header_size > 0x1000 {
            return Err(HcaDecoderError::InvalidHeader);
        }

        // Read full header
        let mut full_header = vec![0u8; header_size];
        reader.seek(SeekFrom::Start(0))?;
        reader.read_exact(&mut full_header)?;

        // Initialize decoder
        let mut handle = ClHca::new();
        handle.decode_header(&full_header)?;

        let info = handle.get_info()?;

        // Allocate buffers
        let buf = vec![0u8; info.block_size as usize];
        let fbuf = vec![0.0f32; info.channel_count as usize * info.samples_per_block];

        let current_delay = info.encoder_delay as i32;

        Ok(Self {
            reader,
            info,
            handle,
            buf,
            fbuf,
            current_delay,
            current_block: 0,
            owns_file: false,
        })
    }

    /// Reset the decoder to the beginning
    pub fn reset(&mut self) {
        self.handle.decode_reset();
        self.current_block = 0;
        self.current_delay = self.info.encoder_delay as i32;
    }

    /// Get the HCA file information
    pub fn info(&self) -> &HcaInfo {
        &self.info
    }

    /// Set the decryption key
    pub fn set_encryption_key(&mut self, keycode: u64, subkey: u64) {
        let key = if subkey != 0 {
            keycode.wrapping_mul((subkey << 16) | (!subkey as u16 as u64).wrapping_add(2))
        } else {
            keycode
        };
        self.handle.set_key(key);
    }

    /// Read a single HCA frame/block
    fn read_packet(&mut self) -> Result<(), HcaDecoderError> {
        if self.current_block >= self.info.block_count {
            return Err(HcaDecoderError::Eof);
        }

        let offset = self.info.header_size as u64 + self.current_block as u64 * self.info.block_size as u64;
        self.reader.seek(SeekFrom::Start(offset))?;
        self.reader.read_exact(&mut self.buf)?;

        self.current_block += 1;
        Ok(())
    }

    /// Decode a single frame and return the samples
    /// Returns (samples slice, num samples) or error
    pub fn decode_frame(&mut self) -> Result<(&[f32], usize), HcaDecoderError> {
        // Read packet
        self.read_packet()?;

        // Decode frame
        self.handle.decode_block(&mut self.buf)?;

        // Read samples
        self.handle.read_samples(&mut self.fbuf);

        let samples = self.info.samples_per_block as i32;
        let mut discard = 0;

        // Handle encoder delay
        if self.current_delay > 0 {
            if self.current_delay >= samples {
                self.current_delay -= samples;
                return Ok((&[], 0));
            }
            discard = self.current_delay;
            self.current_delay = 0;
        }

        let start_idx = discard as usize * self.info.channel_count as usize;
        let num_samples = (samples - discard) as usize;
        Ok((&self.fbuf[start_idx..], num_samples))
    }

    /// Decode the entire HCA file and return all samples
    pub fn decode_all(&mut self) -> Result<Vec<f32>, HcaDecoderError> {
        self.reset();

        let channel_count = self.info.channel_count as usize;
        let total_samples = self.info.block_count as usize * self.info.samples_per_block;
        let mut all_samples = Vec::with_capacity(total_samples * channel_count);

        loop {
            match self.decode_frame() {
                Ok((samples, num_samples)) => {
                    let samples_to_add = num_samples * channel_count;
                    all_samples.extend_from_slice(&samples[..samples_to_add]);
                }
                Err(HcaDecoderError::Eof) => break,
                Err(e) => return Err(e),
            }
        }

        Ok(all_samples)
    }

    /// Seek to a specific sample position
    pub fn seek(&mut self, sample_num: u32) {
        let target_sample = sample_num + self.info.encoder_delay;
        let loop_start_block = target_sample / self.info.samples_per_block as u32;
        let loop_start_delay = target_sample - (loop_start_block * self.info.samples_per_block as u32);

        self.current_block = loop_start_block;
        self.current_delay = loop_start_delay as i32;
    }

    /// Test if a key correctly decrypts the HCA file
    pub fn test_key(&mut self, kt: &mut KeyTest) {
        let score = self.test_hca_score(kt);

        // Wrong key
        if score < 0 {
            return;
        }

        // Update if something better is found
        if kt.best_score <= 0 || (score < kt.best_score && score > 0) {
            kt.best_score = score;
            kt.best_key = kt.key;
        }
    }

    /// Test a number of frames to see if key decrypts correctly
    fn test_hca_score(&mut self, kt: &mut KeyTest) -> i32 {
        let mut test_frames = 0;
        let mut current_frame = 0u32;
        let mut blank_frames = 0;
        let mut total_score = 0;

        let mut offset = kt.start_offset;
        if offset == 0 {
            offset = self.info.header_size;
        }

        self.set_encryption_key(kt.key, kt.subkey);

        while test_frames < HCA_KEY_MAX_TEST_FRAMES && current_frame < self.info.block_count {
            let (score, should_break, new_offset) = self.test_single_frame(kt, offset, blank_frames);
            offset = new_offset;

            if should_break {
                total_score = -1;
                break;
            }

            if score < 0 {
                break;
            }

            current_frame += 1;

            if score == 0 && blank_frames < HCA_KEY_MAX_SKIP_BLANKS {
                blank_frames += 1;
                continue;
            }

            test_frames += 1;
            total_score += scale_frame_score(score);

            if total_score > HCA_KEY_MAX_TOTAL_SCORE {
                break;
            }
        }

        self.handle.decode_reset();
        finalize_score(total_score, test_frames)
    }

    fn test_single_frame(&mut self, kt: &mut KeyTest, offset: u32, _blank_frames: i32) -> (i32, bool, u32) {
        if self.reader.seek(SeekFrom::Start(offset as u64)).is_err() {
            return (-1, false, offset);
        }

        if self.reader.read_exact(&mut self.buf).is_err() {
            return (-1, false, offset);
        }

        let score = self.handle.test_block(&mut self.buf);

        // Get first non-blank frame
        if kt.start_offset == 0 && score != 0 {
            kt.start_offset = offset;
        }

        let new_offset = offset + self.info.block_size;

        if score < 0 || score > HCA_KEY_MAX_FRAME_SCORE {
            return (0, true, new_offset);
        }

        (score, false, new_offset)
    }

    /// Decode the entire file to 16-bit WAV stream
    pub fn decode_to_wav<W: Write>(&mut self, w: &mut W) -> Result<(), HcaDecoderError> {
        self.reset();

        let total_samples = (self.info.block_count * self.info.samples_per_block as u32)
            .saturating_sub(self.info.encoder_delay) as usize;
        let total_pcm_bytes = total_samples * self.info.channel_count as usize * 2;

        // Write WAV header
        let mut header = [0u8; 44];
        header[0..4].copy_from_slice(b"RIFF");
        header[4..8].copy_from_slice(&((36 + total_pcm_bytes) as u32).to_le_bytes());
        header[8..12].copy_from_slice(b"WAVE");
        header[12..16].copy_from_slice(b"fmt ");
        header[16..20].copy_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        header[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM format
        header[22..24].copy_from_slice(&(self.info.channel_count as u16).to_le_bytes());
        header[24..28].copy_from_slice(&self.info.sampling_rate.to_le_bytes());
        let byte_rate = self.info.sampling_rate * self.info.channel_count * 2;
        header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
        let block_align = (self.info.channel_count * 2) as u16;
        header[32..34].copy_from_slice(&block_align.to_le_bytes());
        header[34..36].copy_from_slice(&16u16.to_le_bytes()); // bits per sample
        header[36..40].copy_from_slice(b"data");
        header[40..44].copy_from_slice(&(total_pcm_bytes as u32).to_le_bytes());

        w.write_all(&header)?;

        let mut pcm_buf = vec![0i16; self.info.samples_per_block * self.info.channel_count as usize];

        loop {
            match self.read_packet() {
                Ok(()) => {}
                Err(HcaDecoderError::Eof) => break,
                Err(e) => return Err(e),
            }

            self.handle.decode_block(&mut self.buf)?;
            self.handle.read_samples_16(&mut pcm_buf);

            let samples = self.info.samples_per_block as i32;
            let mut discard = 0;

            if self.current_delay > 0 {
                if self.current_delay >= samples {
                    self.current_delay -= samples;
                    continue;
                }
                discard = self.current_delay;
                self.current_delay = 0;
            }

            let start = discard as usize * self.info.channel_count as usize;
            let end = samples as usize * self.info.channel_count as usize;

            if start >= end || end > pcm_buf.len() {
                return Err(HcaDecoderError::InvalidSampleRange);
            }

            // Write samples as little-endian bytes
            let mut data = vec![0u8; (end - start) * 2];
            for (i, &sample) in pcm_buf[start..end].iter().enumerate() {
                data[i * 2..i * 2 + 2].copy_from_slice(&sample.to_le_bytes());
            }
            w.write_all(&data)?;
        }

        Ok(())
    }
}

fn scale_frame_score(score: i32) -> i32 {
    match score {
        1 => 1,
        0 => 3 * HCA_KEY_SCORE_SCALE,
        _ => score * HCA_KEY_SCORE_SCALE,
    }
}

fn finalize_score(total_score: i32, test_frames: i32) -> i32 {
    // Signal best possible score
    if test_frames > HCA_KEY_MIN_TEST_FRAMES && total_score > 0 && total_score <= test_frames {
        return 1;
    }
    total_score
}

/// Errors that can occur during HCA decoding
#[derive(Debug)]
pub enum HcaDecoderError {
    Io(io::Error),
    Hca(HcaError),
    InvalidHeader,
    InvalidSampleRange,
    Eof,
}

impl std::fmt::Display for HcaDecoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Hca(e) => write!(f, "HCA error: {}", e),
            Self::InvalidHeader => write!(f, "Invalid HCA header"),
            Self::InvalidSampleRange => write!(f, "Invalid sample range"),
            Self::Eof => write!(f, "End of file"),
        }
    }
}

impl std::error::Error for HcaDecoderError {}

impl From<io::Error> for HcaDecoderError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<HcaError> for HcaDecoderError {
    fn from(e: HcaError) -> Self {
        Self::Hca(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_frame_score() {
        assert_eq!(scale_frame_score(1), 1);
        assert_eq!(scale_frame_score(0), 3 * HCA_KEY_SCORE_SCALE); // 30
        assert_eq!(scale_frame_score(5), 5 * HCA_KEY_SCORE_SCALE); // 50
        assert_eq!(scale_frame_score(-1), -1 * HCA_KEY_SCORE_SCALE); // -10
    }

    #[test]
    fn test_finalize_score() {
        // Best possible: enough frames, small positive score
        assert_eq!(finalize_score(4, 5), 1); // total_score(4) <= test_frames(5), frames > 3
        // Not enough frames
        assert_eq!(finalize_score(2, 2), 2); // test_frames(2) <= MIN_TEST_FRAMES(3)
        // Score too high
        assert_eq!(finalize_score(100, 5), 100);
        // Negative
        assert_eq!(finalize_score(-1, 5), -1);
    }

    #[test]
    fn test_key_test_default() {
        let kt = KeyTest::default();
        assert_eq!(kt.key, 0);
        assert_eq!(kt.subkey, 0);
        assert_eq!(kt.start_offset, 0);
        assert_eq!(kt.best_score, 0);
        assert_eq!(kt.best_key, 0);
    }

    #[test]
    fn test_hca_decoder_error_display() {
        let err = HcaDecoderError::InvalidHeader;
        assert_eq!(format!("{}", err), "Invalid HCA header");

        let err = HcaDecoderError::Eof;
        assert_eq!(format!("{}", err), "End of file");

        let err = HcaDecoderError::InvalidSampleRange;
        assert_eq!(format!("{}", err), "Invalid sample range");
    }
}
