# Anim v1.2 residual curve layout (`0x_308` / `0x_408` / `0x_309` / `0x_409`)

## Summary

The residual codecs have a **fully deterministic** header. No offset inference,
endpoint-size guessing, or slack ranking is needed — and none is permitted,
because a misaligned layout still produces finite, plausible-looking values.

### Blocked curves — `0x3309`, `0x4309`, `0x3409`, `0x4409`

```text
magic        u32
key_count    u32
unk1         f32                   // 1.0 in every observed VS2/EXVS2 buffer
[0x_309]     frame_indices u8[key_count], align 4
base_scale   f32
block_count  u16
block_words  u16[block_count - 1]  // u32-word offset of block b's residual,
                                   // relative to the residual stream start;
                                   // 0 marks a block that stores no residual
             align 4
endpoints    VecN[block_count + 1] // Vec3 for 0x3xxx, unit Vec4 for 0x4xxx
residual     blocks
```

- `block_count == ceil((key_count - 1) / 33)`, integer form `(key_count - 2) / 33 + 1`.
  This is redundant with `key_count`; a mismatch means the buffer is not this
  layout and decoding must fail.
- `endpoints_offset = align_up(base_scale_offset + 4 + 2 * block_count, 4)`.
- A block whose word offset is `0` decodes as a pure endpoint interpolation.
  Empty blocks appear mid-clip, not just in the tail.
- Key → block: `block_idx = min(key_idx / 33, block_count - 1)`,
  `local = key_idx - 33 * block_idx`,
  `block_len = key_count - 33 * block_idx - 1` for the last block, else 33.
- Sample = `lerp(endpoints[b], endpoints[b + 1], local / block_len) + residual`.

### Single-block curves — `0x3308`, `0x4308`, `0x3408`, `0x4408`

```text
magic        u32
key_count    u32
unk1         f32
[0x_308]     frame_indices u8[key_count], align 4
base_scale   f32
endpoints    VecN[2]
residual     one block, block_len = key_count - 1
```

`key_count` never exceeds 34 (33 interpolation steps plus the final key), so this
family always has exactly one block and two endpoints.

## The field names ssbh_data used to use

`block_count` was called `flags` and `block_words[0]` was called `bits`. Reading
`flags` as an opaque enum produced a table of special cases (`flags == 3` adds a
`u32`, `flags == 4` is "a single residual stream", `flags == 5` needs a "pad")
that are all just consequences of the word table's length and its zero entries.

## Corpus validation (2026-08-09)

`E:\XB\mod\003motion`, 3302 `.nuanmb`:

| check | result |
|-------|--------|
| `block_count == ceil((key_count-1)/33)` | 32,487 / 32,487 |
| block starts predicted by the word table | 32,487 / 32,487 |
| residual ends exactly at end of buffer | 32,487 / 32,487 |
| `0x4xxx` endpoints are unit quaternions | all |
| single-block family exact closure | 19,477 / 19,477 |
| `unk1 == 1.0` on every keyed buffer | 8,911 / 8,911 |

## Why this mattered

The inference-based decoders silently mis-decoded three paths:

| codec | defect | worst observed effect |
|-------|--------|----------------------|
| `0x4309` | endpoint offset hardcoded to the `block_count == 3` layout; 94% of buffers are `block_count == 2` | clavicle at 168 deg/frame instead of 1.9 |
| `0x4408` | `base_scale` taken from `endpoint0.x` instead of offset 12 | residual amplified 104x on low-motion bones |
| `0x3408` | layout scan always settled on `endpoints@12` + `base_scale@36` | root translation off by 16 units |

None of them raised an error — a full-corpus `scan_nuanmb` reported `ok_rate:
100%` throughout. Layout inference converts a structural bug into silent data
corruption, which is why the decoders now fail loudly on an inconsistent header.

## IDA references (`vsac27_Release.exe`)

- `sub_1400561E0` @ `0x1400561E0` — block index `key / 0x21`, residual + endpoint lerp
- `sub_140056750` @ `0x140056750` — residual vector (kernel)
- `sub_140055880` @ `0x140055880` — short4 endpoints
- `sub_140055930` @ `0x140055930` — nibble residual coeffs

## Code

- `common::read_blocked_header` / `BlockedHeader` — the blocked header parser
- `translate::read_single_block_header` — the `0x_308` / `0x_408` header
- Fixtures: `ssbh_data/src/anim_data/v1/fixtures/`
