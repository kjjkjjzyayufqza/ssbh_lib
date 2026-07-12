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

**Phase:** 0 — branch + migration doc  
**Status:** IN PROGRESS  
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
- [ ] Merge `master` into `wmmt2-merge`
- [ ] Inventory conflict map after merge
- [ ] Remove obsolete parallel anim modules once master `anim_data/v1` covers behavior

### nuanmb / Anim rewrite (priority)

- [ ] Adopt master `ssbh_data/src/anim_data/{error,v1,v2}` layout
- [ ] Ensure `AnimData::to_anim` / `from` paths for v1.2 use `v1::` (compressed EXVS2-compatible)
- [ ] Port any wmmt2-only v1.2 behaviors not already on master (if any) as thin overlays on `v1`
- [ ] Delete or thin `anim_create_v12.rs`, `nuanmb_v12/`, `nuanmb_v12_encode.rs` after coverage
- [ ] TDD: unit tests call real `AnimData` read/write entry points (v1.2 + v2)

### Mesh EXVS2 format retention

- [ ] Keep `AttributeUsageV8` extensions 10–12 (EXVS2 Color4/Color5/etc.)
- [ ] Keep `MeshWriteProfile` + EXVS2 buffer2 omission behavior
- [ ] Keep V8 subindex ordering for binormals/tangents in buffer0
- [ ] Rewrite mesh_data to master glam / modern patterns while retaining above
- [ ] Tests: `write_profile_tests`, attribute EXVS2 cases green

### Matl VS2/EXVS2 format retention

- [ ] Keep ParamV15 Type4 / ParamV16 Type4 if still needed on wmmt2 samples
- [ ] Merge master BlendFactor enum expansion
- [ ] glam conversions for matl where master has them
- [ ] Tests for V15/V16 type4 + blend factors

### Verification

- [ ] `cargo test -p ssbh_lib`
- [ ] `cargo test -p ssbh_data --lib`
- [ ] Capture logs to goal scratch dir
- [ ] Spot-check style (glam/RelPtr/bilge) + EXVS2 identifiers still present
- [ ] Final status refresh in this file

---

## Conflict map (filled during merge)

| File | Strategy | Resolution notes |
|------|----------|------------------|
| `Cargo.toml` | Prefer master workspace deps; keep any wmmt2-only members if still needed | TBD |
| `ssbh_data/Cargo.toml` | Prefer master (glam, etc.) | TBD |
| `ssbh_data/src/anim_data.rs` | **Take master** as primary; re-export / thin shims only if needed | nuanmb rewrite |
| `ssbh_data/src/anim_data/v2/compression.rs` | Prefer master | TBD |
| `ssbh_data/src/matl_data.rs` | Master glam + re-apply Type4 / V15–V16 extras | TBD |
| `ssbh_data/src/mesh_data.rs` | Master glam + re-apply MeshWriteProfile / EXVS2 | TBD |
| `ssbh_data/src/mesh_data/mesh_attributes.rs` | Master + AttributeUsageV8 10–12 | TBD |
| `ssbh_data/src/mesh_data/vector_data.rs` | Prefer master glam | TBD |
| `ssbh_lib/Cargo.toml` | Prefer master | TBD |
| `ssbh_lib/src/formats/matl.rs` | Master BlendFactor + wmmt2 Type4 | TBD |
| `ssbh_lib/src/lib.rs` | Prefer master (RelPtr, glam helpers) | TBD |
| `ssbh_lib/src/formats/mesh.rs` | Master style + EXVS2 AttributeUsageV8 | TBD |
| `ssbh_lib/src/formats/anim.rs` | Prefer master anim 1.2 headers | TBD |

*(Update each row when resolved.)*

---

## Master themes integrated (fill as we go)

Expected master themes post-merge-base `211414d`:

- Workspace dependencies, 2024 edition / fmt
- bilge instead of modular-bitfield
- glam for skel/anim/mesh/matl data + ssbh_lib conversions
- RelPtr generic over pointer type
- Anim 1.2 compressed headers / blocks / EXVS2 compression path
- Flattened anim_data hierarchy → `anim_data/v1` + `anim_data/v2`
- BlendFactor enum expansion
- Shader metadata result type fix
- CI release.yml update

## EXVS2 / VS2 format KEEP list

### Format keep (semantics / on-disk)

| Area | Keep | Notes |
|------|------|-------|
| Anim v1.2 | yes | major/minor 1.2; compressed blocks 0x3409 / 0x4409 style as on master |
| Mesh AttributeUsageV8 10–12 | yes | EXVS2 Color semantics from wmmt2 |
| MeshWriteProfile EXVS2 | yes | buffer2 omission / canonical write |
| Mesh V8 buffer0 binormal/tangent subindex order | yes | wmmt2 fix |
| Matl ParamV15/V16 Type4 | yes if used by assets | 16-byte / structured type4 |
| Matl V15/V16 entries | yes | already on both lineages to some degree |

### Rewrite keep (implementation only → master style)

| Area | Rewrite to |
|------|------------|
| nuanmb encode/decode | `ssbh_data::anim_data::v1` |
| Vector/matrix math | glam |
| Bitfields | bilge |
| RelPtr | generic RelPtr |
| Errors | `anim_data::error` |
| Workspace deps | root `Cargo.toml` workspace |

### Drop after rewrite

| Artifact | Condition to drop |
|----------|-------------------|
| `ssbh_data/src/nuanmb_v12/` | Tests pass via `anim_data::v1` |
| `ssbh_data/src/nuanmb_v12_encode.rs` | Same |
| `ssbh_data/src/anim_create_v12.rs` | Same |
| Flat `anim_data/{buffers,compression}.rs` if only for old path | After v2 path is master’s |

---

## nuanmb rewrite notes

### wmmt2 layout (legacy)

```
ssbh_data/src/anim_data.rs          # entry + #[path] to v12 modules
ssbh_data/src/anim_create_v12.rs
ssbh_data/src/nuanmb_v12/{mod,common,rotate_*,translate}.rs
ssbh_data/src/nuanmb_v12_encode.rs
ssbh_data/src/anim_data/{bitutils,buffers,compression}.rs  # mostly v2
```

### master layout (target)

```
ssbh_data/src/anim_data.rs
ssbh_data/src/anim_data/error.rs
ssbh_data/src/anim_data/v1/{mod?,buffers,common,encode,rotate_*,translate}.rs
ssbh_data/src/anim_data/v2/{buffers,compression}.rs
ssbh_data/src/anim_data/bitutils.rs
```

### Target public behavior

- `AnimData::from_file` / `read` works for v1.2 and v2.x
- `AnimData::to_anim()` for `(1,2)` uses compressed v1 path (`v1::create_anim_v12`)
- Optional uncompressed v1.2 API only if still needed by callers; prefer single path + flags if master already chose compression
- Track hierarchy Group → Node → Track unchanged at data model level
- glam `Quat` / `Vec3` / `Vec4` in high-level data (master)

### TDD plan for nuanmb

1. After merge, ensure master’s existing anim unit tests compile/run.
2. Add/adjust tests that exercise **public** `AnimData` for:
   - v1.2 round-trip synthetic (small track set)
   - v2.0/v2.1 existing cases
   - any retained EXVS2 compressed block types if fixtures exist
3. Remove dual-stack modules only when those tests are green without them.

---

## Known risks

1. **Heavy anim divergence** — merge will not auto-resolve; rewrite expected.
2. **“全部内容” vs format keep** — style rewrite; not binary identity.
3. **MeshWriteProfile / AttributeUsage 10–12** may not exist on master; must re-apply carefully after glam port.
4. **Matl Type4** may conflict with master’s BlendFactor work; combine both.
5. **Session scope** — peripheral CLIs/fuzz may lag; track open todos rather than silent drop.
6. **glam API surface** — wmmt2 code using `ssbh_lib::Vector3` directly in data crate needs conversion helpers from master.

---

## Master commits / themes integrated (evidence log)

| When | What landed | Notes |
|------|-------------|-------|
| (pending merge) | | |

## wmmt2 format bits retained (evidence log)

| When | What retained | Where |
|------|---------------|-------|
| (pending) | AttributeUsageV8 10–12 | formats/mesh.rs |
| (pending) | MeshWriteProfile | mesh_data.rs |
| (pending) | Matl Type4 | formats/matl.rs + matl_data |

---

## Session / decision log

| Date | Decision |
|------|----------|
| 2026-07-12 | Branch `wmmt2-merge` from `wmmt2` only; never force `wmmt2`. |
| 2026-07-12 | Prefer master structure for anim; re-apply EXVS2 format deltas as overlays. |
| 2026-07-12 | This file is the single operational board for the merge rewrite. |

## Deviations from goal plan

_(none yet; plan deviations also go to goal plan.md Deviations section)_
