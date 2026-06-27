"""Type stubs for cridecoder — CRI codec library (ACB/AWB, HCA audio, USM video).

These signatures mirror the PyO3 bindings in `src/python.rs`. Arguments typed
`Optional[...]` without a default are required but accept ``None``; arguments
with a default are optional.
"""

from typing import Optional

__all__ = [
    "extract_acb",
    "extract_acb_tracks",
    "decode_acb_to_wav",
    "build_acb",
    "build_acb_bytes",
    "build_music_acb_bytes",
    "decode_hca",
    "decode_hca_bytes",
    "encode_hca",
    "encode_hca_bytes",
    "extract_usm",
    "build_usm",
    "build_usm_bytes",
    "read_usm_metadata",
]

# --- ACB ---------------------------------------------------------------------

def extract_acb(acb_path: str, output_dir: str) -> Optional[list[str]]:
    """Extract audio tracks from an ACB file to ``output_dir``.

    Returns the list of written file paths, or ``None`` if the file is invalid.
    """
    ...

def extract_acb_tracks(
    acb_path: str, output_dir: str
) -> Optional[list[dict[str, object]]]:
    """Extract audio tracks from an ACB, returning per-track metadata.

    Like :func:`extract_acb`, but each entry is a dict with ``path`` (written
    file), ``name`` (cue name), ``cue_id`` and ``subkey`` — the AFS2 subkey of
    the originating AWB, needed (with the global keycode) to decode type-56
    encrypted HCA. Returns ``None`` if the file is invalid.
    """
    ...

def decode_acb_to_wav(
    acb_path: str, output_dir: str, key: Optional[int] = ...
) -> list[str]:
    """Extract an ACB and decode its HCA tracks straight to WAV files.

    The per-AWB AFS2 subkey is applied automatically, so encrypted (type-56)
    ACBs only need the global ``key`` (omit/``None`` for unencrypted ACBs).
    Non-HCA tracks are written verbatim with their original extension. Returns
    the list of written file paths.
    """
    ...

def build_acb(tracks: list[tuple[str, int, bytes]], output_path: str) -> None:
    """Build an ACB file from ``(name, cue_id, hca_data)`` tuples, writing to disk."""
    ...

def build_acb_bytes(tracks: list[tuple[str, int, bytes]]) -> bytes:
    """Build an ACB from ``(name, cue_id, hca_data)`` tuples and return the bytes."""
    ...

def build_music_acb_bytes(
    name: str,
    hca_data: bytes,
    cue_id: int,
    virtual_cue_suffix: Optional[str],
    memory_awb_id: int,
    reference_num_samples: int,
    reference_length_ms: int,
    acb_version: int,
    acf_md5_hash: bytes,
    acb_guid: bytes,
    version_string: str,
    acb_volume: float,
    category_extension: int,
    cue_priority_type: int,
    acf_category_name: str,
    acf_category_id: int,
    acf_bus_names: list[str],
) -> bytes:
    """Build a single-track music ACB from one HCA track and return the bytes.

    ``acf_md5_hash`` and ``acb_guid`` are 16-byte values; ``virtual_cue_suffix``
    may be ``None`` for no paired virtual cue.
    """
    ...

# --- HCA ---------------------------------------------------------------------

def decode_hca(
    hca_path: str,
    wav_path: str,
    key: Optional[int] = ...,
    subkey: Optional[int] = ...,
) -> dict[str, int]:
    """Decode an HCA file to a WAV file.

    ``key``/``subkey`` apply the type-56 decryption keycode for encrypted HCA
    (no-op for unencrypted files). Returns a dict with ``sample_rate``,
    ``channels``, ``block_count``, ``block_size``, ``encoder_delay`` and
    ``samples_per_block``.
    """
    ...

def decode_hca_bytes(
    hca_data: bytes,
    key: Optional[int] = ...,
    subkey: Optional[int] = ...,
) -> bytes:
    """Decode HCA bytes to WAV bytes in memory (``key``/``subkey`` as in :func:`decode_hca`)."""
    ...

def encode_hca_bytes(
    wav_data: bytes,
    sample_rate: Optional[int] = ...,
    channels: Optional[int] = ...,
    bitrate: int = ...,
    encryption_key: Optional[int] = ...,
) -> bytes:
    """Encode WAV bytes to HCA bytes.

    ``sample_rate``/``channels`` default to the WAV header when ``None``;
    ``bitrate`` defaults to 256000 bps. Supports 16/24/32-bit PCM input.
    """
    ...

def encode_hca(
    wav_path: str,
    hca_path: str,
    bitrate: int = ...,
    encryption_key: Optional[int] = ...,
) -> dict[str, int]:
    """Encode a WAV file to an HCA file. Returns a dict with ``size`` and ``bitrate``."""
    ...

# --- USM ---------------------------------------------------------------------

def extract_usm(
    usm_path: str,
    output_dir: str,
    key: Optional[int] = ...,
    export_audio: bool = ...,
) -> list[str]:
    """Extract video (and optionally audio) streams from a USM file.

    ``key`` decrypts encrypted USMs; ``export_audio`` (default ``False``) also
    writes audio streams. Returns the list of written file paths.
    """
    ...

def build_usm(
    name: str,
    video_data: bytes,
    output_path: str,
    encryption_key: Optional[int] = ...,
) -> None:
    """Build a USM file from M2V video data, writing to disk."""
    ...

def build_usm_bytes(
    name: str,
    video_data: bytes,
    encryption_key: Optional[int] = ...,
) -> bytes:
    """Build a USM from M2V video data and return the bytes."""
    ...

def read_usm_metadata(usm_path: str) -> str:
    """Read USM metadata and return it as a pretty-printed JSON string."""
    ...
