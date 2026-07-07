//! ACB (CRI Audio Container) module

mod afs;
mod builder;
mod consts;
mod decode;
mod extractor;
mod track;
mod utf;

pub use afs::{AfsArchive, AfsFileEntry};
pub use builder::{
    AcbBuilder, AfsArchiveBuilder, BuilderError, ColumnDef, TrackInput, UtfTableBuilder,
};
pub use consts::*;
pub use decode::{
    decode_acb_to_wav, decode_acb_to_wav_from_file, decode_acb_to_wav_from_file_parallel,
    decode_acb_to_wav_parallel, decode_acb_to_wav_to_memory,
    decode_acb_to_wav_to_memory_parallel, DecodeAcbError,
    DecodedAcbTrack,
};
pub use extractor::{
    extract_acb, extract_acb_from_file, extract_acb_to_memory, extract_acb_tracks,
    extract_acb_tracks_from_file, extract_acb_unique_to_memory, AcbCueRef, ExtractedAcbTrack,
    ExtractedTrackFile, UniqueWaveform,
};
pub use track::{Track, TrackList};
pub use utf::{UtfHeader, UtfTable, Value};
