# Anim v1.2 residual block count (0x3409 / 0x4409)

## Summary

VS2 residual curves use 33-key blocks. The correct block count is:

```text
block_count = ceil((key_count - 1) / 33)
# integer (key_count >= 2): (key_count - 2) / 33 + 1
```

**Not** `(key_count - 1) / 33 + 1` (off-by-one when `key_count = 33*n + 1`).

## Why it mattered

| key_count | old formula | correct | endpoints (count+1) |
|-----------|-------------|---------|---------------------|
| 40 | 2 | 2 | 3 |
| 100 | **4** | **3** | 4 |

VS2 `20headgrab_*` clips use **100 keys** and residual **flags/type = 3** (extra `u32` after header → endpoints often at `0x18`). Wrong block count made layout inference fail → `AnimData`/`ssbh_data_json` returned `InvalidData`.

## IDA references (`vsac27_Release.exe`)

- `sub_1400561E0` @ `0x1400561E0` — block index `key / 0x21`, residual + endpoint lerp
- `sub_140056750` @ `0x140056750` — residual vector (kernel)
- `sub_140055880` @ `0x140055880` — short4 endpoints
- `sub_140055930` @ `0x140055930` — nibble residual coeffs

## Last-block key mapping

When the final block holds more than 33 keys:

```text
block_idx = min(key_idx / 33, block_count - 1)
local     = key_idx - 33 * block_idx
```

## flags/type endpoint base & residual variants

| flags | Observed role |
|------:|---------------|
| 2 | Endpoints often at `0x14` |
| 3 | Extra `u32` @ `0x14` → endpoints @ `0x18` |
| 4 | Endpoints @ `0x18`; **single residual stream** for whole clip (multi-block residual walk fails) |
| 5 | Endpoints @ `0x18`; may need **pad** between endpoints and residual |

Decoders must **not** slack-rank `0x14` ahead of `0x18` for type 3/5: a misaligned `0x14` walk can still finish residual bytes and yield finite garbage (shifted components).

- `0x4409`: odd flags → base `0x18`; scan residual pads `0..=0x60`.
- `0x3308` / `0x4208`: single residual block; **`key_count` may be 34** (not hard-capped at 33).

## Code

- `compute_block_count` / `block_index_for_key` in `ssbh_data/src/anim_data/v1/common.rs`
- Fixtures: `ssbh_data/src/anim_data/v1/fixtures/headgrab_100_*_flags3.bin`
- Board: `docs/superpowers/plans/2026-07-12-nuanmb-v12-residual-gaps.md`
