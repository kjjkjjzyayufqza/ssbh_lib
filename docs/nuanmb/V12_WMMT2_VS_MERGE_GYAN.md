# wmmt2 vs wmmt2-merge: gyan nuanmb numeric compare

Sample: `001hito_001gundam_005gyan00_001_boostloop_stk_gnd_fr.nuanmb`

## Result

`AnimData` TRS samples are **bit-identical** between `wmmt2` and `wmmt2-merge`
(960 frames compared, max abs diff `0`).

Residual / 0x4308 / 0x4409 curve decoders are **not** the source of the "random bones"
visual for this corpus.

## What differs

| | wmmt2 | wmmt2-merge (after flags fix) |
|--|-------|------------------------------|
| Frame floats | same | same |
| `transform_flags` on limbs | all `false` | `override_translation/scale = true` (no Translate/Scale props) |

## Why wmmt2 "looked fine" in TAURI

Historical preview applied **inverted** translation flags:

- `override_translation == false` → use **skeleton** translation  
  (opposite of SSBH docs)

With wmmt2 all-false flags that yielded: **skel translation + anim rotation** (desired hybrid).

After merge set correct flags (`override_translation=true` for missing Translate),
the inverted preview path switched limbs to **anim translation (0,0,0)** → hierarchy collapse.

## Correct stack

1. Library: property-sparse → set `override_*` for missing channels (merge).
2. Preview: honor SSBH flag semantics (not inverted).
