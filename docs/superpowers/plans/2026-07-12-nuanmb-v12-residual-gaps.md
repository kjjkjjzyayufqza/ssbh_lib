# Plan: Close incomplete Anim v1.2 residual gaps (VS2 nuanmb)

## Goal

Complete analysis and TDD fixes for Anim v1.2 (`.nuanmb`) residual decode paths that are still incomplete relative to VS2 shipped assets, using IDA Pro (`vsac27_Release.exe`) evidence and **sampled** copies under project `temp/` (never write the game tree).

## Global constraints

- Source corpus (read-only): `E:\XB\解包\vs2\x64\003motion` (~46k `.nuanmb`).
- **Never** modify or write into the game unpack tree.
- All working copies / decode dumps go under project `/temp/` (gitignored).
- **Never** commit `temp/`, large dumps, or MCP noise.
- **No full crate/workspace tests**; only filtered `cargo test -p ssbh_data --lib <filter>`.
- Default v1.2 **write** stays uncompressed (wmmt2 policy); residual encode remains opt-in.
- “100%” means complete analysis + TDD of **known incomplete gaps** proven on samples—not exhaustive decode of all 46k files in one pass.

## Sampling rule

1. Copy ≤ N files (N≈12) into `temp/nuanmb_samples/`.
2. SHA-256 source == dest after copy (`{SCRATCH}/nuanmb_copy_manifest.json`).
3. Histogram headers with `temp/scan_nuanmb_headers.py` → `{SCRATCH}/nuanmb_header_sample.json`.
4. Probe high-level decode via `ssbh_data_json` into `temp/decode_out/` only.

## Gap inventory (pre-fix)

| Gap | Status before | Evidence |
|-----|---------------|----------|
| Unknown property headers in 12-sample VS2 set | None in sample (`unsupported_in_sample: {}`) | histogram |
| Dual-header EXVS2 `final=60` / `unk2=end` | Already handled | prior merge |
| Residual `0x3409`/`0x4409` **flags/type=2** | OK | bound_facedwn samples decode |
| Residual `0x3409`/`0x4409` **flags/type=3**, **key_count=100** (`33*n+1`) | **FAIL `InvalidData`** | headgrab samples |
| `0x4309` layout inference | Fragile; not hit as sole failure in sample | prior docs |
| Block count formula off-by-one | Root cause of flags=3 100-key fail | IDA + brute residual walk |

### Sample header histogram (N=12, temp copies)

Dominant: `0x1013`, `0x4409`, `0x4003`, `0x3003`, `0x4408`, `0x3300`, `0x4308`, `0x4300`, `0x3409`, `0x3408`, `0x3308`, `0x4309`.

Failing files (before fix):

- `001hito_000common_000common_001_20headgrab_stk_air_bk.nuanmb`
- `001hito_000common_000common_001_20headgrab_stk_air_fr.nuanmb`

Both use dual-header `final_frame_index=60`, `unk2=99`, and residual buffers with **flags=3**, **frames=100**.

## IDA evidence (vsac27_Release.exe)

| Symbol | Address | Role |
|--------|---------|------|
| `sub_140056750` | `0x140056750` | Residual vector decode (kernel / nibble family); callees include `sub_140055930` |
| `sub_1400561E0` | `0x1400561E0` | Blocked curve sample: `key/0x21` block index, residual + endpoint lerp |
| `sub_140056490` | `0x140056490` | Sibling blocked evaluator (short4 endpoint path via `sub_140055880`) |
| `sub_140055880` | `0x140055880` | short4 endpoints × `gCurveShort4Scale` (`0x141b4f600`) |
| `sub_140055930` | `0x140055930` | Nibble residual pair decode (`gCurveNibbleBias4` / `gCurveNibbleScale4`) |

### Segmentation math (from `sub_1400561E0` + type-9 notes)

- Block index uses division by `0x21` (33).
- Correct **block_count** = `ceil((key_count - 1) / 33)`  
  integer: `(key_count - 2) / 33 + 1` for `key_count >= 2`.
- Bug: old code used `(key_count - 1) / 33 + 1`, which for **`key_count = 100`** yields **4** instead of **3**, over-reading endpoints and breaking residual layout inference (especially flags=3 with extra `u32` at `0x14` → endpoints at `0x18`).
- When the last block holds >33 keys, `block_idx = min(key_idx / 33, block_count - 1)`.

## Tasks

- [x] Ensure `temp/` exists and is gitignored; copy-only sampling rule documented here.
- [x] Build gap inventory + sample header histogram from `temp/` copies.
- [x] Use IDA Pro MCP on residual path; record addresses above.
- [x] TDD: failing fixtures → fix `compute_block_count` + `block_index_for_key` → filtered tests green.
- [x] Re-decode all 12 sample `.nuanmb` via shipped `ssbh_data_json` (all ok=True).
- [x] Capture logs under implementer scratch; confirm no `temp/` pollution in git status.

## Implementation (shipped)

- `ssbh_data/src/anim_data/v1/common.rs`: correct `compute_block_count` / `compute_block_len`; add `block_index_for_key`.
- Decoders/encoders: `translate.rs`, `rotate_4409.rs`, `rotate_inferred.rs`, `rotate_basic.rs`, `encode.rs` use clamped block index.
- Fixtures (committed, tiny): `ssbh_data/src/anim_data/v1/fixtures/headgrab_100_{3409,4409}_flags3.bin`.
- Tests: `compute_block_count_matches_ida_ceil_rule`, `decode_vs2_headgrab_3409_flags3_100_keys`, `decode_vs2_headgrab_4409_flags3_100_keys`.

## Decision log

| Date | Decision |
|------|----------|
| 2026-07-12 | Interpret “100%” as gap-complete on sampled VS2, not full 46k scan. |
| 2026-07-12 | Highest-priority gap = residual block_count off-by-one at `key_count=33*n+1`. |
| 2026-07-12 | Align 3409/4409/4309 block_count with type-9 ceil formula. |
| 2026-07-12 | Keep default nuanmb write uncompressed; residual write opt-in only. |
| 2026-07-12 | flags/type==3 forces endpoint base 0x18 (never slack-pick 0x14); TDD asserts frame[0]/frame[last] == endpoints@0x18. |

## Verification commands (filtered only)

```text
cargo test -p ssbh_data --lib compute_block_count_matches -- --nocapture
cargo test -p ssbh_data --lib decode_vs2_headgrab -- --nocapture
cargo test -p ssbh_data --lib encode_decode_vector3_3409 -- --nocapture
```

Logs: `{SCRATCH}/nuanmb_sample_tests.log`, `{SCRATCH}/nuanmb_sample_decode.log`, `{SCRATCH}/nuanmb_header_sample.json`.

## Remaining (out of this closure or future sessions)

- Exhaustive residual accuracy vs Maya `.anim` ground truth for all flags/type variants.
- Bit-identical residual **encode** (non-goal).
- Pokémon Snap / non-VS2 variants (#157).
- Full-corpus header scan (forbidden by sampling constraint; optional offline later).
