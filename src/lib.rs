//! CRI Codec Library
//!
//! This library provides parsers and decoders for CRI Middleware formats:
//! - ACB/AWB: Audio container formats
//! - HCA: High Compression Audio codec
//! - USM: Video container format

pub mod reader;
pub mod acb;
pub mod hca;
pub mod usm;

#[cfg(feature = "python")]
mod python;

pub use acb::{extract_acb, extract_acb_from_file};
pub use hca::{HcaDecoder, HcaInfo, HcaDecoderError};
pub use usm::{extract_usm, extract_usm_file, Metadata, UsmError};

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
#[pymodule(gil_used = false)]
fn cridecoder(m: &Bound<'_, PyModule>) -> PyResult<()> {
    python::register(m)
}
