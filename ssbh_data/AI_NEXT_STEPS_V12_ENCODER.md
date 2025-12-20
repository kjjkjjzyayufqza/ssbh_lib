## Goal

Generate Anim v1.2 (`.nuanmb`) files from `.anim` that the game can load and play correctly (no crash, correct motion).

This document focuses on a single strategic direction:

- Stop relying on "uncompressed" / indexed keyframe formats for multi-frame Transform properties.
- Always use the game's residual-based compressed formats for multi-frame data, using the 33-key block model.

## Why We Are Changing Direction

### 1) `0x3300` has a hard frame index ceiling

`0x3300` stores per-key frame indices as `u8` (1 byte). That means the maximum frame index is `0xFF` (255).
Any animation longer than 256 frames cannot be represented without either splitting timelines or switching format.

For files like `final_frame_index = 360` (361 frames), `0x3300` is fundamentally the wrong encoding.

### 2) "Uncompressed" formats may not be supported by the runtime path we need

Even if a format can be decoded by our tooling, the game runtime may only support a subset of v1.2 formats (or may only enable some of them for Transform playback).
Symptoms we saw that match this:

- First frame looks correct, but no motion afterward.
- Motion is different from the source `.anim`.
- In some cases, the game crashes (strict decoder reads unexpected buffer layout).

### 3) Residual compression is the most likely to match game expectations

Known-good v1.2 assets often use:

- `0x3409` for Vector3 curves (Translate / Scale).
- `0x4409` for Quaternion curves (Rotate).

These are block-based formats with:

- 33-key blocks
- endpoints per block
- residual stream using the kernel/weight model

If we can match these buffers structurally and semantically, we maximize the chance of correct in-game playback.

## Update (POC): A Deterministic 0x3409 Encoder Exists in This Repo

We implemented a standalone **curve-magic** `0x3409` encoder and verified it can reproduce a known-good `3409.bin` **byte-for-byte**.

Files:

- `test/curve_3409_encode.py`
  - Reads per-frame samples from CSV (`frame,x,y,z`).
  - Uses `test/kernel_table.py` (DCT-IV kernel) and `test/lib_3409.py` (verified residual decoder model).
  - Encodes endpoints + residual stream into a curve-level `0x3409` buffer.
  - Supports a template mode (`--ref-bin`) to match header fields and residual header layout exactly.
  - Uses **float32-style rounding** during critical math steps to achieve byte-level reproducibility.
- `test/map_3409_offset.py`
  - Debug helper that maps a byte offset to (block, component, coefficient region) when investigating diffs.

How it works (high-level):

- **Endpoints**
  - Uses the curve-magic 33-key block model.
  - `block_count = (N - 1) // 33 + 1` for `N > 1`
  - `endpoint_count = block_count + 1`
  - Endpoint `b` is `sample[b*33]`, plus the final endpoint `sample[N-1]`.
- **Kernel baseline**
  - For each block and each component, baseline is linear interpolation between adjacent endpoints:
    - `t = local / block_len`, `block_len = 33` except the last block where `block_len = N - 33*block_idx - 1`
    - `K(local) = lerp(endpoint[b], endpoint[b+1], t)`
- **Residual**
  - `R(local) = sample(local) - K(local)`
  - Residual is encoded using the same kernel/weight model as the verified decoder (`sub_140056750` behavior).
  - The encoder solves for coefficient vectors in a way that matches the decoder’s dot-product reconstruction and then quantizes into i16 / i8 / nibble-packed lanes.
  - Some headers use `v12` as implicit (non-stored) slots that still affect the kernel size; the encoder supports this.

Validation:

- Decode `test/3409.bin` to CSV:
  - `python .\nuanmb_decompress.py --bin .\3409.bin --out .\3409_decoded.csv --magic 0x3409`
- Re-encode and compare:
  - `python .\curve_3409_encode.py --in-csv .\3409_decoded.csv --ref-bin .\3409.bin --out-bin .\3409_reencoded.bin --compare`
  - Expected output: `byte_compare: identical`

## Target Output Formats

For Transform tracks:

- **Translate (Vector3)**:
  - Single frame: `0x3003`
  - Multi-frame: `0x3409`

- **Scale (Vector3)**:
  - Single frame: `0x3003`
  - Multi-frame: `0x3409`

- **Rotate (Experimental: Euler Vector3)**:
  - Single frame: `0x4003` (Quaternion Vector4)
  - Multi-frame: `0x3409`

Important note about Rotate:

- The “known-good” direction is still `0x4409` for quaternion curves, but this document records an **experimental hypothesis**:
  - Convert quaternion rotation to a stable Euler representation, then compress Euler as `Vector3` using curve-magic `0x3409`.
- This may fail in runtime or produce different motion due to Euler conventions, discontinuities, or gimbal behavior.
  - If this happens, revert multi-frame Rotate back to quaternion (`0x4409`) and keep `0x3409` for Translate/Scale.

This plan intentionally does not rely on `0x3200`, `0x3300`, or `0x3400` for multi-frame playback.

## Work Plan (Phased)

### Phase A: Build a comparison and inspection harness (must-have)

We need a deterministic way to compare:

- a known-good `.nuanmb` from the game/assets
- a generated `.nuanmb` from our converter

Deliverables:

- A small CLI/tool that:
  - Lists Anim::V12 header fields.
  - Lists tracks and their property names (Scale/Rotate/Translate/Visibility/CompensateScale).
  - For each property buffer, prints:
    - buffer header (u32 magic)
    - frame/key count
    - the key header fields used by the runtime (`unk1`, base scale, flags/bits, etc.)
    - buffer byte length
    - a short hex prefix of the payload

Success criteria:

- For a given bone, we can say exactly which properties differ and why.
- For a given buffer, we can confirm the header fields match typical known-good values.

### Phase B: Preserve property presence semantics

The game treats "property absent" differently from "property present but equals default".
If we always write Translate=(0,0,0) or Scale=(1,1,1), we may override rest pose defaults and break motion.

Deliverables:

- Extend the high-level representation to track whether Scale/Rotate/Translate were authored/present.
- When converting `.anim`, only author properties that were actually present in the source intent (or provide a mode switch).

Success criteria:

- Generated `.nuanmb` property lists match known-good files for the same track set.

### Phase C: Implement strict v1.2 residual encoders (core work)

#### C1) `0x3409` Vector3 encoder (Translate / Scale)

Requirements:

- Use 33-key blocks.
- Write endpoints and residual stream in a layout that the game runtime expects.
- Populate header fields (`unk1`, base scale, flags/bits) in a way consistent with known-good files.
  - Use float32-style math for reproducible quantization decisions.

Validation:

- A "strict parser" (no inference) can parse our output without scanning.
- The file loads and plays in-game without crashing.
- Numeric error is within acceptable lossy tolerance.

#### C2) `0x3409` Rotate encoder (Experimental: Euler Vector3)

Requirements:

- Convert quaternion curves to Euler consistently (choose and document one convention).
- Enforce continuity rules in Euler space (avoid sudden ±360 jumps).
- Encode Euler as `Vector3` using the same `0x3409` encoder logic as C1.

Validation:

- Same strict parsing and in-game criteria as `0x3409`.
- If runtime playback is incorrect or unstable, switch Rotate back to quaternion `0x4409`.

### Phase D: Integration in `ssbh_editor` export path

Deliverables:

- Update `.anim -> .nuanmb v1.2` to use the compressed writer by default:
  - Translate multi-frame -> `0x3409`
  - Scale multi-frame -> `0x3409`
  - Rotate multi-frame -> `0x3409` (experimental Euler path)

Optional:

- Add a UI toggle: "Safe compatibility mode" (compressed) vs "Debug uncompressed mode".

## Practical Notes

### Block math

For a curve of `N` keys:

- `block_count = (N - 1) / 33 + 1` when `N > 1`
- endpoints count is `block_count + 1`

### Testing strategy

- Unit tests:
  - Encode -> decode roundtrip using the same mathematical model.
  - Verify headers and buffer lengths are consistent.
- Binary inspection:
  - Compare against known-good `.nuanmb` files in `test/`.
- In-game:
  - Verify no crash.
  - Verify the bone motion matches expectations.

## Definition of Done

- Converting a 361-frame `.anim` produces a `.nuanmb` that:
  - loads in-game without crashing
  - plays translation/rotation/scale over time (not only the first frame)
  - matches the source animation within expected lossy compression error


