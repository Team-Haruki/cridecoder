//! HCA (High Compression Audio) codec module
//!
//! This module provides both decoding and encoding for CRI HCA audio format.

mod ath;
mod bitreader;
mod cipher;
mod decoder;
mod encoder;
mod hca_file;
mod imdct;
mod tables;

pub use decoder::{ClHca, HcaError};
pub use encoder::{encode_wav_to_hca, HcaEncoder, HcaEncoderConfig, HcaEncoderError};
pub use hca_file::{HcaDecoder, HcaDecoderError, HcaInfo, KeyTest};
