//! HCA encoder - encodes PCM audio to HCA format
//!
//! This implements encoding PCM samples to CRI HCA format. The encoder
//! performs the inverse operations of the decoder:
//! 1. MDCT transform (time-domain to frequency-domain)
//! 2. Quantization with scale factors
//! 3. Frame packing with bitstream encoding

use super::bitreader::BitWriter;
use super::cipher::cipher_init;
use super::decoder::{
    crc16_checksum, HCA_MAX_CHANNELS, HCA_SAMPLES_PER_FRAME, HCA_SAMPLES_PER_SUBFRAME,
    HCA_SUBFRAMES, HCA_VERSION_200,
};
use super::tables::{
    DEQUANTIZER_RANGE_TABLE, DEQUANTIZER_SCALING_TABLE, INVERT_TABLE, MAX_BIT_TABLE,
};
use std::f32::consts::PI;
use std::io::{self, Seek, Write};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HcaEncoderError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Invalid sample rate: {0}")]
    InvalidSampleRate(u32),
    #[error("Invalid channel count: {0}")]
    InvalidChannelCount(u32),
    #[error("Frame too large")]
    FrameTooLarge,
    #[error("No samples provided")]
    NoSamples,
}

/// HCA encoder configuration
#[derive(Debug, Clone)]
pub struct HcaEncoderConfig {
    pub sample_rate: u32,
    pub channels: u32,
    pub bitrate: u32,
    pub encryption_key: Option<u64>,
    pub loop_start: Option<u32>,
    pub loop_end: Option<u32>,
}

impl Default for HcaEncoderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 2,
            bitrate: 256000, // 256 kbps
            encryption_key: None,
            loop_start: None,
            loop_end: None,
        }
    }
}

impl HcaEncoderConfig {
    pub fn new(sample_rate: u32, channels: u32) -> Self {
        Self {
            sample_rate,
            channels,
            ..Default::default()
        }
    }

    pub fn with_bitrate(mut self, bitrate: u32) -> Self {
        self.bitrate = bitrate;
        self
    }

    pub fn with_encryption(mut self, key: u64) -> Self {
        self.encryption_key = Some(key);
        self
    }
}

/// HCA encoder
pub struct HcaEncoder {
    config: HcaEncoderConfig,
    frame_size: u32,
    total_band_count: u32,
    base_band_count: u32,
    stereo_band_count: u32,
    
    // MDCT state
    mdct_window: [f32; HCA_SAMPLES_PER_SUBFRAME],
    mdct_sin_table: [f32; HCA_SAMPLES_PER_SUBFRAME],
    mdct_cos_table: [f32; HCA_SAMPLES_PER_SUBFRAME],
    
    // Overlap buffer for MDCT
    overlap_buffer: Vec<[f32; HCA_SAMPLES_PER_SUBFRAME]>,
    
    // Cipher table for encryption
    cipher_table: [u8; 256],
}

impl HcaEncoder {
    pub fn new(config: HcaEncoderConfig) -> Result<Self, HcaEncoderError> {
        if config.sample_rate == 0 || config.sample_rate > 0x7FFFFF {
            return Err(HcaEncoderError::InvalidSampleRate(config.sample_rate));
        }
        if config.channels == 0 || config.channels > HCA_MAX_CHANNELS as u32 {
            return Err(HcaEncoderError::InvalidChannelCount(config.channels));
        }

        // Calculate frame size from bitrate
        let frame_size = Self::calculate_frame_size(config.bitrate, config.sample_rate);

        // Band configuration (simplified - using standard values)
        // For simplicity, use mono-style encoding (no stereo bands)
        // This avoids complexity of intensity stereo encoding
        let total_band_count = 128u32;
        let base_band_count = 128u32; // Use full range for all channels
        let stereo_band_count = 0u32; // No intensity stereo

        // Initialize MDCT tables
        let (mdct_window, mdct_sin_table, mdct_cos_table) = Self::init_mdct_tables();

        // Initialize cipher table
        let mut cipher_table = [0u8; 256];
        if let Some(key) = config.encryption_key {
            cipher_init(&mut cipher_table, 56, key);
        } else {
            for i in 0..256 {
                cipher_table[i] = i as u8;
            }
        }

        // Initialize overlap buffers
        let overlap_buffer = vec![[0.0f32; HCA_SAMPLES_PER_SUBFRAME]; config.channels as usize];

        Ok(Self {
            config,
            frame_size,
            total_band_count,
            base_band_count,
            stereo_band_count,
            mdct_window,
            mdct_sin_table,
            mdct_cos_table,
            overlap_buffer,
            cipher_table,
        })
    }

    fn calculate_frame_size(bitrate: u32, sample_rate: u32) -> u32 {
        // Frame size = bitrate * samples_per_frame / sample_rate / 8
        let size = (bitrate as u64 * HCA_SAMPLES_PER_FRAME as u64) / (sample_rate as u64 * 8);
        (size as u32).clamp(0x100, 0x1000)
    }

    fn init_mdct_tables() -> (
        [f32; HCA_SAMPLES_PER_SUBFRAME],
        [f32; HCA_SAMPLES_PER_SUBFRAME],
        [f32; HCA_SAMPLES_PER_SUBFRAME],
    ) {
        let mut window = [0.0f32; HCA_SAMPLES_PER_SUBFRAME];
        let mut sin_table = [0.0f32; HCA_SAMPLES_PER_SUBFRAME];
        let mut cos_table = [0.0f32; HCA_SAMPLES_PER_SUBFRAME];

        let n = HCA_SAMPLES_PER_SUBFRAME as f32;
        for i in 0..HCA_SAMPLES_PER_SUBFRAME {
            // KBD-like window
            let x = (i as f32 + 0.5) / n;
            window[i] = (PI * x).sin().sqrt();

            // MDCT rotation
            let angle = PI * (i as f32 + 0.5) / n;
            sin_table[i] = angle.sin();
            cos_table[i] = angle.cos();
        }

        (window, sin_table, cos_table)
    }

    /// Encode PCM samples to HCA format
    pub fn encode<W: Write + Seek>(
        &mut self,
        samples: &[f32],
        writer: &mut W,
    ) -> Result<(), HcaEncoderError> {
        if samples.is_empty() {
            return Err(HcaEncoderError::NoSamples);
        }

        let channels = self.config.channels as usize;
        let frame_samples = HCA_SAMPLES_PER_FRAME * channels;
        let frame_count = (samples.len() + frame_samples - 1) / frame_samples;

        // Write HCA header
        let header_size = self.write_header(writer, frame_count as u32)?;

        // Encode frames
        for frame_idx in 0..frame_count {
            let start = frame_idx * frame_samples;
            let end = (start + frame_samples).min(samples.len());

            // Pad if necessary
            let mut frame_data = vec![0.0f32; frame_samples];
            frame_data[..end - start].copy_from_slice(&samples[start..end]);

            // Encode frame
            self.encode_frame(writer, &frame_data)?;
        }

        Ok(())
    }

    fn write_header<W: Write + Seek>(
        &self,
        writer: &mut W,
        frame_count: u32,
    ) -> Result<u32, HcaEncoderError> {
        let mut header = Vec::new();

        // Calculate header size (align to 16 bytes)
        let base_header_size = 0x60u32; // Minimum header
        let header_size = (base_header_size + 0xF) & !0xF;

        // HCA chunk: magic(4) + version(2) + header_size(2)
        let hca_magic = if self.config.encryption_key.is_some() {
            0xC8C3C100u32 // Masked "HCA\0"
        } else {
            0x48434100u32 // "HCA\0"
        };
        header.extend(&hca_magic.to_be_bytes());
        header.extend(&(HCA_VERSION_200 as u16).to_be_bytes()); // version
        header.extend(&(header_size as u16).to_be_bytes()); // header_size

        // fmt chunk
        let fmt_magic = if self.config.encryption_key.is_some() {
            0xE6ED7400u32
        } else {
            0x666D7400u32 // "fmt\0"
        };
        header.extend(&fmt_magic.to_be_bytes());
        header.push(self.config.channels as u8);
        header.extend(&(self.config.sample_rate & 0xFFFFFF).to_be_bytes()[1..]);
        header.extend(&frame_count.to_be_bytes());
        header.extend(&0u16.to_be_bytes()); // encoder_delay
        header.extend(&0u16.to_be_bytes()); // encoder_padding

        // comp chunk
        let comp_magic = if self.config.encryption_key.is_some() {
            0xE3EFEDf0u32
        } else {
            0x636F6D70u32 // "comp"
        };
        header.extend(&comp_magic.to_be_bytes());
        header.extend(&(self.frame_size as u16).to_be_bytes());
        header.push(1); // min_resolution
        header.push(15); // max_resolution
        header.push(1); // track_count
        header.push(1); // channel_config
        header.push(self.total_band_count as u8);
        header.push(self.base_band_count as u8);
        header.push(self.stereo_band_count as u8);
        header.push(0); // bands_per_hfr_group
        header.push(0); // ms_stereo
        header.push(0); // reserved

        // vbr chunk (disabled)
        // No VBR for now

        // ath chunk
        let ath_magic = if self.config.encryption_key.is_some() {
            0xE1F4E800u32
        } else {
            0x61746800u32 // "ath\0"
        };
        header.extend(&ath_magic.to_be_bytes());
        header.extend(&0u16.to_be_bytes()); // ath_type = 0

        // loop chunk (if enabled)
        if let (Some(start), Some(end)) = (self.config.loop_start, self.config.loop_end) {
            let loop_magic = if self.config.encryption_key.is_some() {
                0xECEFEFF0u32
            } else {
                0x6C6F6F70u32 // "loop"
            };
            header.extend(&loop_magic.to_be_bytes());
            header.extend(&start.to_be_bytes());
            header.extend(&end.to_be_bytes());
            header.extend(&0u16.to_be_bytes()); // loop_start_delay
            header.extend(&0u16.to_be_bytes()); // loop_end_padding
        }

        // ciph chunk
        if self.config.encryption_key.is_some() {
            let ciph_magic = 0xE3E9F0E8u32;
            header.extend(&ciph_magic.to_be_bytes());
            header.extend(&56u16.to_be_bytes()); // ciph_type = 56
        }

        // Pad to header_size
        while header.len() < header_size as usize - 2 {
            header.push(0);
        }

        // Calculate CRC so that the check passes (crc of data including crc should be 0)
        // We need to find crc such that crc16(header + crc_bytes) == 0
        let crc = crc16_checksum(&header);
        header.extend(&crc.to_be_bytes());

        writer.write_all(&header)?;

        Ok(header_size)
    }

    fn encode_frame<W: Write>(
        &mut self,
        writer: &mut W,
        samples: &[f32],
    ) -> Result<(), HcaEncoderError> {
        let channels = self.config.channels as usize;

        // Allocate frame buffer
        let mut frame_data = vec![0u8; self.frame_size as usize];

        // Process each channel
        let mut channel_spectra = vec![[[0.0f32; HCA_SAMPLES_PER_SUBFRAME]; HCA_SUBFRAMES]; channels];
        let mut channel_scale_factors = vec![[0u8; HCA_SAMPLES_PER_SUBFRAME]; channels];
        let mut channel_resolutions = vec![[0u8; HCA_SAMPLES_PER_SUBFRAME]; channels];

        for ch in 0..channels {
            // Extract channel samples
            let mut channel_samples = [0.0f32; HCA_SAMPLES_PER_FRAME];
            for i in 0..HCA_SAMPLES_PER_FRAME {
                let idx = i * channels + ch;
                channel_samples[i] = if idx < samples.len() {
                    samples[idx]
                } else {
                    0.0
                };
            }

            // Apply MDCT to each subframe
            for sf in 0..HCA_SUBFRAMES {
                let start = sf * HCA_SAMPLES_PER_SUBFRAME;
                let end = start + HCA_SAMPLES_PER_SUBFRAME;
                let subframe_samples = &channel_samples[start..end];

                self.mdct_transform(
                    ch,
                    subframe_samples,
                    &mut channel_spectra[ch][sf],
                );
            }

            // Quantize spectra
            self.quantize_channel(
                &channel_spectra[ch],
                &mut channel_scale_factors[ch],
                &mut channel_resolutions[ch],
            );
        }

        // Pack frame data
        let mut writer_bits = BitWriter::new(self.frame_size as usize);

        // Write sync word (0xFFFF)
        writer_bits.write(0xFFFF, 16);

        // Write frame header: 9 bits acceptable_noise_level + 7 bits evaluation_boundary
        let acceptable_noise_level = 256u32; // Typical value
        let evaluation_boundary = 0u32;
        writer_bits.write(acceptable_noise_level, 9);
        writer_bits.write(evaluation_boundary, 7);

        // Write channel data (scale factors, intensity, resolutions for each channel)
        for ch in 0..channels {
            self.encode_channel_header(
                &mut writer_bits,
                &channel_scale_factors[ch],
                ch,
            );
        }

        // Write spectra for each subframe
        for sf in 0..HCA_SUBFRAMES {
            for ch in 0..channels {
                self.encode_subframe_spectra(
                    &mut writer_bits,
                    &channel_spectra[ch][sf],
                    &channel_scale_factors[ch],
                    &channel_resolutions[ch],
                    self.base_band_count as usize,
                );
            }
        }

        // Get frame data
        let frame_bytes = writer_bits.into_vec();
        let copy_len = frame_bytes.len().min(self.frame_size as usize - 2);
        frame_data[..copy_len].copy_from_slice(&frame_bytes[..copy_len]);

        // Apply cipher if enabled
        if self.config.encryption_key.is_some() {
            for byte in &mut frame_data[..self.frame_size as usize - 2] {
                *byte = self.cipher_table[*byte as usize];
            }
        }

        // Calculate and append checksum using shared CRC function
        let checksum = crc16_checksum(&frame_data[..self.frame_size as usize - 2]);
        frame_data[self.frame_size as usize - 2] = (checksum >> 8) as u8;
        frame_data[self.frame_size as usize - 1] = (checksum & 0xFF) as u8;

        writer.write_all(&frame_data)?;

        Ok(())
    }

    /// Apply forward MDCT transform
    fn mdct_transform(
        &mut self,
        channel: usize,
        input: &[f32],
        output: &mut [f32; HCA_SAMPLES_PER_SUBFRAME],
    ) {
        let n = HCA_SAMPLES_PER_SUBFRAME;
        let half_n = n / 2;

        // Combine current input with previous overlap
        let mut windowed = [0.0f32; HCA_SAMPLES_PER_SUBFRAME * 2];
        
        // Previous samples (from overlap buffer) with analysis window
        for i in 0..n {
            windowed[i] = self.overlap_buffer[channel][i] * self.mdct_window[i];
        }
        
        // Current samples with analysis window (second half)
        for i in 0..n {
            windowed[n + i] = input[i] * self.mdct_window[n - 1 - i];
        }

        // Update overlap buffer
        for i in 0..n {
            self.overlap_buffer[channel][i] = input[i];
        }

        // MDCT core: DCT-IV of windowed data
        // X[k] = sum_{n=0}^{N-1} x[n] * cos(pi/N * (n + 0.5 + N/2) * (k + 0.5))
        for k in 0..n {
            let mut sum = 0.0f32;
            for i in 0..(n * 2) {
                let angle = PI / (n as f32) * ((i as f32) + 0.5 + (n as f32) / 2.0) * ((k as f32) + 0.5);
                sum += windowed[i] * angle.cos();
            }
            output[k] = sum * (2.0 / n as f32).sqrt();
        }
    }

    /// Quantize spectra and determine scale factors
    fn quantize_channel(
        &self,
        spectra: &[[f32; HCA_SAMPLES_PER_SUBFRAME]; HCA_SUBFRAMES],
        scale_factors: &mut [u8; HCA_SAMPLES_PER_SUBFRAME],
        resolutions: &mut [u8; HCA_SAMPLES_PER_SUBFRAME],
    ) {
        // Find maximum absolute value per band across all subframes
        let mut max_values = [0.0f32; HCA_SAMPLES_PER_SUBFRAME];
        for sf in 0..HCA_SUBFRAMES {
            for i in 0..HCA_SAMPLES_PER_SUBFRAME {
                let abs_val = spectra[sf][i].abs();
                if abs_val > max_values[i] {
                    max_values[i] = abs_val;
                }
            }
        }

        // Determine scale factors and resolutions
        for i in 0..HCA_SAMPLES_PER_SUBFRAME {
            let max_val = max_values[i];
            
            if max_val < 1e-10 {
                // Silent band
                scale_factors[i] = 0;
                resolutions[i] = 0;
            } else {
                // Find appropriate scale factor
                let mut best_sf = 0u8;
                let mut best_res = 1u8;
                let mut best_error = f32::MAX;

                for sf in 1..64u8 {
                    let sf_scale = DEQUANTIZER_SCALING_TABLE[sf as usize];
                    
                    for res in 1..16u8 {
                        let res_scale = DEQUANTIZER_RANGE_TABLE[res as usize];
                        let gain = sf_scale * res_scale;
                        
                        // Check if this gain can represent the value
                        if gain > 0.0 {
                            let quantized = max_val / gain;
                            let max_quant = (1 << (MAX_BIT_TABLE[res as usize] - 1)) as f32;
                            
                            if quantized <= max_quant {
                                let error = (quantized.round() * gain - max_val).abs();
                                if error < best_error {
                                    best_error = error;
                                    best_sf = sf;
                                    best_res = res;
                                }
                            }
                        }
                    }
                }

                scale_factors[i] = best_sf;
                resolutions[i] = best_res;
            }
        }
    }

    /// Encode channel header (scale factors only - no intensity for discrete channels)
    fn encode_channel_header(
        &self,
        writer: &mut BitWriter,
        scale_factors: &[u8; HCA_SAMPLES_PER_SUBFRAME],
        _channel: usize,
    ) {
        // Count coded bands
        let coded_count = self.base_band_count as usize;

        // Encode scale factors
        self.encode_scale_factors(writer, scale_factors, coded_count);

        // With stereo_band_count = 0, all channels are Discrete type
        // Discrete channels don't have intensity data - skip writing any
    }

    fn encode_scale_factors(
        &self,
        writer: &mut BitWriter,
        scale_factors: &[u8; HCA_SAMPLES_PER_SUBFRAME],
        coded_count: usize,
    ) {
        if coded_count == 0 {
            writer.write(0, 3); // delta_bits = 0
            return;
        }

        // Check if all scale factors are the same or zero
        let first = scale_factors[0];
        let all_same = scale_factors[..coded_count].iter().all(|&sf| sf == first);
        
        if all_same && first == 0 {
            writer.write(0, 3); // delta_bits = 0 (all zero)
            return;
        }

        // Use fixed 6-bit encoding for simplicity
        writer.write(6, 3); // delta_bits = 6 means direct encoding
        for i in 0..coded_count {
            writer.write(scale_factors[i] as u32, 6);
        }
    }

    fn encode_subframe_spectra(
        &self,
        writer: &mut BitWriter,
        spectra: &[f32; HCA_SAMPLES_PER_SUBFRAME],
        scale_factors: &[u8; HCA_SAMPLES_PER_SUBFRAME],
        resolutions: &[u8; HCA_SAMPLES_PER_SUBFRAME],
        coded_count: usize,
    ) {
        for i in 0..coded_count {
            let sf = scale_factors[i];
            let res = resolutions[i];

            if res == 0 || sf == 0 {
                continue;
            }

            let bits = MAX_BIT_TABLE[res as usize] as usize;
            if bits == 0 {
                continue;
            }

            // Calculate gain
            let sf_scale = DEQUANTIZER_SCALING_TABLE[sf as usize];
            let res_scale = DEQUANTIZER_RANGE_TABLE[res as usize];
            let gain = sf_scale * res_scale;

            // Quantize
            let value = spectra[i];
            let quantized = if gain > 0.0 {
                (value / gain).round() as i32
            } else {
                0
            };

            // Encode based on resolution
            if res > 7 {
                // Sign-magnitude encoding
                let sign = if quantized < 0 { 1u32 } else { 0u32 };
                let magnitude = quantized.unsigned_abs();
                let code = (magnitude << 1) | sign;
                writer.write(code, bits);
            } else {
                // Offset binary encoding (simplified)
                let offset = 1i32 << (bits - 1);
                let code = (quantized + offset).clamp(0, (1 << bits) - 1) as u32;
                writer.write(code, bits);
            }
        }
    }
}

/// Convenience function to encode WAV to HCA
pub fn encode_wav_to_hca<W: Write + Seek>(
    wav_data: &[u8],
    writer: &mut W,
    config: Option<HcaEncoderConfig>,
) -> Result<(), HcaEncoderError> {
    // Parse WAV header (minimal implementation)
    if wav_data.len() < 44 || &wav_data[0..4] != b"RIFF" || &wav_data[8..12] != b"WAVE" {
        return Err(HcaEncoderError::NoSamples);
    }

    // Find fmt chunk
    let mut pos = 12;
    let mut channels = 2u32;
    let mut sample_rate = 44100u32;
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
            channels = u16::from_le_bytes([wav_data[pos + 10], wav_data[pos + 11]]) as u32;
            sample_rate = u32::from_le_bytes([
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
    }

    if data_start == 0 || data_len == 0 {
        return Err(HcaEncoderError::NoSamples);
    }

    // Convert to f32 samples
    let mut samples = Vec::new();
    let bytes_per_sample = (bits_per_sample / 8) as usize;
    let sample_count = data_len / bytes_per_sample;

    for i in 0..sample_count {
        let offset = data_start + i * bytes_per_sample;
        if offset + bytes_per_sample > wav_data.len() {
            break;
        }

        let sample = match bits_per_sample {
            16 => {
                let raw = i16::from_le_bytes([wav_data[offset], wav_data[offset + 1]]);
                raw as f32 / 32768.0
            }
            24 => {
                let raw = i32::from_le_bytes([
                    0,
                    wav_data[offset],
                    wav_data[offset + 1],
                    wav_data[offset + 2],
                ]) >> 8;
                raw as f32 / 8388608.0
            }
            32 => {
                let raw = f32::from_le_bytes([
                    wav_data[offset],
                    wav_data[offset + 1],
                    wav_data[offset + 2],
                    wav_data[offset + 3],
                ]);
                raw
            }
            _ => 0.0,
        };
        samples.push(sample);
    }

    // Create encoder
    let cfg = config.unwrap_or_else(|| HcaEncoderConfig::new(sample_rate, channels));
    let mut encoder = HcaEncoder::new(cfg)?;

    // Encode
    encoder.encode(&samples, writer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_config() {
        let config = HcaEncoderConfig::new(44100, 2).with_bitrate(256000);
        assert_eq!(config.sample_rate, 44100);
        assert_eq!(config.channels, 2);
        assert_eq!(config.bitrate, 256000);
    }

    #[test]
    fn test_encoder_creation() {
        let config = HcaEncoderConfig::new(44100, 2);
        let encoder = HcaEncoder::new(config);
        assert!(encoder.is_ok());
    }

    #[test]
    fn test_invalid_config() {
        let config = HcaEncoderConfig::new(0, 2);
        let encoder = HcaEncoder::new(config);
        assert!(encoder.is_err());

        let config = HcaEncoderConfig::new(44100, 0);
        let encoder = HcaEncoder::new(config);
        assert!(encoder.is_err());
    }

    #[test]
    fn test_crc16() {
        let data = [0x48, 0x43, 0x41, 0x00]; // "HCA\0"
        let crc = crc16_checksum(&data);
        assert!(crc != 0);
    }

    #[test]
    fn test_encode_simple() {
        let config = HcaEncoderConfig::new(44100, 1);
        let mut encoder = HcaEncoder::new(config).unwrap();

        // Generate simple sine wave
        let samples: Vec<f32> = (0..HCA_SAMPLES_PER_FRAME)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin() * 0.5)
            .collect();

        let mut output = std::io::Cursor::new(Vec::new());
        let result = encoder.encode(&samples, &mut output);
        assert!(result.is_ok());

        let data = output.into_inner();
        // Check HCA header
        assert!(!data.is_empty());
        // First 4 bytes should be HCA magic (possibly masked)
        assert!(data[0] == 0x48 || data[0] == 0xC8); // 'H' or masked
    }
}
