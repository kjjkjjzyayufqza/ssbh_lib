## Goal

Generate Anim v1.2 (`.nuanmb`) files from `.anim` that the game can load and play correctly (no crash, correct motion).

This document now focuses on a single strategic direction:

- Prefer **non-compressed** (raw/constant) formats for Transform playback to maximize runtime compatibility.
- Treat `0x3409/0x4409` residual compression as **blocked** until we can reverse engineer the runtime behavior from IDA Pro.

## Why We Are Changing Direction

### 1) The residual-compressed formats are strict and we cannot guarantee correctness yet

We tried hard to generate `0x3409` (Vector3) and `0x4409` (Quaternion) buffers that match shipped assets:

- endpoint-base variants (`0x14` vs `0x18`)
- `flags`/`bits` combinations
- per-block word counts and prefix sums
- residual header shapes and quantization rules

Despite extensive iteration, the game still crashes on our generated compressed buffers.

The root issue is that we do **not** fully understand the compression format semantics.
In particular, a field at **offset `0x14`** is critical for at least one variant and is **not padding**.
Without knowing the exact runtime meaning of this field (and how it interacts with `flags`, `bits`, block layout, and component selection),
we cannot guarantee the game will read the buffer safely.

### 2) Our current decoder behavior is heuristic, not a strict runtime clone

Our Python/Rust decoders can "successfully" decode many inputs by **scanning and picking a plausible layout**.
This is useful for inspection, but it is not proof that we are decoding exactly like the game:

- the tooling is effectively doing *best-effort matching*
- the game runtime likely has a strict path (no scanning) and strict invariants
- therefore, encoder changes that appear to roundtrip in tooling can still crash in-game

Because the decoder is not a strict clone, continuing to "match bytes" is too risky for shipping output.

### 3) We still need long animations (361 frames) without index ceilings

Some "uncompressed" formats have constraints (e.g. `0x3300` uses `u8` indices and cannot represent 361 frames).
Therefore, the non-compressed plan must use formats that support arbitrary frame counts.

## Status Update: Compression Experiments Are Not Shippable Yet

We attempted multiple compression approaches (both "template-matching" and "model-based"):

- matching headers observed in shipped assets (including the `0x18` endpoint-base variants)
- matching residual header shapes and stream sizes
- matching block word counts and writing prefix sums at `0x14` where applicable
- matching quantization math (float32 rounding paths)

Even when a buffer can be decoded by our tooling, the game still crashes.

Conclusion:

- We should **stop shipping compressed output** for now.
- We should only revisit compression after **IDA Pro** analysis confirms the exact runtime rules and invariants.

## Target Output Formats

For Transform tracks:

### Non-compressed (preferred for now)

- **Translate (Vector3)**:
  - Single frame: `0x3003` (constant Vector3)
  - Multi-frame: `0x3400` (raw per-frame Vector3 stream)

- **Scale (Vector3)**:
  - Single frame: `0x3003`
  - Multi-frame: `0x3400`

- **Rotate (Quaternion Vector4)**:
  - Single frame: `0x4003` (constant quaternion)
  - Multi-frame: `0x4300` (raw per-frame quaternion stream)

Notes:

- Avoid `0x3300` for long animations because it uses `u8` frame indices (hard ceiling at 256 frames).
- Avoid `0x3409/0x4409` until runtime behavior is confirmed via IDA Pro.

## Work Plan (Phased)

### Phase A: Build a strict inspection harness (must-have)

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
  - Explicitly marks fields that are currently **unknown / not proven** (e.g. `u32@0x14` for some variants).

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

### Phase C: Ship a safe non-compressed writer (core work)

#### C1) Vector3: `0x3003` / `0x3400`

Requirements:

- Use `0x3003` for constant single-frame values.
- Use `0x3400` for multi-frame values (raw stream, no inference fields).

Validation:

- A strict parser can parse our output without scanning.
- The file loads and plays in-game without crashing.
- Motion matches the source `.anim` exactly (no compression loss).

#### C2) Quaternion: `0x4003` / `0x4300`

Requirements:

- Use `0x4003` for constant single-frame quaternions.
- Use `0x4300` for multi-frame quaternion streams.
- Normalize and enforce sign continuity for stable playback.

Validation:

- Same strict parsing and in-game criteria as C1.

### Phase D: Integration in `ssbh_editor` export path

Deliverables:

- Update `.anim -> .nuanmb v1.2` to use the **non-compressed** writer by default:
  - Translate multi-frame -> `0x3400`
  - Scale multi-frame -> `0x3400`
  - Rotate multi-frame -> `0x4300`

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
  - matches the source animation without compression loss

## When To Revisit Compression

Only after:

- Confirming the exact runtime invariants via **IDA Pro** (including the meaning of `u32@0x14` and how `flags/bits` select variants).
- Replacing heuristic decoding with a strict, single-path implementation that matches the IDA Pro control flow.


