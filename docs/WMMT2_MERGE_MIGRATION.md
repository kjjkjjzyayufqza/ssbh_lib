# wmmt2-merge Migration Log

Living operational board for integrating `master` modern ssbh_lib style into a branch
descended from `wmmt2`, while preserving VS2/EXVS2 **format semantics**.

## Baseline (immutable)

| Ref | SHA | Notes |
|-----|-----|-------|
| `wmmt2` (do not rewrite) | `e242b195558b77634d7febd7a091f3983ada7936` | Branch tip before merge work |
| `master` | `1a686eb2fc1cb7a7252f4f76312ea837e9d18287` | Modern style target |
| merge-base | `211414d504034a705dd6b5c09d98799b58ebd438` | `update dependencies` |
| `wmmt2-merge` start | same as `wmmt2` | Created with `git checkout -b wmmt2-merge wmmt2` |

## Current phase / status

**Phase:** 5 — verification complete (+ EXVS2 V12 header fix)  
**Status:** DONE (core crates green; nuanmb on master-style `anim_data/v1` with EXVS2 dual headers)  
**Last updated:** 2026-07-12

### Status legend

- `TODO` — not started
- `WIP` — in progress
- `DONE` — complete with tests green for that slice
- `BLOCKED` — needs decision / external input

## High-level goal

1. `wmmt2-merge` = descendant of `wmmt2` + master’s post-divergence architecture.
2. `.nuanmb` (Anim / nuanmb) read/write lives on **master-era** `anim_data` (`v1` / `v2`), not
   long-term dual stacks (`nuanmb_v12*`, `anim_create_v12` as permanent homes).
3. VS2/EXVS2 **on-disk format rules and identifiers** from wmmt2 stay; Rust expression
   (glam, RelPtr generics, bilge, workspace deps, error modules, module splits) matches master.
4. TDD: tests for retained/target behavior first (or adjusted red), then implementation green.

## Non-goals (from plan)

- Do not modify `wmmt2` tip.
- No bit-identical guarantee for every historical EXVS2 sample.
- No Python research scripts / scratch binaries as library API.
- No crate publish / PR unless later requested.

---

## Open todos

### Git / structure

- [x] Create `wmmt2-merge` from `wmmt2`
- [x] Record baseline SHAs
- [x] Merge `master` into `wmmt2-merge`
- [x] Inventory conflict map after merge
- [x] Remove obsolete parallel anim modules once master `anim_data/v1` covers behavior

### nuanmb / Anim rewrite (priority)

- [x] Adopt master `ssbh_data/src/anim_data/{error,v1,v2}` layout
- [x] Ensure `AnimData::to_anim` / `from` paths for v1.2 use `v1::` (compressed EXVS2-compatible)
- [x] Port multi-frame Visibility bool stream (0x1019) encode/decode for public round-trip
- [x] Restore EXVS2 V12 dual-header read/write (`final_frame_index`≈60 timebase + `unk2` end frame)
- [x] Delete dual-stack `anim_create_v12.rs`, `nuanmb_v12/`, `nuanmb_v12_encode.rs` (folded into v1)
- [x] TDD: unit tests call real `AnimData` read/write entry points (v1.2 + v2 + EXVS2 headers)

### Mesh EXVS2 format retention

- [x] Keep `AttributeUsageV8` extensions 10–12 (EXVS2 Color4/Color5/etc.)
- [x] Keep `MeshWriteProfile` + EXVS2 buffer2 omission behavior
- [x] Keep V8 subindex ordering for binormals/tangents in buffer0 (wmmt2 path retained in mesh_attributes)
- [x] Rewrite mesh_data to master glam / modern patterns while retaining above
- [x] Tests: `write_profile_tests`, attribute EXVS2 cases green

### Matl VS2/EXVS2 format retention

- [x] Keep ParamV15 Type4 / ParamV16 Type4
- [x] Merge master BlendFactor enum expansion
- [x] glam conversions for matl vectors (master) + Type4 overlays
- [x] RelPtr `.0` access (no Deref) for Type4/String2 paths

### Verification

- [x] `cargo test -p ssbh_lib`
- [x] `cargo test -p ssbh_data --lib`
- [x] Capture logs to goal scratch dir
- [x] Spot-check style (glam/RelPtr/bilge) + EXVS2 identifiers still present
- [x] Final status refresh in this file

### Follow-ups (optional / not gating)

- [ ] Broader workspace crates (`ssbh_*_json`, fuzz) smoke if CI requires
- [ ] Additional Transform multi-frame compressed 0x3409/0x4409 golden fixtures
- [ ] Matl Type4 dedicated unit tests beyond conversion paths

---

## Conflict map (resolved)

| File | Strategy | Resolution notes |
|------|----------|------------------|
| `Cargo.toml` | Prefer master workspace deps | DONE |
| `ssbh_data/Cargo.toml` | Prefer master (glam, etc.) | DONE |
| `ssbh_data/src/anim_data.rs` | **Take master** | nuanmb via `v1`/`v2`; added public API TDD tests |
| `ssbh_data/src/anim_data/v2/compression.rs` | Prefer master | DONE |
| `ssbh_data/src/matl_data.rs` | Master glam + Type4 + RelPtr `.0` | DONE |
| `ssbh_data/src/mesh_data.rs` | Master glam + MeshWriteProfile EXVS2 | DONE; error keeps mesh_object_name |
| `ssbh_data/src/mesh_data/mesh_attributes.rs` | Master + VS2 Float4 vectors | DONE |
| `ssbh_data/src/mesh_data/vector_data.rs` | Master glam + V8 Float4 from_vectors | DONE |
| `ssbh_lib/Cargo.toml` | Prefer master | DONE |
| `ssbh_lib/src/formats/matl.rs` | Master BlendFactor + wmmt2 Type4 | DONE |
| `ssbh_lib/src/lib.rs` | Prefer master | DONE |
| `ssbh_lib/src/formats/mesh.rs` | Auto-merge kept AttributeUsageV8 10–12 | DONE |
| `ssbh_lib/src/formats/anim.rs` | Master layout + EXVS2 V12 field docs; align tests use `SsbhWrite::write` | DONE |

---

## Master themes integrated (evidence log)

| When | What landed | Notes |
|------|-------------|-------|
| 2026-07-12 | Workspace deps, 2024 edition, bilge | From master merge |
| 2026-07-12 | glam for anim/mesh/matl/skel data | Master |
| 2026-07-12 | RelPtr generic; no Deref | Call sites use `.0` |
| 2026-07-12 | Anim 1.2 `anim_data/v1` + EXVS2 compression modules | Replaces nuanmb_v12* |
| 2026-07-12 | BlendFactor expansion | Master matl |
| 2026-07-12 | Shader metadata result type + release.yml | Master tip commits |

## wmmt2 format bits retained (evidence log)

| When | What retained | Where |
|------|---------------|-------|
| 2026-07-12 | AttributeUsageV8 10–12 | `ssbh_lib/src/formats/mesh.rs` |
| 2026-07-12 | MeshWriteProfile + Vs2Canonical buffer2 omit | `ssbh_data/src/mesh_data.rs` + write_profile_tests |
| 2026-07-12 | V8 Vector4 vectors as Float4 (not HalfFloat4) | `vector_data.rs` from_vectors |
| 2026-07-12 | Matl ParamV15/V16 Type4 | `formats/matl.rs` + `matl_data.rs` |
| 2026-07-12 | Anim v1.2 EXVS2 field semantics docs | `formats/anim.rs` V12 |

---

## nuanmb rewrite notes

### Target layout (live)

```
ssbh_data/src/anim_data.rs
ssbh_data/src/anim_data/error.rs
ssbh_data/src/anim_data/v1.rs + v1/{buffers,common,encode,rotate_*,translate}.rs
ssbh_data/src/anim_data/v2.rs + v2/{buffers,compression}.rs
ssbh_data/src/anim_data/bitutils.rs
```

### Removed dual stack

- `ssbh_data/src/nuanmb_v12/`
- `ssbh_data/src/nuanmb_v12_encode.rs`
- `ssbh_data/src/anim_create_v12.rs`
- `ssbh_data/src/anim_data_test.rs` (superseded by module tests)

### Public behavior

- `AnimData::to_anim()` for `(1,2)` → `v1::create_anim_v12`
- `AnimData::to_anim_uncompressed()` for `(1,2)` → `v1::create_anim_v12_uncompressed`
- `AnimData::try_from(&Anim)` reads v1.2 via `v1::read_groups_v12`
- Multi-frame Visibility uses **0x1019** stream (u32 count + u16 samples); constant uses **0x1013**
- **EXVS2 V12 headers** (on write): `unk1 = end/60`, file `final_frame_index = 60.0`, `unk2 = end`, `unk3 = 0`
- **EXVS2 V12 headers** (on read): if file `final_frame_index ≈ 60` and `unk2 >= 0`, high-level end frame = `unk2` and `frame_count = unk2+1`; else Smash-style (`final_frame_index` is end frame)

### TDD additions

- `nuanmb_v12_visibility_round_trip_public_api` — encode/decode via public APIs
- `nuanmb_v12_uncompressed_public_api` — uncompressed path
- `nuanmb_v12_exvs2_header_write_via_to_anim` / `_read_via_try_from` / `_and_visibility_round_trip`
- `nuanmb_v12_smash_style_header_read_via_try_from` — non-EXVS2 path still works
- Fixed encode/decode gap for 0x1019 bool streams discovered by those tests
- Restored EXVS2 dual-header after skeptic review (master merge had dropped wmmt2 convention)

---

## Known risks (remaining)

1. Transform multi-frame still prefers uncompressed-ish layouts in several write paths rather than full 0x3409/0x4409 bit-identical game buffers.
2. Binary identity of EXVS2 samples not guaranteed.
3. Peripheral JSON CLIs / fuzz not re-run as gating in this session.
4. `align_v20` historical assertion (`len % 8 == 2`) outdated vs current writer; test now checks body write 8-alignment with `SsbhWrite::write`.

---

## Session / decision log

| Date | Decision |
|------|----------|
| 2026-07-12 | Branch `wmmt2-merge` from `wmmt2` only; never force `wmmt2`. |
| 2026-07-12 | Prefer master structure for anim; re-apply EXVS2 format deltas as overlays. |
| 2026-07-12 | This file is the single operational board for the merge rewrite. |
| 2026-07-12 | V8 `from_vectors(Vector4)` stays Float4 (VS2 precision), not master HalfFloat4. |
| 2026-07-12 | Visibility multi-frame uses shared 0x1019 layout for compressed and uncompressed writers. |
| 2026-07-12 | Anim V12 write always EXVS2 dual-header; read auto-detects EXVS2 vs Smash style. |

## Deviations from goal plan

- Dual-stack nuanmb modules removed during merge resolution rather than after a long parallel period (cleaner compile graph).
- `align_v20` test updated to match current `SsbhWrite` body length (was failing with inherent `Anim::write` HBSS wrapper / modern padding).
