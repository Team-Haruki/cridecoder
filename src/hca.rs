//! HCA (High Compression Audio) decoder module

mod ath;
mod bitreader;
mod cipher;
mod decoder;
mod hca_file;
mod imdct;
mod tables;

pub use decoder::{ClHca, HcaError};
pub use hca_file::{HcaDecoder, HcaDecoderError, HcaInfo, KeyTest};
