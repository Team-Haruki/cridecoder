from __future__ import annotations

from typing import Optional, TypedDict


class ExtractedTrack(TypedDict):
    name: str
    extension: str
    filename: str
    data: bytes


class HcaInfo(TypedDict):
    sample_rate: int
    channels: int
    block_count: int
    block_size: int
    encoder_delay: int
    samples_per_block: int


class EncodeInfo(TypedDict):
    size: int
    bitrate: int


def extract_acb(acb_path: str, output_dir: str) -> Optional[list[str]]: ...


def extract_acb_bytes(acb_data: bytes, acb_path: Optional[str] = None) -> list[ExtractedTrack]: ...


def build_acb(tracks: list[tuple[str, int, bytes]], output_path: str) -> None: ...


def build_acb_bytes(tracks: list[tuple[str, int, bytes]]) -> bytes: ...


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
) -> bytes: ...


def decode_hca(hca_path: str, wav_path: str) -> HcaInfo: ...


def decode_hca_bytes(hca_data: bytes) -> bytes: ...


def encode_hca(
    wav_path: str,
    hca_path: str,
    bitrate: int = 256000,
    encryption_key: Optional[int] = None,
) -> EncodeInfo: ...


def encode_hca_bytes(
    wav_data: bytes,
    sample_rate: Optional[int] = None,
    channels: Optional[int] = None,
    bitrate: int = 256000,
    encryption_key: Optional[int] = None,
) -> bytes: ...


def extract_usm(
    usm_path: str,
    output_dir: str,
    key: Optional[int] = None,
    export_audio: bool = False,
) -> list[str]: ...


def extract_usm_bytes(
    usm_data: bytes,
    fallback_name: str = "input.usm",
    key: Optional[int] = None,
    export_audio: bool = False,
) -> list[ExtractedTrack]: ...


def build_usm(
    name: str,
    video_data: bytes,
    output_path: str,
    encryption_key: Optional[int] = None,
) -> None: ...


def build_usm_bytes(
    name: str,
    video_data: bytes,
    encryption_key: Optional[int] = None,
) -> bytes: ...


def read_usm_metadata(usm_path: str) -> str: ...
