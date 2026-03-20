//! USM video container module
//!
//! This module provides functionality for extracting video and audio
//! from CRI USM video containers.

mod extractor;
mod metadata;

pub use extractor::{extract_usm, extract_usm_file, UsmError};
pub use metadata::{
    export_metadata_file, read_metadata, read_metadata_file, Metadata, MetadataSection,
};
