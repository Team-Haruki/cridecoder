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
    decode_acb_to_wav, decode_acb_to_wav_from_file, decode_acb_to_wav_to_memory, DecodeAcbError,
    DecodedAcbTrack,
};
pub use extractor::{
    extract_acb, extract_acb_from_file, extract_acb_to_memory, extract_acb_tracks,
    extract_acb_tracks_from_file, ExtractedAcbTrack, ExtractedTrackFile,
};
pub use track::{Track, TrackList};
pub use utf::{UtfHeader, UtfTable, Value};
