"""
Python integration tests for cridecoder.

Tests encoding and decoding functionality:
- HCA encoding/decoding round-trip
- ACB/USM container building (structure verification)
- Real file extraction and HCA encoding
"""

import os
import tempfile
import pytest

# Skip if cridecoder not available
pytest.importorskip("cridecoder")

import cridecoder


class TestHcaRoundtrip:
    """Test HCA encoding and decoding round-trip."""

    def test_encode_decode_roundtrip(self):
        """Test encoding WAV to HCA and decoding back."""
        # Create a simple sine wave WAV
        wav_data = create_test_wav(sample_rate=44100, channels=2, duration_sec=0.5)
        
        # Encode to HCA
        hca_data = cridecoder.encode_hca_bytes(wav_data, bitrate=256000)
        
        # Verify HCA magic
        assert hca_data[:4] == b"HCA\x00", "Should have HCA magic"
        
        # Decode back to WAV
        decoded_wav = cridecoder.decode_hca_bytes(hca_data)
        
        # Verify WAV magic
        assert decoded_wav[:4] == b"RIFF", "Should have RIFF magic"
        assert decoded_wav[8:12] == b"WAVE", "Should have WAVE format"

    def test_encode_with_encryption(self):
        """Test HCA encoding with encryption key."""
        wav_data = create_test_wav(sample_rate=44100, channels=1, duration_sec=0.2)
        
        # Encode with encryption
        hca_data = cridecoder.encode_hca_bytes(
            wav_data, 
            bitrate=128000, 
            encryption_key=0x1234567890ABCDEF
        )
        
        # Verify it's encrypted (header will be masked)
        # Encrypted HCA has masked magic: 0xC8C3C100 instead of 0x48434100
        assert hca_data[0] == 0xC8, "Should have masked HCA magic"

    def test_encode_mono(self):
        """Test encoding mono audio."""
        wav_data = create_test_wav(sample_rate=22050, channels=1, duration_sec=0.3)
        hca_data = cridecoder.encode_hca_bytes(wav_data, bitrate=128000)
        assert hca_data[:4] == b"HCA\x00"
        
        # Verify can decode
        decoded = cridecoder.decode_hca_bytes(hca_data)
        assert decoded[:4] == b"RIFF"

    def test_encode_stereo(self):
        """Test encoding stereo audio."""
        wav_data = create_test_wav(sample_rate=48000, channels=2, duration_sec=0.3)
        hca_data = cridecoder.encode_hca_bytes(wav_data, bitrate=320000)
        assert hca_data[:4] == b"HCA\x00"
        
        decoded = cridecoder.decode_hca_bytes(hca_data)
        assert decoded[:4] == b"RIFF"


class TestContainerBuilding:
    """Test container building (structure verification only)."""

    def test_build_acb_structure(self):
        """Test ACB building produces valid UTF structure."""
        wav_data = create_test_wav(sample_rate=44100, channels=2, duration_sec=0.1)
        hca_data = cridecoder.encode_hca_bytes(wav_data, bitrate=256000)
        
        tracks = [("test_track", 0, hca_data)]
        acb_data = cridecoder.build_acb_bytes(tracks)
        
        # Verify ACB magic (@UTF)
        assert acb_data[:4] == b"@UTF", "Should have UTF magic"
        assert len(acb_data) > 100, "Should have substantial content"

    def test_extract_acb_bytes(self):
        """Test extracting ACB tracks without writing files."""
        wav_data = create_test_wav(sample_rate=44100, channels=1, duration_sec=0.1)
        hca_data = cridecoder.encode_hca_bytes(wav_data, bitrate=128000)
        acb_data = cridecoder.build_acb_bytes([("memory_track", 0, hca_data)])

        tracks = cridecoder.extract_acb_bytes(acb_data)

        assert len(tracks) == 1
        assert tracks[0]["name"] == "memory_track"
        assert tracks[0]["extension"] == "hca"
        assert tracks[0]["filename"] == "memory_track.hca"
        assert tracks[0]["data"][:4] == b"HCA\x00"

    def test_build_acb_multiple_tracks(self):
        """Test ACB building with multiple tracks."""
        tracks = []
        for i in range(3):
            wav_data = create_test_wav(sample_rate=44100, channels=1, duration_sec=0.1)
            hca_data = cridecoder.encode_hca_bytes(wav_data, bitrate=128000)
            tracks.append((f"track_{i}", i, hca_data))
        
        acb_data = cridecoder.build_acb_bytes(tracks)
        assert acb_data[:4] == b"@UTF"

    def test_build_usm_structure(self):
        """Test USM building produces valid CRID structure."""
        # Create minimal M2V header
        video_data = bytes([
            0x00, 0x00, 0x01, 0xB3,  # sequence_header_code
            0x14, 0x00, 0xF0, 0x24,  # picture size
            0xFF, 0xFF, 0xE0, 0x00,  # bit rate
        ])
        
        usm_data = cridecoder.build_usm_bytes("test_video", video_data)
        
        # Verify CRID magic
        assert usm_data[:4] == b"CRID", "Should have CRID magic"
        assert len(usm_data) > 100, "Should have substantial content"


class TestRealFileOperations:
    """Test operations with real fixture files."""

    @pytest.fixture
    def acb_path(self):
        """Path to test ACB file."""
        path = "se_0126_01.acb"
        if not os.path.exists(path):
            pytest.skip("Test fixture se_0126_01.acb not found")
        return path

    @pytest.fixture
    def usm_path(self):
        """Path to test USM file."""
        path = "0703.usm"
        if not os.path.exists(path):
            pytest.skip("Test fixture 0703.usm not found")
        return path

    def test_extract_acb(self, acb_path):
        """Test extracting real ACB file."""
        with tempfile.TemporaryDirectory() as tmpdir:
            extracted = cridecoder.extract_acb(acb_path, tmpdir)
            assert extracted is not None
            assert len(extracted) > 0, "Should extract tracks"
            
            # Verify extracted files exist
            for path in extracted:
                assert os.path.exists(path), f"Extracted file should exist: {path}"

    def test_extract_usm(self, usm_path):
        """Test extracting real USM file."""
        with tempfile.TemporaryDirectory() as tmpdir:
            extracted = cridecoder.extract_usm(usm_path, tmpdir)
            assert len(extracted) > 0, "Should extract files"
            
            # Verify M2V extracted
            m2v_files = [p for p in extracted if p.endswith(".m2v")]
            assert len(m2v_files) > 0, "Should extract M2V video"

    def test_extract_usm_bytes(self, usm_path):
        """Test extracting real USM streams without writing files."""
        with open(usm_path, "rb") as f:
            usm_data = f.read()

        streams = cridecoder.extract_usm_bytes(usm_data, fallback_name=os.path.basename(usm_path))

        assert len(streams) > 0
        m2v_streams = [stream for stream in streams if stream["extension"] == "m2v"]
        assert len(m2v_streams) > 0, "Should extract M2V video"
        assert m2v_streams[0]["data"].startswith(b"\x00\x00\x01")

    def test_hca_decode_and_reencode(self, acb_path):
        """Extract HCA, decode to WAV, and re-encode to HCA."""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Extract ACB
            extracted = cridecoder.extract_acb(acb_path, tmpdir)
            assert extracted is not None and len(extracted) > 0
            
            # Decode first HCA to WAV
            hca_path = extracted[0]
            wav_path = os.path.join(tmpdir, "decoded.wav")
            info = cridecoder.decode_hca(hca_path, wav_path)
            
            assert info["sample_rate"] > 0
            assert info["channels"] > 0
            
            # Read WAV and re-encode
            with open(wav_path, "rb") as f:
                wav_data = f.read()
            
            new_hca_data = cridecoder.encode_hca_bytes(
                wav_data,
                sample_rate=info["sample_rate"],
                channels=info["channels"],
                bitrate=256000
            )
            
            assert new_hca_data[:4] == b"HCA\x00", "Should produce valid HCA"
            
            # Verify the new HCA can be decoded
            decoded_again = cridecoder.decode_hca_bytes(new_hca_data)
            assert decoded_again[:4] == b"RIFF", "Re-encoded HCA should decode"

    def test_usm_extract_and_rebuild_video(self, usm_path):
        """Extract M2V from USM and build new USM with it."""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Extract
            extracted = cridecoder.extract_usm(usm_path, tmpdir)
            
            # Find M2V
            m2v_path = None
            for path in extracted:
                if path.endswith(".m2v"):
                    m2v_path = path
                    break
            
            assert m2v_path is not None, "Should have M2V"
            
            # Read M2V and build new USM
            with open(m2v_path, "rb") as f:
                m2v_data = f.read()
            
            rebuilt_path = os.path.join(tmpdir, "rebuilt.usm")
            cridecoder.build_usm("rebuilt", m2v_data, rebuilt_path)
            
            # Verify file created and has CRID header
            assert os.path.exists(rebuilt_path)
            with open(rebuilt_path, "rb") as f:
                header = f.read(4)
            assert header == b"CRID", "Rebuilt USM should have CRID magic"

    def test_full_acb_pipeline(self, acb_path):
        """Full pipeline: extract ACB -> decode HCA -> encode HCA -> build ACB."""
        with tempfile.TemporaryDirectory() as tmpdir:
            # 1. Extract original ACB
            extract_dir = os.path.join(tmpdir, "extract")
            os.makedirs(extract_dir)
            extracted = cridecoder.extract_acb(acb_path, extract_dir)
            assert extracted and len(extracted) > 0
            
            # 2. Process each track: decode and re-encode
            new_tracks = []
            for i, hca_path in enumerate(extracted[:2]):  # First 2 tracks
                # Decode
                wav_path = os.path.join(tmpdir, f"track{i}.wav")
                info = cridecoder.decode_hca(hca_path, wav_path)
                
                # Re-encode
                with open(wav_path, "rb") as f:
                    wav_data = f.read()
                
                new_hca = cridecoder.encode_hca_bytes(
                    wav_data,
                    sample_rate=info["sample_rate"],
                    channels=info["channels"],
                    bitrate=256000
                )
                
                new_tracks.append((f"track_{i}", i, new_hca))
            
            # 3. Build new ACB
            rebuilt_path = os.path.join(tmpdir, "rebuilt.acb")
            cridecoder.build_acb(new_tracks, rebuilt_path)
            
            # Verify ACB created
            assert os.path.exists(rebuilt_path)
            with open(rebuilt_path, "rb") as f:
                header = f.read(4)
            assert header == b"@UTF", "Rebuilt ACB should have UTF magic"
            
            # Verify substantial size
            size = os.path.getsize(rebuilt_path)
            assert size > 1000, f"ACB should have substantial size, got {size}"


def create_test_wav(sample_rate: int, channels: int, duration_sec: float) -> bytes:
    """Create a simple test WAV file with a sine wave."""
    import struct
    import math
    
    num_samples = int(sample_rate * duration_sec)
    bits_per_sample = 16
    byte_rate = sample_rate * channels * bits_per_sample // 8
    block_align = channels * bits_per_sample // 8
    data_size = num_samples * channels * bits_per_sample // 8
    
    # Generate sine wave samples
    frequency = 440.0  # A4 note
    samples = []
    for i in range(num_samples):
        t = i / sample_rate
        value = int(32767 * 0.5 * math.sin(2 * math.pi * frequency * t))
        for _ in range(channels):
            samples.append(value)
    
    # Build WAV header
    wav = bytearray()
    wav.extend(b"RIFF")
    wav.extend(struct.pack("<I", 36 + data_size))
    wav.extend(b"WAVE")
    wav.extend(b"fmt ")
    wav.extend(struct.pack("<I", 16))  # fmt chunk size
    wav.extend(struct.pack("<H", 1))   # PCM format
    wav.extend(struct.pack("<H", channels))
    wav.extend(struct.pack("<I", sample_rate))
    wav.extend(struct.pack("<I", byte_rate))
    wav.extend(struct.pack("<H", block_align))
    wav.extend(struct.pack("<H", bits_per_sample))
    wav.extend(b"data")
    wav.extend(struct.pack("<I", data_size))
    
    # Add sample data
    for sample in samples:
        wav.extend(struct.pack("<h", sample))
    
    return bytes(wav)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
