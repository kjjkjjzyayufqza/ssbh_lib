# wmmt2-merge EXVS2 animation preview regression postmortem

Date: 2026-07-12

## Scope

This investigation compared `wmmt2` and `wmmt2-merge` with the Gyan assets under:

`E:\XB\解包\vs2\x64\003motion\001hito\001gundam\001gundam_005gyan00_001`

The reported symptom was that NUANMB parsing succeeded, but loading an animation in
TAURI_PROJECT's Unit Model Editor separated rigid model parts across the viewport.

## Animation data result

After the Anim v1.2 header and residual decoder merge fixes, decoded animation values
were not the cause of the final viewport failure.

- 106 Gyan NUANMB files were scanned successfully by `wmmt2-merge`.
- `wmmt2` parsed 104 of those files and rejected two aiming clips.
- All 104 shared files were semantically identical after normalizing the old vector
  object representation and the new glam array representation.
- 973,512 numeric TRS components were compared with maximum absolute difference `0`.
- `wmmt2-merge` correctly sets `override_translation` and `override_scale` when Anim
  v1.2 tracks omit the corresponding sparse properties.

See [V12_WMMT2_VS_MERGE_GYAN.md](V12_WMMT2_VS_MERGE_GYAN.md) for the focused
NUANMB comparison.

## Final root cause: SkelData JSON schema drift

The master merge changed `BoneData.transform` from `[[f32; 4]; 4]` to `glam::Mat4`.
The derived glam serde representation is a flat 16-element array, so JSON silently
changed from:

```json
[[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 10, 0, 1]]
```

to:

```json
[1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 10, 0, 1]
```

TAURI_PROJECT still consumed `transform` as `number[][]`. Indexing a scalar as
`transform[column][row]` returned no values, and the fallback path constructed a zero
rest matrix. The incorrect bind pose could be partially hidden while no animation was
active because the bind matrices and their inverses were created from the same bad
data. Applying real animation locals exposed the error as an exploded model.

## Library fix

`ssbh_data/src/skel_data.rs` now applies a dedicated serde adapter to
`BoneData.transform`:

- serialization preserves the legacy `wmmt2` 4-by-4 column-major JSON schema;
- deserialization accepts both the legacy nested schema and glam's flat 16-element
  schema;
- the in-memory type remains `glam::Mat4`.

`TransformFlags` also exposes `uses_skeleton_translation`,
`uses_skeleton_rotation`, and `uses_skeleton_scale` helpers so downstream preview code
does not invert the meaning of `override_*` again.

## Evidence

For the real 44-bone Gyan NUSKTB, the fixed `wmmt2-merge` JSON and `wmmt2` JSON are
fully equal after PowerShell JSON normalization:

```text
OldBoneCount: 44
NewBoneCount: 44
OldTransformShape: 4x4
NewTransformShape: 4x4
BonesJsonEqual: true
```

The full `ssbh_lib` workspace passed 403 tests with 2 ignored before final integration.
The user then confirmed that the repaired TAURI_PROJECT Unit Model Editor renders the
Gyan animation correctly.

## IDA correlation

The data interpretation is also consistent with `vsac27_Release.exe`:

- `sub_140234CB0` sets missing Scale, Rotate, Translate, and CompensateScale bits as
  `0x4`, `0x2`, `0x1`, and `0x8` respectively.
- `sub_140239FE0` divides the Anim v1 time field by `60.0`, confirming the EXVS2 v1.2
  dual-header timebase.
- residual sampling uses 33-key blocks, matching the repaired v1.2 decoder.

