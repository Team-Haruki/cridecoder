//! Constants for ACB/UTF parsing

// Column flag and type constants (matching vgmstream's cri_utf.c)
// The upper nibble is a bitmask of independent flags, NOT an enum.
// Valid combinations: NAME+DEFAULT (0x30), NAME+ROW (0x50), NAME+DEFAULT+ROW (0x70).
// NAME-only (0x10) means the column has no data.

pub const COLUMN_FLAG_MASK: u8 = 0xF0;
pub const COLUMN_FLAG_NAME: u8 = 0x10; // column has a name
pub const COLUMN_FLAG_DEFAULT: u8 = 0x20; // data relative to schema area (constant for all rows)
pub const COLUMN_FLAG_ROW: u8 = 0x40; // data relative to row start (per-row value)
pub const COLUMN_FLAG_UNDEFINED: u8 = 0x80; // shouldn't exist

pub const COLUMN_TYPE_MASK: u8 = 0x0F;
pub const COLUMN_TYPE_1BYTE: u8 = 0x00; // u8
pub const COLUMN_TYPE_1BYTE2: u8 = 0x01; // i8
pub const COLUMN_TYPE_2BYTE: u8 = 0x02; // u16
pub const COLUMN_TYPE_2BYTE2: u8 = 0x03; // i16
pub const COLUMN_TYPE_4BYTE: u8 = 0x04; // u32
pub const COLUMN_TYPE_4BYTE2: u8 = 0x05; // i32
pub const COLUMN_TYPE_8BYTE: u8 = 0x06; // u64
                                        // COLUMN_TYPE_8BYTE2 = 0x07; // i64 (unused)
pub const COLUMN_TYPE_FLOAT: u8 = 0x08; // f32
                                        // COLUMN_TYPE_DOUBLE = 0x09; // f64 (unused)
pub const COLUMN_TYPE_STRING: u8 = 0x0A;
pub const COLUMN_TYPE_DATA: u8 = 0x0B; // variable-length data (offset+size)

// Waveform encoding types
pub const WAVEFORM_ENCODE_TYPE_ADX: i32 = 0;
pub const WAVEFORM_ENCODE_TYPE_AHX: i32 = 1;
pub const WAVEFORM_ENCODE_TYPE_HCA: i32 = 2;
pub const WAVEFORM_ENCODE_TYPE_ADX_ALT: i32 = 3;
pub const WAVEFORM_ENCODE_TYPE_WII_ADPCM: i32 = 4;
pub const WAVEFORM_ENCODE_TYPE_DS_ADPCM: i32 = 5;
pub const WAVEFORM_ENCODE_TYPE_HCA_MX: i32 = 6;
pub const WAVEFORM_ENCODE_TYPE_VAG: i32 = 7;
pub const WAVEFORM_ENCODE_TYPE_ATRAC3: i32 = 8;
pub const WAVEFORM_ENCODE_TYPE_BCWAV: i32 = 9;
pub const WAVEFORM_ENCODE_TYPE_HEVAG: i32 = 10;
pub const WAVEFORM_ENCODE_TYPE_ATRAC9: i32 = 11;
pub const WAVEFORM_ENCODE_TYPE_XMA: i32 = 12;
pub const WAVEFORM_ENCODE_TYPE_NINTENDO_DSP: i32 = 13;
pub const WAVEFORM_ENCODE_TYPE_PS4_ATRAC9: i32 = 18;
pub const WAVEFORM_ENCODE_TYPE_M4A: i32 = 19;
pub const WAVEFORM_ENCODE_TYPE_SWITCH_OPUS: i32 = 24;

/// Get file extension for a waveform encode type
pub fn wave_type_extension(enc_type: i32) -> &'static str {
    match enc_type {
        WAVEFORM_ENCODE_TYPE_ADX | WAVEFORM_ENCODE_TYPE_ADX_ALT => ".adx",
        WAVEFORM_ENCODE_TYPE_AHX => ".ahx",
        WAVEFORM_ENCODE_TYPE_HCA => ".hca",
        WAVEFORM_ENCODE_TYPE_WII_ADPCM => ".wiiadpcm",
        WAVEFORM_ENCODE_TYPE_DS_ADPCM => ".dsadpcm",
        // HCA-MX is a standard HCA bitstream (HCA\0 magic); vgmstream awb.c:178
        // and PyCriCodecs both return ".hca" for type 6.
        WAVEFORM_ENCODE_TYPE_HCA_MX => ".hca",
        WAVEFORM_ENCODE_TYPE_VAG | WAVEFORM_ENCODE_TYPE_HEVAG => ".vag",
        WAVEFORM_ENCODE_TYPE_ATRAC3 => ".at3",
        WAVEFORM_ENCODE_TYPE_BCWAV => ".bcwav",
        WAVEFORM_ENCODE_TYPE_ATRAC9 | WAVEFORM_ENCODE_TYPE_PS4_ATRAC9 => ".at9",
        WAVEFORM_ENCODE_TYPE_XMA => ".xma",
        WAVEFORM_ENCODE_TYPE_NINTENDO_DSP => ".dsp",
        WAVEFORM_ENCODE_TYPE_M4A => ".m4a",
        WAVEFORM_ENCODE_TYPE_SWITCH_OPUS => ".lopus",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave_type_extension_known() {
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_ADX), ".adx");
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_AHX), ".ahx");
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_HCA), ".hca");
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_ADX_ALT), ".adx");
        assert_eq!(
            wave_type_extension(WAVEFORM_ENCODE_TYPE_WII_ADPCM),
            ".wiiadpcm"
        );
        assert_eq!(
            wave_type_extension(WAVEFORM_ENCODE_TYPE_DS_ADPCM),
            ".dsadpcm"
        );
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_HCA_MX), ".hca");
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_VAG), ".vag");
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_ATRAC3), ".at3");
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_BCWAV), ".bcwav");
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_HEVAG), ".vag");
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_ATRAC9), ".at9");
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_XMA), ".xma");
        assert_eq!(
            wave_type_extension(WAVEFORM_ENCODE_TYPE_NINTENDO_DSP),
            ".dsp"
        );
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_PS4_ATRAC9), ".at9");
        assert_eq!(wave_type_extension(WAVEFORM_ENCODE_TYPE_M4A), ".m4a");
        assert_eq!(
            wave_type_extension(WAVEFORM_ENCODE_TYPE_SWITCH_OPUS),
            ".lopus"
        );
    }

    #[test]
    fn test_wave_type_extension_unknown() {
        assert_eq!(wave_type_extension(-1), "");
        assert_eq!(wave_type_extension(99), "");
        assert_eq!(wave_type_extension(14), "");
    }

    #[test]
    fn test_column_constants() {
        // Verify flag masks are non-overlapping
        assert_eq!(COLUMN_FLAG_MASK & COLUMN_TYPE_MASK, 0);
        assert_eq!(COLUMN_FLAG_MASK | COLUMN_TYPE_MASK, 0xFF);
    }
}
