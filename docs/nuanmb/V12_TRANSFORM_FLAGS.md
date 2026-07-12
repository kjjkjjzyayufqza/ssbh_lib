# Anim v1.2 property-sparse TransformFlags

## Symptom

Decoding `E:\XB\解包\vs2\x64\003motion\...` nuanmb files returns **no error**, but applying `TrackValues::Transform` as absolute local TRS in Blender (or any viewer that ignores flags) makes the character look like **all bones were piled at the origin**.

## Root cause (not residual garbage)

VS2 Anim v1.2 stores transform tracks as **sparse properties**:

| Property   | Typical presence on limb bones |
|------------|--------------------------------|
| `Rotate`   | Present (`0x4308` / `0x4409` / const) |
| `Translate`| **Absent** (only on a few roots like `BASE`, `GBL_RT`) |
| `Scale`    | **Absent** |

Previously, missing channels were materialised as:

- translation → `Vec3::ZERO`
- scale → `Vec3::ONE`
- `TransformFlags` → all `false` (meaning “use animation values”)

Consumers then applied **zero local translation** on every limb → hierarchy collapses. Quaternion residual decode can still be unit-length and look “plausible” while the skeleton is wrong.

This is **not** the same class of bug as residual `block_count` / pad inference (those cause `InvalidData` or non-unit quats when wrong).

## Correct SSBH semantics

From `ssbh_lib::formats::anim::TransformFlags`:

- `override_translation = true` → use **skeleton rest** translation
- `override_translation = false` → use **animation** translation  
  (same for rotation / scale)

## Fix in `ssbh_data` (wmmt2-merge)

When reading Anim v1.2:

```text
override_translation = !has_Translate_property
override_rotation    = !has_Rotate_property
override_scale       = !has_Scale_property
```

Example (`boostloop_stk_gnd_fr`):

- `ASHI_L`: `flags(t=true, r=false, s=true)` — rotate from clip, trans/scale from skel
- `BASE`: `flags(t=false, r=false, s=true)` — clip authors translation + rotation

## Viewer requirements

Any Blender / game / TAURI path that samples AnimData **must** honour `track.transform_flags` when composing with `SkelData`.  
Filling zeros into `Transform.translation` alone is not enough without flags.

## Related

- Dual header: file `final_frame_index=60` (timebase), `unk2=end_frame`; `AnimData.final_frame_index` is the effective end frame.
- Residual layout docs: `docs/nuanmb/V12_RESIDUAL_BLOCK_COUNT.md`
