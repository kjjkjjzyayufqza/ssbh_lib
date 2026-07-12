# Plan: VS2 003motion AnimData decode fail → 0

## Goal

Decode all **46239** `.nuanmb` under `E:\XB\解包\vs2\x64\003motion` via `AnimData::from_file` with **fail = 0**, RE-faithful residual curves (endpoint-aligned), fail-closed, no panic.

## Global constraints

- Read-only game dump; no writes into unpack tree.
- `temp/` gitignored if used; full scan results to stdout/`{SCRATCH}` only.
- IDA + docs; TDD with real fixtures; default v1.2 write remains uncompressed.

## Baseline (pre-fix)

| Metric | Value |
|--------|------:|
| files | 46239 |
| ok | 45554 |
| fail | 685 |
| classes | DecodeFailed(0x4409)=385, (0x3308)=299, (0x3409)=1 |

## Fixes shipped

| Class | Root cause | Fix |
|-------|------------|-----|
| **0x3308** (299) | Hard reject `key_count > 33` | Allow k≥34; `block_len = k-1`; safe residual component access |
| **0x4208** (last 1) | Same artificial k>33 limit | Same pattern as 3308 |
| **0x4409** (385) | flags=5 needs mid pad before residual; only tried pad=0 | Scan pads `0..=0x60`; odd flags force endpoints @0x18 |
| **0x3409** (1) | flags=4 multi-block residual walk fails | Dedicated single residual stream + multi-block endpoints |
| Kernel OOB | `local_idx-1 >= dim` panics | Zero basis row (Python/lib_3409 parity) |
| Errors | Opaque InvalidData | `V12PropertyDecodeFailed { header, property }` for classification |

## IDA (carry-forward)

- `sub_140056750` residual, `sub_1400561E0` block index `/0x21`
- See `docs/nuanmb/V12_RESIDUAL_BLOCK_COUNT.md`

## Tasks

- [x] Classify 685 failures by header
- [x] TDD fixtures + fixes for 3308 / 4409 flags5 / 3409 flags4 / 4208
- [x] Full `scan_nuanmb` → fail=0
- [x] Default write uncompressed preserved
- [x] Docs decision log

## Verification

```text
cargo run -p ssbh_test --release --bin scan_nuanmb -- "E:\XB\解包\vs2\x64\003motion"
# files_scanned: 46239  ok: 46239  fail: 0

cargo test -p ssbh_data --lib decode_vs2
```

## Decision log

| Date | Decision |
|------|----------|
| 2026-07-12 | 3308/4208: single residual block may have key_count=34 (not a hard 33 max). |
| 2026-07-12 | 4409 flags=5: endpoints @0x18 + pad before residual; rank by residual slack. |
| 2026-07-12 | 3409 flags=4: single residual stream over whole clip with multi-block endpoints. |
| 2026-07-12 | Kernel OOB rows are zero (matches prior Python residual research). |
