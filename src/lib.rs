//! CRI Codec Library
//!
//! This library provides parsers, decoders, and encoders for CRI Middleware formats:
//! - ACB/AWB: Audio container formats (extract and build)
//! - HCA: High Compression Audio codec (decode and encode)
//! - USM: Video container format (extract and build)

pub mod acb;
pub mod hca;
pub mod reader;
pub mod usm;

#[cfg(feature = "python")]
mod python;

// ACB/AWB exports
pub use acb::{
    extract_acb, extract_acb_from_file, extract_acb_to_memory, extract_acb_tracks,
    extract_acb_tracks_from_file, ExtractedAcbTrack, ExtractedTrackFile,
};
pub use acb::{AcbBuilder, AfsArchiveBuilder, BuilderError, TrackInput, UtfTableBuilder};

// HCA exports
pub use hca::{encode_wav_to_hca, HcaEncoder, HcaEncoderConfig, HcaEncoderError};
pub use hca::{HcaDecoder, HcaDecoderError, HcaInfo};

// USM exports
pub use usm::{
    extract_usm, extract_usm_file, extract_usm_to_memory, ExtractedUsmStream, Metadata, UsmError,
};
pub use usm::{UsmBuilder, UsmBuilderError};

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
#[pymodule]
fn cridecoder(m: &Bound<'_, PyModule>) -> PyResult<()> {
    python::register(m)
}
