"""Test decoder for 0x3409 compress data (from 0x3409_compression_analysis.md).

Revised approach based on observed segment headers:
- The compress data contains two segments, each preceded by a duplicated
  small header of four u16 values. The last u16 encodes per-segment bit
  parameters; its high byte appears to be the Z bit width (0x1208 → 0x12=18,
  0x1102 → 0x11=17).
- X and Y are effectively zero for the provided example, so we decode Z only.
- Each segment's payload is parsed as a bitstream of Z indices with the
  segment-specific bit width; the two decoded Z index lists are concatenated
  to form 40 frames.
- We try both bit orders (LSB-first, MSB-first) for bit extraction and score
  the result against expected Z values at frames 1, ~34, 35, 40.

All code and comments are in English as required.
"""

from __future__ import annotations

from dataclasses import dataclass
import os
import re
import struct
from typing import Dict, List, Tuple


# Global parameters populated from files
FRAME_COUNT: int = 40
FIRST_Z: float = 0.0
MIDDLE_Z: float = 0.0
LAST_Z: float = 0.0
TAU_SPLIT: float = 0.5  # normalized time where middle keyframe lies


COMPRESS_DATA: List[int] = []


# Small per-segment header patterns inferred from the MD (little-endian u16 stream):
PATTERN_A: List[int] = [1, 0, 1, 0, 1, 0, 8, 18]   # u16s: 0x0001,0x0001,0x0001,0x1208
PATTERN_B: List[int] = [1, 0, 1, 0, 1, 0, 2, 17]   # u16s: 0x0001,0x0001,0x0001,0x1102


def find_double_header_indices(data: List[int], pattern: List[int]) -> List[int]:
    """Find start indices where pattern+pattern occurs."""
    hits: List[int] = []
    doubled = pattern + pattern
    plen = len(doubled)
    for i in range(0, len(data) - plen + 1):
        if data[i : i + plen] == doubled:
            hits.append(i)
    return hits


def extract_segments(data: List[int]) -> List[Tuple[List[int], int]]:
    """Extract two segments as (payload_bytes, z_bit_width).

    We search for the two known duplicated small headers and carve out their
    following payload bytes. Z bit width is taken from the high byte of the
    last u16 in the corresponding header (0x1208 -> 0x12=18, 0x1102 -> 0x11=17).
    """
    # Locate headers
    a_hits = find_double_header_indices(data, PATTERN_A)
    b_hits = find_double_header_indices(data, PATTERN_B)
    if not a_hits or not b_hits:
        raise RuntimeError("Failed to locate both segment headers in compress data.")

    a_start = a_hits[0]
    b_start = b_hits[0]
    if a_start > b_start:
        # Ensure A comes first for this particular dataset
        a_start, b_start = b_start, a_start
        # Swap patterns accordingly (but z bits are derived from constants below)

    a_header_len = len(PATTERN_A) * 2  # doubled header length in bytes
    b_header_len = len(PATTERN_B) * 2

    # Segment A payload begins after its doubled header
    seg_a_payload_start = a_start + a_header_len
    # Segment B payload begins after its doubled header
    seg_b_payload_start = b_start + b_header_len

    # Segment A payload ends at the start of B header
    seg_a_payload = data[seg_a_payload_start:b_start]
    # Segment B payload ends at end of data
    seg_b_payload = data[seg_b_payload_start:]

    # Z bit widths from header constants
    z_bits_a = 0x12  # 18
    z_bits_b = 0x11  # 17

    return [(seg_a_payload, z_bits_a), (seg_b_payload, z_bits_b)]


class BitReader:
    """Bit reader supporting LSB-first or MSB-first bit order within bytes."""

    def __init__(self, data: bytes, bit_order: str) -> None:
        if bit_order not in {"lsb", "msb"}:
            raise ValueError("bit_order must be 'lsb' or 'msb'")
        self._data = data
        self._bit_order = bit_order
        self._byte_index: int = 0
        self._bit_index_in_byte: int = 0 if bit_order == "lsb" else 7

    def _ensure_byte(self) -> None:
        if self._byte_index >= len(self._data):
            raise EOFError("Ran out of bits in the stream")

    def read_bits(self, n_bits: int) -> int:
        if n_bits <= 0:
            return 0
        value: int = 0
        if self._bit_order == "lsb":
            shift = 0
            for _ in range(n_bits):
                self._ensure_byte()
                b = self._data[self._byte_index]
                bit = (b >> self._bit_index_in_byte) & 1
                value |= (bit << shift)
                shift += 1
                self._bit_index_in_byte += 1
                if self._bit_index_in_byte == 8:
                    self._bit_index_in_byte = 0
                    self._byte_index += 1
        else:  # msb
            for _ in range(n_bits):
                self._ensure_byte()
                b = self._data[self._byte_index]
                bit = (b >> self._bit_index_in_byte) & 1
                value = (value << 1) | bit
                self._bit_index_in_byte -= 1
                if self._bit_index_in_byte == -1:
                    self._bit_index_in_byte = 7
                    self._byte_index += 1
        return value


@dataclass
class DecodeConfig:
    bit_order: str  # 'lsb' or 'msb'


def t_from_index(idx: int, bits: int) -> float:
    """Convert an index with given bit width to interpolation parameter t in [0,1]."""
    return float(idx) / float((1 << bits) - 1)


def interpolate_segmented_z(t: float, tau: float) -> float:
    """Segmented linear interpolation for Z using three keyframes with split tau in (0,1)."""
    if tau <= 0.0:
        tau = 1e-6
    if tau >= 1.0:
        tau = 1.0 - 1e-6
    if t <= tau:
        local_t = t / tau
        return FIRST_Z + (MIDDLE_Z - FIRST_Z) * local_t
    local_t = (t - tau) / (1.0 - tau)
    return MIDDLE_Z + (LAST_Z - MIDDLE_Z) * local_t


def decode_z_series_with_offsets(
    segments: List[Tuple[bytes, int, int]],
    config: DecodeConfig,
) -> List[int]:
    """Decode Z indices across segments with per-segment bit width and bit offset.

    segments: list of (payload_bytes, z_bits, bit_offset_bits)
    """
    z_idx: List[int] = []
    for payload, z_bits, bit_offset in segments:
        reader = BitReader(payload, config.bit_order)
        # Skip initial offset bits if any
        if bit_offset:
            _ = reader.read_bits(bit_offset)
        total_bits = len(payload) * 8 - bit_offset
        if total_bits <= 0:
            continue
        entries = total_bits // z_bits
        for _ in range(entries):
            z_idx.append(reader.read_bits(z_bits))
    return z_idx


# --- Alternative decoding path: fixed 21-bit (7+7+7) across full stream ---

def split_21_bits(v: int, bit_order: str) -> Tuple[int, int, int]:
    """Split a 21-bit group into three 7-bit components in read order."""
    if bit_order == "lsb":
        c0 = (v >> 0) & 0x7F
        c1 = (v >> 7) & 0x7F
        c2 = (v >> 14) & 0x7F
    else:  # msb
        c0 = (v >> 14) & 0x7F
        c1 = (v >> 7) & 0x7F
        c2 = (v >> 0) & 0x7F
    return c0, c1, c2


def decode_fixed21_fullstream(payload: bytes, bit_order: str, axis_perm: Tuple[str, str, str]) -> Tuple[List[int], List[int], List[int]]:
    """Decode 40 entries, each 21 bits → (7+7+7), assign to axes via axis_perm."""
    reader = BitReader(payload, bit_order)
    x_idx: List[int] = []
    y_idx: List[int] = []
    z_idx: List[int] = []
    for _ in range(FRAME_COUNT):
        v21 = reader.read_bits(21)
        c0, c1, c2 = split_21_bits(v21, bit_order)
        comps = [c0, c1, c2]
        mapping: Dict[str, int] = {axis_perm[i]: comps[i] for i in range(3)}
        x_idx.append(mapping.get("x", 0))
        y_idx.append(mapping.get("y", 0))
        z_idx.append(mapping.get("z", 0))
    return x_idx, y_idx, z_idx


def evaluate_z_series(z_values: List[float]) -> Tuple[float, Dict[str, float]]:
    """Compute a score against the full Front.anim Z series.

    Lower score is better. Return (score, diagnostics).
    """
    expected = load_front_anim_series("Front.anim")
    if len(expected) != FRAME_COUNT:
        raise RuntimeError("Front.anim series length mismatch")

    # MAE over all frames + emphasize endpoints and middle value proximity
    mae = sum(abs(a - b) for a, b in zip(z_values, expected)) / FRAME_COUNT

    err_first = abs(z_values[0] - expected[0])
    err_last = abs(z_values[-1] - expected[-1])

    # Find frame closest to the middle keyframe value
    diffs = [abs(z - MIDDLE_Z) for z in z_values]
    idx_min = int(min(range(len(diffs)), key=lambda i: diffs[i]))
    err_middle_val = diffs[idx_min]

    score = mae + 2.0 * (err_first + err_last) + 0.5 * err_middle_val

    diag = {
        "mae": mae,
        "err_first": err_first,
        "err_last": err_last,
        "closest_to_middle_frame": float(idx_min + 1),
        "closest_to_middle_error": err_middle_val,
    }
    return score, diag


def load_md_translate_data(md_path: str) -> Tuple[List[int], float, float, float, int]:
    """Parse MD to extract translate data bytes and derive keyframes and frame count.

    Returns (all_bytes, first_z, middle_z, last_z, frame_count).
    """
    with open(md_path, "r", encoding="utf-8") as f:
        text = f.read()

    # Extract the Translate data list (numbers separated by commas)
    m = re.search(r"Translate data:\s*\[(.*?)\]", text, re.DOTALL)
    if not m:
        raise RuntimeError("Failed to find 'Translate data' list in MD")
    nums_str = m.group(1)
    nums = [int(x.strip()) for x in re.split(r"[,\s]+", nums_str) if x.strip()]
    data_bytes = bytes(nums)

    # Parse header
    if len(data_bytes) < 56:
        raise RuntimeError("Translate data too short")
    magic = int.from_bytes(data_bytes[0:4], "little")
    if magic != 0x00003409:
        raise RuntimeError(f"Unexpected magic: 0x{magic:08X}")
    frame_count = int.from_bytes(data_bytes[4:8], "little")
    # unknown1 f32 at 8..12, unknown2 f32 at 12..16
    # flags u16 at 16..18, bits_per_entry u16 at 18..20

    # Read 3 keyframes (3 * Vector3 f32 = 36 bytes)
    keyframe_bytes = data_bytes[20:56]
    if len(keyframe_bytes) != 36:
        raise RuntimeError("Keyframe bytes length mismatch")
    floats = list(struct.unpack("<fffffffff", keyframe_bytes))
    # Order: first(x,y,z), middle(x,y,z), last(x,y,z)
    first_z = floats[2]
    middle_z = floats[5]
    last_z = floats[8]

    # Remaining bytes are compress payload as emitted by the source
    remaining = data_bytes[56:]
    return list(remaining), first_z, middle_z, last_z, frame_count


def load_front_anim_series(anim_path: str) -> List[float]:
    """Parse Front.anim to extract translateZ values for GBL_RT keys (40 frames)."""
    with open(anim_path, "r", encoding="utf-8") as f:
        lines = f.readlines()

    # Locate the block start
    start_idx = None
    for i, line in enumerate(lines):
        if line.strip().startswith("anim translate.translateZ translateZ GBL_RT"):
            start_idx = i
            break
    if start_idx is None:
        raise RuntimeError("Failed to find translateZ GBL_RT block in Front.anim")

    # Find keys { ... } block
    keys_start = None
    keys_end = None
    for i in range(start_idx, len(lines)):
        if lines[i].strip() == "keys {":
            keys_start = i + 1
            break
    if keys_start is None:
        raise RuntimeError("Failed to find keys{ in Front.anim")
    for i in range(keys_start, len(lines)):
        if lines[i].strip() == "}":
            keys_end = i
            break
    if keys_end is None:
        raise RuntimeError("Failed to find end of keys block in Front.anim")

    # Each line in keys block starts with: <time> <value> ...
    pairs: List[Tuple[int, float]] = []
    for i in range(keys_start, keys_end):
        line = lines[i].strip()
        if not line or line.startswith("#"):
            continue
        tokens = line.split()
        if len(tokens) < 2:
            continue
        try:
            t = float(tokens[0])
            v = float(tokens[1])
        except ValueError:
            continue
        # Frame numbers appear near integers; convert to 1-based frame index
        frame_idx = int(round(t))
        pairs.append((frame_idx, v))

    # Build a 1..FRAME_COUNT list
    series: List[float] = [0.0] * FRAME_COUNT
    filled = 0
    seen = set()
    for idx, val in pairs:
        if 1 <= idx <= FRAME_COUNT and idx not in seen:
            series[idx - 1] = val
            seen.add(idx)
            filled += 1
    if filled < FRAME_COUNT:
        raise RuntimeError(f"Only found {filled} frame values in Front.anim, expected {FRAME_COUNT}")
    return series


def main() -> None:
    # Load data from files (no hardcoded arrays)
    md_path = os.path.join(os.getcwd(), "0x3409_compression_analysis.md")
    comp_bytes, first_z, middle_z, last_z, frame_count = load_md_translate_data(md_path)
    global FIRST_Z, MIDDLE_Z, LAST_Z, FRAME_COUNT, COMPRESS_DATA
    FIRST_Z, MIDDLE_Z, LAST_Z = first_z, middle_z, last_z
    FRAME_COUNT = frame_count
    COMPRESS_DATA = comp_bytes

    # Try two strategies: segmented Z-only vs fixed 21-bit 7+7+7 fullstream
    best_score = float("inf")
    best_label = ""
    best_series: List[float] | None = None
    best_diag: Dict[str, float] | None = None
    best_config: DecodeConfig | None = None

    # Strategy 1: segmented Z-only using header-derived bit widths
    try:
        segs_meta = extract_segments(COMPRESS_DATA)
        segments: List[Tuple[bytes, int]] = []
        total_entries = 0
        for payload_list, z_bits in segs_meta:
            payload_bytes = bytes(payload_list)
            segments.append((payload_bytes, z_bits))
            entries = (len(payload_bytes) * 8) // z_bits
            print(f"Segment payload bytes={len(payload_bytes)}, z_bits={z_bits}, entries={entries}")
            total_entries += entries

        if total_entries in (FRAME_COUNT, FRAME_COUNT - 1):
            for bit_order in ("lsb", "msb"):
                cfg = DecodeConfig(bit_order=bit_order)
                try:
                    # Try a small grid of initial bit offsets (to account for padding/alignment)
                    best_local_idx: List[int] | None = None
                    best_local_err = float("inf")
                    for off_a in range(0, 8):
                        for off_b in range(0, 8):
                            segs_with_off = [
                                (segments[0][0], segments[0][1], off_a),
                                (segments[1][0], segments[1][1], off_b),
                            ]
                            try:
                                z_idx_candidate = decode_z_series_with_offsets(segs_with_off, cfg)
                            except EOFError:
                                continue
                            # quick endpoint check against Front.anim
                            front_series = load_front_anim_series("Front.anim")
                            # build rank-based tau=0.5 sequence quickly for a rough error
                            n1 = (len(segs_meta[0][0]) * 8 - off_a) // segs_meta[0][1]
                            n2 = ((len(segs_meta[1][0]) * 8 - off_b) // segs_meta[1][1])
                            base_rank: List[float] = []
                            if n1 > 1:
                                base_rank.extend([k / (n1 - 1) for k in range(n1)])
                            elif n1 == 1:
                                base_rank.append(0.5)
                            if n2 > 1:
                                base_rank.extend([k / (n2 - 1) for k in range(n2)])
                            elif n2 == 1:
                                base_rank.append(0.5)
                            if len(base_rank) == FRAME_COUNT - 1:
                                base_rank.append(1.0)
                            tau = 0.5
                            seq = [r * tau if i < n1 else tau + (r) * (1 - tau) for i, r in enumerate(base_rank)]
                            z_values = [interpolate_segmented_z(t, tau) for t in seq]
                            err = abs(z_values[0] - front_series[0]) + abs(z_values[-1] - front_series[-1])
                            if err < best_local_err:
                                best_local_err = err
                                best_local_idx = z_idx_candidate
                    if best_local_idx is None:
                        continue
                    z_idx = best_local_idx
                except EOFError:
                    continue
                # Build candidate t sequences with different mappings
                # 1) direct mapping per entry using bit width
                z_t_direct_base: List[float] = []
                # 2) inverted mapping 1 - direct
                z_t_invert_base: List[float] = []
                # 3) rank-based linear mapping per segment to [0,0.5] and [0.5,1.0]
                z_t_rank_base: List[float] = []
                # 4) per-segment min-max normalization of raw indices
                z_t_minmax_base: List[float] = []

                idx_pos = 0
                total_seen = 0
                for seg_idx, (payload_bytes, z_bits) in enumerate(segments):
                    entries = (len(payload_bytes) * 8) // z_bits
                    idx_slice = z_idx[idx_pos: idx_pos + entries]
                    if entries > 0:
                        vmin = min(idx_slice)
                        vmax = max(idx_slice)
                        span = max(1, vmax - vmin)
                    else:
                        vmin = 0
                        span = 1
                    # direct/invert
                    for _ in range(entries):
                        tdir = t_from_index(z_idx[idx_pos], z_bits)
                        z_t_direct_base.append(tdir)
                        z_t_invert_base.append(1.0 - tdir)
                        # min-max in [0,1]
                        z_t_minmax_base.append((z_idx[idx_pos] - vmin) / span)
                        idx_pos += 1
                    # rank-based for this segment
                    if entries > 1:
                        for k in range(entries):
                            local = k / (entries - 1)
                            z_t_rank_base.append(local)
                    else:
                        z_t_rank_base.append(0.5)
                    total_seen += entries

                # If one index short, append implicit last frame at local=1.0 for last segment
                if total_entries == FRAME_COUNT - 1:
                    z_t_direct_base.append(1.0)
                    z_t_invert_base.append(0.0)
                    z_t_rank_base.append(1.0)
                    z_t_minmax_base.append(1.0)

                # Try a grid of tau values to align segment boundary in time
                for tau in (x / 200.0 for x in range(60, 191, 5)):  # 0.30..0.95 step 0.025
                    # Map local segment positions to global t using tau
                    # First segment maps [0..1] -> [0..tau], second [0..1] -> [tau..1]
                    # Determine how many belong to first vs second segment
                    n1 = (len(segs_meta[0][0]) * 8) // segs_meta[0][1]
                    n2 = len(z_t_direct_base) - n1
                    def to_global(seq: List[float]) -> List[float]:
                        out: List[float] = []
                        for i, local in enumerate(seq):
                            if i < n1:
                                out.append(local * tau)
                            else:
                                out.append(tau + local * (1.0 - tau))
                        return out

                    # Try gamma shaping for minmax to capture potential non-linear mapping
                    for label, base in (("direct", z_t_direct_base), ("invert", z_t_invert_base), ("rank", z_t_rank_base)):
                        seq = to_global(base)
                        z_values = [interpolate_segmented_z(t, tau) for t in seq]
                        score, diag = evaluate_z_series(z_values)
                        if score < best_score:
                            best_score = score
                            best_label = f"segments({bit_order},{label},tau={tau:.3f})"
                            best_series = z_values
                            best_diag = diag
                            best_config = cfg

                    for gamma in (0.5, 0.75, 1.0, 1.25, 1.5, 2.0):
                        shaped = [pow(max(0.0, min(1.0, v)), gamma) for v in z_t_minmax_base]
                        seq = to_global(shaped)
                        z_values = [interpolate_segmented_z(t, tau) for t in seq]
                        score, diag = evaluate_z_series(z_values)
                        if score < best_score:
                            best_score = score
                            best_label = f"segments({bit_order},minmax^{{{gamma}}},tau={tau:.3f})"
                            best_series = z_values
                            best_diag = diag
                            best_config = cfg

                    for gamma in (1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0):
                        rank_shaped = [pow(max(0.0, min(1.0, v)), gamma) for v in z_t_rank_base]
                        seq = to_global(rank_shaped)
                        z_values = [interpolate_segmented_z(t, tau) for t in seq]
                        score, diag = evaluate_z_series(z_values)
                        if score < best_score:
                            best_score = score
                            best_label = f"segments({bit_order},rank^{{{gamma}}},tau={tau:.3f})"
                            best_series = z_values
                            best_diag = diag
                            best_config = cfg
    except Exception as e:
        print(f"Segmented strategy failed: {e}")

    # Strategy 2: fixed 21-bit fullstream after removing headers
    try:
        data = COMPRESS_DATA
        a_hits = find_double_header_indices(data, PATTERN_A)
        b_hits = find_double_header_indices(data, PATTERN_B)
        to_strip: List[Tuple[int, int]] = []
        for hit in a_hits:
            to_strip.append((hit, hit + len(PATTERN_A) * 2))
        for hit in b_hits:
            to_strip.append((hit, hit + len(PATTERN_B) * 2))
        to_strip.sort()
        keep: List[int] = []
        i = 0
        for s, e in to_strip:
            while i < s:
                keep.append(data[i])
                i += 1
            i = e
        while i < len(data):
            keep.append(data[i])
            i += 1
        payload = bytes(keep)
        if len(payload) * 8 >= FRAME_COUNT * 21:
            for bit_order in ("lsb", "msb"):
                for axis_perm in (("x", "y", "z"), ("x", "z", "y"), ("y", "x", "z"),
                                  ("y", "z", "x"), ("z", "x", "y"), ("z", "y", "x")):
                    try:
                        _, _, z_idx = decode_fixed21_fullstream(payload, bit_order, axis_perm)
                    except EOFError:
                        continue
                    z_t = [t_from_index(i, 7) for i in z_idx]
                    z_values = [interpolate_segmented_z(t, TAU_SPLIT) for t in z_t]
                    score, diag = evaluate_z_series(z_values)
                    if score < best_score:
                        best_score = score
                        best_label = f"fixed21({bit_order},{axis_perm})"
                        best_series = z_values
                        best_diag = diag
                        best_config = DecodeConfig(bit_order=bit_order)
    except Exception as e:
        print(f"Fixed21 strategy failed: {e}")

    if best_config is None or best_series is None or best_diag is None:
        raise RuntimeError("Failed to find a suitable decoding configuration.")

    # Print summary and checkpoints
    print("Best configuration:")
    print(f"  label: {best_label}")
    print(f"  bit_order: {best_config.bit_order}")
    print(f"  score: {best_score:.6f}")
    print("Diagnostics:")
    for k, v in best_diag.items():
        print(f"  {k}: {v}")

    # Report key frames (1-based indices)
    def fmt(v: float) -> str:
        return f"{v:.6f}"

    z = best_series
    print("\nCheckpoints (Z):")
    front_series = load_front_anim_series("Front.anim")
    for f in (1, 2, 3, 34, 35, 40):
        expected = front_series[f - 1]
        actual = z[f - 1]
        diff = abs(actual - expected)
        print(f"  frame {f:2d}: {fmt(actual)}  expected {fmt(expected)}  diff {diff:.6f}")

    # Simple PASS/FAIL based on endpoints and middle closeness
    pass_endpoints = abs(z[0] - front_series[0]) < 1e-5 and abs(z[-1] - front_series[-1]) < 1e-5
    pass_middle_near = abs(min(z, key=lambda val: abs(val - MIDDLE_Z)) - MIDDLE_Z) < 0.01
    print("\nResult:")
    print(f"  Endpoints exact match: {'PASS' if pass_endpoints else 'FAIL'}")
    print(f"  Middle proximity:      {'PASS' if pass_middle_near else 'FAIL'}")


if __name__ == "__main__":
    main()


