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
pub use acb::{extract_acb, extract_acb_from_file};
pub use acb::{AcbBuilder, AfsArchiveBuilder, TrackInput, UtfTableBuilder};

// HCA exports
pub use hca::{HcaDecoder, HcaDecoderError, HcaInfo};
pub use hca::{encode_wav_to_hca, HcaEncoder, HcaEncoderConfig, HcaEncoderError};

// USM exports
pub use usm::{extract_usm, extract_usm_file, Metadata, UsmError};
pub use usm::{UsmBuilder, UsmBuilderError};

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
#[pymodule]
fn cridecoder(m: &Bound<'_, PyModule>) -> PyResult<()> {
    python::register(m)
}
