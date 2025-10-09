### Purpose
This document summarizes the current understanding of the decompiled nuanmb reader function and its implications for decoding Smash Ultimate's compressed animation tracks, focusing on 0x3409 Vector3 compression. The goal is to improve research decoders so their output approaches the game's ground truth (e.g., Front.anim).

### High-level semantics of the decompiled function
- The function processes one frame at a time and computes one or more scalar outputs per call by evaluating a weighted sum over precomputed coefficient tables. It is not a simple “map index → t → linear interpolation” path.
- A small per-frame header packs several nibbles that determine how many sub-groups of terms will be read and how they are combined.
- The code constructs a fixed-size weight vector of length 8 from two 16-bit values and one 8-bit value (normalized and scaled), then uses that weight vector to modulate multiple groups of coefficient vectors read from the bitstream and from a precomputed bank. All contributions are accumulated into a scalar (v29), which is written to the output.

### Inputs and suspected parameter roles
- a1: Output pointer (receives one or more scalar results). The code writes a sequence of scalars, reusing temporary storage.
- a2: A global scale from the track header (used when constructing the weight vector).
- a3: Pointer/cursor into the compressed per-frame data (advanced as groups are consumed).
- a4: Frame index; used to select the correct coefficient slice for this frame.
- a5: The number of scalar outputs to produce (observed via a5-controlled output loop).
- a6: A bound for a4 in an early guard; if a4 == a6, it writes zero.

### Nibble-driven per-frame header and weight vector construction
The function extracts several nibbles from 4 header bytes (byte_5..byte_8) and uses them to determine counts of term groups and the distribution of the 8-weight slots:

```86:95:nuanmb_parse (1).txt
byte_7 = *((unsigned __int8 *)v10 + 6);
byte_7_low_bits = byte_7 & 0xF;
byte_7_high_bits = byte_7 >> 4;
byte_6_high_bits = *((unsigned __int8 *)v10 + 5) >> 4;
byte_6_low_bits = *((_BYTE *)v10 + 5) & 0xF;
byte_8_high_bits = *((unsigned __int8 *)v10 + 7) >> 4;
byte_8_low_bits = *((_BYTE *)v10 + 7) & 0xF;
v18 = byte_6_high_bits + byte_6_low_bits + byte_7_low_bits + byte_7_high_bits;
v19 = 8 - byte_8_low_bits - byte_8_high_bits;
```

- v18 is the total number of term-groups used for this frame; it later selects the coefficient bank and row count.
- v19 is the number of remaining slots after dividing the 8 weights between two segments; the two nibbles of byte_8 determine how many weights go to each segment.

The code then constructs the 8-length weight vector v71 from two 16-bit inputs and an 8-bit input, normalized and scaled by a2. The first segment uses a constant v24; the second uses i; any remaining slots use i multiplied by byte_5 / 255.

```99:121:nuanmb_parse (1).txt
v24 = (float)(_mm_cvtepi32_ps(byte_34).m128_f32[0] * 0.000015259022) * a2; // u16/65535 * a2
for ( i = (float)((float)byte_2 * 0.000015259022) * v24; (unsigned int)v23 < byte_8_high_bits; v23 = (unsigned int)(v23 + 1) )
  *((float *)v71 + v23) = v24;
// Fill the next segment with i, then the rest with i * (byte_5 / 255)
```

### Coefficient bank selection and frame-local slice
The code uses v18 to select a coefficient bank and uses the frame index a4 to pick the bank rows for that frame. The pointer arithmetic shows one row per term-group, advancing by 4 floats each time.

```124:131:nuanmb_parse (1).txt
result = qword_7FF7D2C51480[v18 - 1]; // select coefficient bank by total group count
v30 = (__m128 *)(result + 4i64 * v18 * (4 * a4 - 4)); // frame-local slice
```

This implies a precomputed matrix/table of coefficients per (group-count, frame-index). The SSE code multiplies each input term vector by a weight (from v71) and by a per-term scale, then performs multiply-accumulate with the corresponding coefficient row(s).

### Groups of terms and their formats
The per-frame data a3 provides multiple groups of input vectors, whose counts are driven by the header nibbles:

1) 16-bit groups: repeated `byte_6_high_bits` times. Each group reads 4×int16, normalizes by 1/65535 (and another runtime constant), multiplies by a single weight (from v71), then MACs with `*v30++`.

```128:168:nuanmb_parse (1).txt
// Loop byte_6_high_bits times
// read 4×int16 (two u16 words) → cvtepi32_ps → scale by constants → multiply by weight
// MAC with *v30++; accumulate into v29
```

2) 8-bit groups: repeated `byte_6_low_bits` times. Reads 4×u8, normalizes by 1/255 (and a runtime constant), multiplies by a weight, MAC with `*v30++`.

```171:206:nuanmb_parse (1).txt
// Loop byte_6_low_bits times
// read 4×u8 → cvtepi32_ps → scale by 1/255 (and constant) → multiply by weight → MAC with *v30++
```

3) Paired special groups: repeated `(byte_7_high_bits >> 1)` times. A helper `sub_7FF7D0B65720` fills two 4-float vectors (v68, v70) from a3; each is modulated by a weight from v71 and then MAC'd with two consecutive coefficient rows `*v30` and `*(v30+1)`.

```210:233:nuanmb_parse (1).txt
v52 = byte_7_high_bits >> 1;
// Loop v52 times:
//   sub_7FF7D0B65720(&v68, &v70, a3);
//   v68 * weight → MAC with *v30;  v70 * weight → MAC with *(v30+1); advance v30 by 2 rows
```

All contributions are accumulated into the scalar `v29`, which is then written to the output array; this repeats `a5` times.

### Implications for 0x3409 (Vector3)
- The runtime evaluation is a nibble-driven weighted expansion against a bank of per-frame coefficients. A simple “per-axis index → t → two-segment linear interpolation” model cannot reproduce the game’s output by itself.
- The repeated small headers seen in samples likely contain these nibble counts and segment distributions, not just a monolithic “bits-per-entry”. The last u16’s high byte correlates with one nibble but should not be assumed to be the active bit width.
- Our research decoder improved by adding segmented single-axis mapping, but to approach zero RMSE we need to (a) parse the nibble header, (b) reconstruct the 8-weight vector exactly, and (c) evaluate against the correct coefficient bank.

### Practical decoder plan (research path)
1) Parse the per-frame nibble header (bytes 5–8) to obtain:
   - `byte_6_high_bits`, `byte_6_low_bits`, `byte_7_low_bits`, `byte_7_high_bits`, and the weight slot layout from `byte_8_high_bits`/`byte_8_low_bits`.
2) Build the 8-length weight vector exactly from the two u16 inputs and the u8 byte using the same normalizations and `a2` scale.
3) Extract per-group input vectors from a3 according to the nibble counts and per-group formats (4×int16, 4×u8, and paired groups from the helper routine), applying the same per-group constant scales.
4) Evaluate against the correct coefficient bank row(s) for the frame index. If the bank cannot be dumped, approximate it via regression using known frame outputs (e.g., from Front.anim) to fit a per-(group-count, frame) coefficient matrix.
5) Only as a fallback (when coefficient banks are unavailable), keep the segmented single-axis t-mapping path to produce a reasonable approximation.

### Relation to quaternion tracks (0x4309/0x4409)
- Version 2.x compressed transforms (see anim_data/compression.rs) encode rotation as XYZ compressed floats and a single sign bit for W. Version 1.2 paths (Unk4309/Unk4409) list 3 key Vector4s and bit counts per entry, and likely use a similar nibble-driven basis-expansion with an additional sign/normalization step for quaternions.
- The same “nibble header → grouped inputs → coefficient-bank MAC” pattern appears general and can be adapted to quaternion tracks by adding W reconstruction (sign bit + unit-norm constraint) after evaluating XYZ.

### Why our current Python decoder is still off
- Our `decode_3409_research.py` currently tries per-frame index splitting and segmented t mapping. This improved RMSE significantly but still lacks the true basis expansion and banked coefficients the game uses.
- Implementing the nibble header parsing, weight vector construction, and the MAC against correct coefficient rows is the key to reproducing game-accurate values.

### Next steps
- Add a “diagnostic mode” to the Python tool to dump, per frame: nibble counts, constructed weights, group counts, and inferred input vectors (int16/u8/paired) without evaluating; verify they are stable across samples.
- Attempt to infer/extract the coefficient banks:
  - If banks can be dumped from runtime or symbols, plug them in directly.
  - Otherwise, fit them via least squares using known output frames (Front.anim) as targets, grouping by `v18` and frame index.
- Once banks are available, implement the exact MAC path to drive the decoder’s output errors near zero.


